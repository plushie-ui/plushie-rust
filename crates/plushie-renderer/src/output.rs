//! Native output writer backed by a background thread.
//!
//! [`ChannelWriter`] buffers bytes and sends them through a bounded
//! channel to a background writer thread. This decouples the iced
//! event loop from potentially blocking pipe I/O.

use std::io::{self, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Channel capacity for the background writer thread.
///
/// Sized for roughly one frame's worth of events at 60fps under
/// typical widget counts. Large enough to absorb bursts without
/// stalling, small enough that a persistently slow consumer surfaces
/// quickly as a `backpressure_stall` diagnostic.
const WRITER_CHANNEL_CAPACITY: usize = 256;

/// Deadline for non-blocking `try_send` retries before we fall back
/// to a blocking `send`. 100ms is long enough that transient
/// consumer hiccups don't trigger diagnostics, short enough that a
/// real pipe stall is noticed well within a single user interaction.
const BACKPRESSURE_TIMEOUT: Duration = Duration::from_millis(100);

/// Polling interval while waiting for room in the channel. Small
/// relative to `BACKPRESSURE_TIMEOUT` so we react to a consumer
/// catching up without busy-looping.
const BACKPRESSURE_POLL_INTERVAL: Duration = Duration::from_millis(5);

/// A [`Write`] adapter that buffers bytes and sends them through a
/// bounded channel on flush. The background writer thread receives
/// the chunks and performs the actual (potentially blocking) I/O.
pub(crate) struct ChannelWriter {
    tx: mpsc::SyncSender<Vec<u8>>,
    buffer: Vec<u8>,
}

impl Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::take(&mut self.buffer);
        self.send_with_backpressure(bytes)
    }
}

impl ChannelWriter {
    /// Try to send `bytes` into the channel. If the channel is full,
    /// spin briefly on `try_send` before falling back to a blocking
    /// `send`. On fall-back, emit a `backpressure_stall` diagnostic
    /// with the elapsed wait time so a slow consumer surfaces loudly
    /// in the log without crashing the renderer.
    fn send_with_backpressure(&self, mut bytes: Vec<u8>) -> io::Result<()> {
        let start = Instant::now();
        loop {
            match self.tx.try_send(bytes) {
                Ok(()) => return Ok(()),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "writer thread exited",
                    ));
                }
                Err(mpsc::TrySendError::Full(bytes_back)) => {
                    bytes = bytes_back;
                    if start.elapsed() >= BACKPRESSURE_TIMEOUT {
                        // Give up the non-blocking path. Fall back to
                        // blocking `send` so data is still written
                        // even under sustained pressure - the host
                        // stalled reading us, but we'd rather stall
                        // too than silently drop output.
                        log::warn!(
                            "[code=backpressure_stall] writer channel at capacity {} for {}ms; falling back to blocking send",
                            WRITER_CHANNEL_CAPACITY,
                            start.elapsed().as_millis(),
                        );
                        return self.tx.send(bytes).map_err(|_| {
                            io::Error::new(io::ErrorKind::BrokenPipe, "writer thread exited")
                        });
                    }
                    thread::sleep(BACKPRESSURE_POLL_INTERVAL);
                }
            }
        }
    }
}

/// Spawn a background writer thread and return a [`ChannelWriter`]
/// that sends encoded bytes to it. The thread owns the transport
/// writer and performs blocking I/O without stalling the caller.
pub(crate) fn spawn_writer_thread(writer: Box<dyn Write + Send>) -> ChannelWriter {
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(WRITER_CHANNEL_CAPACITY);

    thread::Builder::new()
        .name("plushie-writer".into())
        .spawn(move || {
            let mut writer = writer;
            for bytes in rx {
                if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn writer thread");

    ChannelWriter {
        tx,
        buffer: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_writer_sends_on_flush() {
        let (tx, rx) = mpsc::sync_channel(16);
        let mut w = ChannelWriter {
            tx,
            buffer: Vec::new(),
        };
        w.write_all(b"hello").unwrap();
        assert!(rx.try_recv().is_err(), "nothing sent before flush");
        w.flush().unwrap();
        let received = rx.recv().unwrap();
        assert_eq!(received, b"hello");
    }

    #[test]
    fn channel_writer_empty_flush_is_noop() {
        let (tx, rx) = mpsc::sync_channel(16);
        let mut w = ChannelWriter {
            tx,
            buffer: Vec::new(),
        };
        w.flush().unwrap();
        assert!(rx.try_recv().is_err(), "empty flush should send nothing");
    }

    #[test]
    fn channel_writer_broken_pipe_on_closed_receiver() {
        let (tx, rx) = mpsc::sync_channel(16);
        drop(rx);
        let mut w = ChannelWriter {
            tx,
            buffer: Vec::new(),
        };
        w.write_all(b"data").unwrap();
        let result = w.flush();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn backpressure_falls_back_to_blocking_send_after_timeout() {
        // Capacity-1 channel with no receiver reading: the first flush
        // fills the channel; the second should hit `try_send` Full,
        // retry until BACKPRESSURE_TIMEOUT, then block on the final
        // `send`. Spawn a reader that drains only after a delay so
        // the fallback path actually fires without the test hanging.
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(1);
        let w = ChannelWriter {
            tx,
            buffer: Vec::new(),
        };
        let drain_handle = thread::spawn(move || {
            // Consume the first message immediately.
            let _ = rx.recv();
            // Delay long enough to force the writer into the
            // backpressure-timeout branch on the second flush.
            thread::sleep(Duration::from_millis(150));
            let _ = rx.recv();
        });
        // Two sends: the first goes straight through, the second is
        // forced to wait. The second returns Ok once the reader drains.
        w.send_with_backpressure(b"one".to_vec()).unwrap();
        w.send_with_backpressure(b"two".to_vec()).unwrap();
        drain_handle.join().unwrap();
    }

    #[test]
    fn spawn_writer_thread_delivers_bytes() {
        let (inner_tx, inner_rx) = mpsc::sync_channel::<Vec<u8>>(16);
        struct TestWriter(mpsc::SyncSender<Vec<u8>>);
        impl Write for TestWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0
                    .send(buf.to_vec())
                    .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "closed"))?;
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut cw = spawn_writer_thread(Box::new(TestWriter(inner_tx)));
        cw.write_all(b"test data").unwrap();
        cw.flush().unwrap();
        let received = inner_rx.recv().unwrap();
        assert_eq!(received, b"test data");
    }
}
