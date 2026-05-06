//! Test infrastructure for plushie apps.
//!
//! [`TestSession`] provides a headless testing environment that
//! exercises the full MVU cycle (init -> update -> view) without
//! rendering. Composite widgets are expanded and their events
//! are intercepted, matching runtime behavior.
//!
//! # Panics
//!
//! Interaction methods (`click`, `type_text`, `toggle`, `select`,
//! `submit`, `slide`, `paste`, `scroll`, `sort`, `canvas_*`, etc.)
//! panic if the selector does not match a widget in the current
//! tree. This is intentional: tests should fail loudly when the
//! target widget is missing. Use [`TestSession::find`] or
//! [`TestSession::find_all`] for non-panicking lookups.
//!
//! Assertion methods (`assert_text`, `assert_role`, `assert_a11y`,
//! `assert_model`, `assert_no_diagnostics`, etc.) panic on
//! assertion failure with a descriptive message.
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
use plushie_core::protocol::{TreeNode, canonical_tree_hash};
use serde_json::Value;

use crate::App;
use crate::automation::Element;
use crate::command::Command;
use crate::event::{AsyncEvent, EffectEvent, EffectResult, Event, EventType, WidgetEvent};
use crate::runtime;
use crate::runtime::subscriptions::{SubOp, SubscriptionManager};
use crate::subscription::Subscription;
use crate::widget::{EventResult, Interception, WidgetStateStore};

// ---------------------------------------------------------------------------
// Sort direction for TestSession::sort
// ---------------------------------------------------------------------------

/// Sort direction hint used by [`TestSession::sort`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    /// Ascending order.
    Asc,
    /// Descending order.
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
    memo_cache: runtime::MemoCache,
    widget_view_cache: runtime::WidgetViewCache,
    /// Completed async task results keyed by tag.
    async_results: HashMap<String, Result<Value, Value>>,
    /// Stubbed effect responses keyed by effect kind.
    effect_stubs: HashMap<String, EffectResult>,
    /// Accumulated view normalization warnings (duplicate IDs,
    /// reserved characters). Collected on every view render cycle.
    diagnostics: Vec<plushie_core::Diagnostic>,
    /// When true, Drop panics if any diagnostics have accumulated.
    /// Defaults to true (strict by default). Disabled by
    /// [`allow_diagnostics`](Self::allow_diagnostics).
    fail_on_diagnostics: bool,
    /// Diffs the app's declared subscriptions on each
    /// [`advance_subscriptions`](Self::advance_subscriptions) call.
    sub_manager: SubscriptionManager,
    /// Ops produced by the most recent subscription diff. Reset on
    /// every `advance_subscriptions` call.
    last_sub_ops: Vec<SubOp>,
    /// Async tasks queued by [`Command::task`] and waiting to be
    /// driven by the next [`run_pending_async`](Self::run_pending_async)
    /// cycle. Queuing rather than running-inline gives
    /// [`Command::cancel`] a window to drop a task before it
    /// completes.
    pending_async: Vec<(String, crate::command::AsyncTaskFn)>,
    /// Stream tasks queued the same way; drained together.
    pending_streams: Vec<(String, crate::command::StreamTaskFn)>,
    /// Non-effect renderer operations the app issued via
    /// [`Command::Renderer`] since session start (or the last
    /// [`drain_issued_ops`](Self::drain_issued_ops) call). Focus,
    /// scroll, window ops, system queries, etc. land here so tests
    /// can assert on behaviour a dispatch was supposed to trigger.
    issued_ops: Vec<crate::command::RendererOp>,
}

impl<A: App> TestSession<A> {
    /// Start a new test session by calling `App::init()`.
    pub fn start() -> Self {
        let (model, init_cmd) = A::init();
        let mut widget_store = WidgetStateStore::new();
        let mut memo_cache = runtime::MemoCache::new();
        let mut widget_view_cache = runtime::WidgetViewCache::new();
        let (tree, warnings) = runtime::prepare_tree::<A>(
            &model,
            &mut widget_store,
            &mut memo_cache,
            &mut widget_view_cache,
        );
        let mut session = Self {
            model,
            tree,
            widget_store,
            memo_cache,
            widget_view_cache,
            async_results: HashMap::new(),
            effect_stubs: HashMap::new(),
            diagnostics: warnings,
            // Strict by default; tests that expect warnings opt out
            // via `allow_diagnostics()`.
            fail_on_diagnostics: true,
            sub_manager: SubscriptionManager::new(),
            last_sub_ops: Vec::new(),
            pending_async: Vec::new(),
            pending_streams: Vec::new(),
            issued_ops: Vec::new(),
        };
        session.execute_command(init_cmd);
        session.run_pending_async();
        session
    }

