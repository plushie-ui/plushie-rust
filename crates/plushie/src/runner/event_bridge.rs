//! Event conversion between renderer output and SDK event types.
//!
//! The event bridge converts renderer sink events (OutgoingEvent,
//! EffectResponse, QueryResponse) and SDK-local events (async
//! results, delayed events) into typed SDK Events. Used by both
//! the direct runner (QueueSink drain) and the wire runner
//! (deserialized wire protocol messages).

use serde_json::Value;

use plushie_core::key::{MouseButton, PointerKind};
use plushie_core::protocol::{EffectResponse, OutgoingEvent};

use crate::event::*;
use crate::types::KeyModifiers;

// ---------------------------------------------------------------------------
// SinkEvent: the union type for all events the bridge converts
// ---------------------------------------------------------------------------

/// An event to be converted to an SDK [`Event`].
///
/// In direct mode, these are collected in the QueueSink. In wire
/// mode, they are constructed from deserialized wire protocol JSON.
#[derive(Debug)]
pub(crate) enum SinkEvent {
    /// An OutgoingEvent from the renderer.
    Event(OutgoingEvent),
    /// An effect response from the renderer.
    EffectResponse(EffectResponse),
    /// A query response from the renderer.
    QueryResponse {
        kind: String,
        tag: String,
        data: Value,
    },
    /// Result of an async task (Command::Async).
    AsyncResult {
        tag: String,
        result: Result<Value, Value>,
    },
    /// Intermediate value from a streaming task (Command::Stream).
    StreamValue { tag: String, value: Value },
    /// A delayed event (Command::SendAfter).
    DelayedEvent(crate::event::Event),
    /// An effect whose deadline elapsed before a response arrived.
    ///
    /// Carries the tracker's wire ID; the dispatcher resolves it
    /// against the tracker to recover the user-facing tag and kind,
    /// then delivers `EffectResult::Timeout` to the app.
    ///
    /// Emitted by runner-local timeout tasks and resolved by the
    /// mode-specific dispatcher because tracker context is required.
    #[cfg_attr(not(feature = "wire"), allow(dead_code))]
    EffectTimeout { wire_id: String },
}

/// Convert a SinkEvent to an SDK Event.
pub(crate) fn sink_event_to_sdk(sink_event: SinkEvent) -> Option<Event> {
    match sink_event {
        SinkEvent::Event(event) => outgoing_to_sdk_event(event),
        SinkEvent::EffectResponse(response) => Some(effect_response_to_sdk(response)),
        SinkEvent::QueryResponse { kind, tag, data } => {
            Some(query_response_to_sdk(&kind, &tag, data))
        }
        SinkEvent::AsyncResult { tag, result } => Some(Event::Async(AsyncEvent { tag, result })),
        SinkEvent::StreamValue { tag, value } => {
            Some(Event::Stream(crate::event::StreamEvent { tag, value }))
        }
        SinkEvent::DelayedEvent(event) => Some(event),
        // EffectTimeout requires tracker context to resolve to an
        // Event; the wire-runner handles it directly and never calls
        // sink_event_to_sdk on this variant.
        SinkEvent::EffectTimeout { .. } => None,
    }
}

/// Convert an OutgoingEvent to an SDK Event.
fn outgoing_to_sdk_event(event: OutgoingEvent) -> Option<Event> {
    let family = event.family.as_str();

    // Subscription events have a tag but typically no widget id.
    if let Some(ref tag) = event.tag {
        return tagged_event_to_sdk(family, tag, &event);
    }

    if let Some(event_type) = bare_system_event_type(family) {
        return Some(Event::System(SystemEvent {
            event_type,
            tag: None,
            value: event.value,
            id: None,
            window_id: None,
        }));
    }

    // Status is the renderer's internal focus-tracking signal. Other SDK
    // runtimes consume it before app dispatch; Rust mirrors that by keeping
    // it out of the app-facing event stream until a separate opt-in lane
    // exists for raw interaction status.
    if family == "status" {
        return None;
    }

    // Widget events: parse canonical wire ID and map family to EventType.
    let sid = plushie_core::ScopedId::parse(&event.id);
    let event_type = family_to_event_type(family);
    let primary_value = event.value.unwrap_or(Value::Null);
    if !validate_widget_payload(family, &primary_value) {
        return None;
    }

    Some(Event::Widget(WidgetEvent {
        event_type,
        scoped_id: sid,
        value: primary_value,
    }))
}

