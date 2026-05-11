use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use iced::{Point, Size, window};
use parking_lot::Mutex as PlMutex;
use serde_json::{Value, json};

use plushie_core::ops::{EffectRequest, RendererOp, WindowQuery};
use plushie_renderer_lib::App;
use plushie_renderer_lib::constants::{SUB_ANIMATION_FRAME, SUB_EVENT, SUB_THEME_CHANGE};
use plushie_renderer_lib::effects::EffectHandler;
use plushie_renderer_lib::emitters::{EventSink, SinkMutex};
use plushie_widget_sdk::protocol::{
    DiagnosticMessage, EffectResponse, IncomingMessage, OutgoingEvent, PropMap, Props, TreeNode,
};
use plushie_widget_sdk::registry::WidgetRegistry;
use plushie_widget_sdk::runtime::Message;

#[derive(Debug, Clone)]
struct QueryRecord {
    kind: String,
    tag: String,
    data: Value,
}

#[derive(Default)]
struct Recorded {
    events: Vec<OutgoingEvent>,
    effects: Vec<EffectResponse>,
    queries: Vec<QueryRecord>,
}

struct RecordingSink {
    recorded: Arc<Mutex<Recorded>>,
}

impl EventSink for RecordingSink {
    fn emit_event(&mut self, event: OutgoingEvent) -> io::Result<()> {
        self.recorded.lock().unwrap().events.push(event);
        Ok(())
    }

    fn emit_effect_response(&mut self, response: EffectResponse) -> io::Result<()> {
        self.recorded.lock().unwrap().effects.push(response);
        Ok(())
    }

