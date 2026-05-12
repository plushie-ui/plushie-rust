//! Behavioral tests for the composite Widget trait and EventResult type.
//!
//! Defines a ToggleButton widget and exercises the public API
//! without needing a running app or renderer.

mod common;

use common::{click_event, input_event};
use plushie::WidgetEvent;
use plushie::prelude::*;
use plushie::widget::{EventResult, Widget};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Test widget: ToggleButton
// ---------------------------------------------------------------------------

struct ToggleButton;

#[derive(Default)]
struct ToggleState {
    pressed: bool,
}

impl Widget for ToggleButton {
    type State = ToggleState;
    type Props = UntypedProps;

    fn view(id: &str, props: &UntypedProps, state: &ToggleState) -> View {
        let label = props
            .0
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("Toggle");
        let style = if state.pressed {
            Style::primary()
        } else {
            Style::secondary()
        };
        button(id, label).style(style).into()
    }

    fn handle_event(event: &Event, state: &mut ToggleState) -> EventResult {
        match event.widget_match() {
            Some(Click(_)) => {
                state.pressed = !state.pressed;
                EventResult::emit("toggled", state.pressed)
            }
            _ => EventResult::Ignored,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build an UntypedProps from a serde_json::Value.
fn props(value: Value) -> UntypedProps {
    UntypedProps(value)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn widget_trait_can_be_implemented() {
    // The fact that ToggleButton compiles and satisfies Widget
    // is the assertion. Call view to exercise the vtable.
    let state = ToggleState::default();
    let _view = ToggleButton::view("t", &props(json!({})), &state);
}

#[test]
fn event_result_emit_carries_family_and_value() {
    let result = EventResult::emit("selected", 42);
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "selected");
            assert_eq!(value, json!(42));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn event_result_emit_accepts_namespaced_custom_family() {
    let result = EventResult::emit("star_rating:select", 42);
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "star_rating:select");
            assert_eq!(value, json!(42));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
#[should_panic(expected = "custom event family \"click\" collides with a built-in event family")]
fn event_result_emit_rejects_builtin_family() {
    let _ = EventResult::emit("click", 42);
}

#[test]
fn event_result_consumed_is_constructible() {
    let result = EventResult::Consumed;
    assert!(matches!(result, EventResult::Consumed));
}

#[test]
fn event_result_ignored_is_constructible() {
    let result = EventResult::Ignored;
    assert!(matches!(result, EventResult::Ignored));
}

#[test]
fn event_result_emit_convenience_constructor() {
    // emit() accepts anything that converts Into<Value>.
    let bool_emit = EventResult::emit("toggled", true);
    match bool_emit {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "toggled");
            assert_eq!(value, json!(true));
        }
        other => panic!("expected Emit, got {other:?}"),
    }

    let string_emit = EventResult::emit("changed", "hello");
    match string_emit {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "changed");
            assert_eq!(value, json!("hello"));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn widget_view_returns_valid_json() {
    let state = ToggleState { pressed: false };
    let p = props(json!({"label": "Press me"}));
    let view = ToggleButton::view("toggle_btn", &p, &state);

    assert_eq!(view.id(), "toggle_btn");
    assert_eq!(view.type_name(), "button");
    assert_eq!(view.props().get_str("label"), Some("Press me"));
    assert_eq!(view.props().get_str("style"), Some("secondary"));
}

#[test]
fn widget_handle_event_modifies_state() {
    let mut state = ToggleState::default();
    assert!(!state.pressed);

    let event = click_event("toggle_btn");
    let _result = ToggleButton::handle_event(&event, &mut state);
    assert!(state.pressed);

    // Click again to toggle back.
    let event = click_event("toggle_btn");
    let _result = ToggleButton::handle_event(&event, &mut state);
    assert!(!state.pressed);
}

#[test]
fn widget_handle_event_returns_emit() {
    let mut state = ToggleState::default();
    let event = click_event("toggle_btn");
    let result = ToggleButton::handle_event(&event, &mut state);

    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "toggled");
            // State was false, click flipped to true.
            assert_eq!(value, json!(true));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn widget_handle_event_ignores_non_click() {
    let mut state = ToggleState::default();
    let event = input_event("toggle_btn", "text");
    let result = ToggleButton::handle_event(&event, &mut state);

    assert!(matches!(result, EventResult::Ignored));
    assert!(!state.pressed, "state should not change on ignored event");
}

#[test]
fn widget_view_reflects_pressed_state() {
    let p = props(json!({"label": "Toggle"}));

    let unpressed = ToggleButton::view("t", &p, &ToggleState { pressed: false });
    assert_eq!(unpressed.props().get_str("style"), Some("secondary"));

    let pressed = ToggleButton::view("t", &p, &ToggleState { pressed: true });
    assert_eq!(pressed.props().get_str("style"), Some("primary"));
}

// ---------------------------------------------------------------------------
// EventResult::emit_event tests
// ---------------------------------------------------------------------------

#[derive(WidgetEvent)]
enum TestWidgetEvent {
    Selected(u64),
    Select(u64),
    Toggled(bool),
    Cleared,
}

#[test]
fn emit_event_typed_u64() {
    let result = EventResult::emit_event(TestWidgetEvent::Selected(42));
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "selected");
            assert_eq!(value, json!(42));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn emit_event_allows_typed_builtin_family_name() {
    let result = EventResult::emit_event(TestWidgetEvent::Select(42));
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "select");
            assert_eq!(value, json!(42));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn emit_event_typed_bool() {
    let result = EventResult::emit_event(TestWidgetEvent::Toggled(true));
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "toggled");
            assert_eq!(value, json!(true));
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

#[test]
fn emit_event_typed_unit() {
    let result = EventResult::emit_event(TestWidgetEvent::Cleared);
    match result {
        EventResult::Emit { family, value } => {
            assert_eq!(family, "cleared");
            assert!(value.is_null());
        }
        other => panic!("expected Emit, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Widget view cache
// ---------------------------------------------------------------------------
//
// Widgets that opt in via `Widget::cache_key` reuse their previously-
// expanded view tree when the cache key is unchanged, skipping
// `view()`. The CountingWidget below records every `view()` call into
// a process-local counter so tests can assert cache hits.

use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTING_VIEWS: AtomicUsize = AtomicUsize::new(0);

struct CountingWidget;

impl Widget for CountingWidget {
    type State = ();
    type Props = UntypedProps;

    fn view(id: &str, props: &UntypedProps, _state: &()) -> View {
        COUNTING_VIEWS.fetch_add(1, Ordering::SeqCst);
        let label = props
            .0
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("counting");
        button(id, label).into()
    }

    fn cache_key(props: &UntypedProps, _state: &()) -> Option<u64> {
        let label = props.0.get("label").and_then(|v| v.as_str()).unwrap_or("");
        Some(plushie::widget::hash_cache_key(label))
    }
}

/// A minimal app that renders a single [`CountingWidget`] with a
/// configurable label. The `rerender()` hook on `TestSession` drives
/// a fresh `prepare_tree` without touching the model, which is all
/// the caching test needs: we mutate the model directly when we want
/// to flip the label.
struct CachedApp;

#[derive(Clone, Default)]
struct CachedModel {
    label: &'static str,
}

impl App for CachedApp {
    type Model = CachedModel;

    fn init() -> (Self::Model, Command) {
        (CachedModel { label: "hello" }, Command::none())
    }

    fn update(model: &Self::Model, _event: Event) -> (Self::Model, Command) {
        (model.clone(), Command::none())
    }

    fn view(model: &Self::Model, widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .child(
                plushie::widget::WidgetView::<CountingWidget>::new("counter")
                    .prop("label", model.label)
                    .register(widgets),
            )
            .into()
    }
}

#[test]
fn widget_view_cache_skips_view_when_key_unchanged() {
    use plushie::test::TestSession;

    COUNTING_VIEWS.store(0, Ordering::SeqCst);
    let mut session = TestSession::<CachedApp>::start();

    // Initial render already ran view() once.
    assert_eq!(
        COUNTING_VIEWS.load(Ordering::SeqCst),
        1,
        "initial render must call view() once"
    );

    // Rerender with no change: the cache key (label) is unchanged,
    // so view() must be skipped.
    session.rerender();
    assert_eq!(
        COUNTING_VIEWS.load(Ordering::SeqCst),
        1,
        "unchanged cache key must reuse cached expansion"
    );

    // Change the label to invalidate the cache key. view() must run.
    session.model_mut().label = "world";
    session.rerender();
    assert_eq!(
        COUNTING_VIEWS.load(Ordering::SeqCst),
        2,
        "changed cache key must re-run view()"
    );

    // Another rerender with the new label unchanged: hit again.
    session.rerender();
    assert_eq!(
        COUNTING_VIEWS.load(Ordering::SeqCst),
        2,
        "cache must hit after the miss that warmed the new key"
    );
}

// ---------------------------------------------------------------------------
// memo() view caching
//
// The `memo()` view builder wraps a subtree in a `__memo__` marker
// node that records the deps hash. Normalization compares the hash
// against the previous render to decide whether to re-walk the
// subtree. The view function still runs (it's pure, the SDK can't
// short-circuit it without thread-local trickery), but the cached
// expansion path is observable: on a cache hit, the memo subtree's
// normalized children must match what they were last cycle.
// ---------------------------------------------------------------------------

#[test]
fn memo_marker_node_is_present_in_view_tree() {
    use plushie::test::TestSession;

    #[derive(Clone)]
    struct MemoApp {
        revision: u32,
    }

    impl App for MemoApp {
        type Model = Self;

        fn init() -> (Self, Command) {
            (MemoApp { revision: 0 }, Command::none())
        }

        fn update(model: &Self, _event: Event) -> (Self, Command) {
            (model.clone(), Command::none())
        }

        fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
            window("main")
                .child(plushie::ui::memo("header", model.revision, || {
                    text("memoised content").id("memo-content").into()
                }))
                .into()
        }
    }

    let session = TestSession::<MemoApp>::start();
    // The memo subtree is observable as a normal node in the
    // tree; the marker `__memo__` type is internal and gets
    // stripped during normalization, so the visible content is
    // what we assert on.
    session.assert_text("memo-content", "memoised content");
}

#[test]
fn memo_subtree_survives_unchanged_deps_across_renders() {
    use plushie::test::TestSession;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static EXPENSIVE_BUILDS: AtomicUsize = AtomicUsize::new(0);

    #[derive(Clone)]
    struct DepsApp {
        revision: u32,
        unrelated_state: u32,
    }

    impl App for DepsApp {
        type Model = Self;

        fn init() -> (Self, Command) {
            EXPENSIVE_BUILDS.store(0, Ordering::SeqCst);
            (
                DepsApp {
                    revision: 0,
                    unrelated_state: 0,
                },
                Command::none(),
            )
        }

        fn update(model: &Self, _event: Event) -> (Self, Command) {
            (model.clone(), Command::none())
        }

        fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
            window("main")
                .child(
                    column().spacing(4.0).children([
                        plushie::ui::memo("section", model.revision, || {
                            EXPENSIVE_BUILDS.fetch_add(1, Ordering::SeqCst);
                            text("memo body").id("memo-body").into()
                        }),
                        text(&format!("unrelated: {}", model.unrelated_state))
                            .id("unrelated")
                            .into(),
                    ]),
                )
                .into()
        }
    }

    let mut session = TestSession::<DepsApp>::start();
    let after_init = EXPENSIVE_BUILDS.load(Ordering::SeqCst);
    assert!(after_init >= 1, "view ran at least once at init");

    // Bumping unrelated_state triggers a re-render. The memo's
    // `view_fn` closure still runs (it's pure, the SDK can't
    // skip it); the load-bearing observable here is that the
    // memo subtree's content is still rendered correctly. A
    // regression in the memo cache that produced wrong subtree
    // content would surface as missing or stale memo content.
    session.model_mut().unrelated_state = 5;
    session.rerender();
    session.assert_text("memo-body", "memo body");
    session.assert_text("unrelated", "unrelated: 5");

    // Bumping the dep invalidates the cache. The memo body still
    // renders because the closure runs every time anyway; the
    // contract here is that no diagnostic fires from the
    // invalidation path and the tree reflects both the changed
    // memo deps and the unrelated state.
    session.model_mut().revision = 7;
    session.rerender();
    session.assert_text("memo-body", "memo body");
    session.assert_text("unrelated", "unrelated: 5");
}

#[test]
fn widget_without_cache_key_always_re_runs_view() {
    // Sanity check: the default `Widget::cache_key` returns None, so
    // widgets that don't opt in should behave exactly like before
    // (one `view()` call per render cycle).
    use plushie::test::TestSession;
    use std::sync::atomic::AtomicUsize;

    static NO_CACHE_VIEWS: AtomicUsize = AtomicUsize::new(0);

    struct NoCacheWidget;

    impl Widget for NoCacheWidget {
        type State = ();
        type Props = UntypedProps;

        fn view(id: &str, _props: &UntypedProps, _state: &()) -> View {
            NO_CACHE_VIEWS.fetch_add(1, Ordering::SeqCst);
            button(id, "nc").into()
        }
    }

    struct NoCacheApp;

    impl App for NoCacheApp {
        type Model = ();

        fn init() -> (Self::Model, Command) {
            ((), Command::none())
        }

        fn update(_model: &Self::Model, _event: Event) -> (Self::Model, Command) {
            ((), Command::none())
        }

        fn view(_model: &Self::Model, widgets: &mut WidgetRegistrar) -> ViewList {
            window("main")
                .child(plushie::widget::WidgetView::<NoCacheWidget>::new("nc").register(widgets))
                .into()
        }
    }

    NO_CACHE_VIEWS.store(0, Ordering::SeqCst);
    let mut session = TestSession::<NoCacheApp>::start();
    session.rerender();
    session.rerender();
    assert_eq!(
        NO_CACHE_VIEWS.load(Ordering::SeqCst),
        3,
        "no cache_key means view() runs every render"
    );
}
