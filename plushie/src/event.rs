//! Events delivered to [`App::update`](crate::App::update).
//!
//! Every user interaction, system notification, and async result
//! arrives as an [`Event`]. Use pattern matching or the convenience
//! accessors (`as_widget`, `widget_match`, `as_key_press`, etc.)
//! to handle specific event types.

use serde_json::Value;

use crate::types::KeyModifiers;

// ---------------------------------------------------------------------------
// Top-level Event enum
// ---------------------------------------------------------------------------

/// An event delivered to the app's update function.
#[derive(Debug, Clone)]
pub enum Event {
    Widget(WidgetEvent),
    Key(KeyEvent),
    Window(WindowEvent),
    Timer(TimerEvent),
    Async(AsyncEvent),
    Stream(StreamEvent),
    Effect(EffectEvent),
    System(SystemEvent),
    WidgetCommandError(WidgetCommandError),
    Modifiers(ModifiersEvent),
    Ime(ImeEvent),
}

impl Event {
    /// Ergonomic typed matching for widget events.
    ///
    /// Returns a [`WidgetMatch`] variant with the widget ID and
    /// typed primary value. Ideal for simple update functions and
    /// [`Widget::handle_event`](crate::Widget::handle_event).
    ///
    /// ```ignore
    /// match event.widget_match() {
    ///     Some(Click("inc")) => model.count += 1,
    ///     Some(Input("name", text)) => model.name = text.to_string(),
    ///     Some(Toggle("dark", on)) => model.dark_mode = on,
    ///     _ => {}
    /// }
    /// ```
    pub fn widget_match(&self) -> Option<WidgetMatch<'_>> {
        let w = self.as_widget()?;
        Some(w.to_match())
    }

    /// Access the inner [`WidgetEvent`] if this is a widget event.
    pub fn as_widget(&self) -> Option<&WidgetEvent> {
        match self {
            Event::Widget(w) => Some(w),
            _ => None,
        }
    }

    /// Access the inner [`KeyEvent`] if this is a key press.
    pub fn as_key_press(&self) -> Option<&KeyEvent> {
        match self {
            Event::Key(k) if k.event_type == KeyEventType::Press => Some(k),
            _ => None,
        }
    }

    /// Access the inner [`KeyEvent`] if this is a key release.
    pub fn as_key_release(&self) -> Option<&KeyEvent> {
        match self {
            Event::Key(k) if k.event_type == KeyEventType::Release => Some(k),
            _ => None,
        }
    }

    /// Access the inner [`WindowEvent`].
    pub fn as_window(&self) -> Option<&WindowEvent> {
        match self {
            Event::Window(w) => Some(w),
            _ => None,
        }
    }

    /// Access the inner [`TimerEvent`].
    pub fn as_timer(&self) -> Option<&TimerEvent> {
        match self {
            Event::Timer(t) => Some(t),
            _ => None,
        }
    }

    /// Access the inner [`AsyncEvent`].
    pub fn as_async(&self) -> Option<&AsyncEvent> {
        match self {
            Event::Async(a) => Some(a),
            _ => None,
        }
    }

    /// Access the inner [`StreamEvent`].
    pub fn as_stream(&self) -> Option<&StreamEvent> {
        match self {
            Event::Stream(s) => Some(s),
            _ => None,
        }
    }

    /// Access the inner [`EffectEvent`].
    pub fn as_effect(&self) -> Option<&EffectEvent> {
        match self {
            Event::Effect(e) => Some(e),
            _ => None,
        }
    }

    /// Access the inner [`SystemEvent`].
    pub fn as_system(&self) -> Option<&SystemEvent> {
        match self {
            Event::System(s) => Some(s),
            _ => None,
        }
    }

    /// Access the widget event's scope chain (reversed ancestor path).
    pub fn scope(&self) -> Option<&[String]> {
        self.as_widget().map(|w| w.scope.as_slice())
    }
}

// ---------------------------------------------------------------------------
// EventType
// ---------------------------------------------------------------------------

/// The kind of widget interaction that occurred.
///
/// `Copy` so it can be used directly in match arms without borrowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Click,
    DoubleClick,
    Input,
    Submit,
    Paste,
    Toggle,
    Select,
    Slide,
    SlideRelease,
    Press,
    Release,
    Move,
    Scroll,
    Scrolled,
    Enter,
    Exit,
    Resize,
    Focused,
    Blurred,
    Drag,
    DragEnd,
    KeyPress,
    KeyRelease,
    Sort,
    Status,
    OptionHovered,
    PaneFocusCycle,
    PaneResized,
    PaneDragged,
    PaneClicked,
    TransitionComplete,
    Open,
    Close,
    KeyBinding,
    /// A custom event family from a native widget.
    Other(u64),
}

