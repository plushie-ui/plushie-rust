//! Integration test for widget-scoped subscription wiring.
//!
//! Exercises the full loop: a widget declares a subscription via
//! `PlushieWidget::subscriptions`, the registry collects it during
//! `prepare_walk`, the renderer's `update()` dispatches matching
//! iced messages to the widget's `handle_message`, and the resulting
//! outgoing events land on the sink. Also checks the lifecycle:
//! once the widget leaves the tree its subscription must stop firing.

use std::io;
use std::pin::Pin;
use std::sync::Mutex;
use std::sync::{Arc, atomic::AtomicUsize, atomic::Ordering};

use iced::widget::text;
use iced::{Element, Theme};
use parking_lot::Mutex as PlMutex;

use plushie_core::ops::EffectRequest;
use plushie_widget_sdk::protocol::{EffectResponse, OutgoingEvent, PropMap, Props, TreeNode};
use plushie_widget_sdk::registry::{
    HandleResult, PlushieWidget, SubscribeCtx, WidgetRegistry, WidgetSubscription,
};
use plushie_widget_sdk::render_ctx::RenderCtx;
use plushie_widget_sdk::runtime::Message;
use plushie_widget_sdk::shared_state::SharedState;

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

// ---------------------------------------------------------------------------
// No-op effect handler
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Test widget: subscribes to animation frames, counts handle_message
// calls, and emits a "tick" OutgoingEvent for each one.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct TickWidget {
    handled: Arc<AtomicUsize>,
}

impl PlushieWidget<iced::Renderer> for TickWidget {
    fn type_names(&self) -> &[&str] {
        &["tick_widget"]
    }

    fn render<'a>(
        &'a self,
        _node: &'a TreeNode,
        _ctx: &RenderCtx<'a, iced::Renderer>,
    ) -> Element<'a, Message, Theme, iced::Renderer> {
        text("tick").into()
    }

    fn subscriptions(&self, node: &TreeNode, _ctx: &SubscribeCtx<'_>) -> Vec<WidgetSubscription> {
        vec![WidgetSubscription::new("on_animation_frame", &node.id)]
    }

    fn handle_message(&mut self, _msg: &Message) -> HandleResult {
        self.handled.fetch_add(1, Ordering::SeqCst);
        HandleResult::emit(vec![OutgoingEvent::generic(
            "tick".to_string(),
            "tick_widget".to_string(),
            None,
        )])
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<iced::Renderer>> {
        Box::new(self.clone())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_app(widget_handled: Arc<AtomicUsize>) -> (App, Arc<Mutex<Vec<OutgoingEvent>>>) {
    let events: Arc<Mutex<Vec<OutgoingEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink {
        events: events.clone(),
    };
    let sink_arc: Arc<SinkMutex> = Arc::new(PlMutex::new(Box::new(sink) as Box<dyn EventSink>));

    let mut registry: WidgetRegistry<iced::Renderer> = WidgetRegistry::new();
    registry.register(Box::new(TickWidget {
        handled: widget_handled,
    }));

    let app = App::new(registry, Box::new(NullEffectHandler), sink_arc);
    (app, events)
}

fn tick_node(id: &str) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: "tick_widget".to_string(),
        props: Props::from(PropMap::new()),
        children: vec![],
    }
}

fn root_with(children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: "root".to_string(),
        type_name: "column".to_string(),
        props: Props::from(PropMap::new()),
        children,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn widget_subscription_active_while_node_is_in_tree() {
    let counter = Arc::new(AtomicUsize::new(0));
    let (mut app, _events) = build_app(counter.clone());

    // Put the widget in the tree and run prepare_walk.
    let mut tree = root_with(vec![tick_node("t1")]);
    let mut shared = SharedState::new();
    app.registry
        .prepare_walk(&mut tree, &mut shared, &Theme::Dark);

    let subs = app.registry.active_widget_subscriptions();
    assert_eq!(
        subs.len(),
        1,
        "one widget in the tree -> one collected subscription"
    );
    assert!(app.registry.has_widget_subscription("on_animation_frame"));
}

#[test]
fn animation_frame_messages_route_back_to_widget_handle_message() {
    let counter = Arc::new(AtomicUsize::new(0));
    let (mut app, events) = build_app(counter.clone());

    let mut tree = root_with(vec![tick_node("t1")]);
    let mut shared = SharedState::new();
    app.registry
        .prepare_walk(&mut tree, &mut shared, &Theme::Dark);

    // Drive three animation frames through the App.
    for _ in 0..3 {
        let _task = app.update(Message::AnimationFrame(iced::time::Instant::now()));
    }

    assert_eq!(
        counter.load(Ordering::SeqCst),
        3,
        "widget handle_message should have fired once per frame",
    );
    // Each handled call emits one tick event.
    let captured = events.lock().unwrap();
    let tick_count = captured.iter().filter(|e| e.family == "tick").count();
    assert_eq!(
        tick_count,
        3,
        "widget-emitted events should reach the sink, got {:?}",
        captured.iter().map(|e| &e.family).collect::<Vec<_>>(),
    );
}

#[test]
fn subscription_stops_when_widget_leaves_tree() {
    let counter = Arc::new(AtomicUsize::new(0));
    let (mut app, _events) = build_app(counter.clone());

    // First pass: widget is present.
    let mut tree = root_with(vec![tick_node("t1")]);
    let mut shared = SharedState::new();
    app.registry
        .prepare_walk(&mut tree, &mut shared, &Theme::Dark);
    let _ = app.update(Message::AnimationFrame(iced::time::Instant::now()));
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Second pass: widget removed. Re-run prepare_walk so the
    // collected subscription is pruned.
    let mut empty = root_with(vec![]);
    app.registry
        .prepare_walk(&mut empty, &mut shared, &Theme::Dark);
    assert!(
        app.registry.active_widget_subscriptions().is_empty(),
        "removing the node must drop its widget subscription"
    );
    assert!(
        !app.registry.has_widget_subscription("on_animation_frame"),
        "no widget subs of this kind should remain"
    );

    // Additional frames must not reach handle_message.
    let before = counter.load(Ordering::SeqCst);
    for _ in 0..3 {
        let _ = app.update(Message::AnimationFrame(iced::time::Instant::now()));
    }
    assert_eq!(
        counter.load(Ordering::SeqCst),
        before,
        "no further dispatches after the node leaves the tree",
    );
}
