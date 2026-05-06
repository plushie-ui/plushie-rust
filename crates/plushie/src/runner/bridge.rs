//! Wire protocol bridge: transport management and message framing.
//!
//! Two transports share a single framing + heartbeat implementation:
//!
//! - `Subprocess`: spawn `plushie-renderer` as a child process and
//!   talk over stdin/stdout. The default shape for `plushie::run`,
//!   `plushie::run_spawn`, and `plushie::run_with_renderer`.
//! - `Socket`: attach to a renderer already listening on a Unix or
//!   TCP socket (started via `plushie-renderer --listen`). Used by
//!   `plushie::run_connect`.
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
use std::process::{Child, Command as ProcessCommand, Stdio};
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

#[cfg(feature = "wire")]
use super::socket::SocketStream;

/// Wire protocol codec selection.
#[cfg(feature = "wire")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
    /// JSON Lines (one JSON object per line, newline-delimited).
    Json,
    /// MessagePack with 4-byte big-endian length prefix.
    MsgPack,
}

/// Type-erased reader for the background reader thread.
#[cfg(feature = "wire")]
type BoxedReader = Box<dyn Read + Send>;

/// The I/O transport `Bridge` is running over.
///
/// `Subprocess` owns a child process plus its stdin/stdout pair.
/// `Socket` owns a pre-connected Unix/TCP stream. Both expose the
/// same read/write surface to the upper layers.
#[cfg(feature = "wire")]
enum Transport {
    Subprocess {
        child: Child,
        /// Child stdin. `None` once Drop closes it.
        stdin: Option<Box<dyn Write + Send>>,
    },
    Socket {
        /// Writer half of the socket stream.
        writer: Box<dyn Write + Send>,
        /// The read-half handle kept around so `shutdown` can reach
        /// both directions when Drop wants to wake the reader thread.
        shutdown_handle: SocketStream,
    },
}

