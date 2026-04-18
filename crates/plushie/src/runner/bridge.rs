//! Wire protocol bridge: subprocess management and message framing.
//!
//! The bridge spawns a plushie renderer binary as a child process and
//! communicates over stdin/stdout using length-prefixed MessagePack
//! or JSONL framing.
//!
//! Reading is handled by a background thread that feeds a bounded
//! channel so the main event loop can `recv_timeout` and detect
//! heartbeat silence without blocking forever on a stuck renderer.
//!
//! Internal helpers in this module are `pub` so the wire runner can
//! compose them directly; their error contracts are captured by the
//! inner `io::Error` / framing layer rather than per-method rustdoc.
#![allow(clippy::missing_errors_doc)]

#[cfg(feature = "wire")]
use std::io::{self, BufRead, BufReader, Read, Write};
#[cfg(feature = "wire")]
use std::process::{Child, ChildStdout, Command as ProcessCommand, Stdio};
#[cfg(feature = "wire")]
use std::sync::Arc;
#[cfg(feature = "wire")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "wire")]
use std::sync::mpsc;
#[cfg(feature = "wire")]
use std::thread::{self, JoinHandle};
#[cfg(feature = "wire")]
use std::time::Duration;

#[cfg(feature = "wire")]
use plushie_core::outgoing_message::OutgoingMessage;
#[cfg(feature = "wire")]
use serde_json::Value;

/// Wire protocol codec selection.
#[cfg(feature = "wire")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// JSON Lines (one JSON object per line, newline-delimited).
    Json,
    /// MessagePack with 4-byte big-endian length prefix.
    MsgPack,
}

/// A connection to a renderer subprocess.
#[cfg(feature = "wire")]
pub struct Bridge {
    child: Child,
    codec: Codec,
    /// Buffered reader owning the child's stdout.
    ///
    /// Held by the struct (rather than created per-call in
    /// [`Self::receive`]) so JSON-mode reads don't discard the
    /// lookahead buffer between calls. `BufReader::new(stdout)` on
    /// every call would drop any bytes the previous call had already
    /// pulled in past the newline, corrupting framing on back-to-back
    /// renderer messages.
    ///
    /// `None` after [`Self::start_reader`] takes ownership for the
    /// background reader thread.
    sync_stdout: Option<BufReader<ChildStdout>>,
    /// Background reader thread state. `None` between spawn() and
    /// the first start_reader() call (used by tests that don't need
    /// a reader).
    reader: Option<ReaderHandle>,
}

