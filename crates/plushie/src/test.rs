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

use plushie_core::key::{EffectKind, KeyPress, MouseButton};
use plushie_core::protocol::TreeNode;
use plushie_core::Selector;
use serde_json::Value;

use crate::automation::Element;
use crate::command::Command;
use crate::App;
use crate::event::{AsyncEvent, EffectEvent, EffectResult, Event, EventType, WidgetEvent};
use crate::runtime;
use crate::widget::{EventResult, Interception, WidgetStateStore};

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
}

impl<A: App> TestSession<A> {
    /// Start a new test session by calling `App::init()`.
    pub fn start() -> Self {
        let (model, init_cmd) = A::init();
        let mut widget_store = WidgetStateStore::new();
        let (tree, _) = runtime::prepare_tree::<A>(&model, &mut widget_store);
        let mut session = Self {
            model,
            tree,
            widget_store,
            async_results: HashMap::new(),
            effect_stubs: HashMap::new(),
        };
        session.execute_command(init_cmd);
        session
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

    /// Simulate a toggle on a checkbox or toggler.
    pub fn toggle(&mut self, selector: impl Into<Selector>, checked: bool) {
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

    /// Simulate a form submission (text input Enter key).
    pub fn submit(&mut self, selector: impl Into<Selector>, text: &str) {
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

    /// Simulate a table column sort click.
    pub fn sort(&mut self, selector: impl Into<Selector>, column: &str) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Sort,
            &id,
            Value::String(column.to_string()),
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
            serde_json::json!({"x": x, "y": y, "button": btn.wire_name()}),
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
            serde_json::json!({"x": x, "y": y, "button": btn.wire_name()}),
        ));
    }

    /// Simulate mouse movement on a canvas to the given coordinates.
    pub fn canvas_move(&mut self, selector: impl Into<Selector>, x: f32, y: f32) {
        let id = self.resolve(selector).id.clone();
        self.dispatch(widget_event(
            EventType::Move,
            &id,
            serde_json::json!({"x": x, "y": y}),
        ));
    }

    /// Dispatch a raw event through the widget interception layer
    /// and then to the app's update function.
    pub fn dispatch(&mut self, event: Event) {
        let cmd = match self.widget_store.intercept_event(&event) {
            Some(Interception {
                result: EventResult::Consumed,
                ..
            })
            | Some(Interception {
                result: EventResult::UpdateState,
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
        let (tree, _) = runtime::prepare_tree::<A>(&self.model, &mut self.widget_store);
        self.tree = tree;
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
    pub fn await_async(&self, tag: &str) -> Option<&Result<Value, Value>> {
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
    /// // Also works with strings:
    /// session.register_effect_stub("clipboard_read", EffectResult::ClipboardText { text: "hello".into() });
    /// ```
    pub fn register_effect_stub(
        &mut self,
        kind: impl Into<EffectKind>,
        response: EffectResult,
    ) {
        self.effect_stubs
            .insert(kind.into().wire_name().to_string(), response);
    }

    /// Remove a previously registered effect stub.
    pub fn unregister_effect_stub(&mut self, kind: impl Into<EffectKind>) {
        self.effect_stubs.remove(kind.into().wire_name());
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
        let (tree, _) = runtime::prepare_tree::<A>(&self.model, &mut self.widget_store);
        self.tree = tree;
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
/// primitives (timers, I/O, channels) internally.
fn run_async_sync(task_fn: crate::command::AsyncTaskFn) -> Result<Value, Value> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create test tokio runtime");
    rt.block_on((task_fn)())
}

fn key_event(
    event_type: crate::event::KeyEventType,
    key: &plushie_core::Key,
    modifiers: crate::types::KeyModifiers,
) -> Event {
    let wire_name = key.wire_name();
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
        key: wire_name,
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