fn validate_widget_payload(family: &str, value: &Value) -> bool {
    match family {
        "press" | "release" | "double_click" => {
            optional_mouse_button(value, family)
                && optional_pointer_kind(value, family)
                && optional_lost(value, family)
        }
        "move" | "scroll" | "drag" | "drag_end" => optional_pointer_kind(value, family),
        _ => true,
    }
}

fn optional_mouse_button(value: &Value, family: &str) -> bool {
    match value.get("button") {
        Some(Value::String(button)) if MouseButton::from_wire(button).is_some() => true,
        Some(Value::String(button)) => {
            log::warn!("dropping renderer event `{family}`: unknown pointer button `{button}`");
            false
        }
        Some(other) => {
            log::warn!(
                "dropping renderer event `{family}`: pointer button must be a string, got {}",
                json_type_name(other)
            );
            false
        }
        None => true,
    }
}

fn optional_pointer_kind(value: &Value, family: &str) -> bool {
    match value.get("pointer") {
        Some(Value::String(pointer)) if PointerKind::from_wire(pointer).is_some() => true,
        Some(Value::String(pointer)) => {
            log::warn!("dropping renderer event `{family}`: unknown pointer kind `{pointer}`");
            false
        }
        Some(other) => {
            log::warn!(
                "dropping renderer event `{family}`: pointer kind must be a string, got {}",
                json_type_name(other)
            );
            false
        }
        None => true,
    }
}

fn optional_lost(value: &Value, family: &str) -> bool {
    match value.get("lost") {
        Some(Value::Bool(_)) | None => true,
        Some(other) => {
            log::warn!(
                "dropping renderer event `{family}`: lost must be a boolean, got {}",
                json_type_name(other)
            );
            false
        }
    }
}