/// Background reader thread and its incoming-message channel.
#[cfg(feature = "wire")]
struct ReaderHandle {
    rx: mpsc::Receiver<io::Result<Value>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// Result of waiting for the next renderer message.
#[cfg(feature = "wire")]
pub enum Incoming {
    /// A message was decoded successfully.
    Message(Value),
    /// Bridge read failed. Typed to support classify_exit downstream.
    Error(io::Error),
    /// No message received within the requested timeout.
    Timeout,
}

#[cfg(feature = "wire")]
impl Bridge {
    /// Spawn a renderer subprocess and negotiate the codec.
    ///
    /// The child env is filtered down to the canonical whitelist (see
    /// [`crate::runner::env::renderer_env`]). Host secrets, tokens, or
    /// other unrelated variables do not reach the renderer even when
    /// they are present in the host process env.
    pub fn spawn(binary_path: &str) -> io::Result<Self> {
        let mut child = ProcessCommand::new(binary_path)
            .env_clear()
            .envs(crate::runner::env::renderer_env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Wrap stdout in a BufReader up-front so the sync receive() path
        // keeps any lookahead buffering across calls. start_reader()
        // takes this back out if the background reader is started.
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdout unavailable"))?;

        Ok(Self {
            child,
            codec: Codec::Json, // default, may be negotiated via hello
            sync_stdout: Some(BufReader::new(stdout)),
            reader: None,
        })
    }

    /// Start the background reader thread.
    ///
    /// Must be called after codec negotiation (i.e. after `set_codec`
    /// if the hello message changed the codec). Takes ownership of
    /// the child's stdout handle.
    pub fn start_reader(&mut self) -> io::Result<()> {
        if self.reader.is_some() {
            return Ok(());
        }
        // Hand the owning BufReader (with any lookahead already
        // buffered by sync receive() calls during hello processing)
        // over to the background reader thread. Framing is preserved
        // across the handoff because the reader thread keeps reading
        // from the same buffered handle.
        let reader = self
            .sync_stdout
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdout already taken"))?;
        let (tx, rx) = mpsc::sync_channel::<io::Result<Value>>(256);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = stop.clone();
        let codec = self.codec;
        let thread = thread::Builder::new()
            .name("plushie-wire-reader".into())
            .spawn(move || reader_loop(reader, codec, tx, stop_for_thread))?;
        self.reader = Some(ReaderHandle {
            rx,
            stop,
            thread: Some(thread),
        });
        Ok(())
    }

    /// Wait for the next message or report a timeout.
    ///
    /// If `timeout` is `None`, blocks until a message arrives or the
    /// channel is closed (indicates the reader thread exited, i.e.
    /// the renderer disconnected).
    pub fn recv_timeout(&mut self, timeout: Option<Duration>) -> Incoming {
        let Some(reader) = self.reader.as_ref() else {
            return Incoming::Error(io::Error::other("reader not started"));
        };
        let recv_result = match timeout {
            Some(dur) => reader.rx.recv_timeout(dur),
            None => reader
                .rx
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match recv_result {
            Ok(Ok(msg)) => Incoming::Message(msg),
            Ok(Err(e)) => Incoming::Error(e),
            Err(mpsc::RecvTimeoutError::Timeout) => Incoming::Timeout,
            Err(mpsc::RecvTimeoutError::Disconnected) => Incoming::Error(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "reader disconnected",
            )),
        }
    }

    /// Stop the background reader thread (if running) and reset so
    /// `start_reader` can be called again after a restart.
    pub fn stop_reader(&mut self) {
        if let Some(mut reader) = self.reader.take() {
            reader.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = reader.thread.take() {
                let _ = handle.join();
            }
        }
    }

    /// Send a typed message to the renderer's stdin.
    ///
    /// Encode failures return [`crate::Error::WireEncode`]; I/O
    /// failures return [`crate::Error::Io`].
    pub fn send(&mut self, message: &OutgoingMessage) -> crate::Result {
        let stdin = self.child.stdin.as_mut().ok_or_else(|| {
            crate::Error::Io(io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed"))
        })?;

        match self.codec {
            Codec::Json => {
                let json = serde_json::to_string(message)
                    .map_err(|e| crate::Error::WireEncode(e.to_string()))?;
                writeln!(stdin, "{json}")?;
                stdin.flush()?;
            }
            Codec::MsgPack => {
                let bytes = rmp_serde::to_vec(message)
                    .map_err(|e| crate::Error::WireEncode(e.to_string()))?;
                let len = (bytes.len() as u32).to_be_bytes();
                stdin.write_all(&len)?;
                stdin.write_all(&bytes)?;
                stdin.flush()?;
            }
        }

        Ok(())
    }

    /// Send a settings message.
    pub fn send_settings(&mut self, settings: &Value) -> crate::Result {
        self.send(&OutgoingMessage::Settings {
            session: String::new(),
            settings: settings.clone(),
        })
    }

    /// Send a full tree snapshot.
    pub fn send_snapshot(&mut self, tree: &Value) -> crate::Result {
        self.send(&OutgoingMessage::Snapshot {
            session: String::new(),
            tree: tree.clone(),
        })
    }

    /// Send incremental patches.
    pub fn send_patch(&mut self, ops: &[Value]) -> crate::Result {
        self.send(&OutgoingMessage::Patch {
            session: String::new(),
            ops: ops.to_vec(),
        })
    }

    /// Send a subscribe message.
    pub fn send_subscribe(
        &mut self,
        kind: &str,
        tag: &str,
        max_rate: Option<u32>,
        window_id: Option<&str>,
    ) -> crate::Result {
        self.send(&OutgoingMessage::Subscribe {
            session: String::new(),
            kind: kind.to_string(),
            tag: tag.to_string(),
            max_rate,
            window_id: window_id.map(String::from),
        })
    }

    /// Send an unsubscribe message.
    pub fn send_unsubscribe(&mut self, kind: &str, tag: &str) -> crate::Result {
        self.send(&OutgoingMessage::Unsubscribe {
            session: String::new(),
            kind: kind.to_string(),
            tag: tag.to_string(),
        })
    }

    /// Send a widget operation (focus, scroll, etc.).
    pub fn send_widget_op(&mut self, op: &str, payload: &Value) -> crate::Result {
        self.send(&OutgoingMessage::WidgetOp {
            session: String::new(),
            op: op.to_string(),
            payload: payload.clone(),
        })
    }

    /// Send a widget-targeted command.
    pub fn send_command(&mut self, id: &str, family: &str, value: &Value) -> crate::Result {
        self.send(&OutgoingMessage::Command {
            session: String::new(),
            id: id.to_string(),
            family: family.to_string(),
            value: value.clone(),
        })
    }

    /// Send an atomic batch of widget-targeted commands.
    pub fn send_commands(
        &mut self,
        commands: Vec<plushie_core::ops::WidgetCommand>,
    ) -> crate::Result {
        self.send(&OutgoingMessage::Commands {
            session: String::new(),
            commands,
        })
    }

    /// Send a window operation.
    ///
    /// Uses the unified `_op` envelope: op-specific data lives under
    /// `payload`; routing fields (`op`, `window_id`) stay flat.
    pub fn send_window_op(&mut self, op: &str, window_id: &str, payload: &Value) -> crate::Result {
        self.send(&OutgoingMessage::WindowOp {
            session: String::new(),
            op: op.to_string(),
            window_id: window_id.to_string(),
            payload: payload.clone(),
        })
    }

    /// Send an effect request.
    pub fn send_effect(&mut self, id: &str, kind: &str, payload: &Value) -> crate::Result {
        self.send(&OutgoingMessage::Effect {
            session: String::new(),
            id: id.to_string(),
            kind: kind.to_string(),
            payload: payload.clone(),
        })
    }

    /// Send an interact message for automation.
    pub fn send_interact(
        &mut self,
        id: &str,
        action: &str,
        selector: &Value,
        payload: &Value,
    ) -> crate::Result {
        self.send(&OutgoingMessage::Interact {
            session: String::new(),
            id: id.to_string(),
            action: action.to_string(),
            selector: selector.clone(),
            payload: payload.clone(),
        })
    }

    /// Send a query message.
    pub fn send_query(
        &mut self,
        id: &str,
        target: &str,
        selector: Option<&Value>,
    ) -> crate::Result {
        self.send(&OutgoingMessage::Query {
            session: String::new(),
            id: id.to_string(),
            target: target.to_string(),
            selector: selector.cloned(),
        })
    }

    /// Send a reset message to reinitialize the renderer session.
    pub fn send_reset(&mut self, id: &str) -> crate::Result {
        self.send(&OutgoingMessage::Reset {
            session: String::new(),
            id: id.to_string(),
        })
    }

    /// Register a stub effect response for testing/automation.
    pub fn send_register_effect_stub(&mut self, kind: &str, response: &Value) -> crate::Result {
        self.send(&OutgoingMessage::RegisterEffectStub {
            session: String::new(),
            kind: kind.to_string(),
            response: response.clone(),
        })
    }

    /// Remove a previously registered effect stub.
    pub fn send_unregister_effect_stub(&mut self, kind: &str) -> crate::Result {
        self.send(&OutgoingMessage::UnregisterEffectStub {
            session: String::new(),
            kind: kind.to_string(),
        })
    }

    /// Read the next message from the renderer's stdout.
    ///
    /// Reads inline from the child's stdout (no reader thread).
    /// Used for the hello handshake before the main event loop
    /// starts the background reader. Prefer
    /// [`recv_timeout`](Self::recv_timeout) in the main loop so
    /// heartbeat silence can be detected.
    pub fn receive(&mut self) -> io::Result<Value> {
        let reader = self
            .sync_stdout
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdout closed"))?;

        read_one(reader, self.codec)
    }

    /// Check if the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    /// Reap the child if it has already exited, returning the exit
    /// code. Does not block.
    pub fn try_reap(&mut self) -> Option<i32> {
        match self.child.try_wait() {
            Ok(Some(status)) => status.code(),
            _ => None,
        }
    }

    /// Wait for the child to exit, returning the exit code.
    ///
    /// Blocks until the child terminates. Use after `kill()` to reap
    /// the process and avoid zombies.
    pub fn wait(&mut self) -> io::Result<Option<i32>> {
        Ok(self.child.wait()?.code())
    }

    /// Set the codec after hello message negotiation.
    pub fn set_codec(&mut self, codec: Codec) {
        self.codec = codec;
    }
}

#[cfg(feature = "wire")]
impl Drop for Bridge {
    fn drop(&mut self) {
        // Order matters. Signalling stop without killing first can
        // deadlock: the reader thread is blocked inside `read_one`,
        // which only returns after the child closes its stdout. The
        // child doesn't close it until we kill or it exits on its own.
        // Kill first, then stop_reader (which joins after read_one
        // returns on the pipe-closed error).
        if let Some(reader) = self.reader.as_ref() {
            reader.stop.store(true, Ordering::SeqCst);
        }
        let _ = self.kill();
        self.stop_reader();
        // Reap the child to capture the exit code and avoid zombies
        // on platforms where kill() returns before the process is
        // actually reaped.
        let _ = self.child.wait();
    }
}

/// Read a single message from a buffered stdout handle, with
/// codec-specific framing. Helper shared by synchronous `receive()`
/// and the background reader loop.
///
/// The caller must own the `BufReader` across calls so JSON-mode
/// lookahead (anything the previous `read_line` pulled in past the
/// delimiter) survives. MsgPack framing is length-prefixed and
/// unaffected by buffering but still benefits from a shared handle.
#[cfg(feature = "wire")]
fn read_one<R: Read>(reader: &mut BufReader<R>, codec: Codec) -> io::Result<Value> {
    match codec {
        Codec::Json => {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "renderer closed",
                ));
            }
            serde_json::from_str(&line).map_err(io::Error::other)
        }
        Codec::MsgPack => {
            const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024; // 64 MB
            let mut len_buf = [0u8; 4];
            reader.read_exact(&mut len_buf)?;
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > MAX_MESSAGE_SIZE {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("message size {len} exceeds {MAX_MESSAGE_SIZE} byte limit"),
                ));
            }
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;
            // Share the widget-sdk's depth pre-check so a pathological
            // msgpack payload cannot blow rmp_serde's recursive parser
            // even when the renderer is the peer. Today the renderer is
            // trusted; this keeps the invariant if that ever changes.
            if let Err(e) = plushie_core::codec_safety::check_msgpack_depth(
                &buf,
                plushie_core::codec_safety::MAX_RMPV_DEPTH,
            ) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("msgpack depth check: {e}"),
                ));
            }
            rmp_serde::from_slice(&buf).map_err(io::Error::other)
        }
    }
}

