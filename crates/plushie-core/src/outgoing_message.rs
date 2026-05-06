//! Typed outgoing wire protocol messages (SDK -> renderer).
//!
//! Mirrors `IncomingMessage` on the renderer side, providing
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
    /// Advance the animation clock by one frame in headless/mock mode.
    AdvanceFrame {
        /// Session.
        session: String,
        /// Timestamp in milliseconds, forwarded to `animation_frame`.
        timestamp: u64,
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
    /// Load Font.
    ///
    /// Typed binary message: in MessagePack mode, font bytes travel as
    /// native binary instead of base64 string. JSON mode encodes
    /// `payload.data` as a base64 string.
    LoadFont {
        /// Session.
        session: String,
        /// Payload.
        payload: Value,
    },
}

#[cfg(test)]
mod tests {
    //! These tests pin the `OutgoingMessage` -> JSON shape that the SDK
    //! emits onto the wire and verify each shape round-trips cleanly into
    //! the renderer's [`IncomingMessage`] type. Drift between the two
    //! type definitions (a renamed field, a missing variant, a tag
    //! mismatch) shows up here as either a mismatched JSON value or a
    //! decode failure on the cross-decode pass.
    //!
    //! Each variant gets two checks:
    //!
    //! 1. The JSON body matches a hand-written expected shape, including
    //!    the `"type"` tag and field names.
    //! 2. The same JSON deserializes into the corresponding
    //!    `IncomingMessage` variant. The only `OutgoingMessage` that has
    //!    no matching `IncomingMessage` variant today (`Subscribe` and
    //!    `Unsubscribe` carry an extra `session` envelope, but the
    //!    payload still parses) is documented inline.
    use super::OutgoingMessage;
    use crate::ops::WidgetCommand;
    use crate::protocol::IncomingMessage;
    use serde_json::{Value, json};

    /// Build the JSON body for a message and deserialize it as
    /// `IncomingMessage`. The `session` envelope field that
    /// `OutgoingMessage` carries is stripped before deserialization
    /// because the renderer's typed envelope handles routing
    /// separately, before tag dispatch reaches `IncomingMessage`.
    fn cross_decode(msg: &OutgoingMessage) -> IncomingMessage {
        let mut value = serde_json::to_value(msg).unwrap();
        if let Value::Object(map) = &mut value {
            map.remove("session");
        }
        serde_json::from_value::<IncomingMessage>(value).unwrap_or_else(|e| {
            panic!("cross-decode failed for {msg:?}: {e}");
        })
    }

    // -----------------------------------------------------------------------
    // Settings / Snapshot / Patch
    // -----------------------------------------------------------------------

    #[test]
    fn settings_round_trips() {
        let msg = OutgoingMessage::Settings {
            session: "s1".into(),
            settings: json!({"default_text_size": 18}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "settings",
                "session": "s1",
                "settings": {"default_text_size": 18},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::Settings { .. }
        ));
    }

