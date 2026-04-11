//! Renderer operations and supporting types.
//!
//! [`RendererOp`] represents every operation the renderer can execute.
//! These are the typed commands that flow from the SDK to the renderer
//! with zero serialization overhead in direct mode.

use std::time::Duration;

use serde_json::Value;

// ---------------------------------------------------------------------------
// RendererOp
// ---------------------------------------------------------------------------

/// An operation the renderer can execute.
///
/// In direct mode, these are passed in-process with zero serialization.
/// In wire mode, they are serialized at the process boundary.
#[derive(Debug)]
pub enum RendererOp {
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
    /// Query window state.
    WindowQuery(WindowQuery),

    // -- System --
    /// Perform a system-level operation.
    SystemOp(SystemOp),
    /// Query system state.
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

    // -- Testing / debugging --
    /// Request a hash of the current widget tree.
    TreeHash { tag: String },
    /// Query which widget currently has keyboard focus.
    FindFocused { tag: String },
    /// Advance the animation frame to the given timestamp.
    AdvanceFrame { timestamp: u64 },
}

// ---------------------------------------------------------------------------
// Window operations
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
    /// Begin an interactive window drag.
    DragWindow(String),
    /// Begin an interactive window resize from the given edge/direction.
    DragResize { window_id: String, direction: String },
    /// Request user attention (taskbar flash or similar).
    RequestAttention { window_id: String, urgency: Option<String> },
    /// Take a screenshot of a window.
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
    /// Set window resize increment constraints.
    SetResizeIncrements { window_id: String, width: f32, height: f32 },
}

/// A query for window state.
#[derive(Debug)]
pub enum WindowQuery {
    GetSize { window_id: String, tag: String },
    GetPosition { window_id: String, tag: String },
    IsMaximized { window_id: String, tag: String },
    IsMinimized { window_id: String, tag: String },
    GetMode { window_id: String, tag: String },
    GetScaleFactor { window_id: String, tag: String },
    MonitorSize { window_id: String, tag: String },
    RawId { window_id: String, tag: String },
}

// ---------------------------------------------------------------------------
// System operations
// ---------------------------------------------------------------------------

/// A system-level operation.
#[derive(Debug)]
pub enum SystemOp {
    /// Enable or disable automatic window tabbing (macOS).
    AllowAutomaticTabbing(bool),
}

/// A system-level query.
#[derive(Debug)]
pub enum SystemQuery {
    /// Query the current OS theme (light/dark).
    GetTheme { tag: String },
    /// Query system information (OS, renderer version, etc.).
    GetInfo { tag: String },
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/// A platform effect request.
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

/// Options for file and directory dialogs.
#[derive(Debug, Default)]
pub struct FileDialogOpts {
    /// Dialog window title.
    pub title: Option<String>,
    /// Initial directory to open in.
    pub directory: Option<String>,
    /// File type filters as `(label, [extensions])` pairs.
    pub filters: Vec<(String, Vec<String>)>,
    /// Default file name for save dialogs.
    pub default_name: Option<String>,
}

impl FileDialogOpts {
    pub fn new() -> Self { Self::default() }

    /// Set the dialog window title.
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    /// Set the initial directory to open in.
    pub fn directory(mut self, dir: &str) -> Self {
        self.directory = Some(dir.to_string());
        self
    }

    /// Add a file type filter (e.g. `("Images", &["png", "jpg"])`).
    pub fn filter(mut self, label: &str, extensions: &[&str]) -> Self {
        self.filters.push((
            label.to_string(),
            extensions.iter().map(|e| e.to_string()).collect(),
        ));
        self
    }

    /// Set the default file name (for save dialogs).
    pub fn default_name(mut self, name: &str) -> Self {
        self.default_name = Some(name.to_string());
        self
    }
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
    /// Sound name to play with the notification.
    pub sound: Option<String>,
}

impl NotificationOpts {
    pub fn new() -> Self { Self::default() }

    /// Set the notification icon path or name.
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Set how long the notification should be displayed.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the urgency level (`"low"`, `"normal"`, or `"critical"`).
    pub fn urgency(mut self, urgency: &str) -> Self {
        self.urgency = Some(urgency.to_string());
        self
    }

    /// Set the sound name to play.
    pub fn sound(mut self, sound: &str) -> Self {
        self.sound = Some(sound.to_string());
        self
    }
}

// ---------------------------------------------------------------------------
// Image operations
// ---------------------------------------------------------------------------

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
    /// List all loaded image handles.
    List { tag: String },
    /// Delete all loaded images.
    Clear,
}

// ---------------------------------------------------------------------------
// PaneGrid operations
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Widget commands
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Wire serialization helpers
// ---------------------------------------------------------------------------

/// Convert an [`EffectRequest`] to the wire format `(kind, payload)`.
pub fn effect_request_to_wire(request: &EffectRequest) -> (&'static str, Value) {
    use serde_json::json;
    match request {
        EffectRequest::FileOpen(opts) => ("file_open", file_dialog_opts_to_value(opts)),
        EffectRequest::FileOpenMultiple(opts) => ("file_open_multiple", file_dialog_opts_to_value(opts)),
        EffectRequest::FileSave(opts) => ("file_save", file_dialog_opts_to_value(opts)),
        EffectRequest::DirectorySelect(opts) => ("directory_select", file_dialog_opts_to_value(opts)),
        EffectRequest::DirectorySelectMultiple(opts) => ("directory_select_multiple", file_dialog_opts_to_value(opts)),
        EffectRequest::ClipboardRead => ("clipboard_read", json!({})),
        EffectRequest::ClipboardWrite(text) => ("clipboard_write", json!({"text": text})),
        EffectRequest::ClipboardReadHtml => ("clipboard_read_html", json!({})),
        EffectRequest::ClipboardWriteHtml { html, alt_text } => {
            let mut payload = json!({"html": html});
            if let Some(alt) = alt_text {
                payload["alt_text"] = json!(alt);
            }
            ("clipboard_write_html", payload)
        }
        EffectRequest::ClipboardClear => ("clipboard_clear", json!({})),
        EffectRequest::ClipboardReadPrimary => ("clipboard_read_primary", json!({})),
        EffectRequest::ClipboardWritePrimary(text) => ("clipboard_write_primary", json!({"text": text})),
        EffectRequest::Notification { title, body, opts } => {
            let mut payload = json!({"title": title, "body": body});
            if let Some(ref icon) = opts.icon { payload["icon"] = json!(icon); }
            if let Some(ref timeout) = opts.timeout { payload["timeout"] = json!(timeout.as_millis() as u64); }
            if let Some(ref urgency) = opts.urgency { payload["urgency"] = json!(urgency); }
            if let Some(ref sound) = opts.sound { payload["sound"] = json!(sound); }
            ("notification", payload)
        }
    }
}

fn file_dialog_opts_to_value(opts: &FileDialogOpts) -> Value {
    use serde_json::json;
    let mut payload = json!({});
    if let Some(ref title) = opts.title { payload["title"] = json!(title); }
    if let Some(ref dir) = opts.directory { payload["directory"] = json!(dir); }
    if !opts.filters.is_empty() {
        let filters: Vec<Value> = opts.filters.iter()
            .map(|(label, exts)| json!([label, exts.join(";")]))
            .collect();
        payload["filters"] = json!(filters);
    }
    if let Some(ref name) = opts.default_name { payload["default_name"] = json!(name); }
    payload
}
