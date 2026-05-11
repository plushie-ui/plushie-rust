//! Internal message enum and serialization helpers.
//!
//! [`Message`] is the iced `Message` type used by the renderer. Widget
//! interactions flow through a single [`Message::Event`] variant with a
//! family string and JSON value. Runtime events (keyboard, mouse, window
//! lifecycle) have dedicated variants because they carry iced-specific
//! types that aren't JSON-representable.
//!
//! The serialization helpers convert iced types (keys, modifiers, mouse
//! buttons, scroll deltas) into the wire-format strings expected by the
//! host.

use iced::widget::text_editor;
use iced::{Point, window};
use serde_json::Value;

use crate::protocol::{KeyModifiers, OutgoingEvent};

// ---------------------------------------------------------------------------
// Event data structs
// ---------------------------------------------------------------------------

/// All fields from an iced keyboard event, packed for Message transport.
#[derive(Debug, Clone)]
pub struct KeyEventData {
    pub key: iced::keyboard::Key,
    pub modified_key: iced::keyboard::Key,
    pub physical_key: iced::keyboard::key::Physical,
    pub location: iced::keyboard::Location,
    pub modifiers: iced::keyboard::Modifiers,
    pub text: Option<String>,
    pub repeat: bool,
    /// Whether iced reported this event as `Captured` (consumed by a widget).
    pub captured: bool,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Message {
    /// A text editor action (window_id, id, action).
    TextEditorAction(String, String, text_editor::Action),
    /// A message arrived from the stdin reader (or stdin closed).
    Stdin(StdinEvent),
    /// No-op: used as return value for fire-and-forget tasks (font loads, etc.)
    NoOp,
    /// A timer subscription ticked (tag).
    TimerTick(String),
    /// A keyboard key was pressed (full event data, window, captured).
    KeyPressed(KeyEventData, window::Id),
    /// A keyboard key was released (full event data, window, captured).
    KeyReleased(KeyEventData, window::Id),
    /// Keyboard modifiers changed (modifiers, window, captured).
    ModifiersChanged(iced::keyboard::Modifiers, window::Id, bool),
    // -- IME events --
    /// IME session opened (window, captured).
    ImeOpened(window::Id, bool),
    /// IME preedit text updated (composing text, optional cursor range, window, captured).
    ImePreedit(String, Option<std::ops::Range<usize>>, window::Id, bool),
    /// IME committed final text (text, window, captured).
    ImeCommit(String, window::Id, bool),
    /// IME session closed (window, captured).
    ImeClosed(window::Id, bool),
    /// A window close was requested by the user (WM close button).
    WindowCloseRequested(window::Id),
    /// A window was actually closed by iced.
    WindowClosed(window::Id),
    /// A new window was opened (iced_id, window_id).
    WindowOpened(window::Id, String),
    // -- Mouse events --
    /// Cursor moved to (x, y) in a window (position, window_id, captured).
    CursorMoved(Point, window::Id, bool),
    /// Cursor entered a window (window_id, captured).
    CursorEntered(window::Id, bool),
    /// Cursor left a window (window_id, captured).
    CursorLeft(window::Id, bool),
    /// Mouse button pressed (button, window_id, captured).
    MouseButtonPressed(iced::mouse::Button, window::Id, bool),
    /// Mouse button released (button, window_id, captured).
    MouseButtonReleased(iced::mouse::Button, window::Id, bool),
    /// Mouse wheel scrolled (delta, window_id, captured).
    WheelScrolled(iced::mouse::ScrollDelta, window::Id, bool),
    // -- Touch events --
    /// Touch finger pressed (finger, position, window_id, captured).
    FingerPressed(iced::touch::Finger, Point, window::Id, bool),
    /// Touch finger moved (finger, position, window_id, captured).
    FingerMoved(iced::touch::Finger, Point, window::Id, bool),
    /// Touch finger lifted (finger, position, window_id, captured).
    FingerLifted(iced::touch::Finger, Point, window::Id, bool),
    /// Touch finger lost (finger, position, window_id, captured).
    FingerLost(iced::touch::Finger, Point, window::Id, bool),
    // -- Window lifecycle events --
    /// A window event from iced (window_id, event).
    WindowEvent(window::Id, window::Event),
    // -- System / animation events --
    /// Animation frame with timestamp.
    AnimationFrame(iced::time::Instant),
    /// System theme mode changed.
    ThemeChanged(iced::theme::Mode),
    /// Focus moved between elements within a canvas. Emitted as a single
    /// iced Message because `Program::update()` can only return one action,
    /// but the emitter splits this into separate blurred and focused
    /// outgoing events (in that order). Internal only, not sent on the wire.
    ///
    /// When `old_element_id` is `None`, only focus is emitted (first focus).
    /// When `new_element_id` is `None`, only blur is emitted (focus cleared).
    CanvasElementFocusChanged {
        window_id: String,
        old_element_id: Option<String>,
        new_element_id: Option<String>,
    },
    /// Renderer-side validation diagnostic (a11y, hit regions, etc.).
    Diagnostic {
        window_id: String,
        canvas_id: String,
        element_id: Option<String>,
        level: String,
        code: String,
        message: String,
    },
    /// PaneGrid pane was resized (window_id, grid_id, resize_event).
    PaneResized(String, String, iced::widget::pane_grid::ResizeEvent),
    /// PaneGrid pane was dragged (window_id, grid_id, drag_event).
    PaneDragged(String, String, iced::widget::pane_grid::DragEvent),
    /// PaneGrid pane was clicked (window_id, grid_id, pane).
    PaneClicked(String, String, iced::widget::pane_grid::Pane),
    /// PaneGrid focus cycle via F6 (window_id, grid_id, target_pane).
    PaneFocusCycle(String, String, iced::widget::pane_grid::Pane),
    /// Unified widget event. All widget interactions (click, input, toggle,
    /// slide, select, scroll, pointer area events, etc.) flow through this
    /// single variant. The `family` string identifies the event kind and
    /// `value` carries the payload as a JSON Value.
    Event {
        window_id: String,
        id: String,
        value: Value,
        family: String,
    },
    /// Internal: flush the event coalesce buffer. Fired by a timer
    /// task scheduled by the EventEmitter when rate-limited events
    /// are pending.
    FlushCoalesce,
}

impl Message {
    /// Extract the widget/node ID from this message, if it carries one.
    ///
    /// Returns the primary node ID that produced this message. For canvas
    /// element messages, returns the canvas ID (not the element ID).
    /// Returns `None` for system messages (keyboard, mouse, window
    /// lifecycle, animation, stdin) that aren't widget-specific.
    pub fn node_id(&self) -> Option<&str> {
        match self {
            Message::TextEditorAction(_, id, ..) => Some(id),
            // FocusChanged uses old or new element ID for routing.
            Message::CanvasElementFocusChanged {
                old_element_id,
                new_element_id,
                ..
            } => new_element_id.as_deref().or(old_element_id.as_deref()),
            // Pane grid events
            Message::PaneResized(_, grid_id, ..)
            | Message::PaneDragged(_, grid_id, ..)
            | Message::PaneClicked(_, grid_id, ..)
            | Message::PaneFocusCycle(_, grid_id, ..) => Some(grid_id),
            // Unified widget events
            Message::Event { id, .. } => Some(id),
            // Diagnostic
            Message::Diagnostic { canvas_id, .. } => Some(canvas_id),
            // System messages (no widget ID)
            _ => None,
        }
    }

