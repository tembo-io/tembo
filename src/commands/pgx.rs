use bollard::container::{
    Config, CreateContainerOptions, DownloadFromContainerOptions, StartContainerOptions,
};
use bollard::models::HostConfig;
use semver::{Version, VersionReq};
use std::collections::HashMap;
use std::default::Default;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::string::FromUtf8Error;
use std::{fs, include_str};

use futures_util::stream::StreamExt;

use rand::Rng;
use tar::Header;
use thiserror::Error;

use bollard::image::BuildImageOptions;
use bollard::Docker;

use crate::sync_utils::ByteStreamSender;
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

fn semver_from_range(pgx_range: &str) -> Result<String, PgxBuildError> {
    let versions = ["0.7.2", "0.7.1"];

    if versions.contains(&pgx_range) {
        // If the input is already a specific version, return it as-is
        return Ok(pgx_range.to_string());
    }

    // If the version is a semver range, convert to a specific version
    let pgx_semver = if let Ok(range) = VersionReq::parse(pgx_range) {
        // The pgx version is a range, so we need to find the highest
        // version that satisfies the range
        versions
            .iter()
            .filter_map(|&s| Version::parse(s).ok())
            .filter(|v| range.matches(v))
            .max()
            .ok_or(PgxBuildError::ManifestError(format!(
                "No supported version of pgx satisfies the range {pgx_range}. \nSupported versions: {versions:?}"
            )))?
    } else {
        // The pgx version is already a specific version
        Version::parse(pgx_range).map_err(|_| {
            PgxBuildError::ManifestError(format!("Invalid pgx version string: {pgx_range}"))
        })?
    };

    let pgx_version = pgx_semver.to_string();
    Ok(pgx_version)
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
    let pgx_range = cargo_toml
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

    println!("Detected pgx version range {}", &pgx_range);

    let pgx_version = semver_from_range(pgx_range)?;
    println!("Using pgx version {pgx_version}");

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
    build_args.insert("PGX_VERSION", pgx_version.as_str());

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
    let output_dir = "/app/trunk-output".to_string();

    let options = Some(DownloadFromContainerOptions { path: output_dir });

    let mut file_stream = docker.download_from_container(&container.id, options);

    let mut file = File::create(format!("{output_path}/result.trunk.tar"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_from_range_specific_version() {
        // Test that a specific version string is returned as-is
        let result = semver_from_range("0.7.1");
        assert_eq!(result.unwrap(), "0.7.1");
        let result = semver_from_range("0.7.2");
        assert_eq!(result.unwrap(), "0.7.2");
    }

    #[test]
    fn test_semver_from_range_specific_version_with_equals() {
        // Test that a specific version string is returned as-is
        let result = semver_from_range("=0.7.1");
        assert_eq!(result.unwrap(), "0.7.1");
        let result = semver_from_range("=0.7.2");
        assert_eq!(result.unwrap(), "0.7.2");
    }

    #[test]
    fn test_semver_from_range_semver_range() {
        // Test that a semver range is converted to the highest matching version
        let result = semver_from_range(">=0.7.1, <0.8.0");
        assert_eq!(result.unwrap(), "0.7.2");
    }
}