/// A connection to a renderer.
#[cfg(feature = "wire")]
pub struct Bridge {
    transport: Transport,
    codec: Codec,
    /// Buffered reader owning the incoming byte stream.
    ///
    /// Held by the struct (rather than created per-call in
    /// [`Self::receive`]) so JSON-mode reads don't discard the
    /// lookahead buffer between calls. `BufReader::new(...)` on every
    /// call would drop any bytes the previous call had already pulled
    /// in past the newline, corrupting framing on back-to-back
    /// renderer messages.
    ///
    /// `None` after [`Self::start_reader`] takes ownership for the
    /// background reader thread.
    sync_reader: Option<BufReader<BoxedReader>>,
    /// Background reader thread state. `None` between construction
    /// and the first `start_reader` call (used by tests that don't
    /// need a reader).
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

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdout unavailable"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin unavailable"))?;

        let sync_reader: BufReader<BoxedReader> =
            BufReader::with_capacity(64 * 1024, Box::new(stdout));

        Ok(Self {
            transport: Transport::Subprocess {
                child,
                stdin: Some(Box::new(stdin)),
            },
            codec: Codec::Json,
            sync_reader: Some(sync_reader),
            reader: None,
        })
    }

    /// Attach to a renderer already listening on a socket.
    ///
    /// The provided [`SocketStream`] must be connected; we clone the
    /// handle (one half for reads, one half for writes) and keep a
    /// third clone around so `Drop` can call `shutdown(Both)` to wake
    /// the reader thread during graceful teardown.
    pub fn connect(stream: SocketStream) -> io::Result<Self> {
        let read_half = stream.try_clone()?;
        let write_half = stream.try_clone()?;
        let sync_reader: BufReader<BoxedReader> =
            BufReader::with_capacity(64 * 1024, Box::new(read_half));
        Ok(Self {
            transport: Transport::Socket {
                writer: Box::new(write_half),
                shutdown_handle: stream,
            },
            codec: Codec::Json,
            sync_reader: Some(sync_reader),
            reader: None,
        })
    }

    /// Start the background reader thread.
    ///
    /// Must be called after codec negotiation (i.e. after `set_codec`
    /// if the hello message changed the codec). Takes ownership of
    /// the buffered reader.
    pub fn start_reader(&mut self) -> io::Result<()> {
        if self.reader.is_some() {
            return Ok(());
        }
        let reader = self
            .sync_reader
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "reader already taken"))?;
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

    /// Return a mutable reference to the writer half. Returns an I/O
    /// error if the transport has been closed (e.g. during Drop).
    fn writer_mut(&mut self) -> io::Result<&mut dyn Write> {
        match &mut self.transport {
            Transport::Subprocess { stdin, .. } => stdin
                .as_deref_mut()
                .map(|w| w as &mut dyn Write)
                .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed")),
            Transport::Socket { writer, .. } => Ok(writer.as_mut()),
        }
    }

    /// Send a typed message to the renderer.
    ///
    /// Encode failures return [`crate::Error::WireEncode`]; I/O
    /// failures return [`crate::Error::Io`].
    pub fn send(&mut self, message: &OutgoingMessage) -> crate::Result {
        let codec = self.codec;
        let writer = self.writer_mut().map_err(crate::Error::Io)?;

        match codec {
            Codec::Json => {
                let json = serde_json::to_string(message)
                    .map_err(|e| crate::Error::WireEncode(e.to_string()))?;
                writeln!(writer, "{json}")?;
                writer.flush()?;
            }
            Codec::MsgPack => {
                // No non-finite-float sanitisation pass here: every
                // numeric field in `OutgoingMessage` either travels
                // through `serde_json::Value` (already sanitised by
                // the constructors that produced it) or as a typed
                // integer / `u32`. The widget-sdk codec runs a
                // defensive `sanitize_rmpv_value` pass after a generic
                // `serde_json::Value` round-trip; here, the
                // `Value`-typed payload constraint already enforces
                // the invariant, so the same pass would only re-walk
                // a structure that cannot contain a `NaN`/`inf` to
                // begin with. Revisit if a future variant gains a
                // bare `f32`/`f64` field that bypasses the JSON
                // intermediate.
                let bytes = rmp_serde::to_vec_named(message)
                    .map_err(|e| crate::Error::WireEncode(e.to_string()))?;
                let len = (bytes.len() as u32).to_be_bytes();
                writer.write_all(&len)?;
                writer.write_all(&bytes)?;
                writer.flush()?;
            }
        }

        Ok(())
    }

    /// Send a `load_font` message with native binary encoding in MsgPack
    /// mode and base64-string encoding in JSON mode.
    ///
    /// Mirrors the renderer's `encode_binary_message` strategy for
    /// outgoing image-data frames: the typed `OutgoingMessage::LoadFont`
    /// variant cannot express native msgpack binary through the
    /// `serde_json::Value` payload alone, so this helper bypasses the
    /// generic `send` path on MsgPack to write the bytes as
    /// `rmpv::Value::Binary` directly.
    pub fn send_load_font(&mut self, family: &str, bytes: &[u8]) -> crate::Result {
        let codec = self.codec;
        let writer = self.writer_mut().map_err(crate::Error::Io)?;

        match codec {
            Codec::Json => {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                let message = OutgoingMessage::LoadFont {
                    session: String::new(),
                    payload: serde_json::json!({
                        "family": family,
                        "data": b64,
                    }),
                };
                let json = serde_json::to_string(&message)
                    .map_err(|e| crate::Error::WireEncode(e.to_string()))?;
                writeln!(writer, "{json}")?;
                writer.flush()?;
            }
            Codec::MsgPack => {
                use rmpv::Value as V;

                let payload = V::Map(vec![
                    (V::String("family".into()), V::String(family.into())),
                    (V::String("data".into()), V::Binary(bytes.to_vec())),
                ]);
                let message = V::Map(vec![
                    (V::String("type".into()), V::String("load_font".into())),
                    (V::String("session".into()), V::String(String::new().into())),
                    (V::String("payload".into()), payload),
                ]);

                let mut buf = Vec::new();
                rmpv::encode::write_value(&mut buf, &message)
                    .map_err(|e| crate::Error::WireEncode(e.to_string()))?;
                let len = u32::try_from(buf.len())
                    .map_err(|_| crate::Error::WireEncode("frame exceeds 4 GiB".into()))?;
                writer.write_all(&len.to_be_bytes())?;
                writer.write_all(&buf)?;
                writer.flush()?;
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

    /// Read the next message from the renderer.
    ///
    /// Reads inline from the held buffered reader (no reader thread).
    /// Used for the hello handshake before the main event loop starts
    /// the background reader. Prefer [`recv_timeout`](Self::recv_timeout)
    /// in the main loop so heartbeat silence can be detected.
    pub fn receive(&mut self) -> io::Result<Value> {
        let codec = self.codec;
        let reader = self.sync_reader.as_mut().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "reader already handed off")
        })?;

        read_one(reader, codec)
    }

    /// Check if the renderer is still connected.
    ///
    /// For subprocess transports this polls the child's exit status.
    /// Socket transports don't expose a non-invasive liveness probe;
    /// they return true here and let the reader thread surface the
    /// disconnect via `recv_timeout`.
    pub fn is_alive(&mut self) -> bool {
        match &mut self.transport {
            Transport::Subprocess { child, .. } => child.try_wait().ok().flatten().is_none(),
            Transport::Socket { .. } => true,
        }
    }

    /// Force-close the renderer connection.
    ///
    /// Subprocess transports send `SIGKILL`; socket transports
    /// shutdown both directions so the renderer observes a clean
    /// close on its end.
    pub fn kill(&mut self) -> io::Result<()> {
        match &mut self.transport {
            Transport::Subprocess { child, .. } => child.kill(),
            Transport::Socket {
                shutdown_handle, ..
            } => {
                shutdown_handle.shutdown();
                Ok(())
            }
        }
    }

    /// Reap the child if it has already exited, returning the exit
    /// code. Does not block. Socket transports always return `None`.
    pub fn try_reap(&mut self) -> Option<i32> {
        match &mut self.transport {
            Transport::Subprocess { child, .. } => match child.try_wait() {
                Ok(Some(status)) => status.code(),
                _ => None,
            },
            Transport::Socket { .. } => None,
        }
    }

    /// Wait for the renderer to exit, returning the exit code.
    ///
    /// Blocks until the child terminates (subprocess) or returns
    /// immediately with `None` (socket, there is no child to reap).
    pub fn wait(&mut self) -> io::Result<Option<i32>> {
        match &mut self.transport {
            Transport::Subprocess { child, .. } => Ok(child.wait()?.code()),
            Transport::Socket { .. } => Ok(None),
        }
    }

    /// Set the codec after hello message negotiation.
    pub fn set_codec(&mut self, codec: Codec) {
        self.codec = codec;
    }
}

