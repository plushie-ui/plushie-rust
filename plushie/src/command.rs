//! Side effects returned from [`App::update`](crate::App::update).
//!
//! Commands are data, not closures (except `Async` and `Stream`).
//! This makes them testable: you can assert which commands an
//! update call returns without executing them.

use std::time::Duration;

use serde_json::Value;

use crate::event::Event;

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// A side effect returned from the update function.
///
/// Use the builder methods (`Command::focus`, `Command::async_task`,
/// `Command::close_window`, etc.) for ergonomic construction.
#[derive(Debug)]
pub enum Command {
    /// No side effect.
    None,
    /// Execute multiple commands.
    Batch(Vec<Command>),
    /// Exit the application.
    Exit,

    // -- Async work --
    /// Run an async task. The result is delivered as an [`AsyncEvent`](crate::event::AsyncEvent).
    Async {
        tag: String,
        task: Box<dyn std::any::Any + Send>,
    },
    /// Cancel a running async task or stream by tag.
    Cancel { tag: String },
    /// Deliver an event after a delay.
    SendAfter { delay: Duration, event: Box<Event> },

    // -- Focus --
    /// Move keyboard focus to the widget with the given ID.
    Focus(String),
    /// Move keyboard focus to the next focusable widget.
    FocusNext,
    /// Move keyboard focus to the previous focusable widget.
    FocusPrevious,

    // -- Text operations --
    /// Select all text in a text input.
    SelectAll(String),
    /// Move the cursor to the start of a text input.
    MoveCursorToFront(String),
    /// Move the cursor to the end of a text input.
    MoveCursorToEnd(String),
    /// Move the cursor to a specific position in a text input.
    MoveCursorTo { target: String, position: usize },
    /// Select a range of text in a text input.
    SelectRange { target: String, start: usize, end: usize },

    // -- Scroll --
    /// Scroll to an absolute position (animated).
    ScrollTo { target: String, x: f32, y: f32 },
    /// Scroll by a relative offset (animated).
    ScrollBy { target: String, x: f32, y: f32 },
    /// Snap to an absolute scroll position (instant, no animation).
    SnapTo { target: String, x: f32, y: f32 },
    /// Snap to the end of the scrollable content.
    SnapToEnd(String),

    // -- Window operations --
    /// Perform a window operation (close, resize, move, etc.).
    Window(WindowOp),
    /// Query window state. Result delivered as a [`SystemEvent`](crate::event::SystemEvent).
    WindowQuery(WindowQuery),

    // -- System --
    /// Perform a system-level operation.
    SystemOp(SystemOp),
    /// Query system state. Result delivered as a [`SystemEvent`](crate::event::SystemEvent).
    SystemQuery(SystemQuery),

    // -- Platform effects --
    /// Request a platform effect (file dialog, clipboard, notification).
    Effect { tag: String, request: EffectRequest },

    // -- Images --
    /// Perform an image operation (create, update, delete).
    Image(ImageOp),

    // -- PaneGrid --
    /// Perform a pane grid operation (split, close, swap).
    PaneGrid(PaneGridOp),

    // -- Native widget commands --
    /// Send a single command to a native widget.
    WidgetCommand { node_id: String, op: String, payload: Value },
    /// Send multiple native widget commands in a batch.
    WidgetCommands(Vec<WidgetCommandItem>),

    // -- Accessibility --
    /// Announce text to screen readers.
    Announce(String),
    /// Load a font from raw byte data.
    LoadFont(Vec<u8>),

