//! Side effects returned from [`App::update`](crate::App::update).
//!
//! Commands are data, not closures (except `Async` and `Stream`).
//! This makes them testable: you can assert which commands an
//! update call returns without executing them.
//!
//! Operation types ([`WindowOp`], [`ImageOp`], [`EffectRequest`], etc.)
//! are defined in [`plushie_core::ops`] and re-exported here.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::Value;

use crate::event::Event;

// Re-export all operation types from plushie-core.
pub use plushie_core::ops::*;

/// A boxed async closure that produces a result.
///
/// The closure is called once to produce a future. The future resolves
/// to `Ok(value)` or `Err(value)`, delivered as
/// [`AsyncEvent`](crate::event::AsyncEvent).
pub type AsyncTaskFn =
    Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = Result<Value, Value>> + Send>> + Send>;

/// A boxed streaming async closure. Receives a [`StreamEmitter`] to push
/// intermediate values as [`StreamEvent`](crate::event::StreamEvent)s;
/// returns a final [`AsyncEvent`](crate::event::AsyncEvent) when the
/// future resolves.
pub type StreamTaskFn = Box<
    dyn FnOnce(StreamEmitter) -> Pin<Box<dyn Future<Output = Result<Value, Value>> + Send>> + Send,
>;

/// A cloneable sink for pushing values from a streaming task.
///
/// Runtime-specific sinks are installed when the command begins
/// executing. Until then (and in test mode), emits are buffered
/// locally and drained by the runner.
#[derive(Clone)]
pub struct StreamEmitter {
    tag: String,
    inner: Arc<Mutex<StreamEmitterInner>>,
}

enum StreamEmitterInner {
    /// Values accumulated before a runtime sink is attached.
    Buffer(Vec<Value>),
    /// Values routed through the runtime sink.
    Sink(Box<dyn FnMut(String, Value) + Send>),
}

impl StreamEmitter {
    /// Create an emitter backed by an in-memory buffer. Used by test
    /// runners and as the default until a runtime installs a sink.
    pub fn buffered(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            inner: Arc::new(Mutex::new(StreamEmitterInner::Buffer(Vec::new()))),
        }
    }

    /// Replace the underlying delivery mechanism with a sink closure.
    /// Any buffered values are flushed through the new sink in order.
    pub fn attach_sink(&self, mut sink: Box<dyn FnMut(String, Value) + Send>) {
        let mut guard = self.inner.lock().unwrap();
        if let StreamEmitterInner::Buffer(values) = &mut *guard {
            for v in values.drain(..) {
                sink(self.tag.clone(), v);
            }
        }
        *guard = StreamEmitterInner::Sink(sink);
    }

    /// Drain buffered values. Only meaningful when the emitter is
    /// still in buffer mode; returns empty otherwise.
    pub fn drain_buffer(&self) -> Vec<Value> {
        let mut guard = self.inner.lock().unwrap();
        match &mut *guard {
            StreamEmitterInner::Buffer(v) => std::mem::take(v),
            StreamEmitterInner::Sink(_) => Vec::new(),
        }
    }

    /// The tag this emitter is bound to.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Emit an intermediate value to the runtime. Delivered as
    /// [`StreamEvent`](crate::event::StreamEvent) with this emitter's
    /// tag.
    pub fn emit(&self, value: impl Into<Value>) {
        let value = value.into();
        let mut guard = self.inner.lock().unwrap();
        match &mut *guard {
            StreamEmitterInner::Buffer(buf) => buf.push(value),
            StreamEmitterInner::Sink(sink) => sink(self.tag.clone(), value),
        }
    }

    /// Emit a typed widget event, encoding it to the wire format first.
    /// The tag used is the emitter's tag; the event's family is not
    /// inspected.
    pub fn emit_event(&self, event: impl plushie_core::types::WidgetEventEncode) {
        let (_family, value) = event.to_wire();
        self.emit(Value::from(value));
    }
}

impl std::fmt::Debug for StreamEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamEmitter")
            .field("tag", &self.tag)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// A side effect returned from the update function.
