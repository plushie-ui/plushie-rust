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

    /// Send a JSON message to the renderer's stdin.
    pub fn send(&mut self, message: &Value) -> io::Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed"))?;

        match self.codec {
            Codec::Json => {
                let json = serde_json::to_string(message).map_err(io::Error::other)?;
                writeln!(stdin, "{json}")?;
                stdin.flush()?;
            }
            Codec::MsgPack => {
                let bytes = rmp_serde::to_vec(message).map_err(io::Error::other)?;
                let len = (bytes.len() as u32).to_be_bytes();
                stdin.write_all(&len)?;
                stdin.write_all(&bytes)?;
                stdin.flush()?;
            }
        }

        Ok(())
    }

    /// Send a settings message.
    pub fn send_settings(&mut self, settings: &Value) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "settings",
            "session": "",
            "settings": settings,
        });
        self.send(&msg)
    }

    /// Send a full tree snapshot.
    pub fn send_snapshot(&mut self, tree: &Value) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "snapshot",
            "session": "",
            "tree": tree,
        });
        self.send(&msg)
    }

    /// Send incremental patches.
    pub fn send_patch(&mut self, ops: &[Value]) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "patch",
            "session": "",
            "ops": ops,
        });
        self.send(&msg)
    }

    /// Send a subscribe message.
    pub fn send_subscribe(
        &mut self,
        kind: &str,
        tag: &str,
        max_rate: Option<u32>,
        window_id: Option<&str>,
    ) -> io::Result<()> {
        let mut msg = serde_json::json!({
            "type": "subscribe",
            "session": "",
            "kind": kind,
            "tag": tag,
        });
        if let Some(rate) = max_rate {
            msg["max_rate"] = serde_json::json!(rate);
        }
        if let Some(wid) = window_id {
            msg["window_id"] = serde_json::json!(wid);
        }
        self.send(&msg)
    }

    /// Send an unsubscribe message.
    pub fn send_unsubscribe(&mut self, kind: &str, tag: &str) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "unsubscribe",
            "session": "",
            "kind": kind,
            "tag": tag,
        });
        self.send(&msg)
    }

    /// Send a widget operation (focus, scroll, etc.).
    pub fn send_widget_op(&mut self, op: &str, payload: &Value) -> io::Result<()> {
        let mut msg = serde_json::json!({
            "type": "widget_op",
            "session": "",
            "op": op,
        });
        msg["payload"] = payload.clone();
        self.send(&msg)
    }

    /// Send a widget-targeted command.
    pub fn send_command(&mut self, id: &str, family: &str, value: &Value) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "command",
            "session": "",
            "id": id,
            "family": family,
            "value": value,
        });
        self.send(&msg)
    }

    /// Send a window operation.
    pub fn send_window_op(
        &mut self,
        op: &str,
        window_id: &str,
        settings: &Value,
    ) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "window_op",
            "session": "",
            "op": op,
            "window_id": window_id,
            "settings": settings,
        });
        self.send(&msg)
    }

    /// Send an effect request.
    pub fn send_effect(&mut self, id: &str, kind: &str, payload: &Value) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "effect",
            "session": "",
            "id": id,
            "kind": kind,
            "payload": payload,
        });
        self.send(&msg)
    }

    /// Send an interact message for automation.
    ///
    /// The renderer resolves the selector against its tree, performs
    /// the interaction, and sends back an `interact_response` with
    /// the resulting events.
    pub fn send_interact(
        &mut self,
        id: &str,
        action: &str,
        selector: &Value,
        payload: &Value,
    ) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "interact",
            "session": "",
            "id": id,
            "action": action,
            "selector": selector,
            "payload": payload,
        });
        self.send(&msg)
    }

    /// Send a query message.
    pub fn send_query(
        &mut self,
        id: &str,
        target: &str,
        selector: Option<&Value>,
    ) -> io::Result<()> {
        let mut msg = serde_json::json!({
            "type": "query",
            "session": "",
            "id": id,
            "target": target,
        });
        if let Some(sel) = selector {
            msg["selector"] = sel.clone();
        }
        self.send(&msg)
    }

    /// Send a reset message to reinitialize the renderer session.
    pub fn send_reset(&mut self, id: &str) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "reset",
            "session": "",
            "id": id,
        });
        self.send(&msg)
    }

    /// Register a stub effect response for testing/automation.
    pub fn send_register_effect_stub(
        &mut self,
        kind: &str,
        response: &Value,
    ) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "register_effect_stub",
            "session": "",
            "kind": kind,
            "response": response,
        });
        self.send(&msg)
    }

    /// Remove a previously registered effect stub.
    pub fn send_unregister_effect_stub(&mut self, kind: &str) -> io::Result<()> {
        let msg = serde_json::json!({
            "type": "unregister_effect_stub",
            "session": "",
            "kind": kind,
        });
        self.send(&msg)
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

    /// Set the codec after hello message negotiation.
    pub fn set_codec(&mut self, codec: Codec) {
        self.codec = codec;
    }
}

#[cfg(feature = "wire")]
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}
