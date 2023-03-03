use std::default::Default;
use std::include_str;
use std::io;
use std::io::{ErrorKind, Write};
use std::path::Path;

use futures_util::stream::StreamExt;

use tar::Header;
use thiserror::Error;

use bollard::image::BuildImageOptions;
use bollard::Docker;

use bollard::models::BuildInfo;
use hyper::Body;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Error, Debug)]
pub enum PgxBuildError {
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Docker Error: {0}")]
    DockerError(#[from] bollard::errors::Error),
}

/// Sends a byte stream in chunks to [tokio::mpsc] channel
///
/// It implements [std::io::Write] so it can be used in a sync task
pub(crate) struct ByteStream {
    sender: mpsc::Sender<Result<Vec<u8>, io::Error>>,
    buffer: Vec<u8>,
}

impl ByteStream {
    /// Creates a new ByteStream
    pub(crate) fn new() -> (
        mpsc::Receiver<Result<Vec<u8>, io::Error>>,
        mpsc::Sender<Result<Vec<u8>, io::Error>>,
        Self,
    ) {
        let (sender, receiver) = mpsc::channel(1);
        let stream = Self {
            sender: sender.clone(),
            buffer: Vec::new(),
        };
        (receiver, sender, stream)
    }
}

impl Drop for ByteStream {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

const BUFFER_SIZE: usize = 8192;

impl Write for ByteStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        if self.buffer.len() > BUFFER_SIZE {
            self.flush()?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut message = std::mem::replace(&mut self.buffer, Vec::new());
        loop {
            match self.sender.try_send(Ok(message)) {
                // Success
                Ok(()) => return Ok(()),
                // Retry
                Err(TrySendError::Full(Ok(msg))) => message = msg,
                // We never send errors, so this is unreachable
                Err(TrySendError::Full(Err(_))) => unreachable!(),
                // No longer need to send anything
                Err(TrySendError::Closed(_)) => {
                    return Err(std::io::Error::from(ErrorKind::BrokenPipe))
                }
            }
        }
    }
}

pub async fn build_pgx(path: &Path, _output_path: &str) -> Result<(), PgxBuildError> {
    println!("Building pgx extension at path {}", &path.display());
    let dockerfile = include_str!("./pgx_builder/Dockerfile");

    let (receiver, sender, stream) = ByteStream::new();
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
        ()
    });

    let options = BuildImageOptions {
        dockerfile: "Dockerfile",
        t: "temp",
        rm: true,
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

    Ok(())
}
