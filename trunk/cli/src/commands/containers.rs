use std::collections::HashMap;
use bollard::container::{CreateContainerOptions, DownloadFromContainerOptions, StartContainerOptions};
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use bollard::Docker;
use elf::endian::AnyEndian;
use elf::ElfBytes;
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use bollard::image::BuildImageOptions;
use bollard::models::{BuildInfo, HostConfig};
use bollard::container::{Config};

use crate::commands::generic_build::GenericBuildError;
use crate::manifest::{Manifest, PackagedFile};
use crate::sync_utils::{ByteStreamSyncReceiver, ByteStreamSyncSender};
use futures_util::stream::StreamExt;
use hyper::Body;
use rand::Rng;
use tar::{Archive, Builder, EntryType, Header};
use tee_readwrite::TeeReader;
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;
use tokio_task_manager::Task;

/// Used to stop container when dropped, relies on using [tokio_task_manager::TaskManager::wait]
/// to ensure `Drop` will run to completion
pub struct ReclaimableContainer {
    pub id: String,
    docker: Docker,
    task: Task,
}

impl ReclaimableContainer {
    #[must_use]
    pub fn new(name: String, docker: &Docker, task: Task) -> Self {
        Self {
            id: name,
            docker: docker.clone(),
            task,
        }
    }
}

impl Drop for ReclaimableContainer {
    fn drop(&mut self) {
        let docker = self.docker.clone();
        let id = self.id.clone();
        let handle = tokio::runtime::Handle::current();
        let mut task = self.task.clone();
        handle.spawn(async move {
            println!("Stopping {id}");
            docker
                .stop_container(id.clone().as_str(), None)
                .await
                .expect("error stopping container");
            println!("Stopped {id}");
            task.wait().await;
        });
    }
}

pub async fn exec_in_container(
    docker: Docker,
    container_id: &str,
    command: Vec<&str>,
) -> Result<String, anyhow::Error> {
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
    let start_exec_result = log_output.await?;

    let mut total_output = String::new();
    match start_exec_result {
        StartExecResults::Attached { output, .. } => {
            let mut output = output
                .map(|result| match result {
                    Ok(log_output) => {
                        println!("{log_output}");
                        total_output.push_str(log_output.to_string().as_str());
                    }
                    Err(error) => eprintln!("Error while reading log output: {error}"),
                })
                .fuse();

            // Run the output stream to completion.
            while output.next().await.is_some() {}
        }
        StartExecResults::Detached => {
            println!("Exec started in detached mode");
        }
    }
    Ok::<String, anyhow::Error>(total_output)
}

pub async fn run_temporary_container(
    docker: Docker,
    image: &str,
    _task: Task
) -> Result<ReclaimableContainer, anyhow::Error> {

    let options = Some(CreateContainerOptions {
        name: image.to_string(),
        platform: None,
    });

    let host_config = HostConfig {
        auto_remove: Some(true),
        ..Default::default()
    };

    let config = Config {
        image: Some(image.to_string()),
        entrypoint: Some(vec!["sleep".to_string()]),
        cmd: Some(vec!["300".to_string()]),
        user: Some("root".to_string()),
        host_config: Some(host_config),
        ..Default::default()
    };

    let container = docker.create_container(options, config).await?;
    docker
        .start_container(&container.id, None::<StartContainerOptions<String>>)
        .await?;

    // This will stop the container, whether we return an error or not
    return Ok(ReclaimableContainer::new(container.id.clone(), &docker, _task));
}



pub async fn find_installed_extension_files(docker: Docker, container_id: &str) -> Result<HashMap<String, Vec<String>>, anyhow::Error> {

    let sharedir = exec_in_container(
        docker.clone(),
        container_id,
        vec!["pg_config", "--sharedir"],
    )
        .await?;
    let sharedir = sharedir.trim();

    let pkglibdir = exec_in_container(
        docker.clone(),
        container_id,
        vec!["pg_config", "--pkglibdir"],
    )
        .await?;
    let pkglibdir = pkglibdir.trim();

    // collect changes from container filesystem
    println!("Collecting files installed by this extension...");
    let changes = docker
        .container_changes(container_id)
        .await?
        .expect("Expected to find changed files");

    let mut pkglibdir_list = vec![];
    let mut sharedir_list = vec![];

    for change in changes {
        if change.kind == 1
            && (change.path.ends_with(".so")
            || change.path.ends_with(".bc")
            || change.path.ends_with(".sql")
            || change.path.ends_with(".control"))
        {
            if change.path.starts_with(pkglibdir.clone()) {
                let file_in_pkglibdir = change.path;
                let file_in_pkglibdir = file_in_pkglibdir.strip_prefix(pkglibdir);
                let file_in_pkglibdir = file_in_pkglibdir.unwrap();
                let file_in_pkglibdir = file_in_pkglibdir.trim_start_matches('/');
                pkglibdir_list.push(file_in_pkglibdir.to_owned());
            } else if change.path.starts_with(sharedir.clone()) {
                let file_in_sharedir = change.path;
                let file_in_sharedir = file_in_sharedir.strip_prefix(sharedir);
                let file_in_sharedir = file_in_sharedir.unwrap();
                let file_in_sharedir = file_in_sharedir.trim_start_matches('/');
                sharedir_list.push(file_in_sharedir.to_owned());
            } else {
                println!(
                    "WARNING: file {} is not in pkglibdir or sharedir",
                    change.path
                );
            }
        }
    }

    println!("Sharedir files:");
    for sharedir_file in sharedir_list.clone() {
        println!("{sharedir_file}");
    }
    println!("Pkglibdir files:");
    for pkglibdir_file in pkglibdir_list.clone() {
        println!("{pkglibdir_file}");
    }

    let mut result = HashMap::new();
    result.insert("sharedir".to_string(), sharedir_list);
    result.insert("pkglibdir".to_string(), pkglibdir_list);
    return Ok(result);
}

