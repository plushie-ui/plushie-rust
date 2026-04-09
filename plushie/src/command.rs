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
    Focus(String),
    FocusNext,
    FocusPrevious,

    // -- Text operations --
    SelectAll(String),
    MoveCursorToFront(String),
    MoveCursorToEnd(String),
    MoveCursorTo { target: String, position: usize },
    SelectRange { target: String, start: usize, end: usize },

    // -- Scroll --
    ScrollTo { target: String, x: f32, y: f32 },
    ScrollBy { target: String, x: f32, y: f32 },
    SnapTo { target: String, x: f32, y: f32 },
    SnapToEnd(String),

    // -- Window operations --
    Window(WindowOp),
    /// Query window state. Result delivered as a [`SystemEvent`](crate::event::SystemEvent).
    WindowQuery(WindowQuery),

    // -- System --
    SystemOp(SystemOp),
    SystemQuery(SystemQuery),

    // -- Platform effects --
    Effect { tag: String, request: EffectRequest },

    // -- Images --
    Image(ImageOp),

    // -- PaneGrid --
    PaneGrid(PaneGridOp),

    // -- Native widget commands --
    WidgetCommand { node_id: String, op: String, payload: Value },
    WidgetCommands(Vec<WidgetCommandItem>),

    // -- Accessibility --
    Announce(String),
    LoadFont(Vec<u8>),

    // -- Queries --
    TreeHash { tag: String },
    FindFocused { tag: String },
    AdvanceFrame { timestamp: u64 },
}

// ---------------------------------------------------------------------------
// Builder methods
// ---------------------------------------------------------------------------

impl Command {
    pub fn none() -> Self { Self::None }

    pub fn batch(cmds: impl IntoIterator<Item = Command>) -> Self {
        Self::Batch(cmds.into_iter().collect())
    }

    pub fn exit() -> Self { Self::Exit }

    pub fn focus(id: &str) -> Self { Self::Focus(id.to_string()) }
    pub fn focus_next() -> Self { Self::FocusNext }
    pub fn focus_previous() -> Self { Self::FocusPrevious }

    pub fn send_after(delay: Duration, event: Event) -> Self {
        Self::SendAfter { delay, event: Box::new(event) }
    }

    pub fn cancel(tag: &str) -> Self {
        Self::Cancel { tag: tag.to_string() }
    }

    // -- Window shortcuts --

    pub fn close_window(id: &str) -> Self {
        Self::Window(WindowOp::Close(id.to_string()))
    }

    pub fn resize_window(id: &str, width: f32, height: f32) -> Self {
        Self::Window(WindowOp::Resize { window_id: id.to_string(), width, height })
    }

    pub fn move_window(id: &str, x: f32, y: f32) -> Self {
        Self::Window(WindowOp::Move { window_id: id.to_string(), x, y })
    }

    // -- Effect shortcuts --

    pub fn file_open(tag: &str) -> Self {
        Self::Effect { tag: tag.to_string(), request: EffectRequest::FileOpen(Default::default()) }
    }

    pub fn clipboard_read(tag: &str) -> Self {
        Self::Effect { tag: tag.to_string(), request: EffectRequest::ClipboardRead }
    }

    pub fn clipboard_write(tag: &str, text: &str) -> Self {
        Self::Effect {
            tag: tag.to_string(),
            request: EffectRequest::ClipboardWrite(text.to_string()),
        }
    }

    // -- Scroll shortcuts --

    pub fn scroll_to(target: &str, x: f32, y: f32) -> Self {
        Self::ScrollTo { target: target.to_string(), x, y }
    }

    // -- Widget command shortcuts --

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

#[derive(Debug)]
pub enum WindowOp {
    Close(String),
    Resize { window_id: String, width: f32, height: f32 },
    Move { window_id: String, x: f32, y: f32 },
    Maximize { window_id: String, maximized: bool },
    Minimize { window_id: String, minimized: bool },
    SetMode { window_id: String, mode: String },
    ToggleMaximize(String),
    ToggleDecorations(String),
    FocusWindow(String),
    SetLevel { window_id: String, level: String },
    DragWindow(String),
    DragResize { window_id: String, direction: String },
    RequestAttention { window_id: String, urgency: Option<String> },
    Screenshot { window_id: String, tag: String },
    SetResizable { window_id: String, resizable: bool },
    SetMinSize { window_id: String, width: f32, height: f32 },
    SetMaxSize { window_id: String, width: f32, height: f32 },
    EnableMousePassthrough(String),
    DisableMousePassthrough(String),
    ShowSystemMenu(String),
    SetIcon { window_id: String, data: Vec<u8>, width: u32, height: u32 },
}

#[derive(Debug)]
pub enum WindowQuery {
    GetSize { window_id: String, tag: String },
    GetPosition { window_id: String, tag: String },
    IsMaximized { window_id: String, tag: String },
    IsMinimized { window_id: String, tag: String },
    GetMode { window_id: String, tag: String },
    GetScaleFactor { window_id: String, tag: String },
    MonitorSize { window_id: String, tag: String },
}

#[derive(Debug)]
pub enum SystemOp {
    AllowAutomaticTabbing(bool),
}

#[derive(Debug)]
pub enum SystemQuery {
    GetTheme { tag: String },
    GetInfo { tag: String },
}

#[derive(Debug)]
pub enum EffectRequest {
    FileOpen(FileDialogOpts),
    FileOpenMultiple(FileDialogOpts),
    FileSave(FileDialogOpts),
    DirectorySelect(FileDialogOpts),
    DirectorySelectMultiple(FileDialogOpts),
    ClipboardRead,
    ClipboardWrite(String),
    ClipboardReadHtml,
    ClipboardWriteHtml { html: String, alt_text: Option<String> },
    ClipboardClear,
    ClipboardReadPrimary,
    ClipboardWritePrimary(String),
    Notification { title: String, body: String, opts: NotificationOpts },
}

#[derive(Debug, Default)]
pub struct FileDialogOpts {
    pub title: Option<String>,
    pub directory: Option<String>,
    pub filters: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Default)]
pub struct NotificationOpts {
    pub icon: Option<String>,
    pub timeout: Option<Duration>,
    pub urgency: Option<String>,
    pub sound: Option<bool>,
}

#[derive(Debug)]
pub enum ImageOp {
    Create { handle: String, data: Vec<u8> },
    CreateRaw { handle: String, width: u32, height: u32, pixels: Vec<u8> },
    Update { handle: String, data: Vec<u8> },
    UpdateRaw { handle: String, width: u32, height: u32, pixels: Vec<u8> },
    Delete(String),
    List { tag: String },
    Clear,
}

#[derive(Debug)]
pub enum PaneGridOp {
    Split { target: String, pane: String, axis: String, new_pane: String },
    Close { target: String, pane: String },
    Swap { target: String, a: String, b: String },
    Maximize { target: String, pane: String },
    Restore(String),
}

#[derive(Debug, Clone)]
pub struct WidgetCommandItem {
    pub node_id: String,
    pub op: String,
    pub payload: Value,
}