///
/// Every operation has a builder method for ergonomic construction
/// (focus, scroll, window ops/queries, effects, images, pane grid,
/// system, async tasks, etc.).
///
/// Commands that go to the renderer are wrapped in
/// [`Command::Renderer`]. SDK-local commands (async tasks, timers)
/// are handled in-process and never reach the renderer.
#[non_exhaustive]
pub enum Command {
    /// No side effect.
    None,
    /// Execute multiple commands.
    Batch(Vec<Command>),
    /// Exit the application.
    Exit,

    // -- SDK-local (never sent to renderer) --
    /// Run an async task. Result delivered as
    /// [`AsyncEvent`](crate::event::AsyncEvent).
    Async { tag: String, task: AsyncTaskFn },
    /// Run a streaming async task. Intermediate emits deliver as
    /// [`StreamEvent`](crate::event::StreamEvent); the final result
    /// delivers as [`AsyncEvent`](crate::event::AsyncEvent).
    Stream { tag: String, task: StreamTaskFn },
    /// Cancel a running async task or stream by tag.
    Cancel { tag: String },
    /// Deliver an event after a delay.
    SendAfter { delay: Duration, event: Box<Event> },

    // -- Renderer operations (typed, zero-overhead in direct mode) --
    /// An operation for the renderer to execute.
    Renderer(RendererOp),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Batch(cmds) => f.debug_tuple("Batch").field(cmds).finish(),
            Self::Exit => write!(f, "Exit"),
            Self::Async { tag, .. } => f.debug_struct("Async").field("tag", tag).finish(),
            Self::Stream { tag, .. } => f.debug_struct("Stream").field("tag", tag).finish(),
            Self::Cancel { tag } => f.debug_struct("Cancel").field("tag", tag).finish(),
            Self::SendAfter { delay, .. } => {
                f.debug_struct("SendAfter").field("delay", delay).finish()
            }
            Self::Renderer(op) => f.debug_tuple("Renderer").field(op).finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl Command {
    /// A no-op command.
    pub fn none() -> Self {
        Self::None
    }

    /// Execute multiple commands together.
    pub fn batch(cmds: impl IntoIterator<Item = Command>) -> Self {
        Self::Batch(cmds.into_iter().collect())
    }

    /// Exit the application.
    pub fn exit() -> Self {
        Self::Exit
    }

    /// Deliver an event after a delay.
    pub fn send_after(delay: Duration, event: Event) -> Self {
        Self::SendAfter {
            delay,
            event: Box::new(event),
        }
    }

    /// Deliver an event through the normal update pipeline as soon
    /// as possible. Equivalent to
    /// [`send_after`](Self::send_after) with a zero delay, but named
    /// to make the intent clear at the call site.
    ///
    /// Useful for lifting a locally-derived value back into the MVU
    /// loop, e.g. kicking off a follow-up update after computing a
    /// value in the current `update`:
    ///
    /// ```ignore
    /// Command::dispatch(Event::Widget(WidgetEvent {
    ///     event_type: EventType::Click,
    ///     scoped_id: ScopedId::new("next", vec![], None),
    ///     value: Value::Null,
    /// }))
    /// ```
    pub fn dispatch(event: Event) -> Self {
        Self::send_after(Duration::ZERO, event)
    }

    /// Run an async task. The result is delivered as
    /// [`AsyncEvent`](crate::event::AsyncEvent).
    ///
    /// ```ignore
    /// Command::async_task("fetch", || async {
    ///     let data = reqwest::get("https://api.example.com/data")
    ///         .await
    ///         .map_err(|e| serde_json::json!(e.to_string()))?
    ///         .text()
    ///         .await
    ///         .map_err(|e| serde_json::json!(e.to_string()))?;
    ///     Ok(serde_json::json!(data))
    /// })
    /// ```
    ///
    /// # Delivery contract
    ///
    /// For every `async_task` the MVU loop sees exactly one of:
    ///
    /// - `AsyncEvent(Ok(value))` when the future resolves successfully.
    /// - `AsyncEvent(Err(value))` when the future resolves to `Err`,
    ///   or when it panics (the runner and
    ///   [`TestSession`](crate::test::TestSession) wrap the future
    ///   in a panic guard that converts panics to
    ///   `Err(json!({"error": "panic", "message": ...}))` instead of
    ///   unwinding).
    /// - Nothing at all if the task is cancelled via
    ///   [`Command::cancel`] with the same `tag` before it completes.
    pub fn async_task<F, Fut>(tag: &str, f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, Value>> + Send + 'static,
    {
        Self::Async {
            tag: tag.to_string(),
            task: Box::new(move || Box::pin(f())),
        }
    }

    /// Cancel a running async task or stream by tag.
    ///
    /// # Semantics
    ///
    /// Cancellation is best-effort:
    ///
    /// - If a task with `tag` is still queued (has not started
    ///   running), it is dropped without running and no
    ///   `AsyncEvent`/`StreamEvent` is delivered.
    /// - If a task with `tag` is already in flight, the runner
    ///   aborts it where possible. The task's result is discarded
    ///   and no `AsyncEvent` is delivered. A panic racing
    ///   cancellation is swallowed.
    /// - If no task with `tag` exists, the command is a no-op.
    ///
    /// Tags are how `Command::async_task`, `Command::stream`, and
    /// `Command::cancel` rendezvous. Reusing a tag while an earlier
    /// task is still in flight replaces the earlier task (the
    /// runner cancels it on the author's behalf).
    pub fn cancel(tag: &str) -> Self {
        Self::Cancel {
            tag: tag.to_string(),
        }
    }

    /// Run a streaming async task. Intermediate emits deliver as
    /// [`StreamEvent`](crate::event::StreamEvent)s; the final future
    /// result delivers as [`AsyncEvent`](crate::event::AsyncEvent).
    ///
    /// The task receives a cloneable [`StreamEmitter`] it can pass
    /// around to produce values over time:
    ///
    /// ```ignore
    /// Command::stream("import", |emitter| async move {
    ///     for line in fetch_lines().await? {
    ///         emitter.emit(line);
    ///     }
    ///     Ok(serde_json::json!({"done": true}))
    /// })
    /// ```
    ///
    /// Cancel via [`Command::cancel`] with the same tag.
    pub fn stream<F, Fut>(tag: &str, f: F) -> Self
    where
        F: FnOnce(StreamEmitter) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, Value>> + Send + 'static,
    {
        Self::Stream {
            tag: tag.to_string(),
            task: Box::new(move |emitter| Box::pin(f(emitter))),
        }
    }

    // -- Focus --

    /// Move keyboard focus to the widget with the given ID.
    pub fn focus(id: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: id.to_string(),
            family: "focus".to_string(),
            value: Value::Null,
        })
    }