#[cfg(feature = "wire")]
impl Drop for Bridge {
    fn drop(&mut self) {
        // Signal the reader thread to stop; the transport-specific
        // teardown below wakes it up by closing its read side.
        if let Some(reader) = self.reader.as_ref() {
            reader.stop.store(true, Ordering::SeqCst);
        }

        // Release the transport's outbound half (close stdin / drop
        // the writer + shutdown the socket) in a scoped borrow so we
        // can call self.stop_reader() afterwards without fighting the
        // borrow checker over `self`.
        {
            match &mut self.transport {
                Transport::Subprocess { child, stdin } => {
                    // Order matters (inherited from the pre-refactor
                    // Bridge::Drop). Signalling stop without closing
                    // stdin can deadlock the reader thread because
                    // `read_one` only returns after the child closes
                    // its stdout, which doesn't happen until we close
                    // stdin or SIGKILL.
                    //
                    // Sequence:
                    //   1. Close stdin -> child observes EOF and can
                    //      drain its graceful-shutdown path.
                    //   2. Wait up to GRACE for a clean exit.
                    //   3. SIGKILL if the child is still alive.
                    drop(stdin.take());

                    const GRACE: Duration = Duration::from_millis(500);
                    let deadline = std::time::Instant::now() + GRACE;
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) if std::time::Instant::now() >= deadline => break,
                            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                            Err(_) => break,
                        }
                    }

                    if matches!(child.try_wait(), Ok(None)) {
                        let _ = child.kill();
                    }
                }
                Transport::Socket {
                    writer,
                    shutdown_handle,
                } => {
                    // Drop the writer first so the peer observes EOF
                    // on its read side, then shutdown the kept handle
                    // so our own reader thread's blocked read returns
                    // with EOF and the loop exits.
                    drop(std::mem::replace(writer, Box::new(io::sink())));
                    shutdown_handle.shutdown();
                }
            }
        }

        // Reader thread wakes up on the EOF produced above; join it
        // before reaping the child (subprocess path only).
        self.stop_reader();

        if let Transport::Subprocess { child, .. } = &mut self.transport {
            // Reap to capture the exit code and avoid zombies on
            // platforms where kill() returns before the process is
            // actually reaped.
            let _ = child.wait();
        }
    }
}