    fn emit_query_response(&mut self, kind: &str, tag: &str, data: &Value) -> io::Result<()> {
        self.recorded.lock().unwrap().queries.push(QueryRecord {
            kind: kind.to_string(),
            tag: tag.to_string(),
            data: data.clone(),
        });
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

    fn emit_diagnostic(&mut self, _: DiagnosticMessage) -> io::Result<()> {
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
        Box::pin(async move { EffectResponse::ok(id, Value::Null) })
    }

    fn is_async(&self, _request: &EffectRequest) -> bool {
        false
    }
}

fn build_app() -> (App, Arc<Mutex<Recorded>>) {
    let recorded = Arc::new(Mutex::new(Recorded::default()));
    let sink = RecordingSink {
        recorded: recorded.clone(),
    };
    let sink_arc: Arc<SinkMutex> = Arc::new(PlMutex::new(Box::new(sink) as Box<dyn EventSink>));
    let app = App::new(WidgetRegistry::new(), Box::new(NullEffectHandler), sink_arc);
    (app, recorded)
}

fn subscribe(app: &mut App, kind: &str, tag: &str) {
    app.core.apply(IncomingMessage::Subscribe {
        kind: kind.to_string(),
        tag: tag.to_string(),
        window_id: None,
        max_rate: None,
    });
}

fn empty_root() -> TreeNode {
    TreeNode {
        id: "root".to_string(),
        type_name: "column".to_string(),
        props: Props::from(PropMap::new()),
        children: vec![],
    }
}

#[test]
fn direct_tree_hash_uses_tag_for_query_response() {
    let (mut app, recorded) = build_app();
    app.core
        .apply(IncomingMessage::Snapshot { tree: empty_root() });

    let _ = app.execute(RendererOp::TreeHash {
        tag: "hash_tag".to_string(),
    });

    let recorded = recorded.lock().unwrap();
    assert_eq!(recorded.queries.len(), 1);
    assert_eq!(recorded.queries[0].kind, "tree_hash");
    assert_eq!(recorded.queries[0].tag, "hash_tag");
    assert!(recorded.queries[0].data.get("hash").is_some());
}

#[test]
fn unknown_window_query_completes_as_query_response() {
    let (mut app, recorded) = build_app();

    let _ = app.dispatch_window_query(WindowQuery::GetSize {
        window_id: "missing".to_string(),
        tag: "size_tag".to_string(),
    });

    let recorded = recorded.lock().unwrap();
    assert!(recorded.effects.is_empty());
    assert_eq!(recorded.queries.len(), 1);
    assert_eq!(recorded.queries[0].kind, "get_size");
    assert_eq!(recorded.queries[0].tag, "size_tag");
    assert_eq!(recorded.queries[0].data["error"], "unknown_window");
    assert_eq!(recorded.queries[0].data["window_id"], "missing");
}

#[test]
fn animation_and_theme_events_fan_out_to_all_matching_entries() {
    let (mut app, recorded) = build_app();
    subscribe(&mut app, SUB_ANIMATION_FRAME, "anim_a");
    subscribe(&mut app, SUB_ANIMATION_FRAME, "anim_b");
    subscribe(&mut app, SUB_THEME_CHANGE, "theme_a");
    subscribe(&mut app, SUB_THEME_CHANGE, "theme_b");

    let _ = app.update(Message::AnimationFrame(iced::time::Instant::now()));
    let _ = app.update(Message::ThemeChanged(iced::theme::Mode::Dark));

    let recorded = recorded.lock().unwrap();
    let animation_tags: Vec<_> = recorded
        .events
        .iter()
        .filter(|event| event.family == "animation_frame")
        .filter_map(|event| event.tag.as_deref())
        .collect();
    assert_eq!(animation_tags, vec!["anim_a", "anim_b"]);

    let theme_tags: Vec<_> = recorded
        .events
        .iter()
        .filter(|event| event.family == "theme_changed")
        .filter_map(|event| event.tag.as_deref())
        .collect();
    assert_eq!(theme_tags, vec!["theme_a", "theme_b"]);
}

#[test]
fn on_event_receives_window_animation_and_theme_events() {
    let (mut app, recorded) = build_app();
    subscribe(&mut app, SUB_EVENT, "all_events");
    let iced_id = window::Id::unique();
    app.windows.insert("main".to_string(), iced_id);

    let _ = app.update(Message::WindowEvent(
        iced_id,
        window::Event::Moved(Point::new(10.0, 20.0)),
    ));
    let _ = app.update(Message::AnimationFrame(iced::time::Instant::now()));
    let _ = app.update(Message::ThemeChanged(iced::theme::Mode::Light));

    let recorded = recorded.lock().unwrap();
    let families: Vec<_> = recorded
        .events
        .iter()
        .filter(|event| event.tag.as_deref() == Some("all_events"))
        .map(|event| event.family.as_str())
        .collect();
    assert!(families.contains(&"window_moved"));
    assert!(families.contains(&"animation_frame"));
    assert!(families.contains(&"theme_changed"));
}

#[test]
fn oversized_widget_event_rate_is_ignored_instead_of_wrapping() {
    let (mut app, _recorded) = build_app();
    let node = TreeNode {
        id: "fast".to_string(),
        type_name: "button".to_string(),
        props: Props::from_json(json!({"event_rate": u64::MAX})),
        children: vec![],
    };
    let root = TreeNode {
        id: "root".to_string(),
        type_name: "column".to_string(),
        props: Props::from(PropMap::new()),
        children: vec![node],
    };
    app.core.apply(IncomingMessage::Snapshot { tree: root });

    assert_eq!(app.lookup_widget_event_rate("fast"), None);
}

#[test]
fn on_event_window_subscription_respects_window_scope() {
    let (mut app, recorded) = build_app();
    app.core.apply(IncomingMessage::Subscribe {
        kind: SUB_EVENT.to_string(),
        tag: "main_only".to_string(),
        window_id: Some("main".to_string()),
        max_rate: None,
    });
    let main_id = window::Id::unique();
    let popup_id = window::Id::unique();
    app.windows.insert("main".to_string(), main_id);
    app.windows.insert("popup".to_string(), popup_id);

    let _ = app.update(Message::WindowEvent(
        popup_id,
        window::Event::Resized(Size::new(800.0, 600.0)),
    ));

    assert!(recorded.lock().unwrap().events.is_empty());
}