    // -- Queries --
    /// Request a hash of the current widget tree (for golden-file testing).
    TreeHash { tag: String },
    /// Query which widget currently has keyboard focus.
    FindFocused { tag: String },
    /// Advance the animation frame to the given timestamp.
    AdvanceFrame { timestamp: u64 },
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl Command {
    /// A no-op command. Useful as a default return value.
    pub fn none() -> Self { Self::None }

    /// Execute multiple commands together.
    pub fn batch(cmds: impl IntoIterator<Item = Command>) -> Self {
        Self::Batch(cmds.into_iter().collect())
    }

    /// Exit the application.
    pub fn exit() -> Self { Self::Exit }

    /// Move keyboard focus to the widget with the given ID.
    pub fn focus(id: &str) -> Self { Self::Focus(id.to_string()) }
    /// Move keyboard focus to the next focusable widget.
    pub fn focus_next() -> Self { Self::FocusNext }
    /// Move keyboard focus to the previous focusable widget.
    pub fn focus_previous() -> Self { Self::FocusPrevious }

    /// Deliver an event after a delay.
    pub fn send_after(delay: Duration, event: Event) -> Self {
        Self::SendAfter { delay, event: Box::new(event) }
    }

    /// Cancel a running async task or stream by tag.
    pub fn cancel(tag: &str) -> Self {
        Self::Cancel { tag: tag.to_string() }
    }

    // -- Window shortcuts --

    /// Close the window with the given ID.
    pub fn close_window(id: &str) -> Self {
        Self::Window(WindowOp::Close(id.to_string()))
    }

    /// Resize a window to the given dimensions in logical pixels.
    pub fn resize_window(id: &str, width: f32, height: f32) -> Self {
        Self::Window(WindowOp::Resize { window_id: id.to_string(), width, height })
    }

    /// Move a window to the given position in logical pixels.
    pub fn move_window(id: &str, x: f32, y: f32) -> Self {
        Self::Window(WindowOp::Move { window_id: id.to_string(), x, y })
    }

    // -- Effect shortcuts --

    /// Open a file-open dialog. Result delivered as an [`EffectEvent`](crate::event::EffectEvent).
    pub fn file_open(tag: &str) -> Self {
        Self::Effect { tag: tag.to_string(), request: EffectRequest::FileOpen(Default::default()) }
    }

    /// Read text from the system clipboard.
    pub fn clipboard_read(tag: &str) -> Self {
        Self::Effect { tag: tag.to_string(), request: EffectRequest::ClipboardRead }
    }

    /// Write text to the system clipboard.
    pub fn clipboard_write(tag: &str, text: &str) -> Self {
        Self::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardWrite(text.to_string()),
        }
    }

    // -- Scroll shortcuts --

    /// Scroll a scrollable widget to an absolute position.
    pub fn scroll_to(target: &str, x: f32, y: f32) -> Self {
        Self::ScrollTo { target: target.to_string(), x, y }
    }

    // -- Widget command shortcuts --

    /// Send a command to a native widget.
    pub fn widget_command(node_id: &str, op: &str, payload: Value) -> Self {
        Self::WidgetCommand {
            node_id: node_id.to_string(),
            op: op.to_string(),
            payload,
        }
    }
}

// ---------------------------------------------------------------------------
// Nested operation enums
// ---------------------------------------------------------------------------

/// A window management operation.
#[derive(Debug)]
pub enum WindowOp {
    /// Close a window.
    Close(String),
    /// Resize a window to the given logical dimensions.
    Resize { window_id: String, width: f32, height: f32 },
    /// Move a window to the given logical position.
    Move { window_id: String, x: f32, y: f32 },
    /// Set or unset the maximized state.
    Maximize { window_id: String, maximized: bool },
    /// Set or unset the minimized state.
    Minimize { window_id: String, minimized: bool },
    /// Set the window mode (e.g. "fullscreen", "windowed").
    SetMode { window_id: String, mode: String },
    /// Toggle between maximized and restored states.
    ToggleMaximize(String),
    /// Toggle window decorations (title bar, borders).
    ToggleDecorations(String),
    /// Bring a window to the front and give it focus.
    FocusWindow(String),
    /// Set the window stacking level (e.g. "always_on_top", "normal").
    SetLevel { window_id: String, level: String },
    /// Begin an interactive window drag (initiated by the user).
    DragWindow(String),
    /// Begin an interactive window resize from the given edge/direction.
    DragResize { window_id: String, direction: String },
    /// Request user attention (taskbar flash or similar).
    RequestAttention { window_id: String, urgency: Option<String> },
    /// Take a screenshot of a window. Result delivered as a [`SystemEvent`](crate::event::SystemEvent).
    Screenshot { window_id: String, tag: String },
    /// Set whether the window is user-resizable.
    SetResizable { window_id: String, resizable: bool },
    /// Set the minimum window size.
    SetMinSize { window_id: String, width: f32, height: f32 },
    /// Set the maximum window size.
    SetMaxSize { window_id: String, width: f32, height: f32 },
    /// Allow mouse events to pass through the window.
    EnableMousePassthrough(String),
    /// Stop mouse events from passing through the window.
    DisableMousePassthrough(String),
    /// Show the native system menu (right-click title bar menu).
    ShowSystemMenu(String),
    /// Set the window icon from raw RGBA pixel data.
    SetIcon { window_id: String, data: Vec<u8>, width: u32, height: u32 },
}