    #[test]
    fn snapshot_round_trips() {
        let tree = json!({"id": "root", "type": "column", "props": {}, "children": []});
        let msg = OutgoingMessage::Snapshot {
            session: "s1".into(),
            tree: tree.clone(),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"type": "snapshot", "session": "s1", "tree": tree})
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::Snapshot { .. }
        ));
    }

    #[test]
    fn patch_round_trips() {
        let ops = vec![json!({"op": "remove_child", "path": [], "index": 0})];
        let msg = OutgoingMessage::Patch {
            session: "s1".into(),
            ops: ops.clone(),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"type": "patch", "session": "s1", "ops": ops})
        );
        assert!(matches!(cross_decode(&msg), IncomingMessage::Patch { .. }));
    }

    // -----------------------------------------------------------------------
    // Subscribe / Unsubscribe
    // -----------------------------------------------------------------------

    #[test]
    fn subscribe_round_trips_with_optional_fields_omitted() {
        let msg = OutgoingMessage::Subscribe {
            session: "s1".into(),
            kind: "on_key_press".into(),
            tag: "keys".into(),
            max_rate: None,
            window_id: None,
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "subscribe",
                "session": "s1",
                "kind": "on_key_press",
                "tag": "keys",
            })
        );
        match cross_decode(&msg) {
            IncomingMessage::Subscribe {
                kind,
                tag,
                max_rate,
                window_id,
            } => {
                assert_eq!(kind, "on_key_press");
                assert_eq!(tag, "keys");
                assert_eq!(max_rate, None);
                assert_eq!(window_id, None);
            }
            other => panic!("expected Subscribe, got {other:?}"),
        }
    }

    #[test]
    fn subscribe_round_trips_with_max_rate_and_window_id() {
        let msg = OutgoingMessage::Subscribe {
            session: "s1".into(),
            kind: "on_pointer_move".into(),
            tag: "mouse".into(),
            max_rate: Some(60),
            window_id: Some("main".into()),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "subscribe",
                "session": "s1",
                "kind": "on_pointer_move",
                "tag": "mouse",
                "max_rate": 60,
                "window_id": "main",
            })
        );
        match cross_decode(&msg) {
            IncomingMessage::Subscribe {
                max_rate,
                window_id,
                ..
            } => {
                assert_eq!(max_rate, Some(60));
                assert_eq!(window_id, Some("main".into()));
            }
            other => panic!("expected Subscribe, got {other:?}"),
        }
    }

    #[test]
    fn unsubscribe_round_trips() {
        let msg = OutgoingMessage::Unsubscribe {
            session: "s1".into(),
            kind: "on_key_press".into(),
            tag: "keys".into(),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "unsubscribe",
                "session": "s1",
                "kind": "on_key_press",
                "tag": "keys",
            })
        );
        // OutgoingMessage::Unsubscribe always sends a tag; the renderer's
        // IncomingMessage::Unsubscribe accepts an Optional<tag>, so the
        // tag survives the cross-decode.
        match cross_decode(&msg) {
            IncomingMessage::Unsubscribe { kind, tag } => {
                assert_eq!(kind, "on_key_press");
                assert_eq!(tag.as_deref(), Some("keys"));
            }
            other => panic!("expected Unsubscribe, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Widget / Window / System ops
    // -----------------------------------------------------------------------

    #[test]
    fn widget_op_round_trips() {
        let msg = OutgoingMessage::WidgetOp {
            session: "s1".into(),
            op: "focus".into(),
            payload: json!({"target": "input1"}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "widget_op",
                "session": "s1",
                "op": "focus",
                "payload": {"target": "input1"},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::WidgetOp { .. }
        ));
    }

    #[test]
    fn window_op_round_trips() {
        let msg = OutgoingMessage::WindowOp {
            session: "s1".into(),
            op: "resize".into(),
            window_id: "main".into(),
            payload: json!({"width": 800, "height": 600}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "window_op",
                "session": "s1",
                "op": "resize",
                "window_id": "main",
                "payload": {"width": 800, "height": 600},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::WindowOp { .. }
        ));
    }

    #[test]
    fn system_op_round_trips() {
        let msg = OutgoingMessage::SystemOp {
            session: "s1".into(),
            op: "allow_automatic_tabbing".into(),
            payload: json!({"enabled": true}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "system_op",
                "session": "s1",
                "op": "allow_automatic_tabbing",
                "payload": {"enabled": true},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::SystemOp { .. }
        ));
    }

    #[test]
    fn system_query_round_trips() {
        let msg = OutgoingMessage::SystemQuery {
            session: "s1".into(),
            op: "get_system_theme".into(),
            payload: json!({"tag": "t1"}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "system_query",
                "session": "s1",
                "op": "get_system_theme",
                "payload": {"tag": "t1"},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::SystemQuery { .. }
        ));
    }

    #[test]
    fn image_op_round_trips() {
        let msg = OutgoingMessage::ImageOp {
            session: "s1".into(),
            op: "list".into(),
            payload: json!({"tag": "snapshot"}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "image_op",
                "session": "s1",
                "op": "list",
                "payload": {"tag": "snapshot"},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::ImageOp { .. }
        ));
    }

    // -----------------------------------------------------------------------
    // Command / Commands
    // -----------------------------------------------------------------------

    #[test]
    fn command_round_trips() {
        let msg = OutgoingMessage::Command {
            session: "s1".into(),
            id: "term-1".into(),
            family: "write".into(),
            value: json!({"data": "hello"}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "command",
                "session": "s1",
                "id": "term-1",
                "family": "write",
                "value": {"data": "hello"},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::Command { .. }
        ));
    }

    #[test]
    fn commands_round_trips() {
        let cmds = vec![
            WidgetCommand::raw("term-1", "write", json!({"data": "a"})),
            WidgetCommand::raw("log-1", "append", json!({"line": "x"})),
        ];
        let msg = OutgoingMessage::Commands {
            session: "s1".into(),
            commands: cmds,
        };
        // The serde shape for WidgetCommand mirrors the IncomingMessage
        // accepts; just verify the envelope and round-trip.
        let value = serde_json::to_value(&msg).unwrap();
        assert_eq!(value["type"], "commands");
        assert_eq!(value["session"], "s1");
        assert_eq!(value["commands"][0]["id"], "term-1");
        match cross_decode(&msg) {
            IncomingMessage::Commands { commands } => {
                assert_eq!(commands.len(), 2);
                assert_eq!(commands[0].id, "term-1");
                assert_eq!(commands[1].family, "append");
            }
            other => panic!("expected Commands, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Effect
    // -----------------------------------------------------------------------

    #[test]
    fn effect_round_trips() {
        let msg = OutgoingMessage::Effect {
            session: "s1".into(),
            id: "e1".into(),
            kind: "clipboard_write".into(),
            payload: json!({"text": "hi"}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "effect",
                "session": "s1",
                "id": "e1",
                "kind": "clipboard_write",
                "payload": {"text": "hi"},
            })
        );
        assert!(matches!(cross_decode(&msg), IncomingMessage::Effect { .. }));
    }

    // -----------------------------------------------------------------------
    // Interact / Query / Reset
    // -----------------------------------------------------------------------

    #[test]
    fn interact_round_trips() {
        let msg = OutgoingMessage::Interact {
            session: "s1".into(),
            id: "i1".into(),
            action: "click".into(),
            selector: json!({"by": "id", "value": "btn"}),
            payload: json!({}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "interact",
                "session": "s1",
                "id": "i1",
                "action": "click",
                "selector": {"by": "id", "value": "btn"},
                "payload": {},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::Interact { .. }
        ));
    }

    #[test]
    fn query_round_trips_with_selector() {
        let msg = OutgoingMessage::Query {
            session: "s1".into(),
            id: "q1".into(),
            target: "find".into(),
            selector: Some(json!({"by": "role", "value": "button"})),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "query",
                "session": "s1",
                "id": "q1",
                "target": "find",
                "selector": {"by": "role", "value": "button"},
            })
        );
        assert!(matches!(cross_decode(&msg), IncomingMessage::Query { .. }));
    }

    #[test]
    fn query_round_trips_without_selector() {
        let msg = OutgoingMessage::Query {
            session: "s1".into(),
            id: "q1".into(),
            target: "tree".into(),
            selector: None,
        };
        // The Optional `selector` field is omitted from the JSON output;
        // a missing selector is still a legal Query envelope downstream.
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "query",
                "session": "s1",
                "id": "q1",
                "target": "tree",
            })
        );
        assert!(matches!(cross_decode(&msg), IncomingMessage::Query { .. }));
    }

    #[test]
    fn reset_round_trips() {
        let msg = OutgoingMessage::Reset {
            session: "s1".into(),
            id: "r1".into(),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({"type": "reset", "session": "s1", "id": "r1"})
        );
        assert!(matches!(cross_decode(&msg), IncomingMessage::Reset { .. }));
    }

    // -----------------------------------------------------------------------
    // AdvanceFrame
    // -----------------------------------------------------------------------

    #[test]
    fn advance_frame_serializes_as_top_level_message() {
        let msg = OutgoingMessage::AdvanceFrame {
            session: "s1".to_string(),
            timestamp: 16_000,
        };

        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "advance_frame",
                "session": "s1",
                "timestamp": 16_000,
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::AdvanceFrame { .. }
        ));
    }

    // -----------------------------------------------------------------------
    // Effect stub register / unregister
    // -----------------------------------------------------------------------

    #[test]
    fn register_effect_stub_round_trips() {
        let msg = OutgoingMessage::RegisterEffectStub {
            session: "s1".into(),
            kind: "file_open".into(),
            response: json!({"status": "ok", "result": {"path": "/tmp/test"}}),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "register_effect_stub",
                "session": "s1",
                "kind": "file_open",
                "response": {"status": "ok", "result": {"path": "/tmp/test"}},
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::RegisterEffectStub { .. }
        ));
    }

    #[test]
    fn unregister_effect_stub_round_trips() {
        let msg = OutgoingMessage::UnregisterEffectStub {
            session: "s1".into(),
            kind: "file_open".into(),
        };
        assert_eq!(
            serde_json::to_value(&msg).unwrap(),
            json!({
                "type": "unregister_effect_stub",
                "session": "s1",
                "kind": "file_open",
            })
        );
        assert!(matches!(
            cross_decode(&msg),
            IncomingMessage::UnregisterEffectStub { .. }
        ));
    }

    // -----------------------------------------------------------------------
    // LoadFont (typed binary message; JSON path)
    // -----------------------------------------------------------------------

    #[test]
    fn load_font_serializes_with_payload_envelope() {
        // The JSON path encodes `data` as a base64 string. The MsgPack
        // path uses native binary; that branch is exercised by the wire
        // integration test in `crates/plushie/tests/wire_load_font.rs`.
        use base64::Engine as _;
        let bytes = vec![0x00, 0x01, 0x02, 0xFFu8];
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

        let msg = OutgoingMessage::LoadFont {
            session: "s1".into(),
            payload: json!({"family": "Inter", "data": b64}),
        };

        let value = serde_json::to_value(&msg).unwrap();
        assert_eq!(value["type"], "load_font");
        assert_eq!(value["session"], "s1");
        assert_eq!(value["payload"]["family"], "Inter");
        assert!(value["payload"]["data"].is_string());

        match cross_decode(&msg) {
            IncomingMessage::LoadFont { payload } => {
                assert_eq!(payload.family, "Inter");
                assert_eq!(payload.data, Some(bytes));
            }
            other => panic!("expected LoadFont, got {other:?}"),
        }
    }
}