// ---------------------------------------------------------------------------
// WidgetEvent
// ---------------------------------------------------------------------------

/// An event from a widget interaction (click, input, toggle, etc.).
#[derive(Debug, Clone)]
pub struct WidgetEvent {
    /// What kind of interaction occurred.
    pub event_type: EventType,
    /// The widget's local ID (without scope prefix).
    pub id: String,
    /// The window this widget belongs to.
    pub window_id: String,
    /// Reversed ancestor scope chain. First element is the
    /// immediate parent's ID.
    pub scope: Vec<String>,
    /// The event's primary value (text for Input, bool for Toggle, etc.).
    pub value: Value,
}

impl WidgetEvent {
    /// Extract the value as a string.
    pub fn value_string(&self) -> Option<String> {
        self.value.as_str().map(|s| s.to_string())
    }

    /// Extract the value as a bool.
    pub fn value_bool(&self) -> Option<bool> {
        self.value.as_bool()
    }

    /// Extract the value as an f64.
    pub fn value_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }

    /// Reconstruct the full scoped path (e.g. "form/save").
    pub fn target(&self) -> String {
        if self.scope.is_empty() {
            self.id.clone()
        } else {
            let mut parts: Vec<&str> = self.scope.iter().rev().map(|s| s.as_str()).collect();
            parts.push(&self.id);
            parts.join("/")
        }
    }

    /// Convert to a [`WidgetMatch`] for ergonomic pattern matching.
    fn to_match(&self) -> WidgetMatch<'_> {
        use EventType::*;
        match self.event_type {
            Click => WidgetMatch::Click(&self.id),
            DoubleClick => WidgetMatch::DoubleClick(&self.id),
            Input => WidgetMatch::Input(
                &self.id,
                self.value.as_str().unwrap_or_default(),
            ),
            Submit => WidgetMatch::Submit(
                &self.id,
                self.value.as_str().unwrap_or_default(),
            ),
            Toggle => WidgetMatch::Toggle(
                &self.id,
                self.value.as_bool().unwrap_or_default(),
            ),
            Select => WidgetMatch::Select(
                &self.id,
                self.value.as_str().unwrap_or_default(),
            ),
            Slide => WidgetMatch::Slide(
                &self.id,
                self.value.as_f64().unwrap_or_default(),
            ),
            SlideRelease => WidgetMatch::SlideRelease(
                &self.id,
                self.value.as_f64().unwrap_or_default(),
            ),
            Paste => WidgetMatch::Paste(
                &self.id,
                self.value.as_str().unwrap_or_default(),
            ),
            Press => WidgetMatch::Press(&self.id),
            Release => WidgetMatch::Release(&self.id),
            Enter => WidgetMatch::Enter(&self.id),
            Exit => WidgetMatch::Exit(&self.id),
            Drag => WidgetMatch::Drag(&self.id),
            DragEnd => WidgetMatch::DragEnd(&self.id),
            Focused => WidgetMatch::Focused(&self.id),
            Blurred => WidgetMatch::Blurred(&self.id),
            Resize => WidgetMatch::Resize(&self.id),
            _ => WidgetMatch::Other(&self.id, self.event_type),
        }
    }
}

// ---------------------------------------------------------------------------
// WidgetMatch
// ---------------------------------------------------------------------------

