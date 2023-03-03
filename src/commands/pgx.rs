use bollard::container::{
    Config, CreateContainerOptions, DownloadFromContainerOptions, StartContainerOptions,
};
use bollard::models::HostConfig;
use std::collections::HashMap;
use std::default::Default;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::string::FromUtf8Error;
use std::{fs, include_str};

use futures_util::stream::StreamExt;

use rand::Rng;
use tar::{Archive, Header};
use thiserror::Error;

use bollard::image::BuildImageOptions;
use bollard::Docker;

use crate::sync_utils::{ByteStreamReceiver, ByteStreamSender};
use bollard::models::BuildInfo;
use futures_util::FutureExt;
use hyper::Body;
use tokio::sync::mpsc;
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;
use toml::Value;

#[derive(Error, Debug)]
pub enum PgxBuildError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Docker Error: {0}")]
    DockerError(#[from] bollard::errors::Error),

    #[error("Error converting binary to utf8: {0}")]
    FromUft8Error(#[from] FromUtf8Error),

    #[error("Internal sending error: {0}")]
    InternalSendingError(#[from] mpsc::error::SendError<Vec<u8>>),

    #[error("Cargo manifest error: {0}")]
    ManifestError(String),
}

pub async fn build_pgx(
    path: &Path,
    output_path: &str,
    cargo_toml: toml::Table,
) -> Result<(), PgxBuildError> {
    let cargo_package_info = cargo_toml
        .get("package")
        .into_iter()
        .filter_map(Value::as_table)
        .next()
        .ok_or(PgxBuildError::ManifestError(
            "Could not find package info in Cargo.toml".to_string(),
        ))?;
    let extension_name = cargo_package_info
        .get("name")
        .into_iter()
        .filter_map(Value::as_str)
        .next()
        .ok_or(PgxBuildError::ManifestError(
            "Could not find package name in Cargo.toml".to_string(),
        ))?;
    let extension_version = cargo_package_info
        .get("version")
        .into_iter()
        .filter_map(Value::as_str)
        .next()
        .ok_or(PgxBuildError::ManifestError(
            "Could not find package version in Cargo.toml".to_string(),
        ))?;
    let pgx_version = cargo_toml
        .get("dependencies")
        .into_iter()
        .filter_map(Value::as_table)
        .next()
        .ok_or(PgxBuildError::ManifestError(
            "Could not find dependencies info in Cargo.toml".to_string(),
        ))?
        .get("pgx")
        .into_iter()
        .filter_map(Value::as_str)
        .next()
        .ok_or(PgxBuildError::ManifestError(
            "Could not find pgx dependency info in Cargo.toml".to_string(),
        ))?;

    println!("Building pgx extension at path {}", &path.display());
    let dockerfile = include_str!("./pgx_builder/Dockerfile");

    let (receiver, sender, stream) = ByteStreamSender::new();
    // Making path owned so we can send it to the tarring task below without having to worry
    // about the lifetime of the reference.
    let path = path.to_owned();
    task::spawn_blocking(move || {
        let f = || {
            let mut tar = tar::Builder::new(stream);
            tar.append_dir_all(".", path)?;

            let mut header = Header::new_gnu();
            header.set_size(dockerfile.len() as u64);
            header.set_cksum();
            tar.append_data(&mut header, "Dockerfile", dockerfile.as_bytes())?;
            Ok(())
        };
        match f() {
            Ok(()) => (),
            Err(err) => sender.try_send(Err(err)).map(|_| ()).unwrap_or_default(),
        }
    });

    let mut image_name = "pgx_builder_".to_string();

    let random_suffix = {
        let mut rng = rand::thread_rng();
        rng.gen_range(0..1000000).to_string()
    };

    image_name.push_str(&random_suffix);
    let image_name = image_name.as_str().to_owned();

    let mut build_args = HashMap::new();
    build_args.insert("EXTENSION_NAME", extension_name);
    build_args.insert("EXTENSION_VERSION", extension_version);
    build_args.insert("PGX_VERSION", pgx_version);

    // TODO: build args in the Dockerfile such as postgres version should be configurable
    let options = BuildImageOptions {
        dockerfile: "Dockerfile",
        t: &image_name.clone(),
        rm: true,
        buildargs: build_args,
        ..Default::default()
    };

    let docker = Docker::connect_with_local_defaults()?;
    let mut image_build_stream = docker.build_image(
        options,
        None,
        Some(Body::wrap_stream(ReceiverStream::new(receiver))),
    );

    while let Some(next) = image_build_stream.next().await {
        match next {
            Ok(BuildInfo {
                stream: Some(s), ..
            }) => {
                print!("{s}");
            }
            Ok(BuildInfo {
                error: Some(err),
                error_detail,
                ..
            }) => {
                eprintln!(
                    "ERROR: {} (detail: {})",
                    err,
                    error_detail.unwrap_or_default().message.unwrap_or_default()
                );
            }
            Ok(_) => {}
            Err(err) => {
                return Err(err)?;
            }
        }
    }

    let options = Some(CreateContainerOptions {
        name: image_name.to_string(),
        platform: None,
    });

    let host_config = HostConfig {
        auto_remove: Some(true),
        ..Default::default()
    };

    let config = Config {
        image: Some(image_name.to_string()),
        entrypoint: Some(vec!["sleep".to_string()]),
        cmd: Some(vec!["300".to_string()]),
        host_config: Some(host_config),
        ..Default::default()
    };

    let container = docker.create_container(options, config).await?;
    docker
        .start_container(&container.id, None::<StartContainerOptions<String>>)
        .await?;

    // output_path is the locally output path
    fs::create_dir_all(output_path)?;

    // output_dir is the path inside the image
    // where we can find the files we want to download
    let output_dir = format!("/app/trunk-output");

    let options = Some(DownloadFromContainerOptions { path: output_dir });

    let mut file_stream = docker.download_from_container(&container.id, options);

    let mut file = File::create(format!("{output_path}/result.tar"))?;
    while let Some(next) = file_stream.next().await {
        match next {
            Ok(bytes) => {
                file.write_all(&bytes).unwrap();
            }
            Err(err) => {
                return Err(err)?;
            }
        }
    }

    // stop the container
    docker.stop_container(&container.id, None).await?;

    Ok(())
}
