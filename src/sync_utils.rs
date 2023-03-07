use futures_util::stream::StreamExt;
use futures_util::Stream;
use std::fmt::Display;
use std::io::{Read, Write};
use std::{cmp, io};
use tokio::sync::mpsc;

/// Sends a byte stream in chunks to [tokio::mpsc] channel
///
/// It implements [std::io::Write] so it can be used in a sync task
pub(crate) struct ByteStreamSyncSender {
    sender: mpsc::Sender<Result<Vec<u8>, io::Error>>,
    buffer: Vec<u8>,
}

impl ByteStreamSyncSender {
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

impl Drop for ByteStreamSyncSender {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl Write for ByteStreamSyncSender {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        if self.buffer.len() > BUFFER_SIZE {
            self.flush()?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let message = std::mem::take(&mut self.buffer);
        self.sender
            .blocking_send(Ok(message))
            // The error happens when the channel is closed, and at this point we
            // don't need to send anything as there's no receiver.
            .or(Ok(()))
    }
}

/// Receives a byte stream in chunks from [tokio::mpsc] channel
///
/// It implements [std::io::Read] so it can be used in a sync task
pub struct ByteStreamSyncReceiver {
    receiver: mpsc::Receiver<Vec<u8>>,
    sender: mpsc::Sender<Vec<u8>>,
    buffer: Vec<u8>,
}

/// Used to send byte stream from async to sync ([ByteStreamSyncReceiver])
pub struct ByteStreamReceiverAsyncSender {
    sender: mpsc::Sender<Vec<u8>>,
}

// The number is completely arbitrary at the moment
const RECEIVER_CHANNEL_BUFFER_SIZE: usize = 24;

impl ByteStreamSyncReceiver {
    /// Creates a new ByteStream
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(RECEIVER_CHANNEL_BUFFER_SIZE);
        Self {
            receiver,
            sender,
            buffer: Vec::new(),
        }
    }

    /// Returns a handle that can send
    pub fn sender(&self) -> ByteStreamReceiverAsyncSender {
        ByteStreamReceiverAsyncSender {
            sender: self.sender.clone(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ByteStreamReceiverSenderError<E: Display> {
    #[error("send error: {0}")]
    SendError(#[from] mpsc::error::SendError<Vec<u8>>),

    #[error("stream error: {0}")]
    StreamError(E),
}

impl ByteStreamReceiverAsyncSender {
    pub async fn stream_to_end<
        S: Unpin + Stream<Item = Result<B, E>>,
        B: Into<Vec<u8>>,
        E: Display,
    >(
        self,
        mut stream: S,
    ) -> Result<(), ByteStreamReceiverSenderError<E>> {
        while let Some(next) = stream.next().await {
            match next {
                Ok(bytes) => {
                    self.sender.send(bytes.into()).await?;
                }
                Err(err) => {
                    return Err(ByteStreamReceiverSenderError::StreamError(err));
                }
            }
        }
        Ok(())
    }
}

impl Read for ByteStreamSyncReceiver {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Serve from the buffer first
        let mut received_bytes = if !self.buffer.is_empty() {
            std::mem::take(&mut self.buffer)
        } else {
            // Otherwise, read from the receiver
            match self.receiver.blocking_recv() {
                None => return Ok(0),
                Some(bytes) => bytes,
            }
        };
        // Combine existing buffer with the received bytes
        // TODO: optimize for the first case (of serving the buffer)
        let mut bytes = std::mem::take(&mut self.buffer);
        bytes.append(&mut received_bytes);

        // Finding how much we can it into `buf`
        let amt = cmp::min(buf.len(), bytes.len());
        let (a, b) = bytes.split_at(amt);

        // The remainder of the entire buffer goes back into the buffer
        if !b.is_empty() {
            self.buffer.extend_from_slice(b);
        }

        // First check if the amount of bytes we want to read is small:
        // `copy_from_slice` will generally expand to a call to `memcpy`, and
        // for a single byte the overhead is significant.
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        Ok(amt)
    }
}

const BUFFER_SIZE: usize = 8192;
