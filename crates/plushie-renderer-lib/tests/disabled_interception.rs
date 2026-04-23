//! Functional disabled interception for input-family widgets.
//!
//! iced's native `Status::Disabled` is style-only: events from
//! widgets the user considers "disabled" still reach `update()`. The
//! renderer's dispatcher swallows those events so `disabled: true`
//! blocks interaction as every host SDK documents.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use parking_lot::Mutex as PlMutex;
use serde_json::json;

use plushie_core::ops::EffectRequest;
use plushie_widget_sdk::protocol::{
    EffectResponse, IncomingMessage, OutgoingEvent, PropMap, Props, TreeNode,
};
use plushie_widget_sdk::registry::WidgetRegistry;
use plushie_widget_sdk::runtime::Message;

use plushie_renderer_lib::App;
use plushie_renderer_lib::effects::EffectHandler;
use plushie_renderer_lib::emitters::{EventSink, SinkMutex};

// ---------------------------------------------------------------------------
// Recording sink
// ---------------------------------------------------------------------------

struct RecordingSink {
    events: Arc<Mutex<Vec<OutgoingEvent>>>,
}

impl EventSink for RecordingSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
    fn emit_effect_response(&mut self, _: EffectResponse) -> io::Result<()> {
        Ok(())
    }
    fn emit_query_response(&mut self, _: &str, _: &str, _: &serde_json::Value) -> io::Result<()> {
        Ok(())
    }
    fn emit_screenshot_response(
        &mut self,
        _: &str,
        _: &str,
        _: &str,
        _: u32,
        _: u32,
        _: &[u8],
    ) -> io::Result<()> {
        Ok(())
    }
    fn emit_hello(&mut self, _: &str, _: &str, _: &[&str], _: &[&str], _: &str) -> io::Result<()> {
        Ok(())
    }
    fn emit_diagnostic(
        &mut self,
        _: plushie_widget_sdk::protocol::DiagnosticMessage,
    ) -> io::Result<()> {
        Ok(())
    }
    fn write_raw(&mut self, _: &[u8]) -> io::Result<()> {
        Ok(())
    }
}

struct NullEffectHandler;
impl EffectHandler for NullEffectHandler {
    fn handle_sync(&self, _id: &str, _request: &EffectRequest) -> Option<EffectResponse> {
        None
    }
    fn handle_async(
        &self,
        id: String,
        _request: EffectRequest,
    ) -> Pin<Box<dyn std::future::Future<Output = EffectResponse> + Send>> {
        Box::pin(async move { EffectResponse::ok(id, serde_json::Value::Null) })
    }
    fn is_async(&self, _request: &EffectRequest) -> bool {
        false
    }
}

fn build_app() -> (App, Arc<Mutex<Vec<OutgoingEvent>>>) {
    let events: Arc<Mutex<Vec<OutgoingEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink {
        events: events.clone(),
    };
    let sink_arc: Arc<SinkMutex> = Arc::new(PlMutex::new(Box::new(sink) as Box<dyn EventSink>));
    let registry: WidgetRegistry<iced::Renderer> = WidgetRegistry::new();
    let app = App::new(registry, Box::new(NullEffectHandler), sink_arc);
    (app, events)
}

/// Install a tree that contains a single widget of `type_name` at
/// `id`, with whatever props the caller supplies (typically `disabled`
/// and the `window#` scope).
fn seed_tree(app: &mut App, node_id: &str, type_name: &str, disabled: bool) {
    let mut props = PropMap::new();
    props.insert("disabled", disabled);
    let node = TreeNode {
        id: node_id.to_string(),
        type_name: type_name.to_string(),
        props: Props::from(props),
        children: vec![],
    };
    let root = TreeNode {
        id: "main".to_string(),
        type_name: "window".to_string(),
        props: Props::from(PropMap::new()),
        children: vec![node],
    };
    // Snapshot drives the internal tree; Core validates IDs on
    // insertion, so the tree is live before we dispatch.
    let _ = app.core.apply(IncomingMessage::Snapshot { tree: root });
}

fn event_msg(id: &str, family: &str, value: serde_json::Value) -> Message {
    Message::Event {
        window_id: "main".to_string(),
        id: id.to_string(),
        value,
        family: family.to_string(),
    }
}

#[test]
fn disabled_text_input_swallows_input_event() {
    let (mut app, events) = build_app();
    seed_tree(&mut app, "email", "text_input", true);

    let _ = app.update(event_msg("email", "input", json!("foo")));

    let captured = events.lock().unwrap();
    assert!(
        captured.is_empty(),
        "disabled text_input must not emit events, got {captured:?}"
    );
}

#[test]
fn enabled_text_input_emits_event() {
    let (mut app, events) = build_app();
    seed_tree(&mut app, "email", "text_input", false);

    let _ = app.update(event_msg("email", "input", json!("foo")));

    let captured = events.lock().unwrap();
    assert_eq!(
        captured.len(),
        1,
        "enabled text_input should emit its event, got {captured:?}"
    );
    assert_eq!(captured[0].family, "input");
}

#[test]
fn disabled_text_editor_swallows_event() {
    let (mut app, events) = build_app();
    seed_tree(&mut app, "notes", "text_editor", true);

    let _ = app.update(event_msg("notes", "input", json!("x")));

    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn disabled_combo_box_swallows_event() {
    let (mut app, events) = build_app();
    seed_tree(&mut app, "lang", "combo_box", true);

    let _ = app.update(event_msg("lang", "select", json!("Rust")));

    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn disabled_pick_list_swallows_event() {
    let (mut app, events) = build_app();
    seed_tree(&mut app, "color", "pick_list", true);

    let _ = app.update(event_msg("color", "select", json!("red")));

    assert!(events.lock().unwrap().is_empty());
}

#[test]
fn disabled_button_events_pass_through() {
    // Button is not in the text_input family; the interception
    // scope is deliberate and narrow. Buttons disable via iced's
    // `disabled()` in their own render path and don't need the
    // dispatch-layer swallow.
    let (mut app, events) = build_app();
    seed_tree(&mut app, "save", "button", true);

    let _ = app.update(event_msg("save", "click", json!(null)));

    assert_eq!(events.lock().unwrap().len(), 1);
}

#[test]
fn unknown_widget_id_does_not_swallow() {
    let (mut app, events) = build_app();
    // Tree contains no widget with id "ghost"; the dispatcher must
    // still emit the event so we don't silently drop stale wire
    // events (e.g. from an already-removed widget).
    seed_tree(&mut app, "email", "text_input", true);

    let _ = app.update(event_msg("ghost", "input", json!("x")));

    let captured = events.lock().unwrap();
    assert_eq!(captured.len(), 1);
}