    /// Disable the strict-diagnostics default, so the session does
    /// not panic on Drop when normalization warnings have
    /// accumulated. Use as the opt-out for tests that intentionally
    /// exercise diagnostic paths (e.g. assert_diagnostic_count).
    /// Sessions default to strict mode: any accumulated diagnostic
    /// at drop time is a test failure.
    pub fn allow_diagnostics(mut self) -> Self {
        self.fail_on_diagnostics = false;
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
    /// message if the widget is not found. Lists the available IDs
    /// in the tree to aid debugging.
    fn resolve(&self, selector: impl Into<Selector>) -> &TreeNode {
        let sel = selector.into();
        sel.find(&self.tree).unwrap_or_else(|| {
            let ids = collect_tree_ids(&self.tree);
            if ids.is_empty() {
                panic!("widget not found: {sel}\n  tree has no IDs");
            }
            panic!(
                "widget not found: {sel}\n  available IDs:\n    {}",
                ids.join("\n    ")
            );
        })
    }

    // -----------------------------------------------------------------------
    // Interactions
    // -----------------------------------------------------------------------

    /// Simulate a click on a widget.
    ///
    /// # Panics
    ///
    /// Panics if the selector does not match a widget in the current
    /// tree. Use [`TestSession::find`] for a non-panicking lookup.
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
    ///
    /// Combo strings passed through this helper are best-effort:
    /// unknown modifier segments are treated as part of a literal key
    /// name. Use `str::parse::<KeyPress>()` when tests should fail
    /// fast on a misspelled modifier.
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
        self.run_pending_async();

        // Re-render and expand widgets.
        let (tree, warnings) = runtime::prepare_tree::<A>(
            &self.model,
            &mut self.widget_store,
            &mut self.memo_cache,
            &mut self.widget_view_cache,
        );
        self.tree = tree;
        self.diagnostics.extend(warnings);

        // Refresh subscriptions: the model may have flipped a flag
        // that gates a subscription start/stop, and tests that want
        // to assert on the ops this produced can read
        // [`last_subscription_ops`](Self::last_subscription_ops).
        let subs = A::subscribe(&self.model);
        let ops = self.sub_manager.sync(subs);
        if !ops.is_empty() {
            self.last_sub_ops = ops;
        }
    }

    /// Rebuild the view tree from the current model without
    /// dispatching an event.
    ///
    /// Useful when a test mutates the model through
    /// [`model_mut`](Self::model_mut) and needs the tree to reflect
    /// the change before the next interaction or assertion runs.
    /// Equivalent in effect to the legacy
    /// `dispatch(AnimationFrame)` trick used by
    /// [`WidgetTestSession::start`] but names the intent.
    pub fn rerender(&mut self) {
        let (tree, warnings) = runtime::prepare_tree::<A>(
            &self.model,
            &mut self.widget_store,
            &mut self.memo_cache,
            &mut self.widget_view_cache,
        );
        self.tree = tree;
        self.diagnostics.extend(warnings);
    }

    // -----------------------------------------------------------------------
    // Command execution
    // -----------------------------------------------------------------------

    /// Process a command returned from update.
    ///
    /// Async/Stream tasks are buffered in `pending_async`/`pending_streams`
    /// and driven on the next [`run_pending_async`](Self::run_pending_async)
    /// pass. This lets a subsequent `Command::Cancel` for the same tag
    /// drop the task before it runs. Effects with stubs get immediate
    /// responses. Other commands are logged and ignored.
    ///
    /// Recursive chains driven by [`Command::dispatch`] / [`Command::SendAfter`]
    /// and effect-stub responses are capped at
    /// [`runtime::DISPATCH_DEPTH_LIMIT`]; exceeding the cap emits a
    /// [`plushie_core::Diagnostic::DispatchLoopExceeded`] and drops the
    /// offending command so the test session keeps running.
    fn execute_command(&mut self, cmd: Command) {
        self.execute_command_at_depth(cmd, 0);
    }

