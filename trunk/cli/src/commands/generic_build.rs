use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::models::HostConfig;

use std::collections::HashMap;
use std::default::Default;

use std::include_str;
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

use crate::commands::containers::exec_in_container;

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
    _output_path: &str,
    extension_name: &str,
    extension_version: &str,
    _task: Task,
) -> Result<(), GenericBuildError> {
    println!("Building with name {}", &extension_name);
    println!("Building with version {}", &extension_version);

    let dockerfile = include_str!("./builders/Dockerfile.generic");

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

    let mut image_name = "generic_trunk_builder_".to_string();

    let random_suffix = {
        let mut rng = rand::thread_rng();
        rng.gen_range(0..1000000000).to_string()
    };

    image_name.push_str(&random_suffix);
    let image_name = image_name.as_str().to_owned();

    let build_args = HashMap::new();
    // build_args.insert("EXTENSION_NAME", extension_name);
    // build_args.insert("EXTENSION_VERSION", extension_version);

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

    // This will stop the container, whether we return an error or not
    // let _ = ReclaimableContainer::new(&container.id, &docker, task);

    println!("sharedir is:");
    let sharedir = exec_in_container(
        docker.clone(),
        &container.id,
        vec!["pg_config", "--sharedir"],
    )
    .await?;
    let sharedir = sharedir.trim();
    println!("pkglibdir is:");
    let pkglibdir = exec_in_container(
        docker.clone(),
        &container.id,
        vec!["pg_config", "--pkglibdir"],
    )
    .await?;
    let pkglibdir = pkglibdir.trim();

    println!("Determining installation files...");
    let _exec_output =
        exec_in_container(docker.clone(), &container.id, vec!["make", "install"]).await?;

    // collect changes from container filesystem
    println!("Collecting files...");
    let changes = docker
        .container_changes(&container.id)
        .await?
        .expect("Expected to find changed files");
    // print all the changes
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
                return Err(GenericBuildError::InvalidFileInstalled(change.path));
            }
        }
    }

    println!("Sharedir files:");
    for sharedir_file in sharedir_list {
        println!("{sharedir_file}");
    }
    println!("Pkglibdir files:");
    for pkglibdir_file in pkglibdir_list {
        println!("{pkglibdir_file}");
    }

    // output_path is the locally output path
    // fs::create_dir_all(output_path)?;

    // let mut manifest_files = Vec::new();

    // let output_path = output_path.to_owned();
    // let package_path = format!("{output_path}/{extension_name}-{extension_version}.tar.gz");
    // let file = File::create(&package_path)?;

    // let options = Some(DownloadFromContainerOptions { path: output_dir });
    // let file_stream = docker.download_from_container(&container.id, options);

    // let receiver = ByteStreamSyncReceiver::new();
    // let receiver_sender = receiver.sender();
    // let output_path = output_path.to_owned();
    // let extension_name = extension_name.to_owned();
    // let extension_version = extension_version.to_owned();
    // let package_path = format!("{output_path}/{extension_name}-{extension_version}.tar.gz");
    // let file = File::create(&package_path)?;

    // let tar_handle = task::spawn_blocking(move || {
    //     let mut archive = Archive::new(receiver);
    //     let mut new_archive = Builder::new(flate2::write::GzEncoder::new(
    //         file,
    //         flate2::Compression::default(),
    //     ));
    //     let mut manifest = Manifest {
    //         extension_name,
    //         extension_version,
    //         sys: "linux".to_string(),
    //         files: None,
    //     };
    //     if let Ok(entries) = archive.entries() {
    //         for entry in entries {
    //             if let Ok(entry) = entry {
    //                 let name = entry.path()?.to_path_buf();
    //                 if name.to_str() == Some("manifest.json") {
    //                     manifest.merge(serde_json::from_reader(entry)?);
    //                 } else {
    //                     let name = name.strip_prefix("trunk-output")?;

    //                     if !name.to_string_lossy().is_empty() {
    //                         let mut header = Header::new_gnu();
    //                         header.set_mode(entry.header().mode()?);
    //                         header.set_mtime(entry.header().mtime()?);
    //                         header.set_size(entry.size());
    //                         header.set_cksum();
    //                         let entry_type = entry.header().entry_type();

    //                         let mut buf = Vec::new();
    //                         let mut tee = TeeReader::new(entry, &mut buf, true);

    //                         new_archive.append_data(&mut header, name, &mut tee)?;

    //                         let (_entry, buf) = tee.into_inner();

    //                         if entry_type == EntryType::file() {
    //                             let file = manifest.add_file(name);
    //                             match file {
    //                                 PackagedFile::SharedObject {
    //                                     ref mut architecture,
    //                                     ..
    //                                 } => {
    //                                     let elf = ElfBytes::<AnyEndian>::minimal_parse(buf)?;
    //                                     let target_arch = match elf.ehdr.e_machine {
    //                                         elf::abi::EM_386 => "x86",
    //                                         elf::abi::EM_X86_64 => "x86_64",
    //                                         elf::abi::EM_AARCH64 => "aarch64",
    //                                         elf::abi::EM_ARM => "aarch32",
    //                                         _ => "unknown",
    //                                     }
    //                                         .to_string();
    //                                     architecture.replace(target_arch);
    //                                 }
    //                                 _ => {}
    //                             }
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //     }
    //     let manifest = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    //     let mut header = Header::new_gnu();
    //     header.set_size(manifest.as_bytes().len() as u64);
    //     header.set_cksum();
    //     header.set_mode(0o644);
    //     new_archive.append_data(&mut header, "manifest.json", Cursor::new(manifest))?;
    //     Ok::<_, GenericBuildError>(())
    // });

    // // Wait until completion of streaming, but ignore its error as it would only error out
    // // if tar_handle errors out.
    // let _ = receiver_sender.stream_to_end(file_stream).await;
    // // Handle the error
    // tar_handle.await??;

    Ok(())
}
