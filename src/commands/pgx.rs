use bollard::container::{
    Config, CreateContainerOptions, DownloadFromContainerOptions, StartContainerOptions,
};
use bollard::models::HostConfig;
use semver::{Version, VersionReq};
use std::collections::HashMap;
use std::default::Default;
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::string::FromUtf8Error;
use std::{fs, include_str};

use futures_util::stream::StreamExt;
use futures_util::TryFutureExt;

use rand::Rng;
use tar::{Archive, Builder, EntryType, Header};
use thiserror::Error;

use bollard::image::BuildImageOptions;
use bollard::Docker;

use crate::sync_utils::{ByteStreamSyncReceiver, ByteStreamSyncSender};
use bollard::models::BuildInfo;
use elf::endian::AnyEndian;
use elf::ElfBytes;
use hyper::Body;
use serde::{Deserialize, Serialize};
use tee_readwrite::TeeReader;
use tokio::sync::mpsc;
use tokio::task;
use tokio::task::JoinError;
use tokio_stream::wrappers::ReceiverStream;
use toml::Value;

/// Packaged file
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PackagedFile {
    ControlFile {
        name: PathBuf,
    },
    SqlFile {
        name: PathBuf,
    },
    SharedObject {
        name: PathBuf,
        architecture: Option<String>,
    },
    Bitcode {
        name: PathBuf,
    },
    Extra {
        name: PathBuf,
    },
}

impl PackagedFile {
    pub fn from<P: AsRef<Path>>(path: P) -> Self {
        let extension = path.as_ref().extension();
        if let Some(ext) = extension {
            match ext.to_str() {
                Some("control") => PackagedFile::ControlFile {
                    name: path.as_ref().to_path_buf(),
                },
                Some("sql") => PackagedFile::SqlFile {
                    name: path.as_ref().to_path_buf(),
                },
                Some("so") => PackagedFile::SharedObject {
                    name: path.as_ref().to_path_buf(),
                    architecture: None,
                },
                Some("bc") => PackagedFile::Bitcode {
                    name: path.as_ref().to_path_buf(),
                },
                Some(_) | None => PackagedFile::Extra {
                    name: path.as_ref().to_path_buf(),
                },
            }
        } else {
            PackagedFile::Extra {
                name: path.as_ref().to_path_buf(),
            }
        }
    }
}

/// Package manifest
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "name")]
    pub extension_name: String,
    #[serde(rename = "version")]
    pub extension_version: String,
    pub sys: String,
    pub files: Option<Vec<PackagedFile>>,
}

impl Manifest {
    pub fn merge(&mut self, other: Self) {
        if let Some(files) = other.files {
            self.files.replace(files);
        }
    }

    pub fn add_file<P: AsRef<Path> + Into<PathBuf>>(&mut self, path: P) -> &mut PackagedFile {
        let files = match self.files {
            None => {
                self.files.replace(Vec::new());
                self.files.as_mut().unwrap()
            }
            Some(ref mut files) => files,
        };
        files.push(PackagedFile::from(path));
        files.last_mut().unwrap()
    }
}

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

    #[error("Other error: {0}")]
    OtherError(#[from] anyhow::Error),
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

    let (receiver, sender, stream) = ByteStreamSyncSender::new();
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

    let file_stream = docker.download_from_container(&container.id, options);

    let receiver = ByteStreamSyncReceiver::new();
    let receiver_sender = receiver.sender();
    let output_path = output_path.to_owned();
    let extension_name = extension_name.to_owned();
    let extension_version = extension_version.to_owned();
    let tar_handle = task::spawn_blocking(move || {
        let file = File::create(format!(
            "{output_path}/{extension_name}-{extension_version}.tar.gz"
        ))?;
        let mut archive = Archive::new(receiver);
        let mut new_archive = Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        let mut manifest = Manifest {
            extension_name,
            extension_version,
            sys: "linux".to_string(),
            files: None,
        };
        if let Ok(entries) = archive.entries() {
            for entry in entries {
                if let Ok(entry) = entry {
                    let name = entry.path()?.to_path_buf();
                    if name.to_str() == Some("manifest.json") {
                        manifest.merge(serde_json::from_reader(entry)?);
                    } else {
                        let name = name.strip_prefix("trunk-output")?;

                        if !name.to_string_lossy().is_empty() {
                            let mut header = Header::new_gnu();
                            header.set_mode(entry.header().mode()?);
                            header.set_mtime(entry.header().mtime()?);
                            header.set_size(entry.size());
                            header.set_cksum();
                            let entry_type = entry.header().entry_type();

                            let mut buf = Vec::new();
                            let mut tee = TeeReader::new(entry, &mut buf, true);

                            new_archive.append_data(&mut header, name, &mut tee)?;

                            let (_entry, buf) = tee.into_inner();

                            if entry_type == EntryType::file() {
                                let file = manifest.add_file(name);
                                match file {
                                    PackagedFile::SharedObject {
                                        ref mut architecture,
                                        ..
                                    } => {
                                        let elf = ElfBytes::<AnyEndian>::minimal_parse(buf)?;
                                        let target_arch = match elf.ehdr.e_machine {
                                            elf::abi::EM_386 => "x86",
                                            elf::abi::EM_X86_64 => "x86_64",
                                            elf::abi::EM_AARCH64 => "aarch64",
                                            elf::abi::EM_ARM => "aarch32",
                                            _ => "unknown",
                                        }
                                        .to_string();
                                        architecture.replace(target_arch);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
        let manifest = serde_json::to_string_pretty(&manifest).unwrap_or_default();
        let mut header = Header::new_gnu();
        header.set_size(manifest.as_bytes().len() as u64);
        header.set_cksum();
        header.set_mode(0o644);
        new_archive.append_data(&mut header, "manifest.json", Cursor::new(manifest))?;
        Ok::<_, anyhow::Error>(())
    });

    let result = tokio::join!(
        receiver_sender
            .stream_to_end(file_stream)
            .map_err(anyhow::Error::from),
        tar_handle.map_err(anyhow::Error::from),
    );
    match result {
        (_, Err(err)) => return Err(err)?,
        (Err(err), _) => return Err(err)?,
        _ => {}
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
