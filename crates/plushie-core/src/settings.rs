//! Application and window configuration.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::types::{PlushieType, Theme};

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
    /// Native widget type names this app requires to be present in
    /// the renderer. Validated during the Settings handshake: any
    /// missing names are surfaced via a `required_widgets_missing`
    /// diagnostic. Non-fatal by design; host SDKs decide whether to
    /// warn or halt based on the diagnostic.
    pub required_widgets: Vec<String>,
}

impl Settings {
    /// Encode the settings into the canonical wire-format JSON object.
    ///
    /// This is the single source of truth for the Settings JSON shape.
    /// Both wire mode (subprocess renderer over stdin/stdout) and
    /// direct mode (in-process renderer) feed the renderer through
    /// this same canonical shape, so any new field added to
    /// [`Settings`] must be handled here once and is automatically
    /// honoured by every code path that ingests the JSON.
    ///
    /// The shape mirrors what [`crate::protocol::IncomingMessage::Settings`]
    /// expects: e.g. `default_font` is an object with a `family` key,
    /// not a bare string. Fields whose value is `None` (or empty
    /// collection) are omitted from the output so the renderer
    /// applies its own defaults.
    ///
    /// `protocol_version` is intentionally not included here. Wire
    /// mode appends it to this Settings object before sending the
    /// handshake message; direct mode consumes this object without a
    /// protocol handshake.
    pub fn to_wire_json(&self) -> Value {
        let mut obj = Map::new();

        if let Some(ref font) = self.default_font {
            let mut font_obj = Map::new();
            font_obj.insert("family".to_string(), Value::String(font.clone()));
            obj.insert("default_font".to_string(), Value::Object(font_obj));
        }
        if let Some(size) = self.default_text_size {
            obj.insert("default_text_size".to_string(), serde_json::json!(size));
        }
        if let Some(antialiasing) = self.antialiasing {
            obj.insert("antialiasing".to_string(), Value::Bool(antialiasing));
        }
        if let Some(vsync) = self.vsync {
            obj.insert("vsync".to_string(), Value::Bool(vsync));
        }
        if let Some(scale) = self.scale_factor {
            obj.insert("scale_factor".to_string(), serde_json::json!(scale));
        }
        if let Some(rate) = self.default_event_rate {
            obj.insert("default_event_rate".to_string(), serde_json::json!(rate));
        }
        if !self.fonts.is_empty() {
            obj.insert("fonts".to_string(), serde_json::json!(self.fonts));
        }
        if !self.widget_config.is_empty() {
            obj.insert(
                "widget_config".to_string(),
                Value::Object(self.widget_config.clone().into_iter().collect()),
            );
        }
        if !self.required_widgets.is_empty() {
            obj.insert(
                "required_widgets".to_string(),
                serde_json::json!(self.required_widgets),
            );
        }
        if let Some(ref theme) = self.theme {
            obj.insert("theme".to_string(), Value::from(theme.wire_encode()));
        }

        Value::Object(obj)
    }
}

/// Per-window defaults. Returned from the SDK's `App::window_config`
/// (defined in the `plushie` crate, not here).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Theme;

    #[test]
    fn empty_settings_serialize_to_empty_object() {
        let json = Settings::default().to_wire_json();
        assert_eq!(json, serde_json::json!({}));
    }

    #[test]
    fn default_font_is_an_object_with_family() {
        // The renderer expects `default_font` as an object so it can
        // also carry a fallback chain. A bare string was a latent bug
        // that broke runtime resolution.
        let settings = Settings {
            default_font: Some("monospace".into()),
            ..Default::default()
        };
        let json = settings.to_wire_json();
        assert_eq!(
            json.get("default_font"),
            Some(&serde_json::json!({"family": "monospace"}))
        );
    }

    #[test]
    fn populated_settings_serialize_round_trip() {
        let settings = Settings {
            default_font: Some("monospace".into()),
            default_text_size: Some(15.0),
            antialiasing: Some(true),
            vsync: Some(false),
            scale_factor: Some(1.25),
            theme: Some(Theme::Named("dark".into())),
            fonts: vec!["/tmp/a.ttf".into()],
            default_event_rate: Some(60),
            widget_config: HashMap::from([("gauge".into(), serde_json::json!({"k": 1}))]),
            required_widgets: vec!["gauge".into()],
        };
        let json = settings.to_wire_json();
        let obj = json.as_object().expect("object");
        assert_eq!(
            obj.get("default_font"),
            Some(&serde_json::json!({"family": "monospace"}))
        );
        assert_eq!(obj.get("default_text_size"), Some(&serde_json::json!(15.0)));
        assert_eq!(obj.get("antialiasing"), Some(&serde_json::json!(true)));
        assert_eq!(obj.get("vsync"), Some(&serde_json::json!(false)));
        assert_eq!(obj.get("scale_factor"), Some(&serde_json::json!(1.25)));
        assert_eq!(obj.get("theme"), Some(&serde_json::json!("dark")));
        assert_eq!(obj.get("fonts"), Some(&serde_json::json!(["/tmp/a.ttf"])));
        assert_eq!(obj.get("default_event_rate"), Some(&serde_json::json!(60)));
        assert_eq!(
            obj.get("widget_config"),
            Some(&serde_json::json!({"gauge": {"k": 1}}))
        );
        assert_eq!(
            obj.get("required_widgets"),
            Some(&serde_json::json!(["gauge"]))
        );
        assert!(obj.get("protocol_version").is_none());
    }
}
