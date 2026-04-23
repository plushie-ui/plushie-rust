//! Events delivered to [`App::update`](crate::App::update).
//!
//! Every user interaction, system notification, and async result
//! arrives as an [`Event`]. Use pattern matching or the convenience
//! accessors (`as_widget`, `widget_match`, `as_key_press`, etc.)
//! to handle specific event types.

use plushie_core::key::{MouseButton, PointerKind};
use serde_json::Value;

// Re-export typed event data from plushie-core so SDK users can
// access them via `plushie::event::PointerPress` etc.
pub use plushie_core::pointer::{
    KeyData, PointerBoundary, PointerDrag, PointerMove, PointerPress, PointerRelease,
    PointerScroll, ResizeDimensions, ScrollPosition,
};

use crate::types::KeyModifiers;

fn get_captured(obj: Option<&serde_json::Map<String, Value>>) -> bool {
    obj.and_then(|o| o.get("captured"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Parse pointer press/release data from an event value.
fn parse_pointer_press(value: &Value) -> PointerPress {
    let obj = value.as_object();
    let get_f32 = |k: &str| -> f32 {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    let get_str = |k: &str| -> &str {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    PointerPress {
        x: get_f32("x"),
        y: get_f32("y"),
        button: MouseButton::from(
            obj.and_then(|o| o.get("button"))
                .and_then(|v| v.as_str())
                .unwrap_or("left"),
        ),
        pointer: PointerKind::from(get_str("pointer")),
        finger: obj.and_then(|o| o.get("finger")).and_then(|v| v.as_u64()),
        modifiers: parse_modifiers(obj),
        captured: get_captured(obj),
    }
}

fn parse_pointer_release(value: &Value) -> PointerRelease {
    let obj = value.as_object();
    let p = parse_pointer_press(value);
    PointerRelease {
        x: p.x,
        y: p.y,
        button: p.button,
        pointer: p.pointer,
        finger: p.finger,
        modifiers: p.modifiers,
        captured: p.captured,
        lost: obj.and_then(|o| o.get("lost")).and_then(|v| v.as_bool()),
    }
}

fn parse_pointer_move(value: &Value) -> PointerMove {
    let obj = value.as_object();
    let get_f32 = |k: &str| -> f32 {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    let get_str = |k: &str| -> &str {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    PointerMove {
        x: get_f32("x"),
        y: get_f32("y"),
        pointer: PointerKind::from(get_str("pointer")),
        finger: obj.and_then(|o| o.get("finger")).and_then(|v| v.as_u64()),
        modifiers: parse_modifiers(obj),
        captured: get_captured(obj),
    }
}

fn parse_pointer_scroll(value: &Value) -> PointerScroll {
    let obj = value.as_object();
    let get_f32 = |k: &str| -> f32 {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    let get_str = |k: &str| -> &str {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    PointerScroll {
        x: get_f32("x"),
        y: get_f32("y"),
        delta_x: get_f32("delta_x"),
        delta_y: get_f32("delta_y"),
        pointer: PointerKind::from(get_str("pointer")),
        modifiers: parse_modifiers(obj),
        captured: get_captured(obj),
    }
}

fn parse_pointer_drag(value: &Value) -> PointerDrag {
    let obj = value.as_object();
    let get_f32 = |k: &str| -> f32 {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    let get_str = |k: &str| -> &str {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or("")
    };
    PointerDrag {
        x: get_f32("x"),
        y: get_f32("y"),
        pointer: PointerKind::from(get_str("pointer")),
        modifiers: parse_modifiers(obj),
        captured: get_captured(obj),
    }
}

fn parse_pointer_boundary(value: &Value) -> PointerBoundary {
    let obj = value.as_object();
    let get_opt_f32 = |k: &str| -> Option<f32> {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .map(|n| n as f32)
    };
    PointerBoundary {
        x: get_opt_f32("x"),
        y: get_opt_f32("y"),
        captured: get_captured(obj),
    }
}

fn parse_scroll_position(value: &Value) -> ScrollPosition {
    let obj = value.as_object();
    let get_f32 = |k: &str| -> f32 {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    ScrollPosition {
        absolute_x: get_f32("absolute_x"),
        absolute_y: get_f32("absolute_y"),
        relative_x: get_f32("relative_x"),
        relative_y: get_f32("relative_y"),
        bounds_width: get_f32("bounds_width"),
        bounds_height: get_f32("bounds_height"),
        content_width: get_f32("content_width"),
        content_height: get_f32("content_height"),
    }
}

fn parse_key_data(value: &Value) -> KeyData {
    let obj = value.as_object();
    let get_str = |k: &str| -> Option<&str> { obj.and_then(|o| o.get(k)).and_then(|v| v.as_str()) };
    let get_key =
        |k: &str| -> Option<plushie_core::Key> { get_str(k).map(plushie_core::Key::from) };
    KeyData {
        key: get_key("key").unwrap_or(plushie_core::Key::Named(String::new())),
        modified_key: get_key("modified_key"),
        physical_key: get_key("physical_key"),
        modifiers: parse_modifiers(obj),
        text: get_str("text").map(String::from),
        repeat: obj
            .and_then(|o| o.get("repeat"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

fn parse_resize(value: &Value) -> ResizeDimensions {
    let obj = value.as_object();
    let get_f32 = |k: &str| -> f32 {
        obj.and_then(|o| o.get(k))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32
    };
    ResizeDimensions {
        width: get_f32("width"),
        height: get_f32("height"),
    }
}

/// Extract the `link` field from a link_click event payload.
///
/// The renderer emits `{"link": "https://..."}` for link-capable widgets
/// (rich_text, markdown, future link emitters). Missing or malformed
/// payloads log a warning and yield an empty string.
fn expect_link<'a>(value: &'a Value, id: &str) -> &'a str {
    match value.get("link").and_then(Value::as_str) {
        Some(link) => link,
        None => {
            log::warn!(
                "event value type mismatch: link_click event for \"{id}\" expected {{\"link\": ...}}, got {value}"
            );
            ""
        }
    }
}

/// Extract a string value, logging a warning if the type is wrong.
fn expect_str<'a>(value: &'a Value, family: &str, id: &str) -> &'a str {
    match value.as_str() {
        Some(s) => s,
        None => {
            log::warn!(
                "event value type mismatch: {family} event for \"{id}\" expected string, got {value}"
            );
            ""
        }
    }
}

/// Extract a bool value, logging a warning if the type is wrong.
fn expect_bool(value: &Value, family: &str, id: &str) -> bool {
    match value.as_bool() {
        Some(b) => b,
        None => {
            log::warn!(
                "event value type mismatch: {family} event for \"{id}\" expected bool, got {value}"
            );
            false
        }
    }
}

/// Extract an f64 value, logging a warning if the type is wrong.
fn expect_f64(value: &Value, family: &str, id: &str) -> f64 {
    match value.as_f64() {
        Some(n) => n,
        None => {
            log::warn!(
                "event value type mismatch: {family} event for \"{id}\" expected number, got {value}"
            );
            0.0
        }
    }
}

fn parse_modifiers(obj: Option<&serde_json::Map<String, Value>>) -> KeyModifiers {
    let mods = obj.and_then(|o| o.get("modifiers"));
    KeyModifiers {
        shift: mods
            .and_then(|m| m.get("shift"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ctrl: mods
            .and_then(|m| m.get("ctrl"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        alt: mods
            .and_then(|m| m.get("alt"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        logo: mods
            .and_then(|m| m.get("logo"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        command: mods
            .and_then(|m| m.get("command"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
    }
}

// ---------------------------------------------------------------------------
// Top-level Event enum
// ---------------------------------------------------------------------------

/// An event delivered to the app's update function.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Event {
    /// Widget-originated event (click, input, change, etc.).
    Widget(WidgetEvent),
    /// Keyboard press or release.
    Key(KeyEvent),
    /// Window lifecycle event (resize, move, close request, focus).
    Window(WindowEvent),
    /// Timer tick from a `Subscription::every` subscription.
    Timer(TimerEvent),
    /// Result from a `Command::task` future.
    Async(AsyncEvent),
    /// Item emitted by a `Subscription::stream`.
    Stream(StreamEvent),
    /// Platform-effect result (file dialog, clipboard, notification).
    Effect(EffectEvent),
    /// System-level event (theme change, etc.).
    System(SystemEvent),
    /// Error emitted by a failed command.
    CommandError(CommandError),
    /// Modifier-state change (Shift/Ctrl/Alt/Super).
    Modifiers(ModifiersEvent),
    /// Input-method editor (IME) composition event.
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
        match self {
            Event::Widget(w) => Some(w.to_match()),
            Event::Timer(t) => Some(WidgetMatch::Timer(&t.tag)),
            _ => None,
        }
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
        self.as_widget().map(|w| w.scoped_id.scope.as_slice())
    }
}

// ---------------------------------------------------------------------------
// EventType (re-exported from plushie-core)
// ---------------------------------------------------------------------------

pub use plushie_core::EventType;

// ---------------------------------------------------------------------------
// WidgetEvent
// ---------------------------------------------------------------------------

/// An event from a widget interaction (click, input, toggle, etc.).
#[derive(Debug, Clone)]
pub struct WidgetEvent {
    /// What kind of interaction occurred.
    pub event_type: EventType,
    /// The widget's identity: local ID, scope chain, window, and
    /// canonical wire ID. Access the local name via `scoped_id.id`.
    pub scoped_id: plushie_core::ScopedId,
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

    /// Reconstruct the full scoped path (e.g. "main#form/save").
    pub fn target(&self) -> &str {
        &self.scoped_id.full
    }

    /// Convert to a [`WidgetMatch`] for ergonomic pattern matching.
    fn to_match(&self) -> WidgetMatch<'_> {
        let id = &self.scoped_id.id;
        use EventType::*;
        match &self.event_type {
            Click => WidgetMatch::Click(id),
            DoubleClick => WidgetMatch::DoubleClick(id, parse_pointer_press(&self.value)),
            Input => WidgetMatch::Input(id, expect_str(&self.value, "input", id)),
            Submit => WidgetMatch::Submit(id, expect_str(&self.value, "submit", id)),
            Toggle => WidgetMatch::Toggle(id, expect_bool(&self.value, "toggle", id)),
            Select => WidgetMatch::Select(id, expect_str(&self.value, "select", id)),
            Slide => WidgetMatch::Slide(id, expect_f64(&self.value, "slide", id)),
            SlideRelease => {
                WidgetMatch::SlideRelease(id, expect_f64(&self.value, "slide_release", id))
            }
            Paste => WidgetMatch::Paste(id, expect_str(&self.value, "paste", id)),
            Press => WidgetMatch::Press(id, parse_pointer_press(&self.value)),
            Release => WidgetMatch::Release(id, parse_pointer_release(&self.value)),
            Move => WidgetMatch::Move(id, parse_pointer_move(&self.value)),
            Scroll => WidgetMatch::Scroll(id, parse_pointer_scroll(&self.value)),
            Scrolled => WidgetMatch::Scrolled(id, parse_scroll_position(&self.value)),
            Enter => WidgetMatch::Enter(id, parse_pointer_boundary(&self.value)),
            Exit => WidgetMatch::Exit(id, parse_pointer_boundary(&self.value)),
            Drag => WidgetMatch::Drag(id, parse_pointer_drag(&self.value)),
            DragEnd => WidgetMatch::DragEnd(id, parse_pointer_drag(&self.value)),
            Focused => WidgetMatch::Focused(id),
            Blurred => WidgetMatch::Blurred(id),
            Resize => WidgetMatch::Resize(id, parse_resize(&self.value)),
            KeyPress => WidgetMatch::KeyPress(id, parse_key_data(&self.value)),
            KeyRelease => WidgetMatch::KeyRelease(id, parse_key_data(&self.value)),
            Sort => WidgetMatch::Sort(id, expect_str(&self.value, "sort", id)),
            Status => WidgetMatch::Status(id, &self.value),
            OptionHovered => WidgetMatch::OptionHovered(id, &self.value),
            Open => WidgetMatch::Open(id),
            Close => WidgetMatch::Close(id),
            KeyBinding => WidgetMatch::KeyBinding(id, &self.value),
            LinkClick => WidgetMatch::LinkClicked(id, expect_link(&self.value, id)),
            TransitionComplete => WidgetMatch::TransitionComplete(id),
            PaneResized => WidgetMatch::PaneResized(id, &self.value),
            PaneDragged => WidgetMatch::PaneDragged(id, &self.value),
            PaneClicked => WidgetMatch::PaneClicked(id, &self.value),
            PaneFocusCycle => WidgetMatch::PaneFocusCycle(id),
            Custom(family) => WidgetMatch::Custom {
                id,
                family,
                value: &self.value,
            },
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
#[non_exhaustive]
pub enum WidgetMatch<'a> {
    /// Primary-button click on the identified widget.
    Click(&'a str),
    /// Primary-button double click.
    DoubleClick(&'a str, PointerPress),
    /// Text input change (full current value).
    Input(&'a str, &'a str),
    /// Submit (Enter pressed on text input).
    Submit(&'a str, &'a str),
    /// Two-state toggle (checkbox, switch) change.
    Toggle(&'a str, bool),
    /// Single-select choice (pick list, radio) change.
    Select(&'a str, &'a str),
    /// Slider drag (continuous value).
    Slide(&'a str, f64),
    /// Slider drag released (committed value).
    SlideRelease(&'a str, f64),
    /// Paste into a text-bearing widget.
    Paste(&'a str, &'a str),
    /// Pointer press on the widget.
    Press(&'a str, PointerPress),
    /// Pointer release on the widget.
    Release(&'a str, PointerRelease),
    /// Pointer movement over the widget.
    Move(&'a str, PointerMove),
    /// Pointer scroll on the widget.
    Scroll(&'a str, PointerScroll),
    /// Scrollable content position changed.
    Scrolled(&'a str, ScrollPosition),
    /// Pointer entered the widget bounds.
    Enter(&'a str, PointerBoundary),
    /// Pointer left the widget bounds.
    Exit(&'a str, PointerBoundary),
    /// Drag in progress.
    Drag(&'a str, PointerDrag),
    /// Drag finished.
    DragEnd(&'a str, PointerDrag),
    /// Widget gained keyboard focus.
    Focused(&'a str),
    /// Widget lost keyboard focus.
    Blurred(&'a str),
    /// Widget resized (for widgets that report dimensions).
    Resize(&'a str, ResizeDimensions),
    /// Key pressed while this widget had focus.
    KeyPress(&'a str, KeyData),
    /// Key released while this widget had focus.
    KeyRelease(&'a str, KeyData),
    /// Sort request from a sortable widget (typically a table).
    Sort(&'a str, &'a str),
    /// Status change carrying a widget-specific payload.
    Status(&'a str, &'a Value),
    /// Option hovered (pick lists, menus).
    OptionHovered(&'a str, &'a Value),
    /// Widget opened (accordion, menu, overlay).
    Open(&'a str),
    /// Widget closed.
    Close(&'a str),
    /// Keyboard shortcut binding triggered.
    KeyBinding(&'a str, &'a Value),
    /// Animation or transition finished.
    TransitionComplete(&'a str),
    /// Pane grid pane resize.
    PaneResized(&'a str, &'a Value),
    /// Pane grid pane drag.
    PaneDragged(&'a str, &'a Value),
    /// Pane grid pane click.
    PaneClicked(&'a str, &'a Value),
    /// Pane grid focus cycled to the pane's region.
    PaneFocusCycle(&'a str),
    /// Hyperlink in a link-capable widget (rich_text, markdown) was clicked.
    /// Carries the widget id and the link URL extracted from the event payload.
    LinkClicked(&'a str, &'a str),
    /// Timer tick (from `Subscription::every`).
    Timer(&'a str),
    /// Custom widget event. `family` is the full family string
    /// (e.g., "star_rating:select"). Match with:
    /// ```ignore
    /// Some(Custom { family: "star_rating:select", value, .. }) => { ... }
    /// ```
    Custom {
        /// Widget node ID that emitted the event.
        id: &'a str,
        /// Full event family string (e.g. `"star_rating:select"`).
        family: &'a str,
        /// Raw payload JSON for the custom event.
        value: &'a Value,
    },
}

/// Convert an event family string to an [`EventType`].
///
/// Delegates to [`EventType::from_family`] (the single source of
/// truth in plushie-core).
pub fn family_to_event_type(family: &str) -> EventType {
    EventType::from_family(family)
}

// ---------------------------------------------------------------------------
// KeyEvent
// ---------------------------------------------------------------------------

/// Key event phase reported by [`KeyEvent::event_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyEventType {
    /// Key was pressed down.
    Press,
    /// Key was released.
    Release,
}

/// A keyboard event from a subscription (global key handling).
///
/// Uses the typed [`Key`](plushie_core::Key) enum for key identity.
/// For widget-level key events, see [`KeyData`] (carried by
/// [`WidgetMatch::KeyPress`] / [`WidgetMatch::KeyRelease`]).
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// Whether this is a press or release event.
    pub event_type: KeyEventType,
    /// The logical key (typed enum with aliases).
    pub key: plushie_core::Key,
    /// The key after applying modifiers (e.g., Shift+a produces 'A').
    pub modified_key: Option<plushie_core::Key>,
    /// The physical key code (layout-independent).
    pub physical_key: Option<plushie_core::Key>,
    /// Which part of the keyboard the key is on.
    pub location: KeyLocation,
    /// Active modifier keys at the time of the event.
    pub modifiers: KeyModifiers,
    /// Text generated by this key press (if any).
    pub text: Option<String>,
    /// Whether this is an auto-repeat event from holding the key.
    pub repeat: bool,
    /// Whether a widget captured this event (preventing propagation).
    pub captured: bool,
    /// The window that had focus when the key was pressed.
    pub window_id: Option<String>,
}

impl KeyEvent {
    /// Returns true if this event represents a key press.
    pub fn is_press(&self) -> bool {
        self.event_type == KeyEventType::Press
    }
    /// Returns true if this event represents a key release.
    pub fn is_release(&self) -> bool {
        self.event_type == KeyEventType::Release
    }
}

/// Physical location of a key on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum KeyLocation {
    /// Default location (not left/right/numpad specific).
    #[default]
    Standard,
    /// Left side (e.g. Left Shift, Left Ctrl).
    Left,
    /// Right side (e.g. Right Shift, Right Ctrl).
    Right,
    /// Numeric keypad.
    Numpad,
}

// ---------------------------------------------------------------------------
// WindowEvent
// ---------------------------------------------------------------------------

/// A window lifecycle event.
#[derive(Debug, Clone)]
pub struct WindowEvent {
    /// The kind of window event that occurred.
    pub event_type: WindowEventType,
    /// The window this event applies to.
    pub window_id: String,
    /// X coordinate (for move events).
    pub x: Option<f32>,
    /// Y coordinate (for move events).
    pub y: Option<f32>,
    /// Window width (for resize events).
    pub width: Option<f32>,
    /// Window height (for resize events).
    pub height: Option<f32>,
    /// Window position as (x, y) (for move events).
    pub position: Option<(f32, f32)>,
    /// File path (for file drop events).
    pub path: Option<String>,
    /// DPI scale factor (for rescale events).
    pub scale_factor: Option<f32>,
}

/// Kind of window lifecycle event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowEventType {
    /// Window was just created.
    Opened,
    /// Window has been closed.
    Closed,
    /// User requested to close the window (not yet closed).
    CloseRequested,
    /// Window moved on the screen.
    Moved,
    /// Window resized.
    Resized,
    /// Window gained keyboard focus.
    Focused,
    /// Window lost keyboard focus.
    Unfocused,
    /// DPI scale factor changed.
    Rescaled,
    /// A file is being hovered over the window (pre-drop).
    FileHovered,
    /// A file was dropped on the window.
    FileDropped,
    /// A hovered file was removed without dropping.
    FilesHoveredLeft,
}

// ---------------------------------------------------------------------------
// TimerEvent
// ---------------------------------------------------------------------------

/// A timer tick from a [`Subscription::every`](crate::Subscription) subscription.
#[derive(Debug, Clone)]
pub struct TimerEvent {
    /// Subscription tag that identifies which timer fired.
    pub tag: String,
    /// Milliseconds since the Unix epoch at the time of the tick.
    pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// AsyncEvent
// ---------------------------------------------------------------------------

/// The result of an async task started with [`Command::task`](crate::Command).
#[derive(Debug, Clone)]
pub struct AsyncEvent {
    /// Task tag used to correlate the result with the originating command.
    pub tag: String,
    /// Success or error payload emitted by the task.
    pub result: Result<Value, Value>,
}

// ---------------------------------------------------------------------------
// StreamEvent
// ---------------------------------------------------------------------------

/// An intermediate value from a streaming task.
#[derive(Debug, Clone)]
pub struct StreamEvent {
    /// Stream tag used to correlate the value with the originating subscription.
    pub tag: String,
    /// Emitted payload.
    pub value: Value,
}

// ---------------------------------------------------------------------------
// EffectEvent
// ---------------------------------------------------------------------------

/// The result of a platform effect (file dialog, clipboard, etc.).
///
/// # Timeouts
///
/// Effects without an explicit `timeout` on the issuing
/// [`Command`](crate::command::Command) fall back to a
/// per-kind default: 120 s for file dialogs, 5 s for clipboard and
/// notifications, 30 s for unknown kinds. See
/// [`plushie::runner::effect_tracker::default_timeout`](crate::runner::effect_tracker::default_timeout)
/// (internal) or pass an explicit `Duration` on the `Effect` command
/// to override.
#[derive(Debug, Clone)]
pub struct EffectEvent {
    /// Effect tag used to correlate the result with the originating command.
    pub tag: String,
    /// Structured outcome of the effect.
    pub result: EffectResult,
}

/// The outcome of a platform effect.
///
/// Typed variants provide structured access to effect results without
/// requiring callers to parse raw JSON. The `parse()` constructor
/// converts the wire-format (kind, status, value) triple into the
/// appropriate variant.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum EffectResult {
    /// A file was selected by the user.
    FileOpened {
        /// Absolute path of the selected file.
        path: String,
    },
    /// Multiple files were selected.
    FilesOpened {
        /// Absolute paths of all selected files.
        paths: Vec<String>,
    },
    /// A file save path was chosen.
    FileSaved {
        /// Absolute path chosen by the user.
        path: String,
    },
    /// A directory was selected.
    DirectorySelected {
        /// Absolute path of the selected directory.
        path: String,
    },
    /// Multiple directories were selected.
    DirectoriesSelected {
        /// Absolute paths of all selected directories.
        paths: Vec<String>,
    },
    /// Clipboard text was read.
    ClipboardText {
        /// Clipboard contents as a UTF-8 string.
        text: String,
    },
    /// Clipboard HTML was read.
    ClipboardHtml {
        /// Clipboard contents as an HTML string.
        html: String,
    },
    /// Clipboard write succeeded.
    ClipboardWritten,
    /// Clipboard was cleared.
    ClipboardCleared,
    /// Notification was shown.
    NotificationShown,
    /// The user cancelled the operation (e.g. dismissed a dialog).
    Cancelled,
    /// The effect timed out.
    Timeout,
    /// A platform error occurred.
    Error(String),
    /// The renderer restarted while the effect was pending.
    RendererRestarted,
    /// The effect kind is not supported by this backend.
    Unsupported,
    /// The runner is shutting down and could not complete the effect.
    ///
    /// Delivered by both direct and wire runners when they drain
    /// pending effects during teardown. Apps should treat this as a
    /// "best-effort abort" and avoid retrying.
    Shutdown,
    /// Unknown or untyped result (fallback for forward compatibility).
    Other(Value),
    /// The tracker no longer has an entry for this effect's wire ID.
    ///
    /// Occurs when an effect response arrives after the renderer was
    /// restarted (and the tracker's in-flight state was flushed), or
    /// for any other path where the typed `tag` -> `wire_id` mapping
    /// was lost. Distinguished from [`Self::Other`] so apps can tell
    /// "a legitimate typed result for an unknown kind" apart from
    /// "the tracker has no memory of this effect at all".
    Orphaned(Value),
}

impl EffectResult {
    /// Parse a typed result from effect kind, status, and raw value.
    ///
    /// The `status` field from the wire protocol determines the
    /// top-level outcome:
    /// - `"ok"`: success, parsed into a typed variant based on `kind`
    /// - `"cancelled"`: user dismissed the dialog (returns `Cancelled`)
    /// - `"unsupported"`: the backend doesn't support this effect
    /// - `"error"`: platform error, `value` contains the error message
    /// - anything else: logged as a warning, returns `Other`
    ///
    /// The `kind` string (e.g. `"file_open"`, `"clipboard_read"`)
    /// controls how `"ok"` results are destructured into typed
    /// variants like `FileOpened`, `ClipboardText`, etc. Unknown
    /// kinds fall through to `Other` with the raw value preserved.
    pub fn parse(kind: &str, status: &str, value: Option<&Value>) -> Self {
        match status {
            "cancelled" => Self::Cancelled,
            "unsupported" => Self::Unsupported,
            "error" => {
                let msg = value
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string();
                Self::Error(msg)
            }
            "ok" => Self::parse_ok(kind, value),
            other => {
                log::warn!("unknown effect status: {other}");
                Self::Other(value.cloned().unwrap_or(Value::Null))
            }
        }
    }

    fn parse_ok(kind: &str, value: Option<&Value>) -> Self {
        match kind {
            "file_open" => {
                let path = value
                    .and_then(|v| v.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::FileOpened { path }
            }
            "file_open_multiple" => {
                let paths = value
                    .and_then(|v| v.get("paths"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                Self::FilesOpened { paths }
            }
            "file_save" => {
                let path = value
                    .and_then(|v| v.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::FileSaved { path }
            }
            "directory_select" => {
                let path = value
                    .and_then(|v| v.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::DirectorySelected { path }
            }
            "directory_select_multiple" => {
                let paths = value
                    .and_then(|v| v.get("paths"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                Self::DirectoriesSelected { paths }
            }
            "clipboard_read" | "clipboard_read_primary" => {
                let text = value
                    .and_then(|v| v.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::ClipboardText { text }
            }
            "clipboard_read_html" => {
                let html = value
                    .and_then(|v| v.get("html"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::ClipboardHtml { html }
            }
            "clipboard_write" | "clipboard_write_html" | "clipboard_write_primary" => {
                Self::ClipboardWritten
            }
            "clipboard_clear" => Self::ClipboardCleared,
            "notification" => Self::NotificationShown,
            _ => Self::Other(value.cloned().unwrap_or(Value::Null)),
        }
    }
}

// ---------------------------------------------------------------------------
// SystemEvent
// ---------------------------------------------------------------------------

/// A system-level event (theme change, window query result, etc.).
#[derive(Debug, Clone)]
pub struct SystemEvent {
    /// Kind of system event that occurred.
    pub event_type: SystemEventType,
    /// Correlation tag for query responses.
    pub tag: Option<String>,
    /// Event-specific payload (e.g. the queried value).
    pub value: Option<Value>,
    /// Node ID associated with the event, if any.
    pub id: Option<String>,
    /// Window ID associated with the event, if any.
    pub window_id: Option<String>,
}

/// Kind of [`SystemEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SystemEventType {
    /// Response to a system-info query (OS, versions, feature flags).
    SystemInfo,
    /// Active OS theme reported by the platform.
    SystemTheme,
    /// Per-frame tick (used for animations).
    AnimationFrame,
    /// Active plushie theme changed.
    ThemeChanged,
    /// The renderer closed its last window.
    AllWindowsClosed,
    /// Response to an image-list query.
    ImageList,
    /// Response to a tree-hash query.
    TreeHash,
    /// Response to a find-focused-widget query.
    FindFocused,
    /// Screen-reader announcement delivered.
    Announce,
    /// Diagnostic emitted by the renderer (validation, warning).
    Diagnostic,
    /// Renderer failed to recover from an error.
    RecoveryFailed,
    /// Renderer reported a session-level failure.
    SessionError,
    /// Renderer closed a session.
    SessionClosed,
    /// Generic renderer-side error.
    Error,
}

// ---------------------------------------------------------------------------
// CommandError
// ---------------------------------------------------------------------------

/// Error from a command.
#[derive(Debug, Clone)]
pub struct CommandError {
    /// The error category (e.g. "not_found", "invalid_op").
    pub reason: String,
    /// The target ID that the command was sent to.
    pub id: Option<String>,
    /// The command family that failed.
    pub family: Option<String>,
    /// The widget type of the target node.
    pub widget_type: Option<String>,
    /// Human-readable error message.
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// ModifiersEvent
// ---------------------------------------------------------------------------

/// Keyboard modifier state changed.
#[derive(Debug, Clone)]
pub struct ModifiersEvent {
    /// The current state of all modifier keys.
    pub modifiers: KeyModifiers,
    /// Whether a widget captured this event.
    pub captured: bool,
    /// The window that had focus when modifiers changed.
    pub window_id: Option<String>,
}

// ---------------------------------------------------------------------------
// ImeEvent
// ---------------------------------------------------------------------------

/// Input Method Editor event (for CJK and complex text input).
#[derive(Debug, Clone)]
pub struct ImeEvent {
    /// The kind of IME event (opened, preedit, commit, closed).
    pub event_type: ImeEventType,
    /// The widget ID that has IME focus.
    pub id: Option<String>,
    /// Reversed ancestor scope chain of the focused widget.
    pub scope: Vec<String>,
    /// Composition or committed text.
    pub text: Option<String>,
    /// Cursor position as (start, end) within the preedit string.
    pub cursor: Option<(usize, usize)>,
    /// Whether a widget captured this event.
    pub captured: bool,
    /// The window containing the focused widget.
    pub window_id: Option<String>,
}

/// Phase of an IME composition event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImeEventType {
    /// IME session opened on a widget.
    Opened,
    /// Preedit (in-progress composition) text update.
    Preedit,
    /// Committed (finalized) text from the IME.
    Commit,
    /// IME session closed.
    Closed,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_file_opened() {
        let value = json!({"path": "/tmp/readme.txt"});
        let result = EffectResult::parse("file_open", "ok", Some(&value));
        match result {
            EffectResult::FileOpened { path } => assert_eq!(path, "/tmp/readme.txt"),
            other => panic!("expected FileOpened, got {other:?}"),
        }
    }

    #[test]
    fn parse_files_opened() {
        let value = json!({"paths": ["/a.txt", "/b.txt"]});
        let result = EffectResult::parse("file_open_multiple", "ok", Some(&value));
        match result {
            EffectResult::FilesOpened { paths } => {
                assert_eq!(paths, vec!["/a.txt", "/b.txt"]);
            }
            other => panic!("expected FilesOpened, got {other:?}"),
        }
    }

    #[test]
    fn parse_file_saved() {
        let value = json!({"path": "/tmp/out.csv"});
        let result = EffectResult::parse("file_save", "ok", Some(&value));
        match result {
            EffectResult::FileSaved { path } => assert_eq!(path, "/tmp/out.csv"),
            other => panic!("expected FileSaved, got {other:?}"),
        }
    }

    #[test]
    fn parse_directory_selected() {
        let value = json!({"path": "/home/user/docs"});
        let result = EffectResult::parse("directory_select", "ok", Some(&value));
        match result {
            EffectResult::DirectorySelected { path } => assert_eq!(path, "/home/user/docs"),
            other => panic!("expected DirectorySelected, got {other:?}"),
        }
    }

    #[test]
    fn parse_directories_selected() {
        let value = json!({"paths": ["/a", "/b", "/c"]});
        let result = EffectResult::parse("directory_select_multiple", "ok", Some(&value));
        match result {
            EffectResult::DirectoriesSelected { paths } => {
                assert_eq!(paths, vec!["/a", "/b", "/c"]);
            }
            other => panic!("expected DirectoriesSelected, got {other:?}"),
        }
    }

    #[test]
    fn parse_clipboard_text() {
        let value = json!({"text": "hello world"});
        let result = EffectResult::parse("clipboard_read", "ok", Some(&value));
        match result {
            EffectResult::ClipboardText { text } => assert_eq!(text, "hello world"),
            other => panic!("expected ClipboardText, got {other:?}"),
        }
    }

    #[test]
    fn parse_clipboard_primary_text() {
        let value = json!({"text": "primary selection"});
        let result = EffectResult::parse("clipboard_read_primary", "ok", Some(&value));
        match result {
            EffectResult::ClipboardText { text } => assert_eq!(text, "primary selection"),
            other => panic!("expected ClipboardText, got {other:?}"),
        }
    }

    #[test]
    fn parse_clipboard_html() {
        let value = json!({"html": "<b>bold</b>"});
        let result = EffectResult::parse("clipboard_read_html", "ok", Some(&value));
        match result {
            EffectResult::ClipboardHtml { html } => assert_eq!(html, "<b>bold</b>"),
            other => panic!("expected ClipboardHtml, got {other:?}"),
        }
    }

    #[test]
    fn parse_clipboard_written() {
        let result = EffectResult::parse("clipboard_write", "ok", None);
        assert!(matches!(result, EffectResult::ClipboardWritten));
    }

    #[test]
    fn parse_clipboard_html_written() {
        let result = EffectResult::parse("clipboard_write_html", "ok", None);
        assert!(matches!(result, EffectResult::ClipboardWritten));
    }

    #[test]
    fn parse_clipboard_cleared() {
        let result = EffectResult::parse("clipboard_clear", "ok", None);
        assert!(matches!(result, EffectResult::ClipboardCleared));
    }

    #[test]
    fn parse_notification_shown() {
        let result = EffectResult::parse("notification", "ok", None);
        assert!(matches!(result, EffectResult::NotificationShown));
    }

    #[test]
    fn parse_cancelled() {
        let result = EffectResult::parse("file_open", "cancelled", None);
        assert!(matches!(result, EffectResult::Cancelled));
    }

    #[test]
    fn parse_unsupported() {
        let result = EffectResult::parse("file_open", "unsupported", None);
        assert!(matches!(result, EffectResult::Unsupported));
    }

    #[test]
    fn parse_error_with_message() {
        let value = json!("permission denied");
        let result = EffectResult::parse("clipboard_read", "error", Some(&value));
        match result {
            EffectResult::Error(msg) => assert_eq!(msg, "permission denied"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parse_error_without_value() {
        let result = EffectResult::parse("clipboard_read", "error", None);
        match result {
            EffectResult::Error(msg) => assert_eq!(msg, "unknown error"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_status_falls_back_to_other() {
        let value = json!(42);
        let result = EffectResult::parse("file_open", "pending", Some(&value));
        match result {
            EffectResult::Other(v) => assert_eq!(v, json!(42)),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_kind_ok_falls_back_to_other() {
        let value = json!({"custom": true});
        let result = EffectResult::parse("future_effect", "ok", Some(&value));
        match result {
            EffectResult::Other(v) => assert_eq!(v, json!({"custom": true})),
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
