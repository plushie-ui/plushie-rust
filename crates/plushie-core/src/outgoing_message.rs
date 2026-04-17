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
    Settings {
        session: String,
        settings: Value,
    },
    Snapshot {
        session: String,
        tree: Value,
    },
    Patch {
        session: String,
        ops: Vec<Value>,
    },
    Subscribe {
        session: String,
        kind: String,
        tag: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_rate: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        window_id: Option<String>,
    },
    Unsubscribe {
        session: String,
        kind: String,
        tag: String,
    },
    WidgetOp {
        session: String,
        op: String,
        payload: Value,
    },
    Command {
        session: String,
        id: String,
        family: String,
        value: Value,
    },
    WindowOp {
        session: String,
        op: String,
        window_id: String,
        payload: Value,
    },
    Effect {
        session: String,
        id: String,
        kind: String,
        payload: Value,
    },
    Interact {
        session: String,
        id: String,
        action: String,
        selector: Value,
        payload: Value,
    },
    Query {
        session: String,
        id: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        selector: Option<Value>,
    },
    Reset {
        session: String,
        id: String,
    },
    RegisterEffectStub {
        session: String,
        kind: String,
        response: Value,
    },
    UnregisterEffectStub {
        session: String,
        kind: String,
    },
    SystemOp {
        session: String,
        op: String,
        payload: Value,
    },
    SystemQuery {
        session: String,
        op: String,
        payload: Value,
    },
    ImageOp {
        session: String,
        op: String,
        payload: Value,
    },
}
