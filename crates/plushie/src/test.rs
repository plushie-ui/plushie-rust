//! Test infrastructure for plushie apps.
//!
//! [`TestSession`] provides a headless testing environment that
//! exercises the full MVU cycle (init -> update -> view) without
//! rendering. Composite widgets are expanded and their events
//! are intercepted, matching runtime behavior.
//!
//! Interactions accept [`Selector`] (or bare strings) for targeting
//! and [`KeyPress`] (or combo strings) for keyboard input:
//!
//! ```ignore
//! use plushie::prelude::*;
//! use plushie::test::TestSession;
//!
//! let mut session = TestSession::<Counter>::start();
//! session.click("inc");                         // by ID
//! session.click(Selector::role("button"));      // by role
//! session.press("Ctrl+s");                      // combo string
//! session.press(Key::Enter);                    // typed key
//! session.canvas_press("canvas", 10.0, 20.0, "right");  // mouse button
//! assert_eq!(session.model().count, 1);
//! ```

use std::collections::HashMap;

use plushie_core::Selector;
use plushie_core::key::{EffectKind, KeyPress, MouseButton};
use plushie_core::protocol::TreeNode;
use serde_json::Value;

use crate::App;
use crate::automation::Element;
use crate::command::Command;
use crate::event::{AsyncEvent, EffectEvent, EffectResult, Event, EventType, WidgetEvent};
use crate::runtime;
use crate::widget::{EventResult, Interception, WidgetStateStore};

// ---------------------------------------------------------------------------
// Sort direction for TestSession::sort
// ---------------------------------------------------------------------------

/// Sort direction hint used by [`TestSession::sort`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    fn as_str(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }
}

// ---------------------------------------------------------------------------
// TestSession
// ---------------------------------------------------------------------------

/// A headless test session for a plushie app.
///
/// Runs the app's MVU loop without rendering. Composite widgets are
/// expanded during each view cycle and their `handle_event` is called
/// before events reach the app's `update`.
///
/// All interaction methods accept `impl Into<Selector>`, so you can
/// pass bare `&str` (ID selector) or a typed `Selector` for richer
/// matching (text, role, label, focused).
pub struct TestSession<A: App> {
    model: A::Model,
    tree: TreeNode,
    widget_store: WidgetStateStore,
    /// Completed async task results keyed by tag.
    async_results: HashMap<String, Result<Value, Value>>,
    /// Stubbed effect responses keyed by effect kind.
    effect_stubs: HashMap<String, EffectResult>,
    /// Accumulated view normalization warnings (duplicate IDs,
    /// reserved characters). Collected on every view render cycle.
    diagnostics: Vec<String>,
    /// When true, Drop panics if any diagnostics have accumulated.
    /// Set via [`strict_diagnostics`](Self::strict_diagnostics).
    fail_on_diagnostics: bool,
}

impl<A: App> TestSession<A> {
    /// Start a new test session by calling `App::init()`.
    pub fn start() -> Self {
        let (model, init_cmd) = A::init();
        let mut widget_store = WidgetStateStore::new();
        let (tree, warnings) = runtime::prepare_tree::<A>(&model, &mut widget_store);
        let mut session = Self {
            model,
            tree,
            widget_store,
            async_results: HashMap::new(),
            effect_stubs: HashMap::new(),
            diagnostics: warnings,
            fail_on_diagnostics: false,
        };
        session.execute_command(init_cmd);
        session
    }

    /// Enable strict diagnostic mode: the session will panic on Drop
    /// if any normalization warnings have accumulated.
    ///
    /// This catches accidental prop validation issues without
    /// requiring an explicit `assert_no_diagnostics()` call.
    pub fn strict_diagnostics(mut self) -> Self {
        self.fail_on_diagnostics = true;
        self
    }

    /// Access the current model state.
    pub fn model(&self) -> &A::Model {
        &self.model
    }

    /// Access the current model state mutably.
    pub fn model_mut(&mut self) -> &mut A::Model {
        &mut self.model
    }

    // -----------------------------------------------------------------------
    // Selector resolution
    // -----------------------------------------------------------------------

    /// Resolve a selector to a tree node, panicking with a clear
    /// message if the widget is not found.
    fn resolve(&self, selector: impl Into<Selector>) -> &TreeNode {
        let sel = selector.into();
        sel.find(&self.tree).unwrap_or_else(|| {
            panic!("widget not found: {sel}");
        })
    }

