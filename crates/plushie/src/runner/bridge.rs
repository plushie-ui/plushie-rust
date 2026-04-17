//! Wire protocol bridge: subprocess management and message framing.
//!
//! The bridge spawns a plushie renderer binary as a child process and
//! communicates over stdin/stdout using length-prefixed MessagePack
//! or JSONL framing.

#[cfg(feature = "wire")]
use std::io::{self, BufRead, BufReader, Read, Write};
#[cfg(feature = "wire")]
use std::process::{Child, Command as ProcessCommand, Stdio};

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
}

#[cfg(feature = "wire")]
impl Bridge {
    /// Spawn a renderer subprocess and negotiate the codec.
    pub fn spawn(binary_path: &str) -> io::Result<Self> {
        let child = ProcessCommand::new(binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        Ok(Self {
            child,
            codec: Codec::Json, // default, may be negotiated via hello
        })
    }

    /// Send a typed message to the renderer's stdin.
    ///
    /// Encode failures return [`crate::Error::WireEncode`]; I/O
    /// failures return [`crate::Error::Io`]. F-2.3.4.
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
    pub fn receive(&mut self) -> io::Result<Value> {
        let stdout = self
            .child
            .stdout
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdout closed"))?;

        match self.codec {
            Codec::Json => {
                let mut reader = BufReader::new(stdout);
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
                stdout.read_exact(&mut len_buf)?;
                let len = u32::from_be_bytes(len_buf) as usize;
                if len > MAX_MESSAGE_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("message size {len} exceeds {MAX_MESSAGE_SIZE} byte limit"),
                    ));
                }
                let mut buf = vec![0u8; len];
                stdout.read_exact(&mut buf)?;
                rmp_serde::from_slice(&buf).map_err(io::Error::other)
            }
        }
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
        let _ = self.kill();
        // Reap the child to capture the exit code and avoid zombies
        // on platforms where kill() returns before the process is
        // actually reaped. F-2.2.4.
        let _ = self.child.wait();
    }
}
