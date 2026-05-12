//! Compile-check for the rustdoc patterns in the plushie SDK crate.
//!
//! Mirrors `plushie-widget-sdk/tests/doc_examples.rs`. The tests here
//! build the same snippets shown in the docs for `test.rs`,
//! `command.rs`, `event.rs`, and `widget.rs`. They never call `run`
//! and don't start any runtime; the point is to fail the build if
//! the public surface drifts away from the documented shape.
//!
//! Because rustdoc snippets in those modules are tagged `ignore`
//! (they reference user-defined `App` impls and can't run standalone),
//! rebuilding the patterns here is the cheapest way to keep the docs
//! honest.

#![allow(dead_code)]

use plushie::prelude::*;
use plushie::test::TestSession;
use plushie::widget::{EventResult, Widget, WidgetView};
use plushie_core::ScopedId;
use plushie_core::protocol::{PropMap, TreeNode};
use plushie_core::types::FromNode;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Counter app: mirrors the `lib.rs` quick-start snippet.
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Counter {
    count: i32,
}

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Counter { count: 0 }, Command::none())
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        match event.widget_match() {
            Some(Click("inc")) => next.count += 1,
            Some(Click("dec")) => next.count -= 1,
            _ => {}
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Counter")
            .child(
                column()
                    .spacing(8.0)
                    .padding(16)
                    .child(text(&format!("Count: {}", model.count)))
                    .child(
                        row()
                            .spacing(8.0)
                            .child(button("inc", "+"))
                            .child(button("dec", "-")),
                    ),
            )
            .into()
    }
}

#[test]
fn counter_init_and_update_compile() {
    let (model, _cmd) = Counter::init();
    let click = Event::Widget(plushie::event::WidgetEvent {
        event_type: plushie::event::EventType::Click,
        scoped_id: ScopedId::new("inc", vec![], Some("main".to_string())),
        value: Value::Null,
    });
    let (model, _cmd) = Counter::update(&model, click);
    assert_eq!(model.count, 1);
}

// ---------------------------------------------------------------------------
// TestSession: the module-level doc example.
// ---------------------------------------------------------------------------

#[test]
fn test_session_interactions_compile() {
    // Counter only has button IDs; canvas interactions from the docs
    // are type-checked here by taking a function pointer rather than
    // calling them (no canvas widget to target in this fixture).
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    session.click(Selector::role("button"));
    session.press("Ctrl+s");
    session.press(Key::Enter);
    let _press: fn(&mut TestSession<Counter>, &str, f32, f32, &str) = |s, sel, x, y, btn| {
        s.canvas_press(sel, x, y, btn);
    };
    let _ = session.model();
}

// ---------------------------------------------------------------------------
// Command builders: mirror the `Command::task` / `Command::stream`
// snippets, plus the widget-command example.
// ---------------------------------------------------------------------------

#[test]
fn async_task_builder_compiles() {
    let _cmd = Command::task("fetch", || async {
        let value: Result<Value, Value> = Ok(json!("data"));
        value
    });
}

#[test]
fn stream_builder_compiles() {
    let _cmd = Command::stream("import", |emitter| async move {
        for chunk in ["alpha", "beta"] {
            emitter.emit(json!(chunk));
        }
        Ok(json!({"done": true}))
    });
}

#[test]
fn cancel_builder_compiles() {
    let _cmd = Command::cancel("fetch");
}

#[derive(plushie::WidgetCommand)]
enum GaugeCommand {
    SetValue(f32),
    Reset,
    SetRange { min: f32, max: f32 },
}

#[test]
fn widget_command_builder_compiles() {
    let _ = Command::widget("temp-gauge", GaugeCommand::SetValue(72.0));
    let _ = Command::widget("temp-gauge", GaugeCommand::Reset);
    let _ = Command::widget(
        "temp-gauge",
        GaugeCommand::SetRange {
            min: 0.0,
            max: 100.0,
        },
    );
}

// ---------------------------------------------------------------------------
// Widget trait: mirrors the composite-widget rustdoc example.
// ---------------------------------------------------------------------------

struct StarRating;

