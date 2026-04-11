//! Converts renderer sink events to SDK events.
//!
//! The event bridge sits between the renderer's EventSink output
//! (OutgoingEvent, EffectResponse, QueryResponse) and the SDK's
//! typed Event enum. This is the single conversion point used by
//! the direct runner's QueueSink drain cycle.

use serde_json::Value;

use plushie_core::protocol::{EffectResponse, OutgoingEvent};

use crate::event::*;
use crate::types::KeyModifiers;

use super::queue_sink::SinkEvent;

/// Convert a SinkEvent to an SDK Event.
pub(crate) fn sink_event_to_sdk(sink_event: SinkEvent) -> Option<Event> {
    match sink_event {
        SinkEvent::Event(event) => outgoing_to_sdk_event(event),
        SinkEvent::EffectResponse(response) => Some(effect_response_to_sdk(response)),
        SinkEvent::QueryResponse { kind, tag, data } => {
            Some(query_response_to_sdk(&kind, &tag, data))
        }
    }
}

/// Convert an OutgoingEvent to an SDK Event.
fn outgoing_to_sdk_event(event: OutgoingEvent) -> Option<Event> {
    let family = event.family.as_str();

    // Subscription events have a tag but typically no widget id.
    if let Some(ref tag) = event.tag {
        return tagged_event_to_sdk(family, tag, &event);
    }

    // Widget events: split scoped id and map family to EventType.
    let (local_id, scope) = split_scoped_id(&event.id);
    let event_type = family_to_event_type(family);
    let primary_value = event.data
        .or(event.value)
        .unwrap_or(Value::Null);
    let window_id = event.window_id.unwrap_or_default();

    Some(Event::Widget(WidgetEvent {
        event_type,
        id: local_id,
        window_id,
        scope,
        value: primary_value,
    }))
}

/// Convert a tagged (subscription) event to an SDK Event.
fn tagged_event_to_sdk(family: &str, tag: &str, event: &OutgoingEvent) -> Option<Event> {
    let window_id = event.window_id.clone();

    match family {
        "key_press" | "key_release" => {
            let data = event.data.as_ref().unwrap_or(&Value::Null);
            Some(Event::Key(KeyEvent {
                event_type: if family == "key_press" {
                    KeyEventType::Press
                } else {
                    KeyEventType::Release
                },
                key: json_str(data, "key"),
                modified_key: json_str_opt(data, "modified_key"),
                physical_key: json_str_opt(data, "physical_key"),
                location: match json_str_opt(data, "location").as_deref() {
                    Some("left") => KeyLocation::Left,
                    Some("right") => KeyLocation::Right,
                    Some("numpad") => KeyLocation::Numpad,
                    _ => KeyLocation::Standard,
                },
                modifiers: extract_modifiers(event),
                text: json_str_opt(data, "text"),
                repeat: data["repeat"].as_bool().unwrap_or(false),
                captured: event.captured.unwrap_or(false),
                window_id,
            }))
        }

        "modifiers_changed" => {
            Some(Event::Modifiers(ModifiersEvent {
                modifiers: extract_modifiers(event),
                captured: event.captured.unwrap_or(false),
                window_id,
            }))
        }

        "window_opened" => Some(window_event(WindowEventType::Opened, event)),
        "window_closed" => Some(window_event(WindowEventType::Closed, event)),
        "window_close_requested" => Some(window_event(WindowEventType::CloseRequested, event)),
        "window_moved" => Some(window_event(WindowEventType::Moved, event)),
        "window_resized" => Some(window_event(WindowEventType::Resized, event)),
        "window_focused" => Some(window_event(WindowEventType::Focused, event)),
        "window_unfocused" => Some(window_event(WindowEventType::Unfocused, event)),
        "window_rescaled" => Some(window_event(WindowEventType::Rescaled, event)),
        "file_hovered" => Some(window_event(WindowEventType::FileHovered, event)),
        "file_dropped" => Some(window_event(WindowEventType::FileDropped, event)),
        "files_hovered_left" => Some(window_event(WindowEventType::FilesHoveredLeft, event)),

        "animation_frame" => {
            Some(Event::System(SystemEvent {
                event_type: SystemEventType::AnimationFrame,
                tag: Some(tag.to_string()),
                value: event.value.clone(),
                id: None,
                window_id,
            }))
        }

        "theme_changed" => {
            Some(Event::System(SystemEvent {
                event_type: SystemEventType::ThemeChanged,
                tag: Some(tag.to_string()),
                value: event.value.clone(),
                id: None,
                window_id: None,
            }))
        }

        "ime_opened" | "ime_preedit" | "ime_commit" | "ime_closed" => {
            let data = event.data.as_ref().unwrap_or(&Value::Null);
            let (local_id, scope) = event.data.as_ref()
                .and_then(|d| d["id"].as_str())
                .map(|id| split_scoped_id(id))
                .unwrap_or_default();
            Some(Event::Ime(ImeEvent {
                event_type: match family {
                    "ime_opened" => ImeEventType::Opened,
                    "ime_preedit" => ImeEventType::Preedit,
                    "ime_commit" => ImeEventType::Commit,
                    _ => ImeEventType::Closed,
                },
                id: if local_id.is_empty() { None } else { Some(local_id) },
                scope,
                text: json_str_opt(data, "text"),
                cursor: data["cursor"].as_array().and_then(|arr| {
                    Some((arr.first()?.as_u64()? as usize, arr.get(1)?.as_u64()? as usize))
                }),
                captured: event.captured.unwrap_or(false),
                window_id,
            }))
        }

        "widget_command_error" => {
            let data = event.data.as_ref().unwrap_or(&Value::Null);
            Some(Event::WidgetCommandError(WidgetCommandError {
                reason: json_str(data, "reason"),
                node_id: json_str_opt(data, "node_id"),
                op: json_str_opt(data, "op"),
                widget_type: json_str_opt(data, "widget_type"),
                message: json_str_opt(data, "message"),
            }))
        }

        // Fall through: treat as a system event with the tag.
        _ => {
            Some(Event::System(SystemEvent {
                event_type: SystemEventType::SystemInfo,
                tag: Some(tag.to_string()),
                value: event.value.clone(),
                id: None,
                window_id,
            }))
        }
    }
}

