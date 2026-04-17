//! Renderer operations and supporting types.
//!
//! [`RendererOp`] represents every operation the renderer can execute.
//! These are the typed commands that flow from the SDK to the renderer
//! with zero serialization overhead in direct mode.

use std::fmt;
use std::time::Duration;

use serde_json::Value;

// ---------------------------------------------------------------------------
// Typed enums for string-based parameters
// ---------------------------------------------------------------------------

/// Window display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    Windowed,
    Fullscreen,
}

impl fmt::Display for WindowMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Windowed => f.write_str("windowed"),
            Self::Fullscreen => f.write_str("fullscreen"),
        }
    }
}

/// Window stacking level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowLevel {
    Normal,
    AlwaysOnTop,
    AlwaysOnBottom,
}

impl fmt::Display for WindowLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => f.write_str("normal"),
            Self::AlwaysOnTop => f.write_str("always_on_top"),
            Self::AlwaysOnBottom => f.write_str("always_on_bottom"),
        }
    }
}

/// Notification urgency level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationUrgency {
    Low,
    Normal,
    Critical,
}

impl fmt::Display for NotificationUrgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => f.write_str("low"),
            Self::Normal => f.write_str("normal"),
            Self::Critical => f.write_str("critical"),
        }
    }
}

// ---------------------------------------------------------------------------
// RendererOp
// ---------------------------------------------------------------------------

/// An operation the renderer can execute.
///
/// In direct mode, these are passed in-process with zero serialization.
/// In wire mode, they are serialized at the process boundary.
#[derive(Debug)]
pub enum RendererOp {
    // -- Widget-targeted command --
    /// Send a command to a widget by ID.
    ///
    /// Subsumes focus, scroll, text cursor, pane grid, and native
    /// widget operations. The `family` string identifies the
    /// operation; the `value` carries typed payload data.
    Command {
        id: String,
        family: String,
        value: Value,
    },
    /// Send multiple widget commands in a batch.
    Commands(Vec<WidgetCommand>),

    // -- Focus (global, no target widget) --
    /// Move keyboard focus to the next focusable widget.
    FocusNext,
    /// Move keyboard focus to the previous focusable widget.
    FocusPrevious,

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
    Effect {
        tag: String,
        request: EffectRequest,
        /// Optional per-effect timeout override. When `None`, the runner
        /// uses `effect_tracker::default_timeout` based on the effect kind.
        timeout: Option<Duration>,
    },

    // -- Images --
    /// Perform an image operation (create, update, delete).
    Image(ImageOp),

    // -- Accessibility --
    /// Announce text to screen readers.
    Announce(String),
    /// Load a font from raw byte data.
    LoadFont(Vec<u8>),

    // -- Subscriptions --
    /// Subscribe to a renderer event source.
    Subscribe {
        kind: String,
        tag: String,
        max_rate: Option<u32>,
        window_id: Option<String>,
    },
    /// Unsubscribe from a renderer event source.
    Unsubscribe { kind: String, tag: String },

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
    Resize {
        window_id: String,
        width: f32,
        height: f32,
    },
    /// Move a window to the given logical position.
    Move { window_id: String, x: f32, y: f32 },
    /// Set or unset the maximized state.
    Maximize { window_id: String, maximized: bool },
    /// Set or unset the minimized state.
    Minimize { window_id: String, minimized: bool },
    /// Set the window display mode.
    SetMode { window_id: String, mode: WindowMode },
    /// Toggle between maximized and restored states.
    ToggleMaximize(String),
    /// Toggle window decorations (title bar, borders).
    ToggleDecorations(String),
    /// Bring a window to the front and give it focus.
    FocusWindow(String),
    /// Set the window stacking level.
    SetLevel {
        window_id: String,
        level: WindowLevel,
    },
    /// Begin an interactive window drag.
    DragWindow(String),
    /// Begin an interactive window resize from the given edge/direction.
    DragResize {
        window_id: String,
        direction: String,
    },
    /// Request user attention (taskbar flash or similar).
    RequestAttention {
        window_id: String,
        urgency: Option<String>,
    },
    /// Take a screenshot of a window.
    Screenshot { window_id: String, tag: String },
    /// Set whether the window is user-resizable.
    SetResizable { window_id: String, resizable: bool },
    /// Set the minimum window size.
    SetMinSize {
        window_id: String,
        width: f32,
        height: f32,
    },
    /// Set the maximum window size.
    SetMaxSize {
        window_id: String,
        width: f32,
        height: f32,
    },
    /// Allow mouse events to pass through the window.
    EnableMousePassthrough(String),
    /// Stop mouse events from passing through the window.
    DisableMousePassthrough(String),
    /// Show the native system menu (right-click title bar menu).
    ShowSystemMenu(String),
    /// Set the window icon from raw RGBA pixel data.
    SetIcon {
        window_id: String,
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// Set window resize increment constraints.
    SetResizeIncrements {
        window_id: String,
        width: f32,
        height: f32,
    },
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
    ClipboardWriteHtml {
        html: String,
        alt_text: Option<String>,
    },
    ClipboardClear,
    ClipboardReadPrimary,
    ClipboardWritePrimary(String),
    Notification {
        title: String,
        body: String,
        opts: NotificationOpts,
    },
}

impl EffectRequest {
    /// The wire-format kind string for this effect request.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::FileOpen(_) => "file_open",
            Self::FileOpenMultiple(_) => "file_open_multiple",
            Self::FileSave(_) => "file_save",
            Self::DirectorySelect(_) => "directory_select",
            Self::DirectorySelectMultiple(_) => "directory_select_multiple",
            Self::ClipboardRead => "clipboard_read",
            Self::ClipboardWrite(_) => "clipboard_write",
            Self::ClipboardReadHtml => "clipboard_read_html",
            Self::ClipboardWriteHtml { .. } => "clipboard_write_html",
            Self::ClipboardClear => "clipboard_clear",
            Self::ClipboardReadPrimary => "clipboard_read_primary",
            Self::ClipboardWritePrimary(_) => "clipboard_write_primary",
            Self::Notification { .. } => "notification",
        }
    }
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
    pub fn new() -> Self {
        Self::default()
    }

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
    /// Urgency level for the notification.
    pub urgency: Option<NotificationUrgency>,
    /// Sound name to play with the notification.
    pub sound: Option<String>,
}

