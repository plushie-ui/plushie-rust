//! Incoming wire messages from the host process.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::types::{PatchOp, TreeNode};
use crate::ops::WidgetCommand;

/// Messages sent from the host to the renderer over stdin.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IncomingMessage {
    /// Replace the entire UI tree with a new snapshot.
    Snapshot {
        /// Tree.
        tree: TreeNode,
    },
    /// Apply incremental changes to the retained UI tree.
    Patch {
        /// Ops.
        ops: Vec<PatchOp>,
    },
    /// Request a platform effect (file dialog, clipboard, notification).
    Effect {
        /// Target widget ID.
        id: String,
        /// Event kind string used on the wire.
        kind: String,
        /// Payload.
        payload: Value,
    },
    /// Perform a widget operation (focus, scroll, select, etc.).
    WidgetOp {
        /// Op.
        op: String,
        #[serde(default)]
        /// Payload.
        payload: Value,
    },
    /// Subscribe to a runtime event source (keyboard, mouse, window, etc.).
    Subscribe {
        /// Event kind string used on the wire.
        kind: String,
        /// Correlation tag used for matching responses.
        tag: String,
        /// Optional window ID to scope this subscription to a specific window.
        /// When set, only events from this window are delivered. When absent,
        /// events from all windows are delivered.
        #[serde(default)]
        window_id: Option<String>,
        /// Maximum events per second for this subscription. Omit for
        /// unlimited (immediate delivery). Zero means "subscribe but
        /// never emit."
        #[serde(default)]
        max_rate: Option<u32>,
    },
    /// Unsubscribe from a runtime event source.
    Unsubscribe {
        /// Event kind string used on the wire.
        kind: String,
        /// If present, only remove the subscription with this specific tag.
        /// If absent, remove all subscriptions for the kind (backwards compat).
        #[serde(default)]
        tag: Option<String>,
    },
    /// Perform a window operation (resize, move, close, etc.).
    ///
    /// Unified `_op` shape: envelope carries routing (`session`, `op`,
    /// `window_id`); command-specific data is nested under `payload`.
    WindowOp {
        /// Op.
        op: String,
        /// Target window ID.
        window_id: String,
        #[serde(default)]
        /// Payload.
        payload: Value,
    },
    /// Perform a system-wide operation that does not target a specific window.
    ///
    /// Unified `_op` shape: `payload` carries the command-specific data.
    SystemOp {
        /// Op.
        op: String,
        #[serde(default)]
        /// Payload.
        payload: Value,
    },
    /// Run a system-wide query that does not target a specific window.
    ///
    /// Unified `_op` shape: `payload` carries the command-specific data
    /// (e.g. `{"tag": "..."}`).
    SystemQuery {
        /// Op.
        op: String,
        #[serde(default)]
        /// Payload.
        payload: Value,
    },
    /// Apply or update renderer settings.
    Settings {
        /// Settings.
        settings: Value,
    },
    /// Query the current tree or find a widget.
    Query {
        /// Target widget ID.
        id: String,
        /// Target identifier.
        target: String,
        #[serde(default)]
        /// Selector.
        selector: Value,
    },
    /// Interact with a widget (click, type, etc.)
    Interact {
        /// Target widget ID.
        id: String,
        /// Action.
        action: String,
        #[serde(default)]
        /// Selector.
        selector: Value,
        #[serde(default)]
        /// Payload.
        payload: Value,
    },
    /// Capture a structural tree hash (hash of JSON tree).
    // Used by the binary crate's headless and test modes. Appears dead
    // from plushie-core's perspective because the usage is in plushie/.
    #[allow(dead_code)]
    /// Tree Hash.
    TreeHash {
        /// Target widget ID.
        id: String,
        /// Identifier string.
        name: String,
    },
    /// Capture a pixel screenshot (GPU-rendered RGBA data).
    #[allow(dead_code)]
    Screenshot {
        /// Target widget ID.
        id: String,
        /// Identifier string.
        name: String,
        #[serde(default)]
        /// Width in pixels.
        width: Option<u32>,
        #[serde(default)]
        /// Height in pixels.
        height: Option<u32>,
    },
    /// Reset the app state.
    Reset {
        /// Target widget ID.
        id: String,
    },
    /// Image operation (create, update, delete in-memory image handles).
    ///
    /// Unified `_op` shape: `payload` carries op-specific fields.
    ///
    /// Payload shapes:
    /// - `create_from_bytes` / `update`: `{handle, data}` (bytes field,
    ///   either msgpack binary or base64-encoded string in JSON).
    /// - `create_from_rgba` / `update_raw`: `{handle, pixels, width, height}`
    ///   (pixels is RGBA bytes).
    /// - `delete`: `{handle}`.
    /// - `list`: `{tag}`.
    /// - `clear`: `{}`.
    ImageOp {
        /// Op.
        op: String,
        #[serde(default)]
        /// Payload.
        payload: ImageOpPayload,
    },
    /// A widget-targeted command (focus, scroll, text, native widget, etc).
    /// Bypasses the normal tree update / diff / patch cycle.
    Command {
        /// Target widget ID.
        id: String,
        /// Event/command family identifier.
        family: String,
        #[serde(default)]
        /// Typed payload value.
        value: Value,
    },
    /// A batch of widget-targeted commands processed in one cycle.
    Commands {
        /// Path command list.
        commands: Vec<WidgetCommand>,
    },
    /// Advance the animation clock by one frame in headless/mock mode.
    /// Emits an `animation_frame` event if `on_animation_frame` is subscribed.
    AdvanceFrame {
        /// Timestamp in milliseconds, forwarded to `animation_frame`.
        timestamp: u64,
    },
    /// Register a stub response for an effect kind. When an effect of
    /// this kind is requested, the renderer returns the stubbed response
    /// immediately without executing the real effect.
    ///
    /// Used for testing (controlled responses) and scripting (no user
    /// interaction). The response value is returned as-is in an
    /// `effect_response` with status "ok".
    RegisterEffectStub {
        /// Event kind string used on the wire.
        kind: String,
        /// Response.
        response: Value,
    },
    /// Remove a previously registered effect stub.
    UnregisterEffectStub {
        /// Event kind string used on the wire.
        kind: String,
    },
    /// Load a font at runtime.
    ///
    /// Typed binary message: in MessagePack mode, font bytes travel as
    /// native binary instead of base64 string. JSON mode continues to
    /// accept base64 strings.
    LoadFont {
        /// Payload.
        #[serde(default)]
        payload: LoadFontPayload,
    },
}