    fn execute_command_at_depth(&mut self, cmd: Command, depth: usize) {
        if depth >= runtime::DISPATCH_DEPTH_LIMIT {
            let diag = plushie_core::Diagnostic::DispatchLoopExceeded {
                depth: depth + 1,
                limit: runtime::DISPATCH_DEPTH_LIMIT,
            };
            log::error!("{diag}");
            self.diagnostics.push(diag);
            return;
        }
        match cmd {
            Command::None | Command::Exit => {}
            Command::Batch(cmds) => {
                for c in cmds {
                    self.execute_command_at_depth(c, depth);
                }
            }
            Command::Async { tag, task } => {
                // Queue the task. Running is deferred to
                // `run_pending_async` so a `Command::Cancel` returned
                // before the drain can still preempt it.
                self.pending_async.push((tag, task));
            }
            Command::Stream { tag, task } => {
                self.pending_streams.push((tag, task));
            }
            Command::SendAfter { event, .. } => {
                // In tests, deliver immediately (ignore delay). The
                // synchronous chain bumps `depth` so a pathological
                // update returning another dispatch trips the guard.
                let cmd = A::update(&mut self.model, *event);
                self.execute_command_at_depth(cmd, depth + 1);
            }
            Command::Cancel { tag } => {
                // Drop any pending async/stream task registered for
                // this tag before it has a chance to run.
                self.pending_async.retain(|(t, _)| t != &tag);
                self.pending_streams.retain(|(t, _)| t != &tag);
            }
            Command::Renderer(op) => {
                // Check for effect requests with stubs.
                if let crate::command::RendererOp::Effect {
                    ref tag,
                    ref request,
                    ..
                } = op
                {
                    let kind = request.kind();
                    if let Some(result) = self.effect_stubs.get(kind).cloned() {
                        let event = Event::Effect(EffectEvent {
                            tag: tag.clone(),
                            result,
                        });
                        let cmd = A::update(&mut self.model, event);
                        self.execute_command_at_depth(cmd, depth + 1);
                        return;
                    }
                }
                // Other renderer ops are not executed in test mode but
                // are recorded so tests can assert which side effects
                // the app requested.
                log::trace!("TestSession: recording renderer op: {op:?}");
                self.issued_ops.push(op);
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
            value: Some(serde_json::json!({ "timestamp": timestamp })),
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
    ///
    /// Diagnostics are replaced by whatever the fresh init render
    /// produces: any stale warnings from the previous run are
    /// dropped, but new init-phase warnings are immediately
    /// visible through [`diagnostics`](Self::diagnostics). Call
    /// [`drain_diagnostics`](Self::drain_diagnostics) after reset
    /// if the test wants a clean slate.
    ///
    /// `issued_ops` is cleared the same way: previous-run renderer
    /// ops go away but any ops the init command issues this cycle
    /// land in the freshly-empty buffer.
    pub fn reset(&mut self) {
        let (model, init_cmd) = A::init();
        self.model = model;
        self.widget_store = WidgetStateStore::new();
        self.memo_cache = runtime::MemoCache::new();
        self.widget_view_cache = runtime::WidgetViewCache::new();
        self.async_results.clear();
        self.effect_stubs.clear();
        self.sub_manager = SubscriptionManager::new();
        self.last_sub_ops.clear();
        self.pending_async.clear();
        self.pending_streams.clear();
        self.issued_ops.clear();
        let (tree, warnings) = runtime::prepare_tree::<A>(
            &self.model,
            &mut self.widget_store,
            &mut self.memo_cache,
            &mut self.widget_view_cache,
        );
        self.tree = tree;
        self.diagnostics = warnings;
        self.execute_command(init_cmd);
        self.run_pending_async();
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

    /// Compute the canonical cross-SDK hash of the current view tree.
    ///
    /// The hash input is recursively key-sorted JSON, then SHA-256 hex.
    /// That keeps golden files aligned with the renderer path and the
    /// sibling SDK test helpers.
    ///
    /// # Panics
    ///
    /// Panics if the current tree fails to serialize to JSON.
    /// `TreeNode` is designed to always serialize successfully, so
    /// this is a sanity check rather than a real failure mode.
    pub fn tree_hash(&self) -> String {
        canonical_tree_hash(Some(&self.tree)).expect("tree serialization failed")
    }

    /// Pretty-printed JSON representation of the current view tree.
    ///
    /// Useful for snapshot testing (e.g., with insta) or manual
    /// inspection. Deterministic within the same tree structure.
    ///
    /// # Panics
    ///
    /// Panics if the current tree fails to serialize to JSON
    /// (never in practice; see [`tree_hash`](Self::tree_hash)).
    pub fn tree_snapshot(&self) -> String {
        serde_json::to_string_pretty(&self.tree).expect("tree serialization failed")
    }

    // -----------------------------------------------------------------------
    // Multi-window support
    // -----------------------------------------------------------------------

    /// Scope a chain of interactions to a specific window.
    ///
    /// Interactions returned by [`WindowScope`] dispatch events whose
    /// `window_id` is set to the given window, matching how real
    /// renderer events are targeted. Window lifecycle helpers
    /// (`opened`, `closed`, `resized`, `focused`, `unfocused`)
    /// synthesise [`WindowEvent`](crate::event::WindowEvent)s with
    /// the same window scope.
    ///
    /// ```ignore
    /// let mut session = TestSession::<MyApp>::start();
    /// session.window("modal").opened();
    /// session.window("modal").click("close");
    /// session.window("modal").closed();
    /// ```
    pub fn window<'a>(&'a mut self, window_id: &str) -> WindowScope<'a, A> {
        WindowScope {
            session: self,
            window_id: window_id.to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Pending async / stream execution
    // -----------------------------------------------------------------------

    /// Drive every queued async/stream task to completion and
    /// deliver the resulting `AsyncEvent`/`StreamEvent`s back into
    /// `App::update`.
    ///
    /// Called automatically after every `dispatch` and `start`, so
    /// most tests never need to invoke it directly. Exposed publicly
    /// for scenarios that queue tasks and then explicitly cancel
    /// them before the next drain.
    ///
    /// Tasks panic-guarded: a user future that panics resolves to
    /// `Err(json!({"error": "panic", "message": ...}))` instead of
    /// tearing down the harness, matching the direct + wire runners'
    /// contract.
    pub fn run_pending_async(&mut self) {
        // Drain queues repeatedly: an async task can return a
        // Command::Async/Stream/Cancel which queues more work or
        // cancels pending work that hasn't started yet.
        loop {
            let async_batch: Vec<_> = std::mem::take(&mut self.pending_async);
            let stream_batch: Vec<_> = std::mem::take(&mut self.pending_streams);
            if async_batch.is_empty() && stream_batch.is_empty() {
                break;
            }

            for (tag, task) in async_batch {
                let result = run_async_sync(&tag, task);
                self.async_results.insert(tag.clone(), result.clone());
                let event = Event::Async(AsyncEvent { tag, result });
                let cmd = A::update(&mut self.model, event);
                self.execute_command(cmd);
            }

            for (tag, task) in stream_batch {
                let emitter = crate::command::StreamEmitter::buffered(&tag);
                let result = run_stream_sync(&tag, task, emitter.clone());
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
        }
    }

    /// Discard any tasks queued under the given tag without running
    /// them. Equivalent to emitting `Command::cancel(tag)` from an
    /// update and then calling [`run_pending_async`](Self::run_pending_async).
    pub fn cancel_pending(&mut self, tag: &str) {
        self.pending_async.retain(|(t, _)| t != tag);
        self.pending_streams.retain(|(t, _)| t != tag);
    }

    /// Number of async tasks currently queued.
    pub fn pending_async_count(&self) -> usize {
        self.pending_async.len()
    }

    // -----------------------------------------------------------------------
    // Subscription lifecycle
    // -----------------------------------------------------------------------

    /// Re-run `App::subscribe(&model)` and diff the result against
    /// the previously active subscription set.
    ///
    /// The runtime normally does this after every update; TestSession
    /// gives tests explicit control so subscription changes tied to
    /// model state can be verified. Use
    /// [`last_subscription_ops`](Self::last_subscription_ops) or
    /// [`active_subscriptions`](Self::active_subscriptions) to
    /// inspect the resulting state.
    pub fn advance_subscriptions(&mut self) {
        let new_subs = A::subscribe(&self.model);
        self.last_sub_ops = self.sub_manager.sync(new_subs);
    }

    /// Currently active subscriptions, as of the last
    /// [`advance_subscriptions`](Self::advance_subscriptions) call.
    ///
    /// Returns an empty slice if subscriptions have never been
    /// advanced.
    pub fn active_subscriptions(&self) -> &[Subscription] {
        self.sub_manager.active()
    }

    /// Ops produced by the most recent subscription diff.
    ///
    /// Reset on each [`advance_subscriptions`](Self::advance_subscriptions)
    /// call. Useful for asserting that a model change produced the
    /// expected `Subscribe`/`Unsubscribe`/`StartTimer`/`StopTimer`
    /// sequence.
    pub fn last_subscription_ops(&self) -> &[SubOp] {
        &self.last_sub_ops
    }

    // -----------------------------------------------------------------------
    // Diagnostics
    // -----------------------------------------------------------------------

    /// View normalization diagnostics accumulated since session start
    /// (or the last `drain_diagnostics` / `reset` call).
    ///
    /// Diagnostics include duplicate ID warnings and reserved
    /// character warnings from the view normalization pass. Returns
    /// each entry as its [`Display`](std::fmt::Display) string so
    /// existing callers that assert on substrings keep working; use
    /// [`typed_diagnostics`](Self::typed_diagnostics) to match on
    /// variant shape directly.
    pub fn diagnostics(&self) -> Vec<String> {
        self.diagnostics.iter().map(|d| d.to_string()).collect()
    }

    /// Take ownership of accumulated diagnostics as rendered strings,
    /// clearing the internal buffer.
    pub fn drain_diagnostics(&mut self) -> Vec<String> {
        std::mem::take(&mut self.diagnostics)
            .into_iter()
            .map(|d| d.to_string())
            .collect()
    }

    /// Accumulated diagnostics as their structured
    /// [`Diagnostic`](plushie_core::Diagnostic) variants.
    ///
    /// Every emit site now produces a typed diagnostic directly;
    /// this accessor hands them back verbatim for tests that want to
    /// match on variant shape.
    pub fn typed_diagnostics(&self) -> Vec<plushie_core::Diagnostic> {
        self.diagnostics.clone()
    }

    /// True when any accumulated diagnostic has the given kind.
    ///
    /// Preferred over substring matching on
    /// [`diagnostics`](Self::diagnostics) when the test only cares
    /// about "did *any* `duplicate_id` fire", not the full payload.
    pub fn has_diagnostic(&self, kind: plushie_core::DiagnosticKind) -> bool {
        self.diagnostics.iter().any(|d| d.kind() == kind)
    }

    /// Renderer ops the app has issued through `Command::Renderer`
    /// since session start (or the last
    /// [`drain_issued_ops`](Self::drain_issued_ops) call).
    ///
    /// Lets tests assert "did the update try to close a window?"
    /// without needing a real renderer. Effect ops resolved through
    /// a registered stub are consumed before recording; everything
    /// else is kept verbatim.
    pub fn issued_ops(&self) -> &[crate::command::RendererOp] {
        &self.issued_ops
    }

    /// Take ownership of all recorded renderer ops, clearing the
    /// buffer. Pairs with [`issued_ops`](Self::issued_ops) for tests
    /// that drive multiple phases and want to isolate per-phase ops.
    pub fn drain_issued_ops(&mut self) -> Vec<crate::command::RendererOp> {
        std::mem::take(&mut self.issued_ops)
    }

    // -----------------------------------------------------------------------
    // Assertions
    // -----------------------------------------------------------------------

    /// Assert that a matching widget exists in the view tree.
    ///
    /// # Panics
    ///
    /// Panics when the selector matches nothing in the current tree.
    pub fn assert_exists(&self, selector: impl Into<Selector>) {
        let sel = selector.into();
        assert!(
            sel.find(&self.tree).is_some(),
            "expected widget {sel} to exist in the view tree"
        );
    }

    /// Assert that no matching widget exists in the view tree.
    ///
    /// # Panics
    ///
    /// Panics when the selector matches at least one widget in the
    /// current tree.
    pub fn assert_not_exists(&self, selector: impl Into<Selector>) {
        let sel = selector.into();
        assert!(
            sel.find(&self.tree).is_none(),
            "expected widget {sel} to NOT exist in the view tree"
        );
    }

    /// Assert that a widget displays the expected text content.
    ///
    /// # Panics
    ///
    /// Panics if the selector does not match any widget, if the match
    /// has no text content, or if the text differs from `expected`.
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
    ///
    /// # Panics
    ///
    /// Panics when the selector matches nothing, when the prop is
    /// missing, or when the prop value differs from `expected`.
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
    ///
    /// # Panics
    ///
    /// Panics if the selector does not match any widget, or if the
    /// resolved role differs from `expected`.
    pub fn assert_role(&self, selector: impl Into<Selector>, expected: &str) {
        let sel = selector.into();
        let elem = self
            .find(sel.clone())
            .unwrap_or_else(|| panic!("assert_role: element not found: {sel}"));
        let actual = elem.inferred_role();
        assert_eq!(actual, expected, "role mismatch for {sel}");
    }

    /// Resolve the final accessibility attributes for a widget the same
    /// way the render pipeline does.
    ///
    /// The normalized tree already carries the author's explicit `a11y`
    /// prop plus host-SDK defaults and normalizer-populated radio
    /// relationships. This helper also layers renderer-side fallbacks so
    /// the returned value matches what AccessKit should hear:
    ///
    /// - `text_input` / `text_editor` / `combo_box` / `pick_list`:
    ///   `placeholder` flows into `description` when unset.
    /// - `image` / `svg` / `qr_code`: `alt` flows into `label` when
    ///   unset through the native widget alt path.
    ///
    /// Returns `None` if the selector does not match any widget in the
    /// tree. An empty `A11y` (no fields set) is returned for widgets
    /// the normalizer left untouched (e.g. plain text without any
    /// explicit a11y).
    ///
    /// Use this in preference to reading the raw `a11y` prop for test
    /// assertions that care about what screen readers will see.
    pub fn resolved_a11y(
        &self,
        selector: impl Into<Selector>,
    ) -> Option<plushie_core::types::A11y> {
        let sel = selector.into();
        let node = sel.find(&self.tree)?;
        Some(resolve_a11y_for_node(node))
    }

    /// Assert that a widget's accessibility properties contain all
    /// expected key-value pairs.
    ///
    /// Uses [`resolved_a11y`](Self::resolved_a11y) so both explicit
    /// overrides, placeholder descriptions, and native alt labels are
    /// visible to the assertion. `expected` must be a JSON object; each
    /// key is compared against the resolved a11y.
    ///
    /// ```ignore
    /// session.assert_a11y("heading", &serde_json::json!({"role": "heading", "level": 1}));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the selector does not match any widget, if `expected`
    /// is not a JSON object, or if any key in `expected` is missing or
    /// has a different value on the resolved a11y.
    pub fn assert_a11y(&self, selector: impl Into<Selector>, expected: &Value) {
        let sel = selector.into();
        let resolved = self
            .resolved_a11y(sel.clone())
            .unwrap_or_else(|| panic!("assert_a11y: element not found: {sel}"));
        let actual = Value::from(
            <plushie_core::types::A11y as plushie_core::types::PlushieType>::wire_encode(&resolved),
        );
        let expected_obj = expected
            .as_object()
            .expect("assert_a11y: expected value must be a JSON object");
        let actual_obj = actual
            .as_object()
            .unwrap_or_else(|| panic!("assert_a11y: resolved a11y is not an object for {sel}"));
        for (key, expected_val) in expected_obj {
            match actual_obj.get(key) {
                Some(actual_val) if actual_val == expected_val => {}
                Some(actual_val) => panic!(
                    "assert_a11y: a11y.{key} mismatch for {sel}\n  expected: {expected_val}\n  actual: {actual_val}\n  full a11y: {actual}"
                ),
                None => panic!(
                    "assert_a11y: a11y.{key} not found on {sel}\n  expected: {expected_val}\n  full a11y: {actual}"
                ),
            }
        }
    }

    /// Assert that no diagnostics have been emitted.
    ///
    /// Checks the accumulated normalization warnings.
    ///
    /// # Panics
    ///
    /// Panics with the full diagnostic details when any warning has
    /// been recorded during the session.
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
    ///
    /// # Panics
    ///
    /// Panics with a diff when the current model does not equal
    /// `expected`.
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
                    "TestSession: diagnostics detected on drop (use allow_diagnostics() to opt out):\n{}",
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
/// Golden files are stored as `{dir}/{name}.sha256` containing
/// the hex-encoded SHA-256 tree hash.
///
/// `golden_dir` is resolved relative to `CARGO_MANIFEST_DIR` (the
/// crate root at compile time), not the test's runtime cwd. That
/// keeps multi-crate workspace layouts sane: `tests/golden` always
/// refers to the same on-disk location regardless of whether
/// `cargo test` is invoked from the workspace root or a subcrate.
/// An absolute path is used verbatim.
///
/// ```ignore
/// let session = TestSession::<Counter>::start();
/// session.click("inc");
/// assert_tree_hash(&session, "counter_after_inc", "tests/golden");
/// ```
///
/// # Panics
///
/// Panics when the stored golden hash cannot be read or parsed,
/// when writing a new golden file fails, or when the current tree
/// hash does not match the stored value.
pub fn assert_tree_hash<A: App>(session: &TestSession<A>, name: &str, golden_dir: &str) {
    let hash = session.tree_hash();
    let golden_path = std::path::Path::new(golden_dir);
    let resolved_dir = if golden_path.is_absolute() {
        golden_path.to_path_buf()
    } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(golden_path)
    };
    let path = format!("{}/{name}.sha256", resolved_dir.display());
    let golden_dir = resolved_dir.display().to_string();

    let update = std::env::var("PLUSHIE_UPDATE_SNAPSHOTS")
        .map(|v| v == "1")
        .unwrap_or(false);

    if update || !std::path::Path::new(&path).exists() {
        std::fs::create_dir_all(&golden_dir).ok();
        std::fs::write(&path, &hash).unwrap_or_else(|e| {
            panic!("failed to write golden file {path}: {e}");
        });
        return;
    }

    let stored = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read golden file {path}: {e}"));
    let expected = stored.trim();

    if hash != expected {
        eprintln!(
            "[debug] tree for {name}:\n{}",
            serde_json::to_string_pretty(&session.tree).unwrap_or_default()
        );
    }
    assert_eq!(
        hash, expected,
        "tree hash mismatch for \"{name}\" (run with PLUSHIE_UPDATE_SNAPSHOTS=1 to update)"
    );
}

// ---------------------------------------------------------------------------
// WindowScope: per-window interaction helper
// ---------------------------------------------------------------------------

/// Per-window chained-interaction helper returned by
/// [`TestSession::window`].
///
/// Re-routes interactions so events carry the correct `window_id`
/// and exposes a small set of lifecycle helpers for synthesising
/// [`WindowEvent`](crate::event::WindowEvent)s (opened, closed,
/// resized, focused, unfocused).
pub struct WindowScope<'a, A: App> {
    session: &'a mut TestSession<A>,
    window_id: String,
}

impl<A: App> WindowScope<'_, A> {
    /// Dispatch a click event scoped to this window.
    pub fn click(&mut self, selector: impl Into<Selector>) {
        let id = self.session.resolve(selector).id.clone();
        let event = widget_event_in_window(EventType::Click, &id, Value::Null, &self.window_id);
        self.session.dispatch(event);
    }

    /// Dispatch a text-input event scoped to this window.
    pub fn type_text(&mut self, selector: impl Into<Selector>, text: &str) {
        let id = self.session.resolve(selector).id.clone();
        let event = widget_event_in_window(
            EventType::Input,
            &id,
            Value::String(text.to_string()),
            &self.window_id,
        );
        self.session.dispatch(event);
    }

    /// Deliver a synthetic `Opened` window event for this window.
    pub fn opened(&mut self) {
        self.session.dispatch(window_lifecycle(
            &self.window_id,
            crate::event::WindowEventType::Opened,
        ));
    }

    /// Deliver a synthetic `CloseRequested` followed by `Closed`.
    ///
    /// Matches the iced sequence: a close intent arrives first so the
    /// app can veto or run teardown, and the runtime then emits a
    /// terminal `Closed` once the window is actually gone.
    pub fn closed(&mut self) {
        self.session.dispatch(window_lifecycle(
            &self.window_id,
            crate::event::WindowEventType::CloseRequested,
        ));
        self.session.dispatch(window_lifecycle(
            &self.window_id,
            crate::event::WindowEventType::Closed,
        ));
    }

    /// Deliver a synthetic `Resized` event carrying new dimensions.
    pub fn resized(&mut self, width: f32, height: f32) {
        let event = Event::Window(crate::event::WindowEvent {
            event_type: crate::event::WindowEventType::Resized,
            window_id: self.window_id.clone(),
            x: None,
            y: None,
            width: Some(width),
            height: Some(height),
            path: None,
            scale_factor: None,
        });
        self.session.dispatch(event);
    }

    /// Deliver a synthetic `Focused` event.
    pub fn focused(&mut self) {
        self.session.dispatch(window_lifecycle(
            &self.window_id,
            crate::event::WindowEventType::Focused,
        ));
    }

    /// Deliver a synthetic `Unfocused` (blurred) event.
    pub fn unfocused(&mut self) {
        self.session.dispatch(window_lifecycle(
            &self.window_id,
            crate::event::WindowEventType::Unfocused,
        ));
    }
}

fn widget_event_in_window(event_type: EventType, id: &str, value: Value, window_id: &str) -> Event {
    // `id` comes from a normalized tree node, which already includes
    // the window prefix (`"main#open_modal"`). Parse it so scoped_id
    // picks up the right window; fall back to an explicit window when
    // the id is bare (no prefix).
    let mut parsed = plushie_core::ScopedId::parse(id);
    if parsed.window_id.is_none() {
        parsed = plushie_core::ScopedId::new(parsed.id, parsed.scope, Some(window_id.to_string()));
    }
    Event::Widget(WidgetEvent {
        event_type,
        scoped_id: parsed,
        value,
    })
}

fn window_lifecycle(window_id: &str, event_type: crate::event::WindowEventType) -> Event {
    Event::Window(crate::event::WindowEvent {
        event_type,
        window_id: window_id.to_string(),
        x: None,
        y: None,
        width: None,
        height: None,
        path: None,
        scale_factor: None,
    })
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

    fn view(model: &Self, widgets: &mut crate::widget::WidgetRegistrar) -> crate::ViewList {
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
        // Re-render to actually show the widget now that the ID is
        // set. Previously this went through a synthetic AnimationFrame
        // dispatch; `rerender` names the intent.
        session.rerender();
        // Drain any init-phase diagnostics produced before the widget
        // ID was set. `TestSession::start` runs an initial view with
        // the default `widget_id = ""`, which can trip the `empty_id`
        // normalize diagnostic before we got a chance to configure the
        // harness. Those are harness-internal, not real test failures.
        let _ = session.drain_diagnostics();
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
        let _ = session.drain_diagnostics(); // see note in `start()`
        session.rerender();
        // Also dispatch an AnimationFrame so time-based transitions
        // get a chance to settle before the test observes the first
        // frame. Preserved for historical parity with `start`.
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
/// primitives (timers, I/O, channels) internally. Panics inside
/// the future are caught and converted to an
/// `Err(json!({"error": "panic", "message": ...}))` payload so
/// the MVU loop sees an `AsyncEvent(Err(..))` rather than unwinding
/// the test harness. Matches the direct and wire runners'
/// `run_task_with_panic_guard` contract.
fn run_async_sync(tag: &str, task_fn: crate::command::AsyncTaskFn) -> Result<Value, Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create test tokio runtime");
    rt.block_on(async move {
        use futures::FutureExt;
        let future = (task_fn)();
        match std::panic::AssertUnwindSafe(future).catch_unwind().await {
            Ok(result) => result,
            Err(payload) => {
                let msg = panic_message(&*payload);
                log::error!("async task `{tag}` panicked: {msg}");
                Err(serde_json::json!({ "error": "panic", "message": msg }))
            }
        }
    })
}

fn run_stream_sync(
    tag: &str,
    task_fn: crate::command::StreamTaskFn,
    emitter: crate::command::StreamEmitter,
) -> Result<Value, Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create test tokio runtime");
    rt.block_on(async move {
        use futures::FutureExt;
        let future = (task_fn)(emitter);
        match std::panic::AssertUnwindSafe(future).catch_unwind().await {
            Ok(result) => result,
            Err(payload) => {
                let msg = panic_message(&*payload);
                log::error!("stream task `{tag}` panicked: {msg}");
                Err(serde_json::json!({ "error": "panic", "message": msg }))
            }
        }
    })
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
        .unwrap_or("(non-string panic)")
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

/// Resolve the merged a11y for a node: author explicit + widget-sdk
/// fallbacks plus native alt labels. The normalizer has already injected
/// tree-authored defaults such as implicit radio relationships, so we
/// only need to layer in renderer-side fallbacks here.
fn resolve_a11y_for_node(node: &TreeNode) -> plushie_core::types::A11y {
    use plushie_core::types::A11y;
    use plushie_core::types::PlushieType;

    let explicit = A11y::extract(&node.props, "a11y").unwrap_or_default();
    let mut inferred = A11y::default();

    // Renderer-side fallbacks for built-ins. Placeholder handling mirrors
    // widget-sdk infer_a11y; alt handling mirrors native image-like widgets.
    // Host SDK builders are expected to set the same defaults directly on
    // the tree, but this keeps tests honest for custom widgets or untouched
    // trees.
    match node.type_name.as_str() {
        "text_input" | "text_editor" | "combo_box" | "pick_list" => {
            if let Some(placeholder) = node.props.get_str("placeholder") {
                inferred.description = Some(placeholder.to_string());
            }
        }
        "image" | "svg" | "qr_code" => {
            if let Some(alt) = node.props.get_str("alt") {
                inferred.label = Some(alt.to_string());
            }
        }
        _ => {}
    }

    A11y::merge(&inferred, &explicit)
}

/// Walk the tree and collect non-empty IDs. Used to enrich
/// "widget not found" panic messages.
fn collect_tree_ids(tree: &TreeNode) -> Vec<String> {
    fn walk(node: &TreeNode, out: &mut Vec<String>) {
        if !node.id.is_empty() {
            out.push(node.id.clone());
        }
        for child in &node.children {
            walk(child, out);
        }
    }
    let mut ids = Vec::new();
    walk(tree, &mut ids);
    ids
}
