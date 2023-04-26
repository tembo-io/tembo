use std::fs::{File, OpenOptions};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use bollard::container::DownloadFromContainerOptions;
use bollard::Docker;
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use elf::ElfBytes;
use elf::endian::AnyEndian;
use futures_util::stream;
use tokio_task_manager::Task;
use futures_util::stream::{select, select_all, StreamExt};
use tar::{Archive, Builder, EntryType, Header};
use tee_readwrite::TeeReader;
use tokio::task;
use crate::commands::generic_build::GenericBuildError;
use crate::manifest::{Manifest, PackagedFile};
use crate::sync_utils::ByteStreamSyncReceiver;

/// Used to stop container when dropped, relies on using [tokio_task_manager::TaskManager::wait]
/// to ensure `Drop` will run to completion
pub struct ReclaimableContainer<'a> {
    id: &'a str,
    docker: Docker,
    task: Task,
}

impl<'a> ReclaimableContainer<'a> {
    #[must_use]
    pub fn new(name: &'a str, docker: &Docker, task: Task) -> Self {
        Self {
            id: name,
            docker: docker.clone(),
            task,
        }
    }
}

impl<'a> Drop for ReclaimableContainer<'a> {
    fn drop(&mut self) {
        let docker = self.docker.clone();
        let id = self.id.to_string();
        let handle = tokio::runtime::Handle::current();
        let mut task = self.task.clone();
        handle.spawn(async move {
            println!("Stopping {id}");
            docker
                .stop_container(&id, None)
                .await
                .expect("error stopping container");
            println!("Stopped {id}");
            task.wait().await;
        });
    }
}

pub async fn exec_in_container(docker: Docker, container_id: &str, command: Vec<&str>) -> Result<String, anyhow::Error> {

    println!("Executing in container: {:?}", command.join(" "));

    let config = CreateExecOptions {
        cmd: Some(command),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        ..Default::default()
    };

    let exec = docker.create_exec(container_id, config).await?;
    let start_exec_options = Some(StartExecOptions {
        detach: false,
        ..StartExecOptions::default()
    });
    let log_output = docker.start_exec(&exec.id, start_exec_options);
    let mut start_exec_result = log_output.await?;

    let mut total_output = String::new();
    match start_exec_result {
        StartExecResults::Attached { output, .. } => {
            let mut output = output
                .map(|result| {
                    match result {
                        Ok(log_output) => {
                            println!("{}", log_output.to_string());
                            total_output.push_str(log_output.to_string().as_str());
                        },
                        Err(error) => eprintln!("Error while reading log output: {}", error),
                    }
                })
                .fuse();

            // Run the output stream to completion.
            while output.next().await.is_some() {}
        },
        StartExecResults::Detached => {
            println!("Exec started in detached mode");
        }

    }
    Ok::<String, anyhow::Error>(total_output)
}

// Scan sharedir and package lib dir from a Trunk builder container for files from a provided list.
// Package these files into a Trunk package.
pub async fn copy_from_container_into_package(docker: Docker, container_id: &str, package_path: &str, sharedir: &str, pkglibdir: &str, sharedir_list: Vec<String>, pkglibdir_list: Vec<String>, extension_name: &str, extension_version: &str) -> Result<(), anyhow::Error> {

    let extension_name = extension_name.to_owned();
    let extension_version = extension_version.to_owned();
    let pkglibdir = pkglibdir.to_owned();
    let sharedir = sharedir.to_owned();

    // In this function, we open and work with .tar only, then we finalize the package with a .gz in a separate call
    let package_path = format!("{package_path}/{extension_name}-{extension_version}.tar");
    println!("Creating package at: {}", package_path);
    let file = File::create(&package_path)?;

    // Stream used to pass information from docker to tar
    let receiver = ByteStreamSyncReceiver::new();
    let receiver_sender = receiver.sender();

    // Open stream to docker for copying files
    // Is there some way to copy from both sharedir and pkglibdir,
    // then combine the steams instead of scanning the whole /usr directory?
    // Looping over everything in that directory makes this way slower.
    let options_usrdir = Some(DownloadFromContainerOptions { path: "/usr" });
    let file_stream = docker.download_from_container(container_id, options_usrdir);

    // Create a sync task within the tokio runtime to copy the file from docker to tar
    let tar_handle = task::spawn_blocking(move || {
        let mut archive = Archive::new(receiver);
        let mut new_archive = Builder::new(
            file,
        );
        let mut manifest = Manifest {
            extension_name,
            extension_version,
            sys: "linux".to_string(),
            files: None,
        };
        // If the docker copy command starts to stream data
        println!("Scanning...");
        if let Ok(entries) = archive.entries() {
            // For each file from the tar stream returned from docker copy
            for entry in entries {
                // If we can get the file from the stream
                if let Ok(entry) = entry {
                    // Then we will handle packaging the file
                    let path = entry.path()?.to_path_buf();
                    // Check if we found a file to package in pkglibdir
                    let full_path = format!("/{}", path.to_str().unwrap_or_else(|| ""));
                    let trimmed = full_path.trim_start_matches(&format!("{}/", pkglibdir)).trim_start_matches(&format!("{}/", sharedir)).to_string();
                    let pkglibdir_match = pkglibdir_list.contains(&trimmed);
                    let sharedir_match = sharedir_list.contains(&trimmed);
                    // Check if we found a file to package
                    if !( sharedir_match || pkglibdir_match ){
                        continue
                    }
                    println!("Detected file to package: {}", trimmed);
                    if path.to_str() == Some("manifest.json") {
                        println!("Found manifest.json, merging additions with existing manifest");
                        manifest.merge(serde_json::from_reader(entry)?);
                    } else {
                        println!("Packaging file {:?}", path.clone());
                        // let path = path.strip_prefix(path_prefix.to_string())?;

                        if !path.to_string_lossy().is_empty() {
                            let mut header = Header::new_gnu();
                            header.set_mode(entry.header().mode()?);
                            header.set_mtime(entry.header().mtime()?);
                            header.set_size(entry.size());
                            header.set_cksum();
                            let entry_type = entry.header().entry_type();

                            let mut buf = Vec::new();
                            let mut tee = TeeReader::new(entry, &mut buf, true);

                            println!("Adding file {} to package", path.clone().to_string_lossy());
                            new_archive.append_data(&mut header, path.clone(), &mut tee)?;
                            println!("Added");

                            let (_entry, buf) = tee.into_inner();

                            if entry_type == EntryType::file() {
                                println!("Adding file {} to manifest", path.clone().to_string_lossy());
                                let file = manifest.add_file(path);
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
                                        println!("Detected architecture: {}", target_arch);
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
        Ok::<_, GenericBuildError>(())
    });

    // Wait until completion of streaming, but ignore its error as it would only error out
    // if tar_handle errors out.
    let _ = receiver_sender.stream_to_end(file_stream).await;
    // Handle the error
    tar_handle.await??;

    println!("Packaged to {package_path}");

    return Ok(());
}
