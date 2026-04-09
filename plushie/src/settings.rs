//! Application and window configuration.

use std::collections::HashMap;

use serde_json::Value;

use crate::types::{Color, Font};

/// Application-level settings. Returned from [`App::settings`](crate::App::settings).
///
/// All fields are optional. The renderer uses sensible defaults
/// when fields are omitted.
#[derive(Debug, Clone, Default)]
pub struct Settings {
    pub default_font: Option<Font>,
    pub default_text_size: Option<f32>,
    pub antialiasing: Option<bool>,
    pub vsync: Option<bool>,
    pub scale_factor: Option<f32>,
    pub theme: Option<Theme>,
    pub fonts: Vec<String>,
    pub default_event_rate: Option<u32>,
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
    pub background: Option<Color>,
    pub text: Option<Color>,
    pub primary: Option<Color>,
    pub success: Option<Color>,
    pub danger: Option<Color>,
}

/// Per-window defaults. Returned from [`App::window_config`](crate::App::window_config).
#[derive(Debug, Clone, Default)]
pub struct WindowConfig {
    pub title: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub position: Option<(f32, f32)>,
    pub min_size: Option<(f32, f32)>,
    pub max_size: Option<(f32, f32)>,
    pub maximized: Option<bool>,
    pub fullscreen: Option<bool>,
    pub visible: Option<bool>,
    pub resizable: Option<bool>,
    pub decorations: Option<bool>,
    pub transparent: Option<bool>,
    pub theme: Option<Theme>,
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