/// Convert a tagged (subscription) event to an SDK Event.
fn tagged_event_to_sdk(family: &str, tag: &str, event: &OutgoingEvent) -> Option<Event> {
    match family {
        "key_press" | "key_release" => {
            let value = event.value.as_ref().unwrap_or(&Value::Null);
            let key = required_json_str(value, "key", family)?;
            Some(Event::Key(KeyEvent {
                event_type: if family == "key_press" {
                    KeyEventType::Press
                } else {
                    KeyEventType::Release
                },
                key: plushie_core::Key::from(key.as_str()),
                modified_key: json_str_opt(value, "modified_key")
                    .map(|s| plushie_core::Key::from(s.as_str())),
                physical_key: json_str_opt(value, "physical_key")
                    .map(|s| plushie_core::Key::from(s.as_str())),
                location: match json_str_opt(value, "location").as_deref() {
                    Some("left") => KeyLocation::Left,
                    Some("right") => KeyLocation::Right,
                    Some("numpad") => KeyLocation::Numpad,
                    _ => KeyLocation::Standard,
                },
                modifiers: extract_modifiers(event),
                text: json_str_opt(value, "text"),
                repeat: value["repeat"].as_bool().unwrap_or(false),
                captured: event.captured.unwrap_or(false),
                window_id: None,
            }))
        }

        "modifiers_changed" => Some(Event::Modifiers(ModifiersEvent {
            modifiers: extract_modifiers(event),
            captured: event.captured.unwrap_or(false),
            window_id: None,
        })),

        "window_opened" => window_event(WindowEventType::Opened, event),
        "window_closed" => window_event(WindowEventType::Closed, event),
        "window_close_requested" => window_event(WindowEventType::CloseRequested, event),
        "window_moved" => window_event(WindowEventType::Moved, event),
        "window_resized" => window_event(WindowEventType::Resized, event),
        "window_focused" => window_event(WindowEventType::Focused, event),
        "window_unfocused" => window_event(WindowEventType::Unfocused, event),
        "window_rescaled" => window_event(WindowEventType::Rescaled, event),
        "file_hovered" => window_event(WindowEventType::FileHovered, event),
        "file_dropped" => window_event(WindowEventType::FileDropped, event),
        "files_hovered_left" => window_event(WindowEventType::FilesHoveredLeft, event),

        "animation_frame" => Some(Event::System(SystemEvent {
            event_type: SystemEventType::AnimationFrame,
            tag: Some(tag.to_string()),
            value: normalize_animation_frame_value(event.value.clone()),
            id: None,
            window_id: None,
        })),

        "theme_changed" => Some(Event::System(SystemEvent {
            event_type: SystemEventType::ThemeChanged,
            tag: Some(tag.to_string()),
            value: event.value.clone(),
            id: None,
            window_id: None,
        })),

        "ime_opened" | "ime_preedit" | "ime_commit" | "ime_closed" => {
            let value = event.value.as_ref().unwrap_or(&Value::Null);
            let sid = value["id"]
                .as_str()
                .map(plushie_core::ScopedId::parse)
                .unwrap_or_else(|| plushie_core::ScopedId::parse(""));
            Some(Event::Ime(ImeEvent {
                event_type: match family {
                    "ime_opened" => ImeEventType::Opened,
                    "ime_preedit" => ImeEventType::Preedit,
                    "ime_commit" => ImeEventType::Commit,
                    _ => ImeEventType::Closed,
                },
                id: if sid.id.is_empty() {
                    None
                } else {
                    Some(sid.id)
                },
                scope: sid.scope,
                text: json_str_opt(value, "text"),
                cursor: {
                    let cursor = &value["cursor"];
                    let start = cursor["start"]
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok());
                    let end = cursor["end"]
                        .as_u64()
                        .and_then(|value| usize::try_from(value).ok());
                    start.zip(end)
                },
                captured: event.captured.unwrap_or(false),
                window_id: None,
            }))
        }

        "command_error" => {
            let value = event.value.as_ref().unwrap_or(&Value::Null);
            let reason = required_json_str(value, "reason", family)?;
            Some(Event::CommandError(CommandError {
                reason,
                id: json_str_opt(value, "id"),
                family: json_str_opt(value, "family"),
                widget_type: json_str_opt(value, "widget_type"),
                message: json_str_opt(value, "message"),
            }))
        }

        // Fall through: treat as a system event with the tag.
        _ => Some(Event::System(SystemEvent {
            event_type: SystemEventType::SystemInfo,
            tag: Some(tag.to_string()),
            value: event.value.clone(),
            id: None,
            window_id: None,
        })),
    }
}