// Build an image
// The Dockerfile and build directory can be in different directories.
// The caller provides an image name prefix, and this function returns
// the complete image name.
pub async fn build_image(
    docker: Docker,
    image_name_prefix: &str,
    dockerfile_path: &str,
    build_directory: &Path,
    build_args: HashMap<&str, &str>
) -> Result<String, anyhow::Error> {

    let dockerfile = dockerfile_path.to_owned();

    let random_suffix = {
        let mut rng = rand::thread_rng();
        rng.gen_range(0..1000000).to_string()
    };

    let image_name = format!("{}{}", image_name_prefix.to_owned(), &random_suffix);

    let (receiver, sender, stream) = ByteStreamSyncSender::new();

    // Making build_directory owned so we can send it to the tarring task below without having to worry
    // about the lifetime of the reference.
    let build_directory = build_directory.to_owned();

    // The docker API receives the build environment as a tar ball.
    task::spawn_blocking(move || {
        let f = || {
            let mut tar = tar::Builder::new(stream);
            tar.append_dir_all(".", build_directory)?;

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

    let image_name = image_name.to_owned();

    let options = BuildImageOptions {
        dockerfile: "Dockerfile",
        t: &image_name.clone(),
        rm: true,
        buildargs: build_args,
        ..Default::default()
    };

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
    return Ok(image_name.to_string());
}

// Scan sharedir and package lib dir from a Trunk builder container for files from a provided list.
// Package these files into a Trunk package.
pub async fn package_installed_extension_files(
    docker: Docker,
    container_id: &str,
    package_path: &str,
    extension_name: &str,
    extension_version: &str,
) -> Result<(), anyhow::Error> {

    let extension_name = extension_name.to_owned();
    let extension_version = extension_version.to_owned();

    let sharedir = exec_in_container(
        docker.clone(),
        container_id,
        vec!["pg_config", "--sharedir"],
    )
        .await?;
    let sharedir = sharedir.trim();

    let pkglibdir = exec_in_container(
        docker.clone(),
        container_id,
        vec!["pg_config", "--pkglibdir"],
    )
        .await?;
    let pkglibdir = pkglibdir.trim();

    let extension_files = find_installed_extension_files(docker.clone(), container_id).await?;

    let sharedir_list = extension_files["sharedir"].clone();
    let pkglibdir_list = extension_files["pkglibdir"].clone();

    let pkglibdir = pkglibdir.to_owned();
    let sharedir = sharedir.to_owned();

    // In this function, we open and work with .tar only, then we finalize the package with a .gz in a separate call
    let package_path = format!("{package_path}/{extension_name}-{extension_version}.tar");
    println!("Creating package at: {package_path}");
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
        let mut new_archive = Builder::new(file);
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
                    let full_path = format!("/{}", path.to_str().unwrap_or(""));
                    let trimmed = full_path
                        .trim_start_matches(&format!("{}/", pkglibdir.clone()))
                        .trim_start_matches(&format!("{}/", sharedir.clone()))
                        .to_string();
                    let pkglibdir_match = pkglibdir_list.contains(&trimmed);
                    let sharedir_match = sharedir_list.contains(&trimmed);
                    // Check if we found a file to package
                    if !(sharedir_match || pkglibdir_match) {
                        continue;
                    }
                    println!("Detected file to package: {trimmed}");
                    if path.to_str() == Some("manifest.json") {
                        println!("Found manifest.json, merging additions with existing manifest");
                        manifest.merge(serde_json::from_reader(entry)?);
                    } else {
                        let root_path = Path::new("/");
                        let path = root_path.join(path);
                        let mut path = path.as_path();
                        println!("Packaging file {:?}", path.clone());
                        // trim pkglibdir or sharedir from start of path
                        if path.to_string_lossy().contains(&pkglibdir) {
                            path = path.strip_prefix(pkglibdir.trim_end_matches("lib").clone())?;
                        } else if path.to_string_lossy().contains(&sharedir) {
                            path = path.strip_prefix(format!("{}/", sharedir.clone()))?;
                        } else {
                            println!("WARNING: Skipping file because it's not in sharedir or pkglibdir {:?}", path.clone());
                            continue;
                        }

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
                                println!(
                                    "Adding file {} to manifest",
                                    path.clone().to_string_lossy()
                                );
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
                                        println!("Detected architecture: {target_arch}");
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

    Ok(())
}
