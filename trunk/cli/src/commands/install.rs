use super::SubCommand;
use crate::manifest::{Manifest, PackagedFile};
use async_trait::async_trait;
use clap::Args;
use flate2::read::GzDecoder;
use reqwest;
use std::fs::File;
use std::io::Seek;
use std::path::{Path, PathBuf};
use tar::{Archive, EntryType};
use tokio_task_manager::Task;

#[derive(Args)]
pub struct InstallCommand {
    name: String,
    #[arg(long = "pg-config", short = 'p')]
    pg_config: Option<PathBuf>,
    #[arg(long = "file", short = 'f')]
    file: Option<PathBuf>,
    #[arg(long = "version", short = 'v')]
    version: String,
    #[arg(
        long = "registry",
        short = 'r',
        default_value = "https://registry.pgtrunk.io"
    )]
    registry: String,
}

#[derive(thiserror::Error, Debug)]
pub enum PgxInstallError {
    #[error("unknown file type")]
    UnknownFileType,

    #[error("pg_config not found")]
    PgConfigNotFound,

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Package manifest not found")]
    ManifestNotFound,
}

#[async_trait]
impl SubCommand for InstallCommand {
    async fn execute(&self, _task: Task) -> Result<(), anyhow::Error> {
        let installed_pg_config = which::which("pg_config").ok();
        let pg_config = self
            .pg_config
            .as_ref()
            .or_else(|| installed_pg_config.as_ref())
            .ok_or(PgxInstallError::PgConfigNotFound)?;
        println!("Using pg_config: {}", pg_config.to_string_lossy());

        let package_lib_dir = std::process::Command::new(pg_config)
            .arg("--pkglibdir")
            .output()?
            .stdout;
        let package_lib_dir = String::from_utf8_lossy(&package_lib_dir)
            .trim_end()
            .to_string();
        let package_lib_dir_path = std::path::PathBuf::from(&package_lib_dir);
        let package_lib_dir = std::fs::canonicalize(&package_lib_dir_path)?;

        let sharedir = std::process::Command::new(pg_config.clone())
            .arg("--sharedir")
            .output()?
            .stdout;

        let sharedir = PathBuf::from(String::from_utf8_lossy(&sharedir).trim_end().to_string());
        let extension_dir_path = sharedir.join("extension");

        let extension_dir = std::fs::canonicalize(extension_dir_path)?;

        let bitcode_dir = std::path::PathBuf::from(&sharedir).join("bitcode");

        // if this is a symlink, then resolve the symlink

        if !package_lib_dir.exists() && !package_lib_dir.is_dir() {
            println!(
                "The package lib dir {} does not exist",
                package_lib_dir.display()
            );
            return Ok(());
        }
        if !extension_dir.exists() && !extension_dir.is_dir() {
            println!("Extension dir {} does not exist", extension_dir.display());
            return Ok(());
        }
        println!("Using pkglibdir: {package_lib_dir:?}");
        println!("Using sharedir: {sharedir:?}");

        // If file is specified
        if let Some(ref file) = self.file {
            let f = File::open(file)?;

            let input = match file
                .extension()
                .into_iter()
                .filter_map(|s| s.to_str())
                .next()
            {
                Some("gz") => {
                    // unzip the archive into a temporary file
                    let decoder = GzDecoder::new(f);
                    let mut tempfile = tempfile::tempfile()?;
                    use read_write_pipe::*;
                    tempfile.write_reader(decoder)?;
                    tempfile.rewind()?;
                    tempfile
                }
                Some("tar") => f,
                _ => return Err(PgxInstallError::UnknownFileType)?,
            };
            install(input, extension_dir, package_lib_dir, bitcode_dir, sharedir).await?;
        } else {
            // If a file is not specified, then we will query the registry
            // and download the latest version of the package
            // Using the reqwest crate, we will run the equivalent of this curl command:
            // curl --request GET --url 'http://localhost:8080/extensions/{self.name}/{self.version}/download'
            let response = reqwest::get(&format!(
                "{}/extensions/{}/{}/download",
                self.registry, self.name, self.version
            ))
            .await?;
            let response_body = response.text().await?;
            let file_response = reqwest::get(response_body).await?;
            let bytes = file_response.bytes().await?;
            // unzip the archive into a temporary file
            let gz = GzDecoder::new(&bytes[..]);
            let mut tempfile = tempfile::tempfile()?;
            use read_write_pipe::*;
            tempfile.write_reader(gz)?;
            tempfile.rewind()?;
            let input = tempfile;
            install(input, extension_dir, package_lib_dir, bitcode_dir, sharedir).await?;
        }
        Ok(())
    }
}

async fn install(
    mut input: File,
    extension_dir: PathBuf,
    package_lib_dir: PathBuf,
    bitcode_dir: PathBuf,
    sharedir: PathBuf,
) -> Result<(), anyhow::Error> {
    // First pass: get to the manifest
    // Because we're going over entries with `Seek` enabled, we're not reading everything.
    let mut archive = Archive::new(&input);

    let mut manifest: Option<Manifest> = None;
    let entries = archive.entries_with_seek()?;
    for entry in entries {
        let entry = entry?;
        let name = entry.path()?;
        if entry.header().entry_type() == EntryType::file() && name == Path::new("manifest.json") {
            manifest.replace(serde_json::from_reader(entry)?);
        }
    }

    // Second pass: extraction
    input.rewind()?;
    let mut archive = Archive::new(&input);

    if let Some(mut manifest) = manifest {
        let manifest_files = manifest.files.take().unwrap_or_default();
        println!(
            "Installing {} {}",
            manifest.extension_name, manifest.extension_version
        );
        let host_arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else if cfg!(target_arch = "arm") {
            "aarch32"
        } else if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target = "x86") {
            "x86"
        } else {
            "unsupported"
        };
        let entries = archive.entries_with_seek()?;
        for entry in entries {
            let mut entry = entry?;
            let name = entry.path()?;
            if let Some(file) = manifest_files.get(name.as_ref()) {
                match file {
                    PackagedFile::ControlFile { .. } => {
                        println!("[+] {} => {}", name.display(), extension_dir.display());
                        entry.unpack_in(&extension_dir)?;
                    }
                    PackagedFile::SqlFile { .. } => {
                        println!("[+] {} => {}", name.display(), extension_dir.display());
                        entry.unpack_in(&extension_dir)?;
                    }
                    PackagedFile::SharedObject {
                        architecture: None, ..
                    } => {
                        println!(
                            "[+] {} (no arch) => {}",
                            name.display(),
                            package_lib_dir.display()
                        );
                        entry.unpack_in(&package_lib_dir)?;
                    }
                    PackagedFile::SharedObject {
                        architecture: Some(ref architecture),
                        ..
                    } if architecture == host_arch => {
                        println!("[+] {} => {}", name.display(), package_lib_dir.display());
                        entry.unpack_in(&package_lib_dir)?;
                    }
                    PackagedFile::SharedObject {
                        architecture: Some(ref architecture),
                        ..
                    } => {
                        println!("[ ] {} (arch) skipped {}", name.display(), architecture);
                    }
                    PackagedFile::Bitcode { .. } => {
                        println!("[+] {} => {}", name.display(), bitcode_dir.display());
                        entry.unpack_in(&bitcode_dir)?;
                    }
                    PackagedFile::Extra { .. } => {
                        println!("[+] {} => {}", name.display(), sharedir.display());
                        entry.unpack_in(&sharedir)?;
                    }
                }
            }
        }
    } else {
        return Err(PgxInstallError::ManifestNotFound)?;
    }
    Ok(())
}