/// Read a single message from a buffered reader handle, with
/// codec-specific framing. Helper shared by synchronous `receive()`
/// and the background reader loop.
///
/// The caller must own the `BufReader` across calls so JSON-mode
/// lookahead (anything the previous `read_line` pulled in past the
/// delimiter) survives. MsgPack framing is length-prefixed and
/// unaffected by buffering but still benefits from a shared handle.
#[cfg(feature = "wire")]
fn read_one<R: Read>(reader: &mut BufReader<R>, codec: Codec) -> io::Result<Value> {
    /// Per-message size cap shared by JSON and msgpack framing in
    /// this bridge. Matches the widget-sdk's `MAX_MESSAGE_SIZE`.
    const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;
    match codec {
        Codec::Json => {
            let mut line = String::new();
            // Bound the in-memory buffer so a renderer emitting an
            // unterminated line can't grow the host process without
            // limit. +1 so an exactly-sized line is readable; any byte
            // past the cap trips the overflow path below.
            let limit = (MAX_MESSAGE_SIZE + 1) as u64;
            let n = reader.take(limit).read_line(&mut line)?;
            if n == 0 && line.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "renderer closed",
                ));
            }
            if line.len() > MAX_MESSAGE_SIZE {
                let diag = plushie_core::Diagnostic::BufferOverflow {
                    size: line.len(),
                    limit: MAX_MESSAGE_SIZE,
                };
                plushie_core::diagnostics::error(diag.clone());
                return Err(io::Error::new(io::ErrorKind::InvalidData, diag.to_string()));
            }
            serde_json::from_str(&line).map_err(io::Error::other)
        }
        Codec::MsgPack => {
            let mut len_buf = [0u8; 4];
            reader.read_exact(&mut len_buf)?;
            let len = u32::from_be_bytes(len_buf) as usize;
            if len > MAX_MESSAGE_SIZE {
                let diag = plushie_core::Diagnostic::BufferOverflow {
                    size: len,
                    limit: MAX_MESSAGE_SIZE,
                };
                plushie_core::diagnostics::error(diag.clone());
                return Err(io::Error::new(io::ErrorKind::InvalidData, diag.to_string()));
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

/// Background reader thread. Reads frames from the buffered stream
/// until an I/O error occurs or `stop` flips. Every frame (or
/// terminating error) is sent to the receiver; when the sender is
/// dropped, the main loop's `recv_timeout` returns Disconnected.
#[cfg(feature = "wire")]
fn reader_loop(
    mut reader: BufReader<BoxedReader>,
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
    use std::io::{Cursor, Read};
    use std::net::{TcpListener, TcpStream};

    fn bridge_socket_pair() -> (SocketStream, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback listener");
        let addr = listener.local_addr().expect("listener address");
        let client = TcpStream::connect(addr).expect("connect client");
        let (server, _) = listener.accept().expect("accept server");
        (SocketStream::Tcp(client), server)
    }

    fn read_msgpack_frame(reader: &mut impl Read) -> Vec<u8> {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).expect("read frame length");
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut payload = vec![0u8; len];
        reader.read_exact(&mut payload).expect("read frame payload");
        payload
    }

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

    /// Msgpack frame with a length prefix past the 64 MiB cap is
    /// rejected with a typed `BufferOverflow` diagnostic payload.
    #[test]
    fn read_one_msgpack_rejects_oversized_length_prefix() {
        // Declare a frame size one byte past the cap. Don't actually
        // send that many bytes; the reader bails out on the length
        // check before touching the payload.
        let oversize = (64 * 1024 * 1024 + 1) as u32;
        let bytes = oversize.to_be_bytes().to_vec();
        let mut reader = BufReader::new(Cursor::new(bytes));
        let err = read_one(&mut reader, Codec::MsgPack).expect_err("expected overflow error");
        let msg = err.to_string();
        assert!(
            msg.contains("buffer_overflow"),
            "unexpected error text: {msg}"
        );
    }

    /// JSON framing rejects a line past the 64 MiB cap with a typed
    /// `BufferOverflow` diagnostic payload rather than silently
    /// growing the host's memory.
    #[test]
    fn read_one_json_rejects_oversized_line() {
        // 70 MiB of `x` plus a closing newline. `Read::take` bounds
        // the allocation to MAX+1 so `read_line` returns that and the
        // overflow guard fires deterministically.
        let payload: Vec<u8> = std::iter::repeat_n(b'x', 70 * 1024 * 1024).collect();
        let mut bytes = payload;
        bytes.push(b'\n');
        let mut reader = BufReader::new(Cursor::new(bytes));
        let err = read_one(&mut reader, Codec::Json).expect_err("expected overflow error");
        assert!(
            err.to_string().contains("buffer_overflow"),
            "unexpected error text: {err}"
        );
    }

    /// MsgPack framing uses `read_exact` on a 4-byte length prefix
    /// followed by the payload; back-to-back frames must both decode
    /// from a single shared BufReader too.
    #[test]
    fn read_one_msgpack_back_to_back_messages_are_both_decoded() {
        use serde_json::json;

        fn frame(value: &Value) -> Vec<u8> {
            let bytes = rmp_serde::to_vec_named(value).unwrap();
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

    #[test]
    fn send_msgpack_uses_renderer_named_message_shape() {
        use serde_json::json;

        let (client, mut server) = bridge_socket_pair();
        let mut bridge = Bridge::connect(client).expect("connect bridge");
        bridge.set_codec(Codec::MsgPack);

        let message = OutgoingMessage::Settings {
            session: String::new(),
            settings: json!({
                "protocol_version": 1,
                "app_id": "test",
            }),
        };
        bridge.send(&message).expect("send message");

        let payload = read_msgpack_frame(&mut server);
        assert_eq!(payload, rmp_serde::to_vec_named(&message).unwrap());

        let decoded: Value = rmp_serde::from_slice(&payload).expect("decode payload");
        assert_eq!(
            decoded.get("type").and_then(Value::as_str),
            Some("settings")
        );
        assert!(decoded.get("settings").is_some());
    }
}
