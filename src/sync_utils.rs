use std::io::{Read, Write};
use std::{cmp, io};
use tokio::sync::mpsc;

/// Sends a byte stream in chunks to [tokio::mpsc] channel
///
/// It implements [std::io::Write] so it can be used in a sync task
pub(crate) struct ByteStreamSender {
    sender: mpsc::Sender<Result<Vec<u8>, io::Error>>,
    buffer: Vec<u8>,
}

impl ByteStreamSender {
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

impl Drop for ByteStreamSender {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl Write for ByteStreamSender {
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
pub struct ByteStreamReceiver {
    receiver: mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
}

// The number is completely arbitrary at the moment
const RECEIVER_CHANNEL_BUFFER_SIZE: usize = 24;

impl ByteStreamReceiver {
    /// Creates a new ByteStream
    pub fn new() -> (mpsc::Sender<Vec<u8>>, Self) {
        let (sender, receiver) = mpsc::channel(RECEIVER_CHANNEL_BUFFER_SIZE);
        (
            sender,
            Self {
                receiver,
                buffer: Vec::new(),
            },
        )
    }
}

impl Read for ByteStreamReceiver {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Serve from the buffer first
        let mut received_bytes = if !self.buffer.is_empty() {
            std::mem::replace(&mut self.buffer, Vec::new())
        } else {
            // Otherwise, read from the receiver
            match self.receiver.blocking_recv() {
                None => return Ok(0),
                Some(bytes) => bytes,
            }
        };
        // Combine existing buffer with the received bytes
        // TODO: optimize for the first case (of serving the buffer)
        let mut bytes = std::mem::replace(&mut self.buffer, Vec::new());
        bytes.append(&mut received_bytes);

        // Finding how much we can it into `buf`
        let amt = cmp::min(buf.len(), bytes.len());
        let (a, b) = bytes.split_at(amt);

        // The remainder of the entire buffer goes back into the buffer
        if b.len() > 0 {
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