/// Convert an EffectResponse to an SDK EffectEvent.
///
/// Fallback path when a response has no matching tracker entry
/// (e.g. stale response after a renderer restart). Without the
/// tracker's kind context, "ok" results land as
/// [`EffectResult::Orphaned`] so apps can distinguish them from a
/// legitimate typed `Other` result delivered through the tracker.
pub(crate) fn effect_response_to_sdk(response: EffectResponse) -> Event {
    // Without tracker context (no kind available), we fall back to
    // untyped variants.
    let result = match response.status {
        "ok" => EffectResult::Orphaned(response.result.unwrap_or(Value::Null)),
        "cancelled" => EffectResult::Cancelled,
        "unsupported" => EffectResult::Unsupported,
        _ => {
            let msg = response
                .error
                .or_else(|| {
                    response
                        .result
                        .as_ref()
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| "unknown error".to_string());
            EffectResult::Error(msg)
        }
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
fn window_event(event_type: WindowEventType, event: &OutgoingEvent) -> Option<Event> {
    let value = event.value.as_ref().unwrap_or(&Value::Null);
    let window_id = required_json_str(value, "window_id", event.family.as_str())?;

    Some(Event::Window(WindowEvent {
        event_type,
        window_id,
        x: value["x"].as_f64().map(|v| v as f32),
        y: value["y"].as_f64().map(|v| v as f32),
        width: value["width"].as_f64().map(|v| v as f32),
        height: value["height"].as_f64().map(|v| v as f32),
        path: json_str_opt(value, "path"),
        scale_factor: value["scale_factor"].as_f64().map(|v| v as f32),
    }))
}

/// Extract KeyModifiers from an OutgoingEvent.
fn extract_modifiers(event: &OutgoingEvent) -> KeyModifiers {
    event.modifiers.unwrap_or_default()
}

fn required_json_str(value: &Value, key: &str, family: &str) -> Option<String> {
    match value.get(key) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(other) => {
            log::warn!(
                "dropping renderer event `{family}`: required field `{key}` must be a string, got {}",
                json_type_name(other)
            );
            None
        }
        None => {
            log::warn!("dropping renderer event `{family}`: required field `{key}` is missing");
            None
        }
    }
}

fn json_str_opt(value: &Value, key: &str) -> Option<String> {
    value[key].as_str().map(|s| s.to_string())
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn normalize_animation_frame_value(value: Option<Value>) -> Option<Value> {
    match value {
        Some(Value::Number(timestamp)) => Some(serde_json::json!({ "timestamp": timestamp })),
        other => other,
    }
}

fn bare_system_event_type(family: &str) -> Option<SystemEventType> {
    match family {
        "session_error" => Some(SystemEventType::SessionError),
        "session_closed" => Some(SystemEventType::SessionClosed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(family: &str, id: &str) -> OutgoingEvent {
        OutgoingEvent::generic(family, id, None)
    }

    fn make_tagged(family: &str, tag: &str) -> OutgoingEvent {
        OutgoingEvent::tagged(family, tag.to_string())
    }

    #[test]
    fn click_event() {
        let event = make_event("click", "save");
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::Widget(w) => {
                assert_eq!(w.event_type, EventType::Click);
                assert_eq!(w.scoped_id.id, "save");
                assert!(w.scoped_id.scope.is_empty());
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
                assert_eq!(w.scoped_id.id, "save");
                assert_eq!(w.scoped_id.scope, vec!["section", "form"]);
            }
            _ => panic!("expected Widget event"),
        }
    }

    #[test]
    fn input_event_uses_value() {
        let mut event = make_event("input", "name");
        event.value = Some(Value::String("typed text".to_string()));
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
    fn status_event_is_internal() {
        let mut event = make_event("status", "name");
        event.value = Some(Value::String("focused".to_string()));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn pointer_event_rejects_unknown_button() {
        let mut event = make_event("press", "canvas");
        event.value = Some(serde_json::json!({
            "x": 1.0,
            "y": 2.0,
            "button": "primary",
            "pointer": "mouse"
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn pointer_event_rejects_unknown_pointer_kind() {
        let mut event = make_event("move", "canvas");
        event.value = Some(serde_json::json!({
            "x": 1.0,
            "y": 2.0,
            "pointer": "trackpad"
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
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
        event.value = Some(serde_json::json!({
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
                assert_eq!(k.key, plushie_core::Key::Char('a'));
                assert_eq!(k.modified_key, Some(plushie_core::Key::Char('A')));
                assert_eq!(k.physical_key, Some(plushie_core::Key::from("KeyA")));
                assert_eq!(k.text, Some("A".to_string()));
                assert!(k.modifiers.shift);
                assert!(!k.modifiers.ctrl);
            }
            _ => panic!("expected Key event"),
        }
    }

    #[test]
    fn key_press_missing_key_is_dropped() {
        let mut event = make_tagged("key_press", "key_events");
        event.value = Some(serde_json::json!({
            "repeat": false,
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn key_press_non_string_key_is_dropped() {
        let mut event = make_tagged("key_press", "key_events");
        event.value = Some(serde_json::json!({
            "key": 42,
            "repeat": false,
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn modifiers_changed_event() {
        let mut event = make_tagged("modifiers_changed", "mods");
        event.modifiers = Some(plushie_core::protocol::KeyModifiers {
            shift: false,
            ctrl: true,
            alt: false,
            logo: false,
            command: false,
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
        event.value = Some(serde_json::json!({
            "window_id": "main",
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
    fn window_event_missing_window_id_is_dropped() {
        let mut event = make_tagged("window_resized", "win_events");
        event.value = Some(serde_json::json!({
            "width": 800.0,
            "height": 600.0,
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn window_event_non_string_window_id_is_dropped() {
        let mut event = make_tagged("window_resized", "win_events");
        event.value = Some(serde_json::json!({
            "window_id": 7,
            "width": 800.0,
            "height": 600.0,
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn animation_frame_event() {
        let mut event = make_tagged("animation_frame", "anim");
        event.value = Some(serde_json::json!({"timestamp": 16_000}));
        let sdk = outgoing_to_sdk_event(event).unwrap();
        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::AnimationFrame);
                assert_eq!(s.tag, Some("anim".to_string()));
                assert_eq!(s.value, Some(serde_json::json!({"timestamp": 16_000})));
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn animation_frame_event_normalizes_legacy_numeric_payload() {
        let mut event = make_tagged("animation_frame", "anim");
        event.value = Some(serde_json::json!(16_000));

        let sdk = outgoing_to_sdk_event(event).unwrap();

        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::AnimationFrame);
                assert_eq!(s.value, Some(serde_json::json!({"timestamp": 16_000})));
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn session_error_decodes_as_system_event() {
        let event = OutgoingEvent::generic(
            "session_error",
            "",
            Some(serde_json::json!({ "reason": "panic" })),
        );

        let sdk = outgoing_to_sdk_event(event).unwrap();

        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::SessionError);
                assert_eq!(s.tag, None);
                assert_eq!(s.value, Some(serde_json::json!({ "reason": "panic" })));
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn session_closed_decodes_as_system_event() {
        let event = OutgoingEvent::generic(
            "session_closed",
            "",
            Some(serde_json::json!({ "reason": "panic" })),
        );

        let sdk = outgoing_to_sdk_event(event).unwrap();

        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::SessionClosed);
                assert_eq!(s.tag, None);
                assert_eq!(s.value, Some(serde_json::json!({ "reason": "panic" })));
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn effect_response_ok_without_kind() {
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
                // Without tracker context, ok results use Orphaned.
                match e.result {
                    EffectResult::Orphaned(v) => assert_eq!(v["path"], "/tmp/file.txt"),
                    _ => panic!("expected Orphaned result, got {:?}", e.result),
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
    fn effect_response_unsupported() {
        let response = EffectResponse {
            message_type: "effect_response",
            session: String::new(),
            id: "dialog".to_string(),
            status: "unsupported",
            result: None,
            error: None,
        };
        let sdk = effect_response_to_sdk(response);
        match sdk {
            Event::Effect(e) => {
                assert_eq!(e.tag, "dialog");
                assert!(matches!(e.result, EffectResult::Unsupported));
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
                match &e.result {
                    EffectResult::Error(msg) => assert_eq!(msg, "permission denied"),
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
        let sdk = query_response_to_sdk(
            "find_focused",
            "f1",
            serde_json::json!({"focused": "input1"}),
        );
        match sdk {
            Event::System(s) => {
                assert_eq!(s.event_type, SystemEventType::FindFocused);
            }
            _ => panic!("expected System event"),
        }
    }

    #[test]
    fn scoped_id_parse_simple() {
        let sid = plushie_core::ScopedId::parse("save");
        assert_eq!(sid.id, "save");
        assert!(sid.scope.is_empty());
        assert_eq!(sid.window_id, None);
    }

    #[test]
    fn scoped_id_parse_nested() {
        let sid = plushie_core::ScopedId::parse("form/section/field");
        assert_eq!(sid.id, "field");
        assert_eq!(sid.scope, vec!["section", "form"]);
        assert_eq!(sid.window_id, None);
    }

    #[test]
    fn scoped_id_parse_with_window() {
        let sid = plushie_core::ScopedId::parse("main#form/email");
        assert_eq!(sid.id, "email");
        assert_eq!(sid.scope, vec!["form"]);
        assert_eq!(sid.window_id, Some("main".to_string()));
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

    #[test]
    fn ime_preedit_cursor_survives_wire_round_trip() {
        // Construct via the public serializer, route the value field through
        // a JSON Value (the same shape the wire layer hands the parser),
        // then convert. Catches drift between the emitted wire shape and
        // the parser in this module.
        let event =
            OutgoingEvent::ime_preedit("editor".to_string(), "konnichiwa".to_string(), Some(2..5));
        let value_json = serde_json::to_value(event.value.as_ref().unwrap()).unwrap();

        let mut rebuilt = make_tagged("ime_preedit", "editor");
        rebuilt.value = Some(value_json);
        let sdk = outgoing_to_sdk_event(rebuilt).expect("ime event");
        match sdk {
            Event::Ime(ime) => {
                assert_eq!(ime.event_type, ImeEventType::Preedit);
                assert_eq!(ime.text.as_deref(), Some("konnichiwa"));
                assert_eq!(ime.cursor, Some((2, 5)));
            }
            other => panic!("expected ime event, got {other:?}"),
        }
    }

    #[test]
    fn command_error_missing_reason_is_dropped() {
        let mut event = make_tagged("command_error", "commands");
        event.value = Some(serde_json::json!({
            "id": "save",
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn command_error_non_string_reason_is_dropped() {
        let mut event = make_tagged("command_error", "commands");
        event.value = Some(serde_json::json!({
            "reason": false,
        }));

        assert!(outgoing_to_sdk_event(event).is_none());
    }

    #[test]
    fn window_opened_carries_top_level_xy() {
        let event = OutgoingEvent::window_opened(
            "windows".to_string(),
            "main".to_string(),
            Some((100.0, 200.0)),
            800.0,
            600.0,
            1.5,
        );
        let value_json = serde_json::to_value(event.value.as_ref().unwrap()).unwrap();

        let mut rebuilt = make_tagged("window_opened", "windows");
        rebuilt.value = Some(value_json);
        let sdk = outgoing_to_sdk_event(rebuilt).expect("window event");
        match sdk {
            Event::Window(w) => {
                assert_eq!(w.event_type, WindowEventType::Opened);
                assert_eq!(w.window_id, "main");
                assert_eq!(w.x, Some(100.0));
                assert_eq!(w.y, Some(200.0));
                assert_eq!(w.width, Some(800.0));
                assert_eq!(w.height, Some(600.0));
                assert_eq!(w.scale_factor, Some(1.5));
            }
            other => panic!("expected window event, got {other:?}"),
        }
    }

    #[test]
    fn window_opened_without_position_omits_xy() {
        let event = OutgoingEvent::window_opened(
            "windows".to_string(),
            "main".to_string(),
            None,
            800.0,
            600.0,
            1.0,
        );
        let value_json = serde_json::to_value(event.value.as_ref().unwrap()).unwrap();

        let mut rebuilt = make_tagged("window_opened", "windows");
        rebuilt.value = Some(value_json);
        let sdk = outgoing_to_sdk_event(rebuilt).expect("window event");
        match sdk {
            Event::Window(w) => {
                assert_eq!(w.x, None);
                assert_eq!(w.y, None);
            }
            other => panic!("expected window event, got {other:?}"),
        }
    }

    #[test]
    fn ime_preedit_without_cursor_is_none() {
        let event = OutgoingEvent::ime_preedit("editor".to_string(), "hello".to_string(), None);
        let value_json = serde_json::to_value(event.value.as_ref().unwrap()).unwrap();

        let mut rebuilt = make_tagged("ime_preedit", "editor");
        rebuilt.value = Some(value_json);
        let sdk = outgoing_to_sdk_event(rebuilt).expect("ime event");
        match sdk {
            Event::Ime(ime) => {
                assert_eq!(ime.cursor, None);
            }
            other => panic!("expected ime event, got {other:?}"),
        }
    }

    #[test]
    #[cfg(target_pointer_width = "32")]
    fn ime_preedit_oversized_cursor_is_ignored() {
        let mut rebuilt = make_tagged("ime_preedit", "editor");
        rebuilt.value = Some(serde_json::json!({
            "id": "editor",
            "text": "hello",
            "cursor": {
                "start": u64::MAX,
                "end": 1,
            },
        }));
        let sdk = outgoing_to_sdk_event(rebuilt).expect("ime event");
        match sdk {
            Event::Ime(ime) => assert_eq!(ime.cursor, None),
            other => panic!("expected ime event, got {other:?}"),
        }
    }
}
