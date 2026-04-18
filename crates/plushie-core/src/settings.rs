//! Application and window configuration.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;

use crate::types::Theme;

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
///
/// Passed to `App::handle_renderer_exit` before the runner returns.
/// The variants mirror the Elixir bridge's exit categorisation so
/// behaviour is consistent across SDKs.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ExitReason {
    /// Renderer crashed. `message` carries the I/O error or panic
    /// description; `code` is the subprocess exit code if we were
    /// able to reap it.
    Crash {
        /// Human-readable message.
        message: String,
        /// Error code.
        code: Option<i32>,
    },
    /// Lost connection to the renderer (pipe closed cleanly without
    /// a full message).
    ConnectionLost,
    /// Renderer shut down at our request (e.g. `Command::Exit`).
    Shutdown,
    /// No messages received within the configured heartbeat interval.
    HeartbeatTimeout,
    /// Auto-restart gave up after exhausting
    /// [`RestartPolicy::max_restarts`]. `last_reason` is the reason
    /// for the final restart attempt.
    MaxRestartsReached {
        /// Last reason.
        last_reason: Box<ExitReason>,
    },
    /// Dev-mode hot-reload requested a renderer swap after a
    /// successful widget-crate rebuild. Treated like a clean exit
    /// for restart-policy purposes: doesn't count against
    /// `max_restarts`, no backoff, just respawn.
    RendererSwap,
}

impl ExitReason {
    /// Short human-friendly label, useful for logs.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Crash { .. } => "crash",
            Self::ConnectionLost => "connection_lost",
            Self::Shutdown => "shutdown",
            Self::HeartbeatTimeout => "heartbeat_timeout",
            Self::MaxRestartsReached { .. } => "max_restarts_reached",
            Self::RendererSwap => "renderer_swap",
        }
    }
}

/// Restart policy for wire mode.
///
/// Returned from `App::restart_policy` to configure auto-restart
/// behaviour on renderer crashes.
///
/// `max_restarts = 0` disables auto-restart entirely; the first crash
/// delivers [`ExitReason::Crash`] to `App::handle_renderer_exit` and
/// the runner returns with `plushie::Error::RendererExit`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RestartPolicy {
    /// Maximum consecutive restart attempts before giving up.
    pub max_restarts: u32,
    /// Base delay for exponential backoff. Actual delay is
    /// `restart_delay * 2.pow(restart_count)`.
    pub restart_delay: Duration,
    /// If `Some`, a watchdog triggers a restart if no wire message is
    /// received within this interval. `None` disables heartbeats.
    pub heartbeat_interval: Option<Duration>,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 5,
            restart_delay: Duration::from_millis(100),
            heartbeat_interval: Some(Duration::from_secs(30)),
        }
    }
}