/// Payload of an [`IncomingMessage::LoadFont`] message.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LoadFontPayload {
    /// Font family name to register the loaded data under.
    #[serde(default)]
    pub family: String,
    /// Font bytes. TrueType (.ttf), OpenType (.otf), or TrueType
    /// Collection (.ttc); variable fonts supported. fontdb sniffs the
    /// format from the bytes themselves.
    #[serde(default, deserialize_with = "deserialize_binary_field")]
    pub data: Option<Vec<u8>>,
}

/// Payload of an [`IncomingMessage::ImageOp`] message.
///
/// Fields are union-style: individual ops use a subset. `data` and
/// `pixels` accept base64-encoded strings (JSON mode) and serde byte
/// arrays (MessagePack mode).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ImageOpPayload {
    #[serde(default)]
    /// Handle.
    pub handle: String,
    #[serde(default, deserialize_with = "deserialize_binary_field")]
    /// Raw bytes (pixels, font, etc.).
    pub data: Option<Vec<u8>>,
    #[serde(default, deserialize_with = "deserialize_binary_field")]
    /// Pixels.
    pub pixels: Option<Vec<u8>>,
    #[serde(default)]
    /// Width in pixels.
    pub width: Option<u32>,
    #[serde(default)]
    /// Height in pixels.
    pub height: Option<u32>,
    #[serde(default)]
    /// Correlation tag used for matching responses.
    pub tag: Option<String>,
}

