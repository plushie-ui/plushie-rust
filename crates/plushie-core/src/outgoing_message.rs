//! Typed outgoing wire protocol messages (SDK -> renderer).
//!
//! Mirrors [`IncomingMessage`] on the renderer side, providing
//! compile-time safety for message construction. Uses serde tagged
//! enum serialization to produce the same JSON/MessagePack format
//! the renderer expects.

use serde::Serialize;
use serde_json::Value;

/// A message sent from the SDK to the renderer over the wire protocol.
///
/// Each variant corresponds to a wire message type. The `#[serde(tag)]`
/// attribute produces `{"type": "variant_name", ...fields}` on serialization,
/// matching the renderer's `IncomingMessage` deserialization format.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutgoingMessage {
    /// Settings.
    Settings {
        /// Session.
        session: String,
        /// Settings.
        settings: Value,
    },
    /// Snapshot.
    Snapshot {
        /// Session.
        session: String,
        /// Tree.
        tree: Value,
    },
    /// Patch.
    Patch {
        /// Session.
        session: String,
        /// Ops.
        ops: Vec<Value>,
    },
    /// Subscribe.
    Subscribe {
        /// Session.
        session: String,
        /// Event kind string used on the wire.
        kind: String,
        /// Correlation tag used for matching responses.
        tag: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        /// Optional max delivery rate (events per second).
        max_rate: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        /// Target window ID.
        window_id: Option<String>,
    },
    /// Unsubscribe.
    Unsubscribe {
        /// Session.
        session: String,
        /// Event kind string used on the wire.
        kind: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Widget Op.
    WidgetOp {
        /// Session.
        session: String,
        /// Op.
        op: String,
        /// Payload.
        payload: Value,
    },
    /// Command.
    Command {
        /// Session.
        session: String,
        /// Target widget ID.
        id: String,
        /// Event/command family identifier.
        family: String,
        /// Typed payload value.
        value: Value,
    },
    /// Commands.
    Commands {
        /// Session.
        session: String,
        /// Path command list.
        commands: Vec<crate::ops::WidgetCommand>,
    },
    /// Window Op.
    WindowOp {
        /// Session.
        session: String,
        /// Op.
        op: String,
        /// Target window ID.
        window_id: String,
        /// Payload.
        payload: Value,
    },
    /// Effect.
    Effect {
        /// Session.
        session: String,
        /// Target widget ID.
        id: String,
        /// Event kind string used on the wire.
        kind: String,
        /// Payload.
        payload: Value,
    },
    /// Interact.
    Interact {
        /// Session.
        session: String,
        /// Target widget ID.
        id: String,
        /// Action.
        action: String,
        /// Selector.
        selector: Value,
        /// Payload.
        payload: Value,
    },
    /// Query.
    Query {
        /// Session.
        session: String,
        /// Target widget ID.
        id: String,
        /// Target identifier.
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        /// Selector.
        selector: Option<Value>,
    },
    /// Reset.
    Reset {
        /// Session.
        session: String,
        /// Target widget ID.
        id: String,
    },
    /// Register Effect Stub.
    RegisterEffectStub {
        /// Session.
        session: String,
        /// Event kind string used on the wire.
        kind: String,
        /// Response.
        response: Value,
    },
    /// Unregister Effect Stub.
    UnregisterEffectStub {
        /// Session.
        session: String,
        /// Event kind string used on the wire.
        kind: String,
    },
    /// System Op.
    SystemOp {
        /// Session.
        session: String,
        /// Op.
        op: String,
        /// Payload.
        payload: Value,
    },
    /// System Query.
    SystemQuery {
        /// Session.
        session: String,
        /// Op.
        op: String,
        /// Payload.
        payload: Value,
    },
    /// Image Op.
    ImageOp {
        /// Session.
        session: String,
        /// Op.
        op: String,
        /// Payload.
        payload: Value,
    },
}
