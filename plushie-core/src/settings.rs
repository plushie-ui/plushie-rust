//! Application and window configuration.

use std::collections::HashMap;

use serde_json::Value;

/// Application-level settings.
///
/// All fields are optional. The renderer uses sensible defaults
/// when fields are omitted.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    /// The default font family name (e.g. `"monospace"`).
    pub default_font: Option<String>,
    /// Default text size in logical pixels.
    pub default_text_size: Option<f32>,
    /// Enable multi-sample anti-aliasing.
    pub antialiasing: Option<bool>,
    /// Enable vertical sync.
    pub vsync: Option<bool>,
    /// Global DPI scale factor override.
    pub scale_factor: Option<f32>,
    /// Application-wide theme.
    pub theme: Option<Theme>,
    /// Paths to font files to load at startup.
    pub fonts: Vec<String>,
    /// Maximum event rate in events per second (throttling).
    pub default_event_rate: Option<u32>,
    /// Per-widget-type configuration passed to native widgets.
    pub widget_config: HashMap<String, Value>,
}

/// A theme specification.
#[derive(Debug, Clone)]
pub enum Theme {
    /// Follow the OS light/dark preference.
    System,
    /// A built-in theme by name.
    Named(String),
    /// A custom palette.
    Custom(ThemePalette),
}

/// Custom theme palette colors.
#[derive(Debug, Clone, Default)]
pub struct ThemePalette {
    /// Background color for surfaces.
    pub background: Option<String>,
    /// Default text color.
    pub text: Option<String>,
    /// Primary accent color (buttons, links, highlights).
    pub primary: Option<String>,
    /// Success state color (confirmations, positive indicators).
    pub success: Option<String>,
    /// Warning state color (caution indicators).
    pub warning: Option<String>,
    /// Danger state color (errors, destructive actions).
    pub danger: Option<String>,
}

/// Per-window defaults. Returned from [`App::window_config`](crate::App::window_config).
#[derive(Debug, Clone, Default)]
pub struct WindowConfig {
    /// Window title bar text.
    pub title: Option<String>,
    /// Initial window width in logical pixels.
    pub width: Option<f32>,
    /// Initial window height in logical pixels.
    pub height: Option<f32>,
    /// Initial window position as (x, y) in logical pixels.
    pub position: Option<(f32, f32)>,
    /// Minimum window size as (width, height).
    pub min_size: Option<(f32, f32)>,
    /// Maximum window size as (width, height).
    pub max_size: Option<(f32, f32)>,
    /// Whether the window starts maximized.
    pub maximized: Option<bool>,
    /// Whether the window starts in fullscreen mode.
    pub fullscreen: Option<bool>,
    /// Whether the window is initially visible.
    pub visible: Option<bool>,
    /// Whether the user can resize the window.
    pub resizable: Option<bool>,
    /// Whether the window has title bar and borders.
    pub decorations: Option<bool>,
    /// Whether the window background is transparent.
    pub transparent: Option<bool>,
    /// Whether the window close button is shown.
    pub closeable: Option<bool>,
    /// Whether the window can be minimized.
    pub minimizable: Option<bool>,
    /// Blur the window background (platform-dependent).
    pub blur: Option<bool>,
    /// Window stacking level.
    pub level: Option<crate::ops::WindowLevel>,
    /// Whether closing the window exits the application.
    pub exit_on_close_request: Option<bool>,
    /// Max events per second for coalescable events.
    pub event_rate: Option<u32>,
    /// Accessibility annotations.
    pub a11y: Option<Value>,
    /// Per-window theme override.
    pub theme: Option<Theme>,
    /// Per-window DPI scale factor override.
    pub scale_factor: Option<f32>,
}

/// Reason the renderer process exited (wire mode only).
#[derive(Debug, Clone)]
pub enum ExitReason {
    /// Normal exit (renderer closed cleanly).
    Normal,
    /// Renderer crashed with an error message.
    Crash(String),
    /// Lost connection to the renderer.
    Disconnected,
}