    // -----------------------------------------------------------------------
    // Interactions
    // -----------------------------------------------------------------------

    /// Simulate a click on a widget.
    pub fn click(&mut self, selector: impl Into<Selector>) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(EventType::Click, &id, Value::Null));
    }

    /// Simulate text input on a widget.
    pub fn type_text(&mut self, selector: impl Into<Selector>, text: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Input,
            &id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate a toggle on a checkbox or toggler by auto-flipping
    /// the current `checked` prop in the tree. Use
    /// [`set_toggle`](Self::set_toggle) when the target value is
    /// known explicitly.
    pub fn toggle(&mut self, selector: impl Into<Selector>) {
        let node = self.resolve(selector);
        let id = node.id.clone();
        let current = node
            .prop_bool("checked")
            .or_else(|| node.prop_bool("is_toggled"))
            .unwrap_or(false);
        self.dispatch(widget_event(EventType::Toggle, &id, Value::Bool(!current)));
    }

    /// Simulate a toggle with an explicit target value.
    pub fn set_toggle(&mut self, selector: impl Into<Selector>, checked: bool) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(EventType::Toggle, &id, Value::Bool(checked)));
    }

    /// Simulate a selection on a pick list, combo box, or radio.
    pub fn select(&mut self, selector: impl Into<Selector>, value: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Select,
            &id,
            Value::String(value.to_string()),
        ));
    }

    /// Simulate a form submission, reading the current value from
    /// the widget's `value` prop in the tree. Use
    /// [`submit_with`](Self::submit_with) to supply an explicit
    /// value.
    pub fn submit(&mut self, selector: impl Into<Selector>) {
        let node = self.resolve(selector);
        let id = node.id.clone();
        let text = node.prop_str("value").unwrap_or("").to_string();
        self.dispatch(widget_event(EventType::Submit, &id, Value::String(text)));
    }

    /// Simulate a form submission with an explicit value.
    pub fn submit_with(&mut self, selector: impl Into<Selector>, text: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Submit,
            &id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate a slider value change.
    pub fn slide(&mut self, selector: impl Into<Selector>, value: f64) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Slide,
            &id,
            serde_json::json!(value),
        ));
    }

    /// Simulate a paste event on a widget.
    pub fn paste(&mut self, selector: impl Into<Selector>, text: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Paste,
            &id,
            Value::String(text.to_string()),
        ));
    }

    /// Simulate scrolling a widget by the given delta.
    pub fn scroll(&mut self, selector: impl Into<Selector>, delta_x: f32, delta_y: f32) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Scroll,
            &id,
            serde_json::json!({"delta_x": delta_x, "delta_y": delta_y}),
        ));
    }

    /// Simulate a table column sort click with a direction.
    ///
    /// Emits a sort event with `{column, direction}` payload so apps
    /// that look at both can see the intended direction explicitly.
    /// Apps that only read the column key continue to work.
    pub fn sort(&mut self, selector: impl Into<Selector>, column: &str, direction: SortDir) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Sort,
            &id,
            serde_json::json!({
                "column": column,
                "direction": direction.as_str(),
            }),
        ));
    }

    /// Simulate a pane grid focus cycle.
    pub fn pane_focus_cycle(&mut self, selector: impl Into<Selector>) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(EventType::PaneFocusCycle, &id, Value::Null));
    }

    // -- Keyboard interactions --

    /// Simulate a key press (key down, no release).
    ///
    /// Accepts combo strings, Key enums, or (Key, KeyModifiers) tuples:
    /// ```ignore
    /// session.press("Enter");
    /// session.press("Ctrl+s");
    /// session.press("Shift + Left_Arrow");
    /// session.press(Key::Enter);
    /// session.press((Key::Char('s'), KeyModifiers { ctrl: true, ..Default::default() }));
    /// ```
    pub fn press(&mut self, key: impl Into<KeyPress>) {
        let kp = key.into();
        self.dispatch(key_event(
            crate::event::KeyEventType::Press,
            &kp.key,
            kp.modifiers,
        ));
    }

    /// Simulate a key release.
    pub fn release(&mut self, key: impl Into<KeyPress>) {
        let kp = key.into();
        self.dispatch(key_event(
            crate::event::KeyEventType::Release,
            &kp.key,
            kp.modifiers,
        ));
    }

    /// Simulate a complete key press and release.
    pub fn type_key(&mut self, key: impl Into<KeyPress>) {
        let kp = key.into();
        self.dispatch(key_event(
            crate::event::KeyEventType::Press,
            &kp.key,
            kp.modifiers,
        ));
        self.dispatch(key_event(
            crate::event::KeyEventType::Release,
            &kp.key,
            kp.modifiers,
        ));
    }

    // -- Canvas interactions --

    /// Simulate a mouse press on a canvas at the given coordinates.
    ///
    /// Button accepts `"left"`, `"right"`, `"middle"`, or
    /// [`MouseButton`] enum values.
    pub fn canvas_press(
        &mut self,
        selector: impl Into<Selector>,
        x: f32,
        y: f32,
        button: impl Into<MouseButton>,
    ) {
        let id = self.resolve(selector).id.clone();
        let btn = button.into();
        self.dispatch(widget_event(
            EventType::Press,
            &id,
            serde_json::json!({"x": x, "y": y, "button": btn.wire_name(), "pointer": "mouse"}),
        ));
    }

    /// Simulate a mouse release on a canvas at the given coordinates.
    pub fn canvas_release(
        &mut self,
        selector: impl Into<Selector>,
        x: f32,
        y: f32,
        button: impl Into<MouseButton>,
    ) {
        let id = self.resolve(selector).id.clone();
        let btn = button.into();
        self.dispatch(widget_event(
            EventType::Release,
            &id,
            serde_json::json!({"x": x, "y": y, "button": btn.wire_name(), "pointer": "mouse"}),
        ));
    }

    /// Simulate mouse movement on a canvas to the given coordinates.
    pub fn canvas_move(&mut self, selector: impl Into<Selector>, x: f32, y: f32) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Move,
            &id,
            serde_json::json!({"x": x, "y": y, "pointer": "mouse"}),
        ));
    }

    // -- Touch interactions --

    /// Simulate a touch press on a canvas at the given coordinates.
    pub fn canvas_touch_press(
        &mut self,
        selector: impl Into<Selector>,
        x: f32,
        y: f32,
        finger: u64,
    ) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Press,
            &id,
            serde_json::json!({"x": x, "y": y, "button": "left", "pointer": "touch", "finger": finger}),
        ));
    }

    /// Simulate a touch release on a canvas at the given coordinates.
    pub fn canvas_touch_release(
        &mut self,
        selector: impl Into<Selector>,
        x: f32,
        y: f32,
        finger: u64,
    ) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Release,
            &id,
            serde_json::json!({"x": x, "y": y, "button": "left", "pointer": "touch", "finger": finger}),
        ));
    }

    /// Simulate a touch move on a canvas to the given coordinates.
    pub fn canvas_touch_move(
        &mut self,
        selector: impl Into<Selector>,
        x: f32,
        y: f32,
        finger: u64,
    ) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Move,
            &id,
            serde_json::json!({"x": x, "y": y, "pointer": "touch", "finger": finger}),
        ));
    }

    /// Dispatch a raw event through the widget interception layer
    /// and then to the app's update function.
    pub fn dispatch(&mut self, event: Event) {
        let cmd = match self.widget_store.intercept_event(&event) {
            Some(Interception {
                result: EventResult::Consumed,
                ..
            }) => {
                // Widget handled it; don't deliver to app.
                Command::None
            }
            Some(Interception {
                result: EventResult::Emit { family, value },
                widget_id,
                outer_scope,
                window_id,
            }) => {
                let new_event = Event::Widget(WidgetEvent {
                    event_type: crate::event::family_to_event_type(&family),
                    scoped_id: plushie_core::ScopedId::new(widget_id, outer_scope, Some(window_id)),
                    value,
                });
                A::update(&mut self.model, new_event)
            }
            Some(Interception {
                result: EventResult::Ignored,
                ..
            })
            | None => A::update(&mut self.model, event),
        };

        self.execute_command(cmd);

        // Re-render and expand widgets.
        let (tree, warnings) = runtime::prepare_tree::<A>(&self.model, &mut self.widget_store);
        self.tree = tree;
        self.diagnostics.extend(warnings);
    }

    // -----------------------------------------------------------------------
    // Command execution
    // -----------------------------------------------------------------------

    /// Process a command returned from update.
    ///
    /// Async tasks are executed synchronously and their results
    /// stored for `await_async`. Effects with stubs get immediate
    /// responses. Other commands are logged and ignored.
    fn execute_command(&mut self, cmd: Command) {
        match cmd {
            Command::None | Command::Exit => {}
            Command::Batch(cmds) => {
                for c in cmds {
                    self.execute_command(c);
                }
            }
            Command::Async { tag, task } => {
                // Execute the async task synchronously. TestSession
                // runs without a persistent runtime, so each task
                // gets a minimal current-thread runtime.
                let result = run_async_sync(task);
                self.async_results.insert(tag.clone(), result.clone());
                // Deliver the result as an event immediately.
                let event = Event::Async(AsyncEvent { tag, result });
                let cmd = A::update(&mut self.model, event);
                self.execute_command(cmd);
            }
            Command::Stream { tag, task } => {
                // Drive the streaming task to completion. Intermediate
                // emits are buffered by the emitter and drained as
                // StreamEvents after the future resolves.
                let emitter = crate::command::StreamEmitter::buffered(&tag);
                let result = run_stream_sync(task, emitter.clone());
                self.async_results.insert(tag.clone(), result.clone());
                for value in emitter.drain_buffer() {
                    let event = Event::Stream(crate::event::StreamEvent {
                        tag: tag.clone(),
                        value,
                    });
                    let cmd = A::update(&mut self.model, event);
                    self.execute_command(cmd);
                }
                let event = Event::Async(AsyncEvent { tag, result });
                let cmd = A::update(&mut self.model, event);
                self.execute_command(cmd);
            }
            Command::SendAfter { event, .. } => {
                // In tests, deliver immediately (ignore delay).
                let cmd = A::update(&mut self.model, *event);
                self.execute_command(cmd);
            }
            Command::Cancel { .. } => {
                // Nothing to cancel in synchronous test mode.
            }
            Command::Renderer(ref op) => {
                // Check for effect requests with stubs.
                if let crate::command::RendererOp::Effect {
                    ref tag,
                    ref request,
                    ..
                } = *op
                {
                    let kind = request.kind();
                    if let Some(result) = self.effect_stubs.get(kind).cloned() {
                        let event = Event::Effect(EffectEvent {
                            tag: tag.clone(),
                            result,
                        });
                        let cmd = A::update(&mut self.model, event);
                        self.execute_command(cmd);
                        return;
                    }
                }
                // Other renderer ops are not executed in test mode.
                log::trace!("TestSession: ignoring renderer op: {op:?}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Utilities
    // -----------------------------------------------------------------------

    /// Check whether an async task with the given tag has completed.
    ///
    /// In TestSession, async tasks are executed synchronously during
    /// dispatch, so this always returns the result if the task was
    /// triggered. Returns `None` if no task with that tag has run.
    ///
    /// The `_timeout` argument is accepted for API parity with
    /// real-backend sessions and is unused in mock mode.
    pub fn await_async(
        &self,
        tag: &str,
        _timeout: std::time::Duration,
    ) -> Option<&Result<Value, Value>> {
        self.async_results.get(tag)
    }

    /// Advance the animation frame to the given timestamp.
    ///
    /// Dispatches a system event that the renderer's animation
    /// engine uses to advance timed transitions and springs.
    pub fn advance_frame(&mut self, timestamp: u64) {
        self.dispatch(Event::System(crate::event::SystemEvent {
            event_type: crate::event::SystemEventType::AnimationFrame,
            tag: None,
            value: Some(serde_json::json!(timestamp)),
            id: None,
            window_id: None,
        }));
    }

    /// Advance the animation clock by 10 seconds to force all
    /// timed transitions and springs to settle.
    pub fn skip_transitions(&mut self) {
        self.advance_frame(10_000);
    }

    /// Register a stub response for a platform effect kind.
    ///
    /// When the app issues a command with a matching effect kind,
    /// the stub response is delivered immediately instead of
    /// invoking the real platform API.
    ///
    /// ```ignore
    /// use plushie::prelude::*;
    /// use plushie::event::EffectResult;
    ///
    /// session.register_effect_stub(
    ///     EffectKind::FileOpen,
    ///     EffectResult::FileOpened { path: "/tmp/test.txt".into() },
    /// );
    /// ```
    pub fn register_effect_stub(&mut self, kind: EffectKind, response: EffectResult) {
        self.effect_stubs
            .insert(kind.wire_name().to_string(), response);
    }

    /// Remove a previously registered effect stub.
    pub fn unregister_effect_stub(&mut self, kind: EffectKind) {
        self.effect_stubs.remove(kind.wire_name());
    }

    /// Reset the session to its initial state.
    ///
    /// Calls `App::init()` again, discarding the current model,
    /// widget state, async results, and effect stubs.
    pub fn reset(&mut self) {
        let (model, init_cmd) = A::init();
        self.model = model;
        self.widget_store = WidgetStateStore::new();
        self.async_results.clear();
        self.effect_stubs.clear();
        let (tree, warnings) = runtime::prepare_tree::<A>(&self.model, &mut self.widget_store);
        self.tree = tree;
        self.diagnostics = warnings;
        self.execute_command(init_cmd);
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Find a widget in the view tree by selector.
    ///
    /// Returns an [`Element`] wrapper with typed accessors, or
    /// `None` if no matching widget exists.
    pub fn find(&self, selector: impl Into<Selector>) -> Option<Element<'_>> {
        let sel = selector.into();
        sel.find(&self.tree).map(Element::new)
    }

    /// Find all widgets matching the selector.
    ///
    /// Returns an empty Vec if no matches. Useful for counting
    /// elements or iterating over a group.
    pub fn find_all(&self, selector: impl Into<Selector>) -> Vec<Element<'_>> {
        let sel = selector.into();
        sel.find_all(&self.tree)
            .into_iter()
            .map(Element::new)
            .collect()
    }

    /// Find the currently focused widget, if any.
    pub fn find_focused(&self) -> Option<Element<'_>> {
        self.find(Selector::focused())
    }

    /// Get the text content of a widget.
    pub fn text_content(&self, selector: impl Into<Selector>) -> Option<String> {
        self.find(selector)?.text().map(|s| s.to_string())
    }

    /// Get a string prop from a widget.
    pub fn prop_str(&self, selector: impl Into<Selector>, key: &str) -> Option<String> {
        self.find(selector)?.prop_str(key).map(|s| s.to_string())
    }

    /// Get a prop value from a widget as an owned JSON Value.
    pub fn prop(&self, selector: impl Into<Selector>, key: &str) -> Option<Value> {
        self.find(selector)?.prop(key)
    }

    /// Get the current normalized view tree.
    pub fn tree(&self) -> &TreeNode {
        &self.tree
    }

    /// Compute a stable hash of the current view tree.
    ///
    /// Uses FNV-1a on the tree's JSON serialization. The hash is
    /// deterministic across builds, so it can be stored as a constant
    /// in tests for regression detection.
    pub fn tree_hash(&self) -> u64 {
        let json = serde_json::to_string(&self.tree).expect("tree serialization failed");
        fnv1a(json.as_bytes())
    }

    /// Pretty-printed JSON representation of the current view tree.
    ///
    /// Useful for snapshot testing (e.g., with insta) or manual
    /// inspection. Deterministic within the same tree structure.
    pub fn tree_snapshot(&self) -> String {
        serde_json::to_string_pretty(&self.tree).expect("tree serialization failed")
    }

    // -----------------------------------------------------------------------
    // Diagnostics
    // -----------------------------------------------------------------------

    /// View normalization diagnostics accumulated since session start
    /// (or the last `drain_diagnostics` / `reset` call).
    ///
    /// Diagnostics include duplicate ID warnings and reserved
    /// character warnings from the view normalization pass.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// Take ownership of accumulated diagnostics, clearing the
    /// internal buffer.
    pub fn drain_diagnostics(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diagnostics)
    }

    // -----------------------------------------------------------------------
    // Assertions
    // -----------------------------------------------------------------------

    /// Assert that a matching widget exists in the view tree.
    pub fn assert_exists(&self, selector: impl Into<Selector>) {
        let sel = selector.into();
        assert!(
            sel.find(&self.tree).is_some(),
            "expected widget {sel} to exist in the view tree"
        );
    }

    /// Assert that no matching widget exists in the view tree.
    pub fn assert_not_exists(&self, selector: impl Into<Selector>) {
        let sel = selector.into();
        assert!(
            sel.find(&self.tree).is_none(),
            "expected widget {sel} to NOT exist in the view tree"
        );
    }

    /// Assert that a widget displays the expected text content.
    pub fn assert_text(&self, selector: impl Into<Selector>, expected: &str) {
        let sel = selector.into();
        let actual = sel
            .find(&self.tree)
            .and_then(|n| Element::new(n).text().map(|s| s.to_string()));
        assert_eq!(
            actual.as_deref(),
            Some(expected),
            "expected widget {sel} to display \"{expected}\", got {actual:?}",
        );
    }

    /// Assert that a widget prop has the expected value.
    pub fn assert_prop(&self, selector: impl Into<Selector>, key: &str, expected: &Value) {
        let sel = selector.into();
        let actual = sel.find(&self.tree).and_then(|n| n.props.get_value(key));
        assert_eq!(
            actual.as_ref(),
            Some(expected),
            "expected widget {sel} prop \"{key}\" to be {expected}, got {actual:?}"
        );
    }

    /// Assert that a widget has the expected accessibility role.
    ///
    /// Checks the [`Element::inferred_role`], which reads the
    /// explicit a11y role or falls back to a widget-type mapping.
    pub fn assert_role(&self, selector: impl Into<Selector>, expected: &str) {
        let sel = selector.into();
        let elem = self
            .find(sel.clone())
            .unwrap_or_else(|| panic!("assert_role: element not found: {sel}"));
        let actual = elem.inferred_role();
        assert_eq!(actual, expected, "role mismatch for {sel}");
    }

    /// Assert that a widget's accessibility properties contain all
    /// expected key-value pairs.
    ///
    /// `expected` must be a JSON object. Each key in it is checked
    /// against the widget's a11y props; missing keys or value
    /// mismatches panic with a detailed message.
    ///
    /// ```ignore
    /// session.assert_a11y("heading", &serde_json::json!({"role": "heading", "level": 1}));
    /// ```
    pub fn assert_a11y(&self, selector: impl Into<Selector>, expected: &Value) {
        let sel = selector.into();
        let elem = self
            .find(sel.clone())
            .unwrap_or_else(|| panic!("assert_a11y: element not found: {sel}"));
        let a11y = elem
            .a11y()
            .unwrap_or_else(|| panic!("assert_a11y: no a11y props on element: {sel}"));
        let expected_obj = expected
            .as_object()
            .expect("assert_a11y: expected value must be a JSON object");
        let actual_obj = a11y
            .as_object()
            .unwrap_or_else(|| panic!("assert_a11y: a11y is not an object on element: {sel}"));
        for (key, expected_val) in expected_obj {
            match actual_obj.get(key) {
                Some(actual_val) if actual_val == expected_val => {}
                Some(actual_val) => panic!(
                    "assert_a11y: a11y.{key} mismatch for {sel}\n  expected: {expected_val}\n  actual: {actual_val}\n  full a11y: {a11y}"
                ),
                None => panic!(
                    "assert_a11y: a11y.{key} not found on {sel}\n  expected: {expected_val}\n  full a11y: {a11y}"
                ),
            }
        }
    }

    /// Assert that no diagnostics have been emitted.
    ///
    /// Checks the accumulated normalization warnings. Panics with
    /// the diagnostic details if any warnings exist.
    pub fn assert_no_diagnostics(&self) {
        if !self.diagnostics.is_empty() {
            let details: Vec<_> = self
                .diagnostics
                .iter()
                .map(|d| format!("  - {d}"))
                .collect();
            panic!(
                "expected no diagnostics, but found:\n{}",
                details.join("\n")
            );
        }
    }
}