/// A query for window state. Results arrive as [`SystemEvent`](crate::event::SystemEvent).
#[derive(Debug)]
pub enum WindowQuery {
    /// Query the window's current size in logical pixels.
    GetSize { window_id: String, tag: String },
    /// Query the window's current position in logical pixels.
    GetPosition { window_id: String, tag: String },
    /// Query whether the window is maximized.
    IsMaximized { window_id: String, tag: String },
    /// Query whether the window is minimized.
    IsMinimized { window_id: String, tag: String },
    /// Query the current window mode (fullscreen, windowed, etc.).
    GetMode { window_id: String, tag: String },
    /// Query the window's DPI scale factor.
    GetScaleFactor { window_id: String, tag: String },
    /// Query the size of the monitor the window is on.
    MonitorSize { window_id: String, tag: String },
}

/// A system-level operation.
#[derive(Debug)]
pub enum SystemOp {
    /// Enable or disable automatic window tabbing (macOS).
    AllowAutomaticTabbing(bool),
}

/// A system-level query. Results arrive as [`SystemEvent`](crate::event::SystemEvent).
#[derive(Debug)]
pub enum SystemQuery {
    /// Query the current OS theme (light/dark).
    GetTheme { tag: String },
    /// Query system information (OS, renderer version, etc.).
    GetInfo { tag: String },
}

/// A platform effect request. Results arrive as [`EffectEvent`](crate::event::EffectEvent).
#[derive(Debug)]
pub enum EffectRequest {
    /// Open a single-file selection dialog.
    FileOpen(FileDialogOpts),
    /// Open a multi-file selection dialog.
    FileOpenMultiple(FileDialogOpts),
    /// Open a file-save dialog.
    FileSave(FileDialogOpts),
    /// Open a single-directory selection dialog.
    DirectorySelect(FileDialogOpts),
    /// Open a multi-directory selection dialog.
    DirectorySelectMultiple(FileDialogOpts),
    /// Read text from the clipboard.
    ClipboardRead,
    /// Write text to the clipboard.
    ClipboardWrite(String),
    /// Read HTML content from the clipboard.
    ClipboardReadHtml,
    /// Write HTML content to the clipboard (with optional plain-text fallback).
    ClipboardWriteHtml { html: String, alt_text: Option<String> },
    /// Clear the clipboard contents.
    ClipboardClear,
    /// Read text from the primary selection (X11/Wayland).
    ClipboardReadPrimary,
    /// Write text to the primary selection (X11/Wayland).
    ClipboardWritePrimary(String),
    /// Show a desktop notification.
    Notification { title: String, body: String, opts: NotificationOpts },
}

/// Options for file and directory dialogs.
#[derive(Debug, Default)]
pub struct FileDialogOpts {
    /// Dialog window title.
    pub title: Option<String>,
    /// Initial directory to open in.
    pub directory: Option<String>,
    /// File type filters as `(label, [extensions])` pairs.
    pub filters: Vec<(String, Vec<String>)>,
}

/// Options for desktop notifications.
#[derive(Debug, Default)]
pub struct NotificationOpts {
    /// Path or name of the notification icon.
    pub icon: Option<String>,
    /// How long the notification should be displayed.
    pub timeout: Option<Duration>,
    /// Urgency level (e.g. "low", "normal", "critical").
    pub urgency: Option<String>,
    /// Whether to play a notification sound.
    pub sound: Option<bool>,
}

/// An image management operation.
#[derive(Debug)]
pub enum ImageOp {
    /// Create an image from encoded bytes (PNG, JPEG, etc.).
    Create { handle: String, data: Vec<u8> },
    /// Create an image from raw RGBA pixel data.
    CreateRaw { handle: String, width: u32, height: u32, pixels: Vec<u8> },
    /// Replace an existing image with new encoded bytes.
    Update { handle: String, data: Vec<u8> },
    /// Replace an existing image with new raw RGBA pixel data.
    UpdateRaw { handle: String, width: u32, height: u32, pixels: Vec<u8> },
    /// Delete an image by handle.
    Delete(String),
    /// List all loaded image handles. Result delivered as a [`SystemEvent`](crate::event::SystemEvent).
    List { tag: String },
    /// Delete all loaded images.
    Clear,
}

/// A pane grid operation.
#[derive(Debug)]
pub enum PaneGridOp {
    /// Split a pane along an axis, creating a new pane.
    Split { target: String, pane: String, axis: String, new_pane: String },
    /// Close a pane.
    Close { target: String, pane: String },
    /// Swap the positions of two panes.
    Swap { target: String, a: String, b: String },
    /// Maximize a pane to fill the entire grid.
    Maximize { target: String, pane: String },
    /// Restore the pane grid from a maximized state.
    Restore(String),
}

/// A single native widget command within a batch.
#[derive(Debug, Clone)]
pub struct WidgetCommandItem {
    /// The target widget's node ID.
    pub node_id: String,
    /// The operation name understood by the widget.
    pub op: String,
    /// Operation-specific data.
    pub payload: Value,
}