// ---------------------------------------------------------------------------
// Binary field deserialization
// ---------------------------------------------------------------------------

/// Deserializes a binary field that may arrive as:
/// - Base64-encoded string (JSON path, or msgpack binary via rmpv conversion)
/// - Array of u8 values (serde byte sequence fallback)
/// - null / absent (returns None)
///
/// When the codec's rmpv-based decode extracts binary fields, it injects them
/// as compact base64 strings. JSON mode uses the same representation.
fn deserialize_binary_field<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let val: Option<Value> = Option::deserialize(deserializer)?;
    match val {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        // Base64 string.
        Some(Value::String(s)) => {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(&s)
                .map(Some)
                .map_err(|e| D::Error::custom(format!("base64 decode: {e}")))
        }
        // Array of u8 values.
        Some(Value::Array(arr)) => {
            let bytes: Result<Vec<u8>, _> = arr
                .into_iter()
                .map(|v| {
                    v.as_u64()
                        .and_then(|n| u8::try_from(n).ok())
                        .ok_or_else(|| D::Error::custom("expected u8 in binary array"))
                })
                .collect();
            bytes.map(Some)
        }
        Some(other) => Err(D::Error::custom(format!(
            "expected string, array, or null for binary field, got {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -----------------------------------------------------------------------
    // IncomingMessage deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn deserialize_snapshot() {
        let json =
            r#"{"type":"snapshot","tree":{"id":"root","type":"column","props":{},"children":[]}}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Snapshot { tree } => {
                assert_eq!(tree.id, "root");
                assert_eq!(tree.type_name, "column");
            }
            _ => panic!("expected Snapshot"),
        }
    }

    #[test]
    fn deserialize_snapshot_nested_tree() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "snapshot",
            "tree": {
                "id": "root",
                "type": "column",
                "props": { "spacing": 10 },
                "children": [{
                    "id": "c1",
                    "type": "text",
                    "props": { "content": "hello" },
                    "children": []
                }]
            }
        }))
        .unwrap();
        match msg {
            IncomingMessage::Snapshot { tree } => {
                assert_eq!(tree.children.len(), 1);
                assert_eq!(tree.children[0].id, "c1");
                assert_eq!(tree.children[0].type_name, "text");
                assert_eq!(tree.props.get_f64("spacing"), Some(10.0));
            }
            _ => panic!("expected Snapshot"),
        }
    }

    #[test]
    fn deserialize_patch_replace_node() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "patch",
            "ops": [{
                "op": "replace_node",
                "path": [0],
                "node": {
                    "id": "x",
                    "type": "text",
                    "props": {},
                    "children": []
                }
            }]
        }))
        .unwrap();
        match msg {
            IncomingMessage::Patch { ops } => {
                assert_eq!(ops.len(), 1);
                assert_eq!(ops[0].op, "replace_node");
                assert_eq!(ops[0].path, vec![0]);
                assert!(ops[0].rest.get("node").is_some());
            }
            _ => panic!("expected Patch"),
        }
    }

    #[test]
    fn deserialize_patch_multiple_ops() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "patch",
            "ops": [
                { "op": "update_props", "path": [0], "props": { "color": "red" } },
                { "op": "remove_child", "path": [], "index": 2 }
            ]
        }))
        .unwrap();
        match msg {
            IncomingMessage::Patch { ops } => {
                assert_eq!(ops.len(), 2);
                assert_eq!(ops[0].op, "update_props");
                assert_eq!(ops[1].op, "remove_child");
            }
            _ => panic!("expected Patch"),
        }
    }

    #[test]
    fn deserialize_effect() {
        let json = r#"{"type":"effect","id":"e1","kind":"clipboard_read","payload":{}}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Effect { id, kind, payload } => {
                assert_eq!(id, "e1");
                assert_eq!(kind, "clipboard_read");
                assert!(payload.is_object());
            }
            _ => panic!("expected Effect"),
        }
    }

    #[test]
    fn deserialize_effect_with_payload() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "effect",
            "id": "e2",
            "kind": "clipboard_write",
            "payload": { "text": "copied" }
        }))
        .unwrap();
        match msg {
            IncomingMessage::Effect { id, kind, payload } => {
                assert_eq!(id, "e2");
                assert_eq!(kind, "clipboard_write");
                assert_eq!(payload["text"], "copied");
            }
            _ => panic!("expected Effect"),
        }
    }

    #[test]
    fn deserialize_widget_op() {
        let json = r#"{"type":"widget_op","op":"focus","payload":{"target":"input1"}}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::WidgetOp { op, payload } => {
                assert_eq!(op, "focus");
                assert_eq!(payload["target"], "input1");
            }
            _ => panic!("expected WidgetOp"),
        }
    }

    #[test]
    fn deserialize_widget_op_no_payload() {
        let json = r#"{"type":"widget_op","op":"blur"}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::WidgetOp { op, payload } => {
                assert_eq!(op, "blur");
                assert!(payload.is_null());
            }
            _ => panic!("expected WidgetOp"),
        }
    }

    #[test]
    fn deserialize_subscribe() {
        let json = r#"{"type":"subscribe","kind":"on_key_press","tag":"keys"}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Subscribe {
                kind,
                tag,
                window_id,
                max_rate,
            } => {
                assert_eq!(kind, "on_key_press");
                assert_eq!(tag, "keys");
                assert_eq!(window_id, None);
                assert_eq!(max_rate, None);
            }
            _ => panic!("expected Subscribe"),
        }
    }

    #[test]
    fn deserialize_subscribe_with_max_rate() {
        let json = r#"{"type":"subscribe","kind":"on_pointer_move","tag":"mouse","max_rate":30}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Subscribe {
                kind,
                tag,
                window_id,
                max_rate,
            } => {
                assert_eq!(kind, "on_pointer_move");
                assert_eq!(tag, "mouse");
                assert_eq!(window_id, None);
                assert_eq!(max_rate, Some(30));
            }
            _ => panic!("expected Subscribe"),
        }
    }

    #[test]
    fn deserialize_subscribe_with_window_id() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "subscribe",
            "kind": "on_key_press",
            "tag": "main_keys",
            "window_id": "main"
        }))
        .unwrap();
        match msg {
            IncomingMessage::Subscribe {
                kind,
                tag,
                window_id,
                max_rate,
            } => {
                assert_eq!(kind, "on_key_press");
                assert_eq!(tag, "main_keys");
                assert_eq!(window_id, Some("main".to_string()));
                assert_eq!(max_rate, None);
            }
            _ => panic!("expected Subscribe"),
        }
    }

    #[test]
    fn deserialize_unsubscribe() {
        let json = r#"{"type":"unsubscribe","kind":"on_key_press"}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Unsubscribe { kind, tag } => {
                assert_eq!(kind, "on_key_press");
                assert_eq!(tag, None);
            }
            _ => panic!("expected Unsubscribe"),
        }
    }

    #[test]
    fn deserialize_unsubscribe_with_tag() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "unsubscribe",
            "kind": "on_key_press",
            "tag": "main_keys"
        }))
        .unwrap();
        match msg {
            IncomingMessage::Unsubscribe { kind, tag } => {
                assert_eq!(kind, "on_key_press");
                assert_eq!(tag, Some("main_keys".to_string()));
            }
            _ => panic!("expected Unsubscribe"),
        }
    }

    #[test]
    fn deserialize_settings() {
        let json = r#"{"type":"settings","settings":{"default_text_size":18}}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Settings { settings } => {
                assert_eq!(settings["default_text_size"], 18);
            }
            _ => panic!("expected Settings"),
        }
    }

    #[test]
    fn deserialize_window_op() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "window_op",
            "op": "resize",
            "window_id": "main",
            "payload": { "width": 800, "height": 600 }
        }))
        .unwrap();
        match msg {
            IncomingMessage::WindowOp {
                op,
                window_id,
                payload,
            } => {
                assert_eq!(op, "resize");
                assert_eq!(window_id, "main");
                assert_eq!(payload["width"], 800);
                assert_eq!(payload["height"], 600);
            }
            _ => panic!("expected WindowOp"),
        }
    }

    #[test]
    fn deserialize_window_op_no_payload() {
        let json = r#"{"type":"window_op","op":"close","window_id":"popup"}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::WindowOp {
                op,
                window_id,
                payload,
            } => {
                assert_eq!(op, "close");
                assert_eq!(window_id, "popup");
                assert!(payload.is_null());
            }
            _ => panic!("expected WindowOp"),
        }
    }

    #[test]
    fn deserialize_malformed_json_missing_field() {
        let json = r#"{"type":"snapshot"}"#;
        let result = serde_json::from_str::<IncomingMessage>(json);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_unknown_type_tag() {
        let json = r#"{"type":"bogus_message","data":42}"#;
        let result = serde_json::from_str::<IncomingMessage>(json);
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_invalid_json_syntax() {
        let json = r#"{"type":"snapshot",,,}"#;
        let result = serde_json::from_str::<IncomingMessage>(json);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Command deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn command_deserializes() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "command",
            "id": "term-1",
            "family": "write",
            "value": { "data": "hello" }
        }))
        .unwrap();
        match msg {
            IncomingMessage::Command { id, family, value } => {
                assert_eq!(id, "term-1");
                assert_eq!(family, "write");
                assert_eq!(value["data"], "hello");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn commands_batch_deserializes() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "commands",
            "commands": [
                { "id": "term-1", "family": "write", "value": { "data": "a" } },
                { "id": "log-1", "family": "append", "value": { "line": "x" } }
            ]
        }))
        .unwrap();
        match msg {
            IncomingMessage::Commands { commands } => {
                assert_eq!(commands.len(), 2);
                assert_eq!(commands[0].id, "term-1");
                assert_eq!(commands[1].family, "append");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn command_with_default_value() {
        let json = r#"{"type":"command","id":"wgt-1","family":"reset"}"#;
        let msg: IncomingMessage = serde_json::from_str(json).unwrap();
        match msg {
            IncomingMessage::Command { value, .. } => {
                assert!(value.is_null());
            }
            _ => panic!("wrong variant"),
        }
    }

    // -----------------------------------------------------------------------
    // Scripting message deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn deserialize_query() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "query",
            "id": "q1",
            "target": "tree"
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::Query { .. }));
    }

    #[test]
    fn deserialize_query_with_selector() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "query",
            "id": "q2",
            "target": "find",
            "selector": {"by": "id", "value": "btn1"}
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::Query { .. }));
    }

    #[test]
    fn deserialize_interact() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "interact",
            "id": "i1",
            "action": "click",
            "selector": {"by": "id", "value": "btn1"},
            "payload": {}
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::Interact { .. }));
    }

    #[test]
    fn deserialize_tree_hash() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "tree_hash",
            "id": "th1",
            "name": "check"
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::TreeHash { .. }));
    }

    #[test]
    fn deserialize_screenshot() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "screenshot",
            "id": "ss1",
            "name": "test"
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::Screenshot { .. }));
    }

    #[test]
    fn deserialize_reset() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "reset",
            "id": "r1"
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::Reset { .. }));
    }

    #[test]
    fn deserialize_advance_frame() {
        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "advance_frame",
            "timestamp": 16
        }))
        .unwrap();
        assert!(matches!(msg, IncomingMessage::AdvanceFrame { .. }));
    }

    #[test]
    fn deserialize_load_font_with_base64_data() {
        use base64::Engine as _;
        let bytes: Vec<u8> = vec![0x00, 0x01, 0x02, 0x03];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        let msg: IncomingMessage = serde_json::from_value(json!({
            "type": "load_font",
            "payload": {
                "family": "Inter",
                "data": b64,
            }
        }))
        .unwrap();

        match msg {
            IncomingMessage::LoadFont { payload } => {
                assert_eq!(payload.family, "Inter");
                assert_eq!(payload.data, Some(bytes));
            }
            other => panic!("expected LoadFont, got {other:?}"),
        }
    }
}