/// Background reader thread. Reads frames from the buffered child
/// stdout until an I/O error occurs or `stop` flips. Every frame (or
/// terminating error) is sent to the receiver; when the sender is
/// dropped, the main loop's `recv_timeout` returns Disconnected.
#[cfg(feature = "wire")]
fn reader_loop(
    mut reader: BufReader<ChildStdout>,
    codec: Codec,
    tx: mpsc::SyncSender<io::Result<Value>>,
    stop: Arc<AtomicBool>,
) {
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let result = read_one(&mut reader, codec);
        let is_err = result.is_err();
        if tx.send(result).is_err() {
            // Main loop dropped the receiver; exit quietly.
            return;
        }
        if is_err {
            return;
        }
    }
}

#[cfg(all(test, feature = "wire"))]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Two JSON messages back-to-back through a single shared
    /// BufReader must both decode. The previous implementation
    /// reconstructed `BufReader::new(...)` on each `read_one` call,
    /// discarding any bytes the previous call had pulled past the
    /// first newline; when the OS delivered both messages in one
    /// read, the second was lost.
    #[test]
    fn read_one_json_back_to_back_messages_are_both_decoded() {
        let bytes = b"{\"type\":\"hello\",\"n\":1}\n{\"type\":\"hello\",\"n\":2}\n";
        let cursor = Cursor::new(bytes.to_vec());
        let mut reader = BufReader::new(cursor);
        let first = read_one(&mut reader, Codec::Json).expect("first decode");
        let second = read_one(&mut reader, Codec::Json).expect("second decode");
        assert_eq!(first.get("n").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(second.get("n").and_then(|v| v.as_u64()), Some(2));
    }

    /// MsgPack framing uses `read_exact` on a 4-byte length prefix
    /// followed by the payload; back-to-back frames must both decode
    /// from a single shared BufReader too.
    #[test]
    fn read_one_msgpack_back_to_back_messages_are_both_decoded() {
        use serde_json::json;

        fn frame(value: &Value) -> Vec<u8> {
            let bytes = rmp_serde::to_vec(value).unwrap();
            let mut buf = (bytes.len() as u32).to_be_bytes().to_vec();
            buf.extend_from_slice(&bytes);
            buf
        }

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&frame(&json!({"type": "hello", "n": 1})));
        bytes.extend_from_slice(&frame(&json!({"type": "hello", "n": 2})));
        let mut reader = BufReader::new(Cursor::new(bytes));
        let first = read_one(&mut reader, Codec::MsgPack).expect("first decode");
        let second = read_one(&mut reader, Codec::MsgPack).expect("second decode");
        assert_eq!(first.get("n").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(second.get("n").and_then(|v| v.as_u64()), Some(2));
    }
}