impl NotificationOpts {
    pub fn new() -> Self {
        Self::default()
    }

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

    /// Set the urgency level.
    pub fn urgency(mut self, urgency: NotificationUrgency) -> Self {
        self.urgency = Some(urgency);
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
    CreateRaw {
        handle: String,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    /// Replace an existing image with new encoded bytes.
    Update { handle: String, data: Vec<u8> },
    /// Replace an existing image with new raw RGBA pixel data.
    UpdateRaw {
        handle: String,
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    /// Delete an image by handle.
    Delete(String),
    /// List all loaded image handles.
    List { tag: String },
    /// Delete all loaded images.
    Clear,
}

// ---------------------------------------------------------------------------
// Widget commands
// ---------------------------------------------------------------------------

/// A single widget-targeted command.
///
/// Used as the element type for atomic widget batches
/// ([`RendererOp::Commands`]) and as the payload of single widget
/// commands built via [`RendererOp::Command`]. Construct via
/// [`WidgetCommand::new`] (typed) or [`WidgetCommand::raw`]
/// (family + value).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WidgetCommand {
    /// The target widget's scoped ID.
    pub id: String,
    /// The command family name.
    pub family: String,
    /// Command-specific data.
    #[serde(default)]
    pub value: Value,
}

impl WidgetCommand {
    /// Build a typed widget command. The family name and wire value
    /// are derived from the typed command via [`WidgetCommandEncode`].
    pub fn new<C: crate::WidgetCommandEncode>(id: &str, cmd: C) -> Self {
        let (family, value) = cmd.to_wire();
        Self {
            id: id.to_string(),
            family: family.to_string(),
            value: Value::from(value),
        }
    }

    /// Build a widget command from raw family string and value.
    pub fn raw(id: &str, family: &str, value: impl Into<Value>) -> Self {
        Self {
            id: id.to_string(),
            family: family.to_string(),
            value: value.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire serialization helpers
// ---------------------------------------------------------------------------

/// Convert an [`EffectRequest`] to the wire format `(kind, payload)`.
pub fn effect_request_to_wire(request: &EffectRequest) -> (&'static str, Value) {
    use serde_json::json;
    match request {
        EffectRequest::FileOpen(opts) => ("file_open", file_dialog_opts_to_value(opts)),
        EffectRequest::FileOpenMultiple(opts) => {
            ("file_open_multiple", file_dialog_opts_to_value(opts))
        }
        EffectRequest::FileSave(opts) => ("file_save", file_dialog_opts_to_value(opts)),
        EffectRequest::DirectorySelect(opts) => {
            ("directory_select", file_dialog_opts_to_value(opts))
        }
        EffectRequest::DirectorySelectMultiple(opts) => {
            ("directory_select_multiple", file_dialog_opts_to_value(opts))
        }
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
        EffectRequest::ClipboardWritePrimary(text) => {
            ("clipboard_write_primary", json!({"text": text}))
        }
        EffectRequest::Notification { title, body, opts } => {
            let mut payload = json!({"title": title, "body": body});
            if let Some(ref icon) = opts.icon {
                payload["icon"] = json!(icon);
            }
            if let Some(ref timeout) = opts.timeout {
                payload["timeout"] = json!(timeout.as_millis() as u64);
            }
            if let Some(ref urgency) = opts.urgency {
                payload["urgency"] = json!(urgency);
            }
            if let Some(ref sound) = opts.sound {
                payload["sound"] = json!(sound);
            }
            ("notification", payload)
        }
    }
}

fn file_dialog_opts_to_value(opts: &FileDialogOpts) -> Value {
    use serde_json::json;
    let mut payload = json!({});
    if let Some(ref title) = opts.title {
        payload["title"] = json!(title);
    }
    if let Some(ref dir) = opts.directory {
        payload["directory"] = json!(dir);
    }
    if !opts.filters.is_empty() {
        let filters: Vec<Value> = opts
            .filters
            .iter()
            .map(|(label, exts)| json!([label, exts.join(";")]))
            .collect();
        payload["filters"] = json!(filters);
    }
    if let Some(ref name) = opts.default_name {
        payload["default_name"] = json!(name);
    }
    payload
}

/// Convert wire format `(kind, payload)` to an [`EffectRequest`].
///
/// Returns `None` for unrecognized kinds.
pub fn effect_request_from_wire(kind: &str, payload: &Value) -> Option<EffectRequest> {
    match kind {
        "file_open" => Some(EffectRequest::FileOpen(file_dialog_opts_from_value(
            payload,
        ))),
        "file_open_multiple" => Some(EffectRequest::FileOpenMultiple(
            file_dialog_opts_from_value(payload),
        )),
        "file_save" => Some(EffectRequest::FileSave(file_dialog_opts_from_value(
            payload,
        ))),
        "directory_select" => Some(EffectRequest::DirectorySelect(file_dialog_opts_from_value(
            payload,
        ))),
        "directory_select_multiple" => Some(EffectRequest::DirectorySelectMultiple(
            file_dialog_opts_from_value(payload),
        )),
        "clipboard_read" => Some(EffectRequest::ClipboardRead),
        "clipboard_write" => {
            let text = payload
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            Some(EffectRequest::ClipboardWrite(text.to_string()))
        }
        "clipboard_read_html" => Some(EffectRequest::ClipboardReadHtml),
        "clipboard_write_html" => {
            let html = payload
                .get("html")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let alt_text = payload
                .get("alt_text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(EffectRequest::ClipboardWriteHtml { html, alt_text })
        }
        "clipboard_clear" => Some(EffectRequest::ClipboardClear),
        "clipboard_read_primary" => Some(EffectRequest::ClipboardReadPrimary),
        "clipboard_write_primary" => {
            let text = payload
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            Some(EffectRequest::ClipboardWritePrimary(text.to_string()))
        }
        "notification" => {
            let title = payload
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let body = payload
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let mut opts = NotificationOpts::default();
            if let Some(icon) = payload.get("icon").and_then(|v| v.as_str()) {
                opts.icon = Some(icon.to_string());
            }
            if let Some(ms) = payload.get("timeout").and_then(|v| v.as_u64()) {
                opts.timeout = Some(Duration::from_millis(ms));
            }
            if let Some(urgency_val) = payload.get("urgency") {
                opts.urgency = serde_json::from_value(urgency_val.clone()).ok();
            }
            if let Some(sound) = payload.get("sound").and_then(|v| v.as_str()) {
                opts.sound = Some(sound.to_string());
            }
            Some(EffectRequest::Notification { title, body, opts })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// PlushieType impls for operation enums
// ---------------------------------------------------------------------------

impl crate::types::PlushieType for WindowLevel {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "normal" => Some(Self::Normal),
            "always_on_top" => Some(Self::AlwaysOnTop),
            "always_on_bottom" => Some(Self::AlwaysOnBottom),
            _ => None,
        }
    }

    fn wire_encode(&self) -> crate::protocol::PropValue {
        crate::protocol::PropValue::Str(
            match self {
                Self::Normal => "normal",
                Self::AlwaysOnTop => "always_on_top",
                Self::AlwaysOnBottom => "always_on_bottom",
            }
            .into(),
        )
    }

    fn type_name() -> &'static str {
        "window_level"
    }
}

fn file_dialog_opts_from_value(payload: &Value) -> FileDialogOpts {
    let mut filters = Vec::new();
    if let Some(filter_arr) = payload.get("filters").and_then(|v| v.as_array()) {
        for filter in filter_arr {
            if let Some(pair) = filter.as_array()
                && pair.len() >= 2
                && let (Some(name), Some(ext)) = (pair[0].as_str(), pair[1].as_str())
            {
                let extensions: Vec<String> = ext
                    .split(';')
                    .map(|e| e.trim().trim_start_matches("*.").to_string())
                    .collect();
                filters.push((name.to_string(), extensions));
            }
        }
    }
    FileDialogOpts {
        title: payload
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        directory: payload
            .get("directory")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        default_name: payload
            .get("default_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        filters,
    }
}