#[derive(plushie::WidgetEvent)]
enum StarRatingEvent {
    Select(u64),
}

#[derive(Default)]
struct StarState {
    hover: Option<usize>,
}

impl Widget for StarRating {
    type State = StarState;
    type Props = UntypedProps;

    fn view(id: &str, _props: &UntypedProps, _state: &StarState) -> View {
        let mut r = row().id(id).spacing(4.0);
        for i in 0..5 {
            r = r.child(button(&format!("star-{i}"), "*"));
        }
        r.into()
    }

    fn handle_event(event: &Event, _state: &mut StarState) -> EventResult {
        match event.widget_match() {
            Some(Click(id)) if id.starts_with("star-") => {
                EventResult::emit_event(StarRatingEvent::Select(1))
            }
            _ => EventResult::Consumed,
        }
    }
}

#[test]
fn widget_trait_compiles() {
    let state = StarState::default();
    let props = UntypedProps::from_node(&TreeNode {
        id: "w".to_string(),
        type_name: "__widget__".to_string(),
        props: plushie_core::protocol::Props::from(PropMap::new()),
        children: vec![],
    });
    let _view = StarRating::view("rating", &props, &state);
}

// ---------------------------------------------------------------------------
// Widget view registration path: mirrors the `WidgetView` snippets.
// ---------------------------------------------------------------------------

struct WrappedApp;

impl App for WrappedApp {
    type Model = ();

    fn init() -> ((), Command) {
        ((), Command::none())
    }

    fn update(_model: &(), _event: Event) -> ((), Command) {
        ((), Command::none())
    }

    fn view(_model: &(), widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .child(
                WidgetView::<StarRating>::new("rating")
                    .prop("rating", 3i64)
                    .register(widgets),
            )
            .into()
    }
}

#[test]
fn wrapped_widget_view_compiles() {
    let (_m, _cmd) = WrappedApp::init();
    let mut registrar = WidgetRegistrar::new();
    let _view = WrappedApp::view(&(), &mut registrar);
}

// ---------------------------------------------------------------------------
// Subscription: mirrors the Subscription::every / on_key_press patterns.
// ---------------------------------------------------------------------------

#[test]
fn subscription_builders_compile() {
    let _every = Subscription::every(std::time::Duration::from_secs(1), "tick");
    let _keys = Subscription::on_key_press();
    let _window_ev = Subscription::on_window_event();
}

// ---------------------------------------------------------------------------
// Assertion helpers: exercise TestSession assertion surface area so a
// signature change surfaces as a test compile error.
// ---------------------------------------------------------------------------

#[test]
fn test_session_assertions_compile() {
    let session = TestSession::<Counter>::start();
    session.assert_exists("inc");
    session.assert_not_exists("missing");
    session.assert_no_diagnostics();
    let _hash = session.tree_hash();
    let _snap = session.tree_snapshot();
}

#[test]
fn test_session_assertion_signatures_compile() {
    // Referencing the generic-over-Selector assertion helpers keeps
    // the documented signature honest without panicking on a missing
    // widget. The actual runtime behaviour is covered by other tests.
    fn _use_assert_text(s: &TestSession<Counter>, sel: &str, exp: &str) {
        let _ = |s: &TestSession<Counter>| s.assert_exists(sel);
        let _ = |_: ()| s.assert_text(sel, exp);
    }
}

// ---------------------------------------------------------------------------
// Prelude glob: ensure every documented re-export remains nameable from
// a single `use plushie::prelude::*;`.
// ---------------------------------------------------------------------------

#[test]
fn prelude_glob_imports_compile() {
    // Smoke-test a sampling of re-exports: one enum, one trait, a few
    // types, an event family, and the animation descriptors. If any of
    // these re-exports is removed from the prelude this test stops
    // compiling, which is the signal we want.
    let _align: Align = Align::Center;
    let _color: Color = Color::red();
    let _ease: Easing = Easing::EaseInOut;
    let _role: Role = Role::Button;
    fn _takes_app<A: App>() {}
    fn _takes_plushie_type<T: PlushieType>() {}
}
