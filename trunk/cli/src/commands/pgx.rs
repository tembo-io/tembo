use semver::{Version, VersionReq};
use std::collections::HashMap;

use std::path::{Path, StripPrefixError};
use std::string::FromUtf8Error;
use std::{fs, include_str};

use thiserror::Error;

use bollard::Docker;

use crate::commands::containers::{
    build_image, exec_in_container, package_installed_extension_files, run_temporary_container,
};
use tokio::sync::mpsc;

use tokio::task::JoinError;

use tokio_task_manager::Task;
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

fn semver_from_range(pgx_range: &str) -> Result<String, PgxBuildError> {
    let versions = ["0.7.4", "0.7.3", "0.7.2", "0.7.1"];

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
    _task: Task,
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
    let dockerfile = include_str!("./builders/Dockerfile.pgx");

    let mut build_args = HashMap::new();
    build_args.insert("EXTENSION_NAME", extension_name);
    build_args.insert("EXTENSION_VERSION", extension_version);
    build_args.insert("PGX_VERSION", pgx_version.as_str());

    let image_name_prefix = "pgx_builder_".to_string();

    let docker = Docker::connect_with_local_defaults()?;

    let image_name = build_image(
        docker.clone(),
        &image_name_prefix,
        dockerfile,
        path,
        build_args,
    )
    .await?;

    let temp_container =
        run_temporary_container(docker.clone(), image_name.as_str(), _task).await?;

    println!("Determining installation files...");
    let _exec_output = exec_in_container(
        docker.clone(),
        &temp_container.id,
        vec![
            "cp",
            "--verbose",
            "-R",
            format!("target/release/{extension_name}-pg15/usr").as_str(),
            "/",
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
        assert_eq!(result.unwrap(), "0.7.4");
    }
}