/// Typed pattern matching for widget events.
///
/// Each variant carries the widget ID and the typed primary value.
/// Use with [`Event::widget_match`]:
///
/// ```ignore
/// use plushie::prelude::*;
/// use WidgetMatch::*;
///
/// match event.widget_match() {
///     Some(Click("save")) => { /* handle save */ }
///     Some(Input("name", text)) => model.name = text.to_string(),
///     Some(Toggle("dark", on)) => model.dark_mode = on,
///     Some(Slide("volume", vol)) => model.volume = vol,
///     _ => {}
/// }
/// ```
#[derive(Debug)]
pub enum WidgetMatch<'a> {
    Click(&'a str),
    DoubleClick(&'a str),
    Input(&'a str, &'a str),
    Submit(&'a str, &'a str),
    Toggle(&'a str, bool),
    Select(&'a str, &'a str),
    Slide(&'a str, f64),
    SlideRelease(&'a str, f64),
    Paste(&'a str, &'a str),
    Press(&'a str),
    Release(&'a str),
    Enter(&'a str),
    Exit(&'a str),
    Drag(&'a str),
    DragEnd(&'a str),
    Focused(&'a str),
    Blurred(&'a str),
    Resize(&'a str),
    Timer(&'a str),
    /// Catch-all for event types not covered by named variants.
    Other(&'a str, EventType),
}

// ---------------------------------------------------------------------------
// KeyEvent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    Press,
    Release,
}

/// A keyboard event.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub event_type: KeyEventType,
    pub key: String,
    pub modified_key: Option<String>,
    pub physical_key: Option<String>,
    pub location: KeyLocation,
    pub modifiers: KeyModifiers,
    pub text: Option<String>,
    pub repeat: bool,
    pub captured: bool,
    pub window_id: Option<String>,
}

impl KeyEvent {
    pub fn is_press(&self) -> bool { self.event_type == KeyEventType::Press }
    pub fn is_release(&self) -> bool { self.event_type == KeyEventType::Release }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyLocation {
    #[default]
    Standard,
    Left,
    Right,
    Numpad,
}

// ---------------------------------------------------------------------------
// WindowEvent
// ---------------------------------------------------------------------------

/// A window lifecycle event.
#[derive(Debug, Clone)]
pub struct WindowEvent {
    pub event_type: WindowEventType,
    pub window_id: String,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub position: Option<(f32, f32)>,
    pub path: Option<String>,
    pub scale_factor: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowEventType {
    Opened,
    Closed,
    CloseRequested,
    Moved,
    Resized,
    Focused,
    Unfocused,
    Rescaled,
    FileHovered,
    FileDropped,
    FilesHoveredLeft,
}

// ---------------------------------------------------------------------------
// TimerEvent
// ---------------------------------------------------------------------------

/// A timer tick from a [`Subscription::every`](crate::Subscription) subscription.
#[derive(Debug, Clone)]
pub struct TimerEvent {
    pub tag: String,
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// AsyncEvent
// ---------------------------------------------------------------------------

/// The result of an async task started with [`Command::async_task`](crate::Command).
#[derive(Debug, Clone)]
pub struct AsyncEvent {
    pub tag: String,
    pub result: Result<Value, Value>,
}

// ---------------------------------------------------------------------------
// StreamEvent
// ---------------------------------------------------------------------------

/// An intermediate value from a streaming task.
#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub tag: String,
    pub value: Value,
}

// ---------------------------------------------------------------------------
// EffectEvent
// ---------------------------------------------------------------------------

/// The result of a platform effect (file dialog, clipboard, etc.).
#[derive(Debug, Clone)]
pub struct EffectEvent {
    pub tag: String,
    pub result: EffectResult,
}

/// The outcome of a platform effect.
#[derive(Debug, Clone)]
pub enum EffectResult {
    Ok(Value),
    Cancelled,
    Error(Value),
}

// ---------------------------------------------------------------------------
// SystemEvent
// ---------------------------------------------------------------------------

/// A system-level event (theme change, window query result, etc.).
#[derive(Debug, Clone)]
pub struct SystemEvent {
    pub event_type: SystemEventType,
    pub tag: Option<String>,
    pub value: Option<Value>,
    pub id: Option<String>,
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEventType {
    SystemInfo,
    SystemTheme,
    AnimationFrame,
    ThemeChanged,
    AllWindowsClosed,
    ImageList,
    TreeHash,
    FindFocused,
    Announce,
    Diagnostic,
    RecoveryFailed,
    Error,
}

// ---------------------------------------------------------------------------
// WidgetCommandError
// ---------------------------------------------------------------------------

/// Error from a native widget command.
#[derive(Debug, Clone)]
pub struct WidgetCommandError {
    pub reason: String,
    pub node_id: Option<String>,
    pub op: Option<String>,
    pub widget_type: Option<String>,
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// ModifiersEvent
// ---------------------------------------------------------------------------

/// Keyboard modifier state changed.
#[derive(Debug, Clone)]
pub struct ModifiersEvent {
    pub modifiers: KeyModifiers,
    pub captured: bool,
    pub window_id: Option<String>,
}

// ---------------------------------------------------------------------------
// ImeEvent
// ---------------------------------------------------------------------------

/// Input Method Editor event.
#[derive(Debug, Clone)]
pub struct ImeEvent {
    pub event_type: ImeEventType,
    pub id: Option<String>,
    pub scope: Vec<String>,
    pub text: Option<String>,
    pub cursor: Option<(usize, usize)>,
    pub captured: bool,
    pub window_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeEventType {
    Opened,
    Preedit,
    Commit,
    Closed,
}
