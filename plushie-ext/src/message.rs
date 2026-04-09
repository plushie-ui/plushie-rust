//! Internal message enum and serialization helpers.
//!
//! [`Message`] is the iced `Message` type used by the renderer. Every
//! widget interaction (click, input, slide, toggle, etc.) and every
//! runtime event (keyboard, mouse, window lifecycle) maps to a variant.
//! The renderer's `update()` method dispatches on these variants to
//! emit outgoing events over the wire protocol.
//!
//! The serialization helpers convert iced types (keys, modifiers, mouse
//! buttons, scroll deltas) into the wire-format strings expected by the
//! host.

use iced::widget::markdown;
use iced::widget::text_editor;
use iced::{Point, window};
use serde_json::Value;

use crate::protocol::{KeyModifiers, OutgoingEvent};

// ---------------------------------------------------------------------------
// Event data structs
// ---------------------------------------------------------------------------

/// Scrollable viewport state, emitted on scroll position changes.
#[derive(Debug, Clone, Copy)]
pub struct ScrollViewport {
    /// Absolute scroll offset on the x axis (pixels from left).
    pub absolute_x: f32,
    /// Absolute scroll offset on the y axis (pixels from top).
    pub absolute_y: f32,
    /// Relative scroll position on the x axis (0.0 = start, 1.0 = end).
    pub relative_x: f32,
    /// Relative scroll position on the y axis (0.0 = top, 1.0 = bottom).
    pub relative_y: f32,
    /// Total content width (may exceed viewport).
    pub content_width: f32,
    /// Total content height (may exceed viewport).
    pub content_height: f32,
    /// Visible viewport width.
    pub viewport_width: f32,
    /// Visible viewport height.
    pub viewport_height: f32,
}

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
    /// A user clicked a button with the given node ID.
    Click(String, String),
    /// A text input value changed (window_id, id, new_value).
    Input(String, String, String),
    /// A text input was submitted (window_id, id, current_value).
    Submit(String, String, String),
    /// A checkbox or toggler was toggled (window_id, id, checked).
    Toggle(String, String, bool),
    /// A slider value changed (window_id, id, value).
    Slide(String, String, f64),
    /// A slider was released (window_id, id).
    SlideRelease(String, String),
    /// A pick_list/combo_box/radio selection (window_id, id, value).
    Select(String, String, String),
    /// A text editor action (window_id, id, action).
    TextEditorAction(String, String, text_editor::Action),
    /// A markdown link was clicked.
    MarkdownUrl(markdown::Uri),
    /// A message arrived from the stdin reader (or stdin closed).
    Stdin(StdinEvent),
    /// No-op: used as return value for fire-and-forget tasks (font loads, etc.)
    NoOp,
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
    /// Sensor widget resize event (window_id, id, width, height).
    SensorResize(String, String, f32, f32),
    /// Canvas interaction event (press, release, move).
    CanvasEvent {
        window_id: String,
        id: String,
        kind: String,
        x: f32,
        y: f32,
        /// Encoded as "button:pointer_type:finger_id" for press/release,
        /// "pointer_type:finger_id" for move. Finger omitted for mouse.
        extra: String,
        /// Keyboard modifiers at the time of the event.
        modifiers: KeyModifiers,
    },
    /// Canvas scroll event.
    CanvasScroll {
        window_id: String,
        id: String,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
        /// Pointer type: "mouse", "touch", or "pen".
        pointer_type: String,
        /// Keyboard modifiers at the time of the event.
        modifiers: KeyModifiers,
    },
    // -- Canvas element events (interactive group interactions) --
    /// Cursor entered an interactive element's hit region.
    CanvasElementEnter {
        window_id: String,
        canvas_id: String,
        element_id: String,
        x: f32,
        y: f32,
    },
    /// Cursor left an interactive element's hit region.
    CanvasElementLeave {
        window_id: String,
        canvas_id: String,
        element_id: String,
    },
    /// Interactive element activated (click or keyboard Enter/Space).
    /// `button` is `"left"`, `"right"`, `"keyboard"`, or `"test"`.
    CanvasElementClick {
        window_id: String,
        canvas_id: String,
        element_id: String,
        x: f32,
        y: f32,
        button: String,
    },
    /// Continuous drag on a draggable element.
    CanvasElementDrag {
        window_id: String,
        canvas_id: String,
        element_id: String,
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    /// Mouse released after a drag.
    CanvasElementDragEnd {
        window_id: String,
        canvas_id: String,
        element_id: String,
        x: f32,
        y: f32,
    },
    /// A focused interactive element received a key that the canvas did not
    /// consume for navigation. Emitted when `arrow_mode` is `"none"` and the
    /// key is one the canvas would normally handle (arrows, Home, End,
    /// PageUp, PageDown). Lets the host implement custom value adjustment
    /// on focused canvas elements (e.g. slider-like controls).
    CanvasElementKeyPress {
        window_id: String,
        canvas_id: String,
        element_id: String,
        key: String,
        modifiers: KeyModifiers,
    },
    /// A focused canvas element received a key release. Mirrors
    /// `CanvasElementKeyPress` for the release phase. Emitted when
    /// `arrow_mode` is `"none"` and the released key is a navigation key.
    CanvasElementKeyRelease {
        window_id: String,
        canvas_id: String,
        element_id: String,
        key: String,
        modifiers: KeyModifiers,
    },
    /// An interactive element gained keyboard focus.
    CanvasElementFocused {
        window_id: String,
        canvas_id: String,
        element_id: String,
    },
    /// An interactive element lost keyboard focus.
    CanvasElementBlurred {
        window_id: String,
        canvas_id: String,
        element_id: String,
    },
    /// Focus moved between elements within a canvas. Emitted as a single
    /// iced Message because `Program::update()` can only return one action,
    /// but the emitter splits this into separate `canvas_element_blurred`
    /// and `canvas_element_focused` outgoing events (in that order).
    ///
    /// When `old_element_id` is `None`, only focus is emitted (first focus).
    /// When `new_element_id` is `None`, only blur is emitted (focus cleared).
    CanvasElementFocusChanged {
        window_id: String,
        canvas_id: String,
        old_element_id: Option<String>,
        new_element_id: Option<String>,
    },
    /// The canvas widget itself gained iced-level focus (Tab or click).
    CanvasFocused {
        window_id: String,
        canvas_id: String,
    },
    /// The canvas widget itself lost iced-level focus.
    CanvasBlurred {
        window_id: String,
        canvas_id: String,
    },
    /// A focusable group gained group-level focus (two-level navigation).
    CanvasGroupFocused {
        window_id: String,
        canvas_id: String,
        group_id: String,
    },
    /// A focusable group lost group-level focus.
    CanvasGroupBlurred {
        window_id: String,
        canvas_id: String,
        group_id: String,
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
    /// Scrollable viewport changed (window_id, id, viewport).
    ScrollEvent(String, String, ScrollViewport),
    /// Text was pasted into a text_input (id, pasted_text).
    Paste(String, String, String),
    /// ComboBox option was hovered (window_id, combo_id, option_value).
    OptionHovered(String, String, String),
    /// MouseArea simple event (window_id, id, kind, x, y). Kind is one of:
    /// right_press, right_release, middle_press, middle_release,
    /// double_click, enter, exit.
    MouseAreaEvent(String, String, String, f32, f32),
    /// MouseArea cursor move event (window_id, id, x, y).
    MouseAreaMove(String, String, f32, f32),
    /// MouseArea scroll event (window_id, id, delta_x, delta_y, x, y).
    MouseAreaScroll(String, String, f32, f32, f32, f32),
    /// Generic widget event. Used for on_open, on_close, sort, and
    /// other events that carry a family string and optional data.
    Event {
        window_id: String,
        id: String,
        data: Value,
        family: String,
    },
    /// Widget status changed (window_id, widget_id, status_name).
    /// Emitted by on_status_change callbacks from iced widgets.
    StatusChanged(String, String, String),
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
            // Standard widget events
            Message::Click(_, id, ..)
            | Message::Input(_, id, ..)
            | Message::Submit(_, id, ..)
            | Message::Toggle(_, id, ..)
            | Message::Slide(_, id, ..)
            | Message::SlideRelease(_, id)
            | Message::Select(_, id, ..)
            | Message::Paste(_, id, ..)
            | Message::OptionHovered(_, id, ..)
            | Message::SensorResize(_, id, ..)
            | Message::ScrollEvent(_, id, ..)
            | Message::StatusChanged(_, id, ..)
            | Message::TextEditorAction(_, id, ..) => Some(id),
            // Mouse area events
            Message::MouseAreaEvent(_, id, ..)
            | Message::MouseAreaMove(_, id, ..)
            | Message::MouseAreaScroll(_, id, ..) => Some(id),
            // Canvas events (use canvas ID, not element ID)
            Message::CanvasEvent { id, .. } | Message::CanvasScroll { id, .. } => Some(id),
            Message::CanvasElementEnter { canvas_id, .. }
            | Message::CanvasElementLeave { canvas_id, .. }
            | Message::CanvasElementClick { canvas_id, .. }
            | Message::CanvasElementDrag { canvas_id, .. }
            | Message::CanvasElementDragEnd { canvas_id, .. }
            | Message::CanvasElementKeyPress { canvas_id, .. }
            | Message::CanvasElementKeyRelease { canvas_id, .. }
            | Message::CanvasElementFocused { canvas_id, .. }
            | Message::CanvasElementBlurred { canvas_id, .. }
            | Message::CanvasElementFocusChanged { canvas_id, .. }
            | Message::CanvasFocused { canvas_id, .. }
            | Message::CanvasBlurred { canvas_id, .. }
            | Message::CanvasGroupFocused { canvas_id, .. }
            | Message::CanvasGroupBlurred { canvas_id, .. } => Some(canvas_id),
            // Pane grid events
            Message::PaneResized(_, grid_id, ..)
            | Message::PaneDragged(_, grid_id, ..)
            | Message::PaneClicked(_, grid_id, ..)
            | Message::PaneFocusCycle(_, grid_id, ..) => Some(grid_id),
            // Extension events
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
    /// outgoing event (system messages, slider tracking, text editor
    /// actions, extension events, pane grid state changes).
    pub fn to_outgoing_event(&self) -> Option<OutgoingEvent> {
        match self {
            Message::Click(window_id, id) => {
                Some(OutgoingEvent::click(id.clone()).with_window_id(window_id.clone()))
            }
            Message::Input(window_id, id, value) => Some(
                OutgoingEvent::input(id.clone(), value.clone()).with_window_id(window_id.clone()),
            ),
            Message::Submit(window_id, id, value) => Some(
                OutgoingEvent::submit(id.clone(), value.clone()).with_window_id(window_id.clone()),
            ),
            Message::Toggle(window_id, id, value) => {
                Some(OutgoingEvent::toggle(id.clone(), *value).with_window_id(window_id.clone()))
            }
            Message::Select(window_id, id, value) => Some(
                OutgoingEvent::select(id.clone(), value.clone()).with_window_id(window_id.clone()),
            ),
            Message::Paste(window_id, id, text) => Some(
                OutgoingEvent::paste(id.clone(), text.clone()).with_window_id(window_id.clone()),
            ),
            Message::OptionHovered(window_id, id, value) => Some(
                OutgoingEvent::option_hovered(id.clone(), value.clone())
                    .with_window_id(window_id.clone()),
            ),
            Message::SensorResize(window_id, id, w, h) => {
                Some(OutgoingEvent::resize(id.clone(), *w, *h).with_window_id(window_id.clone()))
            }
            Message::ScrollEvent(window_id, id, viewport) => Some(
                OutgoingEvent::scroll(
                    id.clone(),
                    viewport.absolute_x,
                    viewport.absolute_y,
                    viewport.relative_x,
                    viewport.relative_y,
                    viewport.viewport_width,
                    viewport.viewport_height,
                    viewport.content_width,
                    viewport.content_height,
                )
                .with_window_id(window_id.clone()),
            ),
            Message::MouseAreaEvent(window_id, id, kind, x, y) => {
                let mods = KeyModifiers::default();
                match kind.as_str() {
                    "right_press" => Some(OutgoingEvent::pointer_press(
                        id.clone(),
                        *x,
                        *y,
                        "right",
                        "mouse",
                        None,
                        mods.clone(),
                    )),
                    "right_release" => Some(OutgoingEvent::pointer_release(
                        id.clone(),
                        *x,
                        *y,
                        "right",
                        "mouse",
                        None,
                        mods.clone(),
                    )),
                    "middle_press" => Some(OutgoingEvent::pointer_press(
                        id.clone(),
                        *x,
                        *y,
                        "middle",
                        "mouse",
                        None,
                        mods.clone(),
                    )),
                    "middle_release" => Some(OutgoingEvent::pointer_release(
                        id.clone(),
                        *x,
                        *y,
                        "middle",
                        "mouse",
                        None,
                        mods.clone(),
                    )),
                    "double_click" => Some(OutgoingEvent::pointer_double_click(
                        id.clone(),
                        *x,
                        *y,
                        "mouse",
                        mods.clone(),
                    )),
                    "enter" => Some(OutgoingEvent::pointer_enter(id.clone())),
                    "exit" => Some(OutgoingEvent::pointer_exit(id.clone())),
                    _ => None,
                }
            }
            .map(|event| event.with_window_id(window_id.clone())),
            Message::MouseAreaMove(window_id, id, x, y) => {
                let mods = KeyModifiers::default();
                Some(
                    OutgoingEvent::pointer_move(id.clone(), *x, *y, "mouse", None, mods)
                        .with_window_id(window_id.clone()),
                )
            }
            Message::MouseAreaScroll(window_id, id, dx, dy, x, y) => {
                let mods = KeyModifiers::default();
                Some(
                    OutgoingEvent::pointer_scroll(id.clone(), *x, *y, *dx, *dy, "mouse", mods)
                        .with_window_id(window_id.clone()),
                )
            }
            Message::CanvasEvent {
                window_id,
                id,
                kind,
                x,
                y,
                extra,
                modifiers,
            } => {
                // `extra` encodes: "button:pointer_type:finger_id" for press/release,
                // "pointer_type:finger_id" for move. Finger omitted for mouse.
                let parts: Vec<&str> = extra.splitn(3, ':').collect();
                let (button, pointer_type, finger) = match kind.as_str() {
                    "press" | "release" => {
                        let btn = parts.first().copied().unwrap_or("left");
                        let ptr = parts.get(1).copied().unwrap_or("mouse");
                        let fng = parts.get(2).and_then(|s| s.parse::<u64>().ok());
                        (btn, ptr, fng)
                    }
                    _ => {
                        let ptr = parts.first().copied().unwrap_or("mouse");
                        let fng = parts.get(1).and_then(|s| s.parse::<u64>().ok());
                        ("", ptr, fng)
                    }
                };
                match kind.as_str() {
                    "press" => Some(OutgoingEvent::pointer_press(
                        id.clone(),
                        *x,
                        *y,
                        button,
                        pointer_type,
                        finger,
                        modifiers.clone(),
                    )),
                    "release" => Some(OutgoingEvent::pointer_release(
                        id.clone(),
                        *x,
                        *y,
                        button,
                        pointer_type,
                        finger,
                        modifiers.clone(),
                    )),
                    "move" => Some(OutgoingEvent::pointer_move(
                        id.clone(),
                        *x,
                        *y,
                        pointer_type,
                        finger,
                        modifiers.clone(),
                    )),
                    _ => None,
                }
                .map(|event| event.with_window_id(window_id.clone()))
            }
            Message::CanvasScroll {
                window_id,
                id,
                x,
                y,
                delta_x,
                delta_y,
                pointer_type,
                modifiers,
            } => Some(
                OutgoingEvent::pointer_scroll(
                    id.clone(),
                    *x,
                    *y,
                    *delta_x,
                    *delta_y,
                    pointer_type,
                    modifiers.clone(),
                )
                .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementEnter {
                window_id,
                canvas_id,
                element_id,
                x,
                y,
            } => Some(
                OutgoingEvent::canvas_element_enter(canvas_id.clone(), element_id.clone(), *x, *y)
                    .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementLeave {
                window_id,
                canvas_id,
                element_id,
            } => Some(
                OutgoingEvent::canvas_element_leave(canvas_id.clone(), element_id.clone())
                    .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementKeyPress {
                window_id,
                canvas_id,
                element_id,
                key,
                modifiers,
            } => Some(
                OutgoingEvent::canvas_element_key_press(
                    canvas_id.clone(),
                    element_id.clone(),
                    key.clone(),
                    modifiers.clone(),
                )
                .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementKeyRelease {
                window_id,
                canvas_id,
                element_id,
                key,
                modifiers,
            } => Some(
                OutgoingEvent::canvas_element_key_release(
                    canvas_id.clone(),
                    element_id.clone(),
                    key.clone(),
                    modifiers.clone(),
                )
                .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementClick {
                window_id,
                canvas_id,
                element_id,
                x,
                y,
                button,
            } => Some(
                OutgoingEvent::canvas_element_click(
                    canvas_id.clone(),
                    element_id.clone(),
                    *x,
                    *y,
                    button.clone(),
                )
                .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementDrag {
                window_id,
                canvas_id,
                element_id,
                x,
                y,
                delta_x,
                delta_y,
            } => Some(
                OutgoingEvent::canvas_element_drag(
                    canvas_id.clone(),
                    element_id.clone(),
                    *x,
                    *y,
                    *delta_x,
                    *delta_y,
                )
                .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementDragEnd {
                window_id,
                canvas_id,
                element_id,
                x,
                y,
            } => Some(
                OutgoingEvent::canvas_element_drag_end(
                    canvas_id.clone(),
                    element_id.clone(),
                    *x,
                    *y,
                )
                .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementFocused {
                window_id,
                canvas_id,
                element_id,
            } => Some(
                OutgoingEvent::canvas_element_focused(canvas_id.clone(), element_id.clone())
                    .with_window_id(window_id.clone()),
            ),
            Message::CanvasElementBlurred {
                window_id,
                canvas_id,
                element_id,
            } => Some(
                OutgoingEvent::canvas_element_blurred(canvas_id.clone(), element_id.clone())
                    .with_window_id(window_id.clone()),
            ),
            Message::CanvasFocused {
                window_id,
                canvas_id,
            } => Some(
                OutgoingEvent::canvas_focused(canvas_id.clone()).with_window_id(window_id.clone()),
            ),
            Message::CanvasBlurred {
                window_id,
                canvas_id,
            } => Some(
                OutgoingEvent::canvas_blurred(canvas_id.clone()).with_window_id(window_id.clone()),
            ),
            Message::CanvasGroupFocused {
                window_id,
                canvas_id,
                group_id,
            } => Some(
                OutgoingEvent::canvas_group_focused(canvas_id.clone(), group_id.clone())
                    .with_window_id(window_id.clone()),
            ),
            Message::CanvasGroupBlurred {
                window_id,
                canvas_id,
                group_id,
            } => Some(
                OutgoingEvent::canvas_group_blurred(canvas_id.clone(), group_id.clone())
                    .with_window_id(window_id.clone()),
            ),
            Message::Diagnostic {
                window_id,
                canvas_id,
                element_id,
                level,
                code,
                message,
            } => Some(
                OutgoingEvent::diagnostic(
                    canvas_id.clone(),
                    element_id.clone(),
                    level,
                    code,
                    message,
                )
                .with_window_id(window_id.clone()),
            ),
            _ => None,
        }
    }

    /// Create a widget event message for use in `on_press`, `on_submit`,
    /// and other iced widget callbacks inside extension `render()` methods.
    ///
    /// ```ignore
    /// button("Click me")
    ///     .on_press(Message::widget_event(&node.id, "clicked", json!({})))
    /// ```
    pub fn widget_event(
        window_id: impl Into<String>,
        id: impl Into<String>,
        family: impl Into<String>,
        data: Value,
    ) -> Self {
        Message::Event {
            window_id: window_id.into(),
            id: id.into(),
            family: family.into(),
            data,
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
