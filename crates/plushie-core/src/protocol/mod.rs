//! Wire protocol types for host-renderer communication.
//!
//! [`IncomingMessage`] is deserialized from the host. [`OutgoingEvent`]
//! and response types are serialized back. The transport (stdin/stdout,
//! socket, test harness) is handled by the binary crate, not here.

mod incoming;
mod outgoing;
mod props;
mod types;

/// Protocol version number. Sent in the `hello` handshake message on startup
/// and checked against the value the host embeds in Settings.
pub const PROTOCOL_VERSION: u32 = 1;

/// Decode a JSON protocol version field into the canonical in-memory type.
///
/// The wire format uses a JSON number, but the Rust protocol model keeps the
/// version as `u32`. Any non-integer, negative, or out-of-range JSON number is
/// rejected at the boundary instead of widening `PROTOCOL_VERSION` and the
/// surrounding public API surface.
pub fn json_protocol_version(value: &serde_json::Value) -> Option<u32> {
    value.as_u64().and_then(|v| u32::try_from(v).ok())
}

pub use incoming::{ImageOpPayload, IncomingMessage, LoadFontPayload};
pub use outgoing::{
    CoalesceHint, DiagnosticLevel, DiagnosticMessage, EffectResponse, EffectStubAck,
    InteractResponse, KeyModifiers, OutgoingEvent, QueryResponse, ResetResponse,
    ScreenshotResponse, TreeHashResponse,
};
pub use props::{PropMap, PropValue, Props};
pub use types::{PatchOp, TreeNode, canonical_tree_hash};

/// An incoming message paired with its session ID.
#[derive(Debug)]
pub struct SessionMessage {
    /// Session.
    pub session: String,
    /// Human-readable message.
    pub message: IncomingMessage,
}

impl SessionMessage {
    /// Extract `session` from a JSON value and deserialize the rest as
    /// [`IncomingMessage`].
    ///
    /// # Errors
    ///
    /// Returns a `serde_json::Error` when the input is not a JSON
    /// object, when the `session` field is present but not a string,
    /// or when the remaining value does not deserialize as an
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_protocol_version_accepts_u32_max() {
        assert_eq!(json_protocol_version(&json!(u32::MAX)), Some(u32::MAX));
    }

    #[test]
    fn json_protocol_version_rejects_out_of_range_values() {
        assert_eq!(json_protocol_version(&json!(u64::from(u32::MAX) + 1)), None);
    }

    #[test]
    fn json_protocol_version_rejects_non_integer_values() {
        assert_eq!(json_protocol_version(&json!(1.5)), None);
        assert_eq!(json_protocol_version(&json!(-1)), None);
        assert_eq!(json_protocol_version(&json!("1")), None);
    }
}