    /// Move keyboard focus to the next focusable widget.
    pub fn focus_next() -> Self {
        Self::Renderer(RendererOp::FocusNext)
    }

    /// Move keyboard focus to the previous focusable widget.
    pub fn focus_previous() -> Self {
        Self::Renderer(RendererOp::FocusPrevious)
    }

    /// Move keyboard focus to the next focusable widget within the
    /// subtree rooted at the given widget ID. Focus wraps within the
    /// scope rather than walking out into siblings.
    ///
    /// Use this for scoped keyboard navigation: a menu, a pane grid,
    /// or anywhere you need a contained Tab cycle. For modal focus
    /// traps, set `a11y.modal = true` on the container; the fork
    /// auto-traps focus at modal boundaries without needing this
    /// command.
    pub fn focus_next_within(scope: &str) -> Self {
        Self::Renderer(RendererOp::FocusNextWithin {
            scope: scope.to_string(),
        })
    }

    /// Move keyboard focus to the previous focusable widget within
    /// the given scope. See [`focus_next_within`](Command::focus_next_within).
    pub fn focus_previous_within(scope: &str) -> Self {
        Self::Renderer(RendererOp::FocusPreviousWithin {
            scope: scope.to_string(),
        })
    }

    // -- Text cursor --

    /// Select all text in a text input or editor.
    pub fn select_all(target: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "select_all".to_string(),
            value: Value::Null,
        })
    }

    /// Move the cursor to the front of a text input or editor.
    pub fn move_cursor_to_front(target: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "move_cursor_to_front".to_string(),
            value: Value::Null,
        })
    }

    /// Move the cursor to the end of a text input or editor.
    pub fn move_cursor_to_end(target: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "move_cursor_to_end".to_string(),
            value: Value::Null,
        })
    }

    /// Move the cursor to a specific position in a text input.
    pub fn move_cursor_to(target: &str, position: usize) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "move_cursor_to".to_string(),
            value: serde_json::json!({"position": position}),
        })
    }

    /// Select a range of text in a text input.
    pub fn select_range(target: &str, start: usize, end: usize) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "select_range".to_string(),
            value: serde_json::json!({"start": start, "end": end}),
        })
    }

    // -- Scroll --

    /// Scroll a scrollable widget to an absolute position.
    pub fn scroll_to(target: &str, x: f32, y: f32) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "scroll_to".to_string(),
            value: serde_json::json!({"x": x, "y": y}),
        })
    }

    /// Scroll a scrollable widget by a relative offset.
    pub fn scroll_by(target: &str, x: f32, y: f32) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "scroll_by".to_string(),
            value: serde_json::json!({"x": x, "y": y}),
        })
    }

    /// Snap a scrollable widget to a position (no animation).
    pub fn snap_to(target: &str, x: f32, y: f32) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "snap_to".to_string(),
            value: serde_json::json!({"x": x, "y": y}),
        })
    }

    /// Snap a scrollable widget to the end of its content.
    pub fn snap_to_end(target: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "snap_to_end".to_string(),
            value: Value::Null,
        })
    }

    // -- Window --

    /// Close the window with the given ID.
    pub fn close_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Close(id.to_string())))
    }

    /// Resize a window to the given dimensions in logical pixels.
    pub fn resize_window(id: &str, width: f32, height: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Resize {
            window_id: id.to_string(),
            width,
            height,
        }))
    }

    /// Move a window to the given position in logical pixels.
    pub fn move_window(id: &str, x: f32, y: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Move {
            window_id: id.to_string(),
            x,
            y,
        }))
    }

    /// Maximize a window.
    pub fn maximize_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Maximize {
            window_id: id.to_string(),
            maximized: true,
        }))
    }

    /// Restore a window from maximized to its previous size.
    pub fn unmaximize_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Maximize {
            window_id: id.to_string(),
            maximized: false,
        }))
    }

    /// Minimize a window.
    pub fn minimize_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Minimize {
            window_id: id.to_string(),
            minimized: true,
        }))
    }

    /// Restore a minimized window.
    pub fn unminimize_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Minimize {
            window_id: id.to_string(),
            minimized: false,
        }))
    }

    /// Set the window display mode.
    pub fn set_window_mode(id: &str, mode: WindowMode) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetMode {
            window_id: id.to_string(),
            mode,
        }))
    }

    /// Toggle a window between maximized and restored states.
    pub fn toggle_maximize(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::ToggleMaximize(id.to_string())))
    }

    /// Toggle window decorations (title bar, borders).
    pub fn toggle_decorations(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::ToggleDecorations(
            id.to_string(),
        )))
    }

    /// Bring a window to the front and give it input focus.
    pub fn focus_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::FocusWindow(id.to_string())))
    }

    /// Set the window stacking level.
    pub fn set_window_level(id: &str, level: WindowLevel) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetLevel {
            window_id: id.to_string(),
            level,
        }))
    }

    /// Begin an interactive window drag.
    pub fn drag_window(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::DragWindow(id.to_string())))
    }

    /// Begin an interactive window resize from the given direction.
    pub fn drag_resize_window(id: &str, direction: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::DragResize {
            window_id: id.to_string(),
            direction: direction.to_string(),
        }))
    }

    /// Request user attention (taskbar flash or similar).
    pub fn request_attention(id: &str, urgency: Option<&str>) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::RequestAttention {
            window_id: id.to_string(),
            urgency: urgency.map(|s| s.to_string()),
        }))
    }

    /// Take a screenshot of a window. Result delivered as a system event.
    pub fn screenshot(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::Screenshot {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Set whether a window is user-resizable.
    pub fn set_resizable(id: &str, resizable: bool) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetResizable {
            window_id: id.to_string(),
            resizable,
        }))
    }

    /// Set the minimum window size.
    pub fn set_min_size(id: &str, width: f32, height: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetMinSize {
            window_id: id.to_string(),
            width,
            height,
        }))
    }

    /// Set the maximum window size.
    pub fn set_max_size(id: &str, width: f32, height: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetMaxSize {
            window_id: id.to_string(),
            width,
            height,
        }))
    }

    /// Allow mouse events to pass through a window.
    pub fn enable_mouse_passthrough(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::EnableMousePassthrough(
            id.to_string(),
        )))
    }

    /// Stop mouse events from passing through a window.
    pub fn disable_mouse_passthrough(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::DisableMousePassthrough(
            id.to_string(),
        )))
    }

    /// Show the native system menu for a window.
    pub fn show_system_menu(id: &str) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::ShowSystemMenu(id.to_string())))
    }

    /// Set the window icon from raw RGBA pixel data.
    pub fn set_icon(id: &str, rgba_data: Vec<u8>, width: u32, height: u32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetIcon {
            window_id: id.to_string(),
            data: rgba_data,
            width,
            height,
        }))
    }

    /// Set window resize increment constraints.
    pub fn set_resize_increments(id: &str, width: f32, height: f32) -> Self {
        Self::Renderer(RendererOp::Window(WindowOp::SetResizeIncrements {
            window_id: id.to_string(),
            width,
            height,
        }))
    }

    // -- Window queries --
    //
    // Results are delivered as `Event::System(SystemEvent)` with the
    // tag you provide, allowing correlation in your update function.

    /// Query the size of a window.
    pub fn window_size(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::GetSize {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query the position of a window.
    pub fn window_position(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::GetPosition {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query whether a window is maximized.
    pub fn is_maximized(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::IsMaximized {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query whether a window is minimized.
    pub fn is_minimized(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::IsMinimized {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query the display mode of a window.
    pub fn window_mode(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::GetMode {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query the scale factor of a window.
    pub fn scale_factor(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::GetScaleFactor {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query the monitor size for a window.
    pub fn monitor_size(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::MonitorSize {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    /// Query the raw platform window ID.
    pub fn raw_id(window_id: &str, tag: &str) -> Self {
        Self::Renderer(RendererOp::WindowQuery(WindowQuery::RawId {
            window_id: window_id.to_string(),
            tag: tag.to_string(),
        }))
    }

    // -- System --

    /// Enable or disable automatic window tabbing (macOS).
    pub fn allow_automatic_tabbing(enabled: bool) -> Self {
        Self::Renderer(RendererOp::SystemOp(SystemOp::AllowAutomaticTabbing(
            enabled,
        )))
    }

    /// Query the current OS theme (light/dark).
    pub fn system_theme(tag: &str) -> Self {
        Self::Renderer(RendererOp::SystemQuery(SystemQuery::GetTheme {
            tag: tag.to_string(),
        }))
    }

    /// Query system information (OS, renderer version, etc.).
    pub fn system_info(tag: &str) -> Self {
        Self::Renderer(RendererOp::SystemQuery(SystemQuery::GetInfo {
            tag: tag.to_string(),
        }))
    }

    // -- Images --

    /// Create an image from encoded bytes (PNG, JPEG, etc.).
    pub fn create_image(handle: &str, data: Vec<u8>) -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::Create {
            handle: handle.to_string(),
            data,
        }))
    }

    /// Create an image from raw RGBA pixel data.
    pub fn create_image_raw(handle: &str, width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::CreateRaw {
            handle: handle.to_string(),
            width,
            height,
            pixels,
        }))
    }

    /// Replace an existing image with new encoded bytes.
    pub fn update_image(handle: &str, data: Vec<u8>) -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::Update {
            handle: handle.to_string(),
            data,
        }))
    }

    /// Replace an existing image with new raw RGBA pixel data.
    pub fn update_image_raw(handle: &str, width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::UpdateRaw {
            handle: handle.to_string(),
            width,
            height,
            pixels,
        }))
    }

    /// Delete an image by handle.
    pub fn delete_image(handle: &str) -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::Delete(handle.to_string())))
    }

    /// List all loaded image handles.
    pub fn list_images(tag: &str) -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::List {
            tag: tag.to_string(),
        }))
    }

    /// Delete all loaded images.
    pub fn clear_images() -> Self {
        Self::Renderer(RendererOp::Image(ImageOp::Clear))
    }

    // -- Pane grid --

    /// Split a pane in a pane grid along the given axis.
    pub fn pane_split(target: &str, pane: &str, axis: &str, new_pane_id: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "pane_split".to_string(),
            value: serde_json::json!({
                "pane": pane,
                "axis": axis,
                "new_pane_id": new_pane_id,
            }),
        })
    }

    /// Close a pane in a pane grid.
    pub fn pane_close(target: &str, pane: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "pane_close".to_string(),
            value: serde_json::json!({"pane": pane}),
        })
    }

    /// Swap two panes in a pane grid.
    pub fn pane_swap(target: &str, a: &str, b: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "pane_swap".to_string(),
            value: serde_json::json!({"a": a, "b": b}),
        })
    }

    /// Maximize a pane in a pane grid.
    pub fn pane_maximize(target: &str, pane: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "pane_maximize".to_string(),
            value: serde_json::json!({"pane": pane}),
        })
    }

    /// Restore all panes in a pane grid from a maximized state.
    pub fn pane_restore(target: &str) -> Self {
        Self::Renderer(RendererOp::Command {
            id: target.to_string(),
            family: "pane_restore".to_string(),
            value: Value::Null,
        })
    }

    // -- Misc --

    /// Announce text to screen readers at the given politeness.
    ///
    /// `politeness`:
    /// - [`Live::Polite`] queues after any ongoing speech; correct
    ///   for status messages, toast feedback, and confirmations.
    /// - [`Live::Assertive`] interrupts ongoing speech; reserved
    ///   for urgent announcements the user must hear immediately.
    ///
    /// [`Live::Polite`]: plushie_core::types::a11y::Live::Polite
    /// [`Live::Assertive`]: plushie_core::types::a11y::Live::Assertive
    pub fn announce(text: &str, politeness: plushie_core::types::a11y::Live) -> Self {
        Self::Renderer(RendererOp::Announce {
            text: text.to_string(),
            politeness,
        })
    }

    /// Announce text politely to screen readers. Shorthand for
    /// [`Command::announce(text, Live::Polite)`](Command::announce).
    pub fn announce_text(text: &str) -> Self {
        Self::announce(text, plushie_core::types::a11y::Live::Polite)
    }

    /// Load a font from raw byte data.
    pub fn load_font(data: Vec<u8>) -> Self {
        Self::Renderer(RendererOp::LoadFont(data))
    }

    /// Request a hash of the current widget tree.
    pub fn tree_hash(tag: &str) -> Self {
        Self::Renderer(RendererOp::TreeHash {
            tag: tag.to_string(),
        })
    }

    /// Query which widget currently has keyboard focus.
    pub fn find_focused(tag: &str) -> Self {
        Self::Renderer(RendererOp::FindFocused {
            tag: tag.to_string(),
        })
    }

    /// Advance the animation frame to the given timestamp.
    pub fn advance_frame(timestamp: u64) -> Self {
        Self::Renderer(RendererOp::AdvanceFrame { timestamp })
    }

    // -- Effects --

    /// Open a file-open dialog.
    pub fn file_open(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::FileOpen(Default::default()),
        })
    }

    /// Open a file-open dialog with options.
    pub fn file_open_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::FileOpen(opts),
        })
    }

    /// Open a multi-file selection dialog.
    pub fn file_open_multiple(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::FileOpenMultiple(Default::default()),
        })
    }

    /// Open a multi-file selection dialog with options.
    pub fn file_open_multiple_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::FileOpenMultiple(opts),
        })
    }

    /// Open a file-save dialog.
    pub fn file_save(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::FileSave(Default::default()),
        })
    }

    /// Open a file-save dialog with options.
    pub fn file_save_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::FileSave(opts),
        })
    }

    /// Open a single-directory selection dialog.
    pub fn directory_select(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::DirectorySelect(Default::default()),
        })
    }

    /// Open a single-directory selection dialog with options.
    pub fn directory_select_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::DirectorySelect(opts),
        })
    }

    /// Open a multi-directory selection dialog.
    pub fn directory_select_multiple(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::DirectorySelectMultiple(Default::default()),
        })
    }

    /// Open a multi-directory selection dialog with options.
    pub fn directory_select_multiple_with(tag: &str, opts: FileDialogOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::DirectorySelectMultiple(opts),
        })
    }

    /// Read text from the system clipboard.
    pub fn clipboard_read(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardRead,
        })
    }

    /// Write text to the system clipboard.
    pub fn clipboard_write(tag: &str, text: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardWrite(text.to_string()),
        })
    }

    /// Read HTML content from the system clipboard.
    pub fn clipboard_read_html(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardReadHtml,
        })
    }

    /// Write HTML content to the system clipboard.
    pub fn clipboard_write_html(tag: &str, html: &str, alt_text: Option<&str>) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardWriteHtml {
                html: html.to_string(),
                alt_text: alt_text.map(|s| s.to_string()),
            },
        })
    }

    /// Clear the system clipboard.
    pub fn clipboard_clear(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardClear,
        })
    }

    /// Read text from the primary selection (X11/Wayland).
    pub fn clipboard_read_primary(tag: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardReadPrimary,
        })
    }

    /// Write text to the primary selection (X11/Wayland).
    pub fn clipboard_write_primary(tag: &str, text: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::ClipboardWritePrimary(text.to_string()),
        })
    }

    /// Show a desktop notification.
    pub fn notification(tag: &str, title: &str, body: &str) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::Notification {
                title: title.to_string(),
                body: body.to_string(),
                opts: Default::default(),
            },
        })
    }

    /// Show a desktop notification with options.
    pub fn notification_with(tag: &str, title: &str, body: &str, opts: NotificationOpts) -> Self {
        Self::Renderer(RendererOp::Effect {
            tag: tag.to_string(),
            timeout: None,
            request: EffectRequest::Notification {
                title: title.to_string(),
                body: body.to_string(),
                opts,
            },
        })
    }

    // -- Widget commands --

    /// Send a typed command to a widget.
    ///
    /// Uses a `#[derive(WidgetCommand)]` enum for type-safe command
    /// construction. The derive macro generates `to_wire()` which
    /// provides the family string and encoded value.
    ///
    /// ```ignore
    /// #[derive(WidgetCommand)]
    /// enum GaugeCommand {
    ///     SetValue(f32),
    ///     Reset,
    ///     SetRange { min: f32, max: f32 },
    /// }
    ///
    /// Command::widget("temp-gauge", GaugeCommand::SetValue(72.0))
    /// ```
    pub fn widget<C: plushie_core::WidgetCommandEncode>(id: &str, cmd: C) -> Self {
        let wc = plushie_core::ops::WidgetCommand::new(id, cmd);
        Self::Renderer(RendererOp::Command {
            id: wc.id,
            family: wc.family,
            value: wc.value,
        })
    }

    /// Send a command to a widget by ID with raw family and value.
    ///
    /// Low-level generic builder. Prefer `Command::widget()` with a
    /// typed command enum derived via `#[derive(WidgetCommand)]`.
    pub fn send(id: &str, family: &str, value: Value) -> Self {
        let wc = plushie_core::ops::WidgetCommand::raw(id, family, value);
        Self::Renderer(RendererOp::Command {
            id: wc.id,
            family: wc.family,
            value: wc.value,
        })
    }

    /// Apply a batch of widget commands atomically.
    ///
    /// Unlike [`Command::batch`], which dispatches each command
    /// independently, `widget_batch` buffers intermediate events so
    /// observers only see a single consistent state after all
    /// commands commit. Build items with
    /// [`WidgetCommand::new`](plushie_core::ops::WidgetCommand::new)
    /// for typed commands and
    /// [`WidgetCommand::raw`](plushie_core::ops::WidgetCommand::raw)
    /// for ad-hoc ones:
    ///
    /// ```ignore
    /// use plushie_core::ops::WidgetCommand;
    ///
    /// Command::widget_batch(vec![
    ///     WidgetCommand::new("pane", SelectPane(1)),
    ///     WidgetCommand::raw("footer", "refresh", Value::Null),
    /// ])
    /// ```
    pub fn widget_batch(cmds: impl IntoIterator<Item = plushie_core::ops::WidgetCommand>) -> Self {
        Self::Renderer(RendererOp::Commands(cmds.into_iter().collect()))
    }
}
