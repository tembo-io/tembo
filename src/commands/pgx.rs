use futures_util::stream::StreamExt;
use std::include_str;
use std::path::Path;
use tar::Header;
use thiserror::Error;

use bollard::image::BuildImageOptions;
use bollard::Docker;

use bollard::models::BuildInfo;
use std::default::Default;

#[derive(Error, Debug)]
pub enum PgxBuildError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Docker Error: {0}")]
    DockerError(#[from] bollard::errors::Error),
}

pub async fn build_pgx(path: &Path, _output_path: &str) -> Result<(), PgxBuildError> {
    // your code for building a pgx extension goes here
    println!("Building pgx extension at path {}", &path.display());
    let dockerfile = include_str!("./pgx_builder/Dockerfile");

    let mut tar = tar::Builder::new(Vec::new());
    tar.append_dir_all(".", path)?;

    let mut header = Header::new_gnu();
    header.set_size(dockerfile.len() as u64);
    header.set_cksum();
    tar.append_data(&mut header, "Dockerfile", dockerfile.as_bytes())?;

    let options = BuildImageOptions {
        dockerfile: "Dockerfile",
        t: "temp",
        rm: true,
        ..Default::default()
    };

    let docker = Docker::connect_with_local_defaults()?;
    let mut image_build_stream = docker.build_image(options, None, Some(tar.into_inner()?.into()));

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

    Ok(())
}
