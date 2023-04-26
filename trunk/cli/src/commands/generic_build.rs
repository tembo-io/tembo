use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::models::HostConfig;

use std::collections::HashMap;
use std::default::Default;

use std::{fs, include_str};
use std::path::{Path, StripPrefixError};
use std::string::FromUtf8Error;

use futures_util::stream::StreamExt;

use rand::Rng;
use tar::Header;
use thiserror::Error;

use bollard::image::BuildImageOptions;
use bollard::Docker;

use crate::sync_utils::ByteStreamSyncSender;
use bollard::models::BuildInfo;

use hyper::Body;

use tokio::sync::mpsc;
use tokio::task;
use tokio::task::JoinError;
use tokio_stream::wrappers::ReceiverStream;
use tokio_task_manager::Task;

use crate::commands::containers::{build_image, exec_in_container, package_installed_extension_files, run_temporary_container};

#[derive(Error, Debug)]
pub enum GenericBuildError {
    #[error("Produced a file outside of postgres sharedir or pkglibdir: {0}")]
    InvalidFileInstalled(String),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Docker Error: {0}")]
    DockerError(#[from] bollard::errors::Error),

    #[error("Error converting binary to utf8: {0}")]
    FromUft8Error(#[from] FromUtf8Error),

    #[error("Internal sending error: {0}")]
    InternalSendingError(#[from] mpsc::error::SendError<Vec<u8>>),

    #[error("Async join error: {0}")]
    JoinError(#[from] JoinError),

    #[error("Parsing ELF file error: {0}")]
    ElfError(#[from] elf::ParseError),

    #[error("Tar layout error: trunk-output not found")]
    TarLayoutError(#[from] StripPrefixError),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Other error: {0}")]
    OtherError(#[from] anyhow::Error),
}

// Generic Trunk builder procedure:
//
// Run image build, providing the user-provided build command
//
// docker build -t test .
//
// Start container from the builder image, with a lifetime
//
// docker run -it --rm --entrypoint=sleep -d test 600
//
// Connect into running container, and run the user-provided install command
//
// docker exec -it 05a11b4b1bd5 make install
//
// Find the files that have changed from the install command
//
// docker diff 05a11b4b1bd5
//
// Any file that has changed, copy out of the container and into the trunk package
pub async fn build_generic(
    path: &Path,
    output_path: &str,
    extension_name: &str,
    extension_version: &str,
    _task: Task,
) -> Result<(), GenericBuildError> {

    println!("Building with name {}", &extension_name);
    println!("Building with version {}", &extension_version);

    let dockerfile = include_str!("./builders/Dockerfile.generic");

    let mut build_args = HashMap::new();
    build_args.insert("EXTENSION_NAME", extension_name);
    build_args.insert("EXTENSION_VERSION", extension_version);

    let mut image_name_prefix = "make_builder_".to_string();

    let docker = Docker::connect_with_local_defaults()?;

    let image_name = build_image(
        docker.clone(),
        &image_name_prefix,
        dockerfile,
        &path,
        build_args
    ).await?;

    let temp_container = run_temporary_container(docker.clone(), &image_name.as_str(), _task).await?;

    println!("Determining installation files...");
    let _exec_output = exec_in_container(
        docker.clone(),
        &temp_container.id,
        vec![
            "make",
            "install"
        ],
    )
        .await?;

    // output_path is the locally output path
    fs::create_dir_all(output_path)?;

    package_installed_extension_files(
        docker.clone(),
        &temp_container.id,
        output_path,
        extension_name,
        extension_version,
    )
        .await?;

    Ok(())
}