// ---------------------------------------------------------------------------
// assert_model (requires PartialEq + Debug on the model type)
// ---------------------------------------------------------------------------

impl<A: App> TestSession<A>
where
    A::Model: PartialEq + std::fmt::Debug,
{
    /// Assert that the current model equals the expected value.
    ///
    /// Requires `PartialEq + Debug` on the model type. Uses
    /// `assert_eq!` for rich diff output on mismatch.
    pub fn assert_model(&self, expected: &A::Model) {
        assert_eq!(self.model(), expected, "model mismatch");
    }
}

// ---------------------------------------------------------------------------
// Drop: auto-fail on diagnostics
// ---------------------------------------------------------------------------

impl<A: App> Drop for TestSession<A> {
    fn drop(&mut self) {
        if self.fail_on_diagnostics && !self.diagnostics.is_empty() {
            // Don't double-panic if we're already unwinding.
            if !std::thread::panicking() {
                let details: Vec<_> = self
                    .diagnostics
                    .iter()
                    .map(|d| format!("  - {d}"))
                    .collect();
                panic!(
                    "TestSession (strict_diagnostics): diagnostics detected on drop:\n{}",
                    details.join("\n")
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Golden-file tree hash
// ---------------------------------------------------------------------------

/// Assert that the current tree hash matches a golden file.
///
/// On first run (golden file doesn't exist), creates it. On
/// subsequent runs, compares. Set `PLUSHIE_UPDATE_SNAPSHOTS=1`
/// to overwrite existing golden files.
///
/// Golden files are stored as `{dir}/{name}.hash` containing
/// the decimal u64 hash value.
///
/// ```ignore
/// let session = TestSession::<Counter>::start();
/// session.click("inc");
/// assert_tree_hash(&session, "counter_after_inc", "tests/golden");
/// ```
pub fn assert_tree_hash<A: App>(session: &TestSession<A>, name: &str, golden_dir: &str) {
    let hash = session.tree_hash();
    let path = format!("{golden_dir}/{name}.hash");

    let update = std::env::var("PLUSHIE_UPDATE_SNAPSHOTS")
        .map(|v| v == "1")
        .unwrap_or(false);

    if update || !std::path::Path::new(&path).exists() {
        std::fs::create_dir_all(golden_dir).ok();
        std::fs::write(&path, hash.to_string()).unwrap_or_else(|e| {
            panic!("failed to write golden file {path}: {e}");
        });
        return;
    }

    let stored = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read golden file {path}: {e}"));
    let expected: u64 = stored
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("invalid golden file {path}: {e}"));

    assert_eq!(
        hash, expected,
        "tree hash mismatch for \"{name}\" (run with PLUSHIE_UPDATE_SNAPSHOTS=1 to update)"
    );
}

// ---------------------------------------------------------------------------
// WidgetTestSession
// ---------------------------------------------------------------------------

/// A test session for composite widgets in isolation.
///
/// Auto-generates a harness app that hosts the widget in a
/// `window > column` container. Emitted events from the widget
/// are recorded and accessible via [`events`](Self::events) and
/// [`last_event`](Self::last_event).
///
/// ```ignore
/// use plushie::test::WidgetTestSession;
///
/// let mut session = WidgetTestSession::<StarRating>::start("stars");
/// session.click("star_3");
/// let (family, value) = session.last_event().unwrap();
/// assert_eq!(family, "select");
/// ```
pub struct WidgetTestSession<W: crate::widget::Widget> {
    inner: TestSession<WidgetHarness<W>>,
}

/// Harness app that hosts a widget and records emitted events.
///
/// Used internally by [`WidgetTestSession`]. The `events` field
/// stores all widget events for test assertions.
pub struct WidgetHarness<W: crate::widget::Widget> {
    widget_id: String,
    props: plushie_core::protocol::PropMap,
    events: Vec<(String, Value)>,
    _marker: std::marker::PhantomData<W>,
}

impl<W: crate::widget::Widget> App for WidgetHarness<W> {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            Self {
                widget_id: String::new(),
                props: plushie_core::protocol::PropMap::new(),
                events: Vec::new(),
                _marker: std::marker::PhantomData,
            },
            Command::None,
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        // Record all widget events emitted by the hosted widget.
        if let Event::Widget(ref w) = event {
            model
                .events
                .push((w.event_type.as_family().to_string(), w.value.clone()));
        }
        Command::None
    }

    fn view(
        model: &Self,
        widgets: &mut crate::widget::WidgetRegistrar,
    ) -> plushie_core::protocol::TreeNode {
        use crate::ui::*;

        let mut wv = crate::widget::WidgetView::<W>::new(&model.widget_id);
        for (key, value) in model.props.iter() {
            wv = wv.prop(key, value.clone());
        }

        window("main")
            .child(column().child(wv.register(widgets)))
            .into()
    }
}

impl<W: crate::widget::Widget> WidgetTestSession<W> {
    /// Start a test session hosting the widget with the given ID.
    pub fn start(id: &str) -> Self {
        let mut session = TestSession::<WidgetHarness<W>>::start();
        session.model_mut().widget_id = id.to_string();
        // Re-render to actually show the widget.
        session.dispatch(Event::System(crate::event::SystemEvent {
            event_type: crate::event::SystemEventType::AnimationFrame,
            tag: None,
            value: None,
            id: None,
            window_id: None,
        }));
        Self { inner: session }
    }

    /// Start with initial props set on the widget.
    pub fn start_with_props(
        id: &str,
        props: Vec<(&str, plushie_core::protocol::PropValue)>,
    ) -> Self {
        let mut session = TestSession::<WidgetHarness<W>>::start();
        session.model_mut().widget_id = id.to_string();
        for (key, value) in props {
            session.model_mut().props.insert(key, value);
        }
        // Re-render with props.
        session.dispatch(Event::System(crate::event::SystemEvent {
            event_type: crate::event::SystemEventType::AnimationFrame,
            tag: None,
            value: None,
            id: None,
            window_id: None,
        }));
        Self { inner: session }
    }

    // -- Event recording --

    /// All events emitted by the widget (oldest first).
    pub fn events(&self) -> &[(String, Value)] {
        &self.inner.model().events
    }

    /// The most recently emitted event, if any.
    pub fn last_event(&self) -> Option<&(String, Value)> {
        self.inner.model().events.last()
    }

    /// Take ownership of all recorded events, clearing the buffer.
    pub fn drain_events(&mut self) -> Vec<(String, Value)> {
        std::mem::take(&mut self.inner.model_mut().events)
    }

    // -- Access to the underlying TestSession --

    /// Access the underlying [`TestSession`] for the full API
    /// (interactions, assertions, queries, diagnostics, etc.).
    ///
    /// ```ignore
    /// session.session().assert_a11y("star_3", &json!({"role": "radio_button"}));
    /// session.session().canvas_press("canvas", 10.0, 20.0, "left");
    /// ```
    pub fn session(&self) -> &TestSession<WidgetHarness<W>> {
        &self.inner
    }

    /// Mutable access to the underlying [`TestSession`].
    pub fn session_mut(&mut self) -> &mut TestSession<WidgetHarness<W>> {
        &mut self.inner
    }

    // -- Convenience delegates for the most common operations --

    /// Simulate a click on a widget.
    pub fn click(&mut self, selector: impl Into<Selector>) {
        self.inner.click(selector);
    }

    /// Simulate text input.
    pub fn type_text(&mut self, selector: impl Into<Selector>, text: &str) {
        self.inner.type_text(selector, text);
    }

    /// Simulate a toggle by reading the current prop and flipping it.
    pub fn toggle(&mut self, selector: impl Into<Selector>) {
        self.inner.toggle(selector);
    }

    /// Simulate a toggle with an explicit target value.
    pub fn set_toggle(&mut self, selector: impl Into<Selector>, checked: bool) {
        self.inner.set_toggle(selector, checked);
    }

    /// Simulate a slider change.
    pub fn slide(&mut self, selector: impl Into<Selector>, value: f64) {
        self.inner.slide(selector, value);
    }

    /// Simulate a key press.
    pub fn press(&mut self, key: impl Into<KeyPress>) {
        self.inner.press(key);
    }

    /// Find a widget in the view tree.
    pub fn find(&self, selector: impl Into<Selector>) -> Option<Element<'_>> {
        self.inner.find(selector)
    }

    /// Assert a widget exists.
    pub fn assert_exists(&self, selector: impl Into<Selector>) {
        self.inner.assert_exists(selector);
    }

    /// Assert a widget displays expected text.
    pub fn assert_text(&self, selector: impl Into<Selector>, expected: &str) {
        self.inner.assert_text(selector, expected);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// FNV-1a hash for stable tree hashing. Deterministic across builds
/// (unlike DefaultHasher which is randomized).
fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn widget_event(event_type: EventType, id: &str, value: Value) -> Event {
    Event::Widget(WidgetEvent {
        event_type,
        scoped_id: plushie_core::ScopedId::parse(id),
        value,
    })
}

/// Execute an async task synchronously for test mode.
///
/// Creates a minimal tokio current-thread runtime to drive the
/// future to completion. This handles tasks that use tokio
/// primitives (timers, I/O, channels) internally.
fn run_async_sync(task_fn: crate::command::AsyncTaskFn) -> Result<Value, Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create test tokio runtime");
    rt.block_on((task_fn)())
}

fn run_stream_sync(
    task_fn: crate::command::StreamTaskFn,
    emitter: crate::command::StreamEmitter,
) -> Result<Value, Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create test tokio runtime");
    rt.block_on((task_fn)(emitter))
}

fn key_event(
    event_type: crate::event::KeyEventType,
    key: &plushie_core::Key,
    modifiers: crate::types::KeyModifiers,
) -> Event {
    let text = if event_type == crate::event::KeyEventType::Press {
        match key {
            plushie_core::Key::Char(c) => Some(c.to_string()),
            _ => None,
        }
    } else {
        None
    };
    Event::Key(crate::event::KeyEvent {
        event_type,
        key: key.clone(),
        modified_key: None,
        physical_key: None,
        location: crate::event::KeyLocation::Standard,
        modifiers,
        text,
        repeat: false,
        captured: false,
        window_id: Some("main".to_string()),
    })
}