/// Convert an EffectResponse to an SDK EffectEvent.
fn effect_response_to_sdk(response: EffectResponse) -> Event {
    let result = match response.status {
        "ok" => EffectResult::Ok(response.result.unwrap_or(Value::Null)),
        "cancelled" => EffectResult::Cancelled,
        _ => EffectResult::Error(
            response.error.map(|e| Value::String(e))
                .or(response.result)
                .unwrap_or(Value::Null)
        ),
    };
    Event::Effect(EffectEvent {
        tag: response.id,
        result,
    })
}

/// Convert a query response to an SDK SystemEvent.
fn query_response_to_sdk(kind: &str, tag: &str, data: Value) -> Event {
    let event_type = match kind {
        "tree_hash" => SystemEventType::TreeHash,
        "find_focused" => SystemEventType::FindFocused,
        "list_images" => SystemEventType::ImageList,
        _ => SystemEventType::SystemInfo,
    };
    Event::System(SystemEvent {
        event_type,
        tag: Some(tag.to_string()),
        value: Some(data),
        id: None,
        window_id: None,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a WindowEvent from an OutgoingEvent.
fn window_event(event_type: WindowEventType, event: &OutgoingEvent) -> Event {
    let data = event.data.as_ref().unwrap_or(&Value::Null);
    let window_id = event.window_id.clone().unwrap_or_default();

    Event::Window(WindowEvent {
        event_type,
        window_id,
        x: data["x"].as_f64().map(|v| v as f32),
        y: data["y"].as_f64().map(|v| v as f32),
        width: data["width"].as_f64().map(|v| v as f32),
        height: data["height"].as_f64().map(|v| v as f32),
        position: data["position"].as_array().and_then(|arr| {
            Some((arr.first()?.as_f64()? as f32, arr.get(1)?.as_f64()? as f32))
        }),
        path: json_str_opt(data, "path"),
        scale_factor: data["scale_factor"].as_f64().map(|v| v as f32),
    })
}

/// Split a scoped ID ("form/section/field") into local ID and reversed scope.
fn split_scoped_id(scoped: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = scoped.split('/').collect();
    if parts.len() <= 1 {
        (scoped.to_string(), vec![])
    } else {
        let local = parts.last().unwrap().to_string();
        let scope = parts[..parts.len() - 1]
            .iter()
            .rev()
            .map(|s| s.to_string())
            .collect();
        (local, scope)
    }
}

/// Extract KeyModifiers from an OutgoingEvent.
fn extract_modifiers(event: &OutgoingEvent) -> KeyModifiers {
    match &event.modifiers {
        Some(m) => KeyModifiers {
            shift: m.shift,
            ctrl: m.ctrl,
            alt: m.alt,
            logo: m.logo,
            command: m.command,
        },
        None => KeyModifiers::default(),
    }
}

fn json_str(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn json_str_opt(value: &Value, key: &str) -> Option<String> {
    value[key].as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(family: &str, id: &str) -> OutgoingEvent {
        OutgoingEvent {
            message_type: "event",
            session: String::new(),
            family: family.to_string(),
            id: id.to_string(),
            window_id: Some("main".to_string()),
            value: None,
            tag: None,
            modifiers: None,
            data: None,
            captured: None,
            coalesce: None,
        }
    }

    fn make_tagged(family: &str, tag: &str) -> OutgoingEvent {
        OutgoingEvent {
            tag: Some(tag.to_string()),
            ..make_event(family, "")
        }
    }

    #[test]
    fn click_event() {
        let event = make_event("click", "save");
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Widget(w) => {
                assert_eq!(w.event_type, EventType::Click);
                assert_eq!(w.id, "save");
                assert_eq!(w.window_id, "main");
                assert!(w.scope.is_empty());
            }
            _ => panic!("expected Widget event"),
        }
    }

    #[test]
    fn scoped_click_event() {
        let event = make_event("click", "form/section/save");
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Widget(w) => {
                assert_eq!(w.id, "save");
                assert_eq!(w.scope, vec!["section", "form"]);
            }
            _ => panic!("expected Widget event"),
        }
    }

    #[test]
    fn input_event_uses_data_over_value() {
        let mut event = make_event("input", "name");
        event.value = Some(Value::String("old".to_string()));
        event.data = Some(Value::String("typed text".to_string()));
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Widget(w) => {
                assert_eq!(w.event_type, EventType::Input);
                assert_eq!(w.value, Value::String("typed text".to_string()));
            }
            _ => panic!("expected Widget event"),
        }
    }

    #[test]
    fn toggle_event() {
        let mut event = make_event("toggle", "dark_mode");
        event.value = Some(Value::Bool(true));
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Widget(w) => {
                assert_eq!(w.event_type, EventType::Toggle);
                assert_eq!(w.value, Value::Bool(true));
            }
            _ => panic!("expected Widget event"),
        }
    }

    #[test]
    fn slide_event() {
        let mut event = make_event("slide", "volume");
        event.value = Some(serde_json::json!(0.75));
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Widget(w) => {
                assert_eq!(w.event_type, EventType::Slide);
                assert_eq!(w.value, serde_json::json!(0.75));
            }
            _ => panic!("expected Widget event"),
        }
    }

    #[test]
    fn key_press_event() {
        let mut event = make_tagged("key_press", "key_events");
        event.data = Some(serde_json::json!({
            "key": "a",
            "modified_key": "A",
            "physical_key": "KeyA",
            "location": "standard",
            "text": "A",
            "repeat": false,
        }));
        event.modifiers = Some(plushie_core::protocol::KeyModifiers {
            shift: true,
            ctrl: false,
            alt: false,
            logo: false,
            command: false,
        });
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Key(k) => {
                assert_eq!(k.event_type, KeyEventType::Press);
                assert_eq!(k.key, "a");
                assert_eq!(k.modified_key, Some("A".to_string()));
                assert_eq!(k.physical_key, Some("KeyA".to_string()));
                assert_eq!(k.text, Some("A".to_string()));
                assert!(k.modifiers.shift);
                assert!(!k.modifiers.ctrl);
            }
            _ => panic!("expected Key event"),
        }
    }

    #[test]
    fn modifiers_changed_event() {
        let mut event = make_tagged("modifiers_changed", "mods");
        event.modifiers = Some(plushie_core::protocol::KeyModifiers {
            shift: false, ctrl: true, alt: false, logo: false, command: false,
        });
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Modifiers(m) => {
                assert!(m.modifiers.ctrl);
                assert!(!m.modifiers.shift);
            }
            _ => panic!("expected Modifiers event"),
        }
    }

    #[test]
    fn window_resized_event() {
        let mut event = make_tagged("window_resized", "win_events");
        event.window_id = Some("main".to_string());
        event.data = Some(serde_json::json!({
            "width": 800.0,
            "height": 600.0,
        }));
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Window(w) => {
                assert_eq!(w.event_type, WindowEventType::Resized);
                assert_eq!(w.window_id, "main");
                assert_eq!(w.width, Some(800.0));
                assert_eq!(w.height, Some(600.0));
            }
            _ => panic!("expected Window event"),
        }
    }

    #[test]
    fn animation_frame_event() {
        let mut event = make_tagged("animation_frame", "anim");
        event.value = Some(serde_json::json!(16.67));
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::AnimationFrame);
                assert_eq!(s.tag, Some("anim".to_string()));
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn effect_response_ok() {
        let response = EffectResponse {
            message_type: "effect_response",
            session: String::new(),
            id: "save_file".to_string(),
            status: "ok",
            result: Some(serde_json::json!({"path": "/tmp/file.txt"})),
            error: None,
        };
        let sdk = effect_response_to_sdk(response);
        match sdk {
            Event::Effect(e) => {
                assert_eq!(e.tag, "save_file");
                match e.result {
                    EffectResult::Ok(v) => assert_eq!(v["path"], "/tmp/file.txt"),
                    _ => panic!("expected Ok result"),
                }
            }
            _ => panic!("expected Effect event"),
        }
    }

    #[test]
    fn effect_response_cancelled() {
        let response = EffectResponse {
            message_type: "effect_response",
            session: String::new(),
            id: "open_file".to_string(),
            status: "cancelled",
            result: None,
            error: None,
        };
        let sdk = effect_response_to_sdk(response);
        match sdk {
            Event::Effect(e) => {
                assert_eq!(e.tag, "open_file");
                assert!(matches!(e.result, EffectResult::Cancelled));
            }
            _ => panic!("expected Effect event"),
        }
    }

    #[test]
    fn effect_response_error() {
        let response = EffectResponse {
            message_type: "effect_response",
            session: String::new(),
            id: "clipboard".to_string(),
            status: "error",
            result: None,
            error: Some("permission denied".to_string()),
        };
        let sdk = effect_response_to_sdk(response);
        match sdk {
            Event::Effect(e) => {
                assert_eq!(e.tag, "clipboard");
                match e.result {
                    EffectResult::Error(v) => assert_eq!(v, "permission denied"),
                    _ => panic!("expected Error result"),
                }
            }
            _ => panic!("expected Effect event"),
        }
    }

    #[test]
    fn query_response_tree_hash() {
        let sdk = query_response_to_sdk("tree_hash", "hash1", serde_json::json!({"hash": "abc"}));
        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::TreeHash);
                assert_eq!(s.tag, Some("hash1".to_string()));
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn query_response_find_focused() {
        let sdk = query_response_to_sdk("find_focused", "f1", serde_json::json!({"focused": "input1"}));
        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::FindFocused);
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn split_scoped_id_simple() {
        let (local, scope) = split_scoped_id("save");
        assert_eq!(local, "save");
        assert!(scope.is_empty());
    }

    #[test]
    fn split_scoped_id_nested() {
        let (local, scope) = split_scoped_id("form/section/field");
        assert_eq!(local, "field");
        assert_eq!(scope, vec!["section", "form"]);
    }

    #[test]
    fn sink_event_dispatches_correctly() {
        let event = SinkEvent::Event(make_event("click", "btn"));
        let sdk = sink_event_to_sdk(event).unwrap();
        assert!(matches!(sdk, Event::Widget(_)));

        let response = SinkEvent::EffectResponse(EffectResponse {
            message_type: "effect_response",
            session: String::new(),
            id: "tag".to_string(),
            status: "ok",
            result: None,
            error: None,
        });
        let sdk = sink_event_to_sdk(response).unwrap();
        assert!(matches!(sdk, Event::Effect(_)));

        let query = SinkEvent::QueryResponse {
            kind: "tree_hash".to_string(),
            tag: "t1".to_string(),
            data: serde_json::json!({}),
        };
        let sdk = sink_event_to_sdk(query).unwrap();
        assert!(matches!(sdk, Event::System(_)));
    }
}
