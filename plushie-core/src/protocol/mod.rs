//! Wire protocol types for host-renderer communication.
//!
//! [`IncomingMessage`] is deserialized from the host. [`OutgoingEvent`]
//! and response types are serialized back. The transport (stdin/stdout,
//! socket, test harness) is handled by the binary crate, not here.

mod incoming;
mod outgoing;
mod types;

/// Protocol version number. Sent in the `hello` handshake message on startup
/// and checked against the value the host embeds in Settings.
pub const PROTOCOL_VERSION: u32 = 1;

pub use incoming::{IncomingMessage, WidgetCommandItem};
pub use outgoing::{
    CoalesceHint, EffectResponse, EffectStubAck, InteractResponse, KeyModifiers, OutgoingEvent,
    QueryResponse, ResetResponse, TreeHashResponse,
};
pub use types::{PatchOp, TreeNode};

/// An incoming message paired with its session ID.
#[derive(Debug)]
pub struct SessionMessage {
    pub session: String,
    pub message: IncomingMessage,
}

impl SessionMessage {
    /// Extract `session` from a JSON value and deserialize the rest as
    /// [`IncomingMessage`].
    pub fn from_value(mut value: serde_json::Value) -> Result<Self, serde_json::Error> {
        let session = match value.as_object_mut() {
            Some(obj) => match obj.remove("session") {
                None => String::new(),
                Some(serde_json::Value::String(s)) => s,
                Some(other) => {
                    return Err(serde::de::Error::custom(format!(
                        "session must be a string, got {}",
                        other
                    )));
                }
            },
            None => {
                return Err(serde::de::Error::custom("expected JSON object"));
            }
        };

        let message = serde_json::from_value(value)?;
        Ok(Self { session, message })
    }
}