    /// Convert this widget [`Message`] to an [`OutgoingEvent`], if applicable.
    ///
    /// Returns `None` for messages that don't map directly to a single
    /// outgoing event (system messages, text editor actions, pane grid
    /// state changes). Widget events are handled by `process_message`
    /// in the registry.
    pub fn to_outgoing_event(&self) -> Option<OutgoingEvent> {
        match self {
            // CanvasElementFocusChanged is internal-only: split into
            // blur + focus events by CanvasEngine::handle_message.
            Message::CanvasElementFocusChanged { .. } => None,
            Message::Diagnostic {
                canvas_id,
                element_id,
                level,
                code,
                message,
                ..
            } => Some(OutgoingEvent::diagnostic(
                canvas_id.clone(),
                element_id.clone(),
                level,
                code,
                message,
            )),
            _ => None,
        }
    }
}

/// What the stdin reader thread sends back.
#[derive(Debug, Clone)]
pub enum StdinEvent {
    Message(crate::protocol::IncomingMessage),
    Closed,
    Warning(String),
}

// ---------------------------------------------------------------------------
// Key serialization helpers
// ---------------------------------------------------------------------------

pub fn serialize_key(key: &iced::keyboard::Key) -> String {
    match key {
        iced::keyboard::Key::Named(named) => format!("{named:?}"),
        iced::keyboard::Key::Character(c) => c.to_string(),
        iced::keyboard::Key::Unidentified => "Unidentified".to_string(),
    }
}

pub fn serialize_modifiers(mods: iced::keyboard::Modifiers) -> KeyModifiers {
    KeyModifiers {
        shift: mods.shift(),
        ctrl: mods.control(),
        alt: mods.alt(),
        logo: mods.logo(),
        command: mods.command(),
    }
}

pub fn serialize_physical_key(physical: &iced::keyboard::key::Physical) -> String {
    match physical {
        iced::keyboard::key::Physical::Code(code) => format!("{code:?}"),
        iced::keyboard::key::Physical::Unidentified(code) => {
            format!("Unidentified({code:?})")
        }
    }
}

pub fn serialize_location(location: &iced::keyboard::Location) -> &'static str {
    match location {
        iced::keyboard::Location::Standard => "standard",
        iced::keyboard::Location::Left => "left",
        iced::keyboard::Location::Right => "right",
        iced::keyboard::Location::Numpad => "numpad",
    }
}

// ---------------------------------------------------------------------------
// Mouse serialization helpers
// ---------------------------------------------------------------------------

pub fn serialize_mouse_button(button: &iced::mouse::Button) -> String {
    match button {
        iced::mouse::Button::Left => "left".to_string(),
        iced::mouse::Button::Right => "right".to_string(),
        iced::mouse::Button::Middle => "middle".to_string(),
        iced::mouse::Button::Back => "back".to_string(),
        iced::mouse::Button::Forward => "forward".to_string(),
        iced::mouse::Button::Other(n) => format!("other_{n}"),
    }
}

pub fn serialize_scroll_delta(delta: &iced::mouse::ScrollDelta) -> (f32, f32, &'static str) {
    match delta {
        iced::mouse::ScrollDelta::Lines { x, y } => (*x, *y, "lines"),
        iced::mouse::ScrollDelta::Pixels { x, y } => (*x, *y, "pixels"),
    }
}
