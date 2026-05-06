//! Renderer operations and supporting types.
//!
//! [`RendererOp`] represents every operation the renderer can execute.
//! These are the typed commands that flow from the SDK to the renderer
//! with zero serialization overhead in direct mode.

use std::fmt;
use std::time::Duration;

use serde_json::Map;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Typed enums for string-based parameters
// ---------------------------------------------------------------------------

/// Window display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    /// Windowed.
    Windowed,
    /// Fullscreen.
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
    /// Normal.
    Normal,
    /// Always On Top.
    AlwaysOnTop,
    /// Always On Bottom.
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
    /// Low.
    Low,
    /// Normal.
    Normal,
    /// Critical.
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
#[non_exhaustive]
pub enum RendererOp {
    // -- Widget-targeted command --
    /// Send a command to a widget by ID.
    ///
    /// Subsumes focus, scroll, text cursor, pane grid, and native
    /// widget operations. The `family` string identifies the
    /// operation; the `value` carries typed payload data.
    Command {
        /// Target widget ID.
        id: String,
        /// Event/command family identifier.
        family: String,
        /// Typed payload value.
        value: Value,
    },
    /// Send multiple widget commands in a batch.
    Commands(Vec<WidgetCommand>),

    // -- Focus (global, no target widget) --
    /// Move keyboard focus to the next focusable widget.
    FocusNext,
    /// Move keyboard focus to the previous focusable widget.
    FocusPrevious,
    /// Move keyboard focus to the next focusable widget within the
    /// given scope. The scope is a widget ID; focus wraps within the
    /// subtree rooted at that widget rather than walking the full
    /// tree. Use for modal focus traps, menus, and other scoped
    /// keyboard-navigation containers.
    FocusNextWithin {
        /// Scope widget ID that bounds the operation.
        scope: String,
    },
    /// Move keyboard focus to the previous focusable widget within
    /// the given scope.
    FocusPreviousWithin {
        /// Scope widget ID that bounds the operation.
        scope: String,
    },

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
        /// Correlation tag used for matching responses.
        tag: String,
        /// Effect request payload.
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
    ///
    /// `politeness` controls whether the announcement interrupts
    /// ongoing speech (assertive) or queues after the current
    /// utterance (polite). App code typically wants polite for
    /// status messages and toast feedback; assertive is reserved
    /// for urgent context that must reach the user immediately.
    Announce {
        /// Text payload.
        text: String,
        /// Screen-reader politeness (polite vs assertive).
        politeness: crate::types::Live,
    },
    /// Load a font from raw byte data.
    ///
    /// `family` is the name the app will use when referring to this font
    /// (via `default_font.family` in Settings or in widget font props).
    /// The renderer records the family in the loaded-font registry so
    /// `resolve_font_with_fallback` can match the name without parsing
    /// font metadata.
    LoadFont {
        /// The family name the app will use to reference this font.
        family: String,
        /// Font file bytes (TrueType, OpenType, or TrueType Collection).
        bytes: Vec<u8>,
    },

    // -- Subscriptions --
    /// Subscribe to a renderer event source.
    Subscribe {
        /// Event kind string used on the wire.
        kind: String,
        /// Correlation tag used for matching responses.
        tag: String,
        /// Optional max delivery rate (events per second).
        max_rate: Option<u32>,
        /// Target window ID.
        window_id: Option<String>,
    },
    /// Unsubscribe from a renderer event source.
    Unsubscribe {
        /// Event kind string used on the wire.
        kind: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },

    // -- Testing / debugging --
    /// Request a hash of the current widget tree.
    TreeHash {
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Query which widget currently has keyboard focus.
    FindFocused {
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Advance renderer-side animation to the given timestamp in
    /// headless/mock wire testing.
    ///
    /// Windowed daemon mode is driven by iced frame ticks instead and
    /// ignores this operation.
    AdvanceFrame {
        /// Timestamp in milliseconds.
        timestamp: u64,
    },
}

// ---------------------------------------------------------------------------
// Window operations
// ---------------------------------------------------------------------------

/// A window management operation.
///
/// Covers the full lifecycle (open, update props, close) plus every
/// in-flight state change the renderer understands. Variants carry
/// the typed data they need; the renderer dispatches on this enum
/// rather than matching on string op names.
#[derive(Debug)]
#[non_exhaustive]
pub enum WindowOp {
    /// Open a new window with the given initial settings.
    ///
    /// `settings` is a JSON object with the subset of
    /// [`WINDOW_PROP_KEYS`] keys the host wants to specify; any
    /// unspecified field falls back to iced's defaults. Runtime-only
    /// fields like `icon_data` are nested under their usual keys.
    Open {
        /// Target window ID.
        window_id: String,
        /// Initial window settings as a JSON object.
        settings: Value,
    },
    /// Apply in-place changes to an already-open window.
    ///
    /// Only keys present in `settings` are applied; the renderer
    /// leaves everything else untouched. Used when a surviving
    /// window's node props change between renders.
    Update {
        /// Target window ID.
        window_id: String,
        /// Subset of window settings to apply.
        settings: Value,
    },
    /// Close a window.
    Close(String),
    /// Resize a window to the given logical dimensions.
    Resize {
        /// Target window ID.
        window_id: String,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
        height: f32,
    },
    /// Move a window to the given logical position.
    Move {
        /// Target window ID.
        window_id: String,
        /// X coordinate.
        x: f32,
        /// Y coordinate.
        y: f32,
    },
    /// Set or unset the maximized state.
    Maximize {
        /// Target window ID.
        window_id: String,
        /// Whether the window is maximized.
        maximized: bool,
    },
    /// Set or unset the minimized state.
    Minimize {
        /// Target window ID.
        window_id: String,
        /// Whether the window is minimized.
        minimized: bool,
    },
    /// Set the window display mode.
    SetMode {
        /// Target window ID.
        window_id: String,
        /// Mode selector.
        mode: WindowMode,
    },
    /// Toggle between maximized and restored states.
    ToggleMaximize(String),
    /// Toggle window decorations (title bar, borders).
    ToggleDecorations(String),
    /// Bring a window to the front and give it focus.
    FocusWindow(String),
    /// Set the window stacking level.
    SetLevel {
        /// Target window ID.
        window_id: String,
        /// Level selector.
        level: WindowLevel,
    },
    /// Begin an interactive window drag.
    DragWindow(String),
    /// Begin an interactive window resize from the given edge/direction.
    DragResize {
        /// Target window ID.
        window_id: String,
        /// Direction of the operation.
        direction: String,
    },
    /// Request user attention (taskbar flash or similar).
    RequestAttention {
        /// Target window ID.
        window_id: String,
        /// Notification urgency level.
        urgency: Option<NotificationUrgency>,
    },
    /// Take a screenshot of a window.
    Screenshot {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Set whether the window is user-resizable.
    SetResizable {
        /// Target window ID.
        window_id: String,
        /// Whether the window is resizable.
        resizable: bool,
    },
    /// Set the minimum window size.
    SetMinSize {
        /// Target window ID.
        window_id: String,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
        height: f32,
    },
    /// Set the maximum window size.
    SetMaxSize {
        /// Target window ID.
        window_id: String,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
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
        /// Target window ID.
        window_id: String,
        /// Raw bytes (pixels, font, etc.).
        data: Vec<u8>,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// Set window resize increment constraints.
    SetResizeIncrements {
        /// Target window ID.
        window_id: String,
        /// Width in pixels.
        width: f32,
        /// Height in pixels.
        height: f32,
    },
}

/// A query for window state.
#[derive(Debug)]
#[non_exhaustive]
pub enum WindowQuery {
    /// Get Size.
    GetSize {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Get Position.
    GetPosition {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Is Maximized.
    IsMaximized {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Is Minimized.
    IsMinimized {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Get Mode.
    GetMode {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Get Scale Factor.
    GetScaleFactor {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Monitor Size.
    MonitorSize {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Raw Id.
    RawId {
        /// Target window ID.
        window_id: String,
        /// Correlation tag used for matching responses.
        tag: String,
    },
}

impl WindowOp {
    /// Build a typed [`WindowOp`] from the wire-protocol `{op, window_id,
    /// payload}` triple. Returns `None` for unrecognised op strings so the
    /// caller can log a diagnostic and continue.
    pub fn from_wire(op: &str, window_id: &str, payload: &Value) -> Option<Self> {
        let wid = || window_id.to_string();
        let f = |key: &str, default: f32| -> f32 {
            payload
                .get(key)
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(default)
        };
        let b = |key: &str, default: bool| -> bool {
            payload
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(default)
        };
        let s = |key: &str| -> String {
            payload
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        match op {
            "open" => Some(Self::Open {
                window_id: wid(),
                settings: payload.clone(),
            }),
            "update" => Some(Self::Update {
                window_id: wid(),
                settings: payload.clone(),
            }),
            "close" => Some(Self::Close(wid())),
            "resize" => Some(Self::Resize {
                window_id: wid(),
                width: f("width", 800.0),
                height: f("height", 600.0),
            }),
            "move" => Some(Self::Move {
                window_id: wid(),
                x: f("x", 0.0),
                y: f("y", 0.0),
            }),
            "maximize" => Some(Self::Maximize {
                window_id: wid(),
                maximized: b("maximized", true),
            }),
            "minimize" => Some(Self::Minimize {
                window_id: wid(),
                minimized: b("minimized", true),
            }),
            "set_mode" => {
                let mode = payload
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .map(|s| match s {
                        "fullscreen" => WindowMode::Fullscreen,
                        _ => WindowMode::Windowed,
                    })
                    .unwrap_or(WindowMode::Windowed);
                Some(Self::SetMode {
                    window_id: wid(),
                    mode,
                })
            }
            "toggle_maximize" => Some(Self::ToggleMaximize(wid())),
            "toggle_decorations" => Some(Self::ToggleDecorations(wid())),
            "gain_focus" => Some(Self::FocusWindow(wid())),
            "set_level" => {
                let level = payload
                    .get("level")
                    .and_then(|v| v.as_str())
                    .map(|s| match s {
                        "always_on_top" => WindowLevel::AlwaysOnTop,
                        "always_on_bottom" => WindowLevel::AlwaysOnBottom,
                        _ => WindowLevel::Normal,
                    })
                    .unwrap_or(WindowLevel::Normal);
                Some(Self::SetLevel {
                    window_id: wid(),
                    level,
                })
            }
            "drag" => Some(Self::DragWindow(wid())),
            "drag_resize" => Some(Self::DragResize {
                window_id: wid(),
                direction: s("direction"),
            }),
            "request_attention" => {
                let urgency =
                    payload
                        .get("urgency")
                        .and_then(|v| v.as_str())
                        .and_then(|s| match s {
                            "low" => Some(NotificationUrgency::Low),
                            "normal" => Some(NotificationUrgency::Normal),
                            "critical" => Some(NotificationUrgency::Critical),
                            _ => None,
                        });
                Some(Self::RequestAttention {
                    window_id: wid(),
                    urgency,
                })
            }
            "screenshot" => Some(Self::Screenshot {
                window_id: wid(),
                tag: s("tag"),
            }),
            "set_resizable" => Some(Self::SetResizable {
                window_id: wid(),
                resizable: b("resizable", true),
            }),
            "set_min_size" => Some(Self::SetMinSize {
                window_id: wid(),
                width: f("width", 0.0),
                height: f("height", 0.0),
            }),
            "set_max_size" => Some(Self::SetMaxSize {
                window_id: wid(),
                width: f("width", 0.0),
                height: f("height", 0.0),
            }),
            "mouse_passthrough" => {
                let enabled = b("enabled", true);
                if enabled {
                    Some(Self::EnableMousePassthrough(wid()))
                } else {
                    Some(Self::DisableMousePassthrough(wid()))
                }
            }
            "show_system_menu" => Some(Self::ShowSystemMenu(wid())),
            "set_icon" => {
                use base64::Engine as _;
                let b64 = payload.get("data").and_then(|v| v.as_str())?;
                let data = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
                let width = payload.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let height = payload.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                Some(Self::SetIcon {
                    window_id: wid(),
                    data,
                    width,
                    height,
                })
            }
            "set_resize_increments" => Some(Self::SetResizeIncrements {
                window_id: wid(),
                width: f("width", 0.0),
                height: f("height", 0.0),
            }),
            _ => None,
        }
    }

    /// Emit the wire-protocol `(op, window_id, payload)` triple for this
    /// typed WindowOp. Used by [`crate::ops::WindowOp`] consumers on the
    /// SDK side that speak the JSON wire format to the renderer.
    pub fn to_wire(&self) -> (&'static str, String, Value) {
        use serde_json::json;
        match self {
            Self::Open {
                window_id,
                settings,
            } => ("open", window_id.clone(), settings.clone()),
            Self::Update {
                window_id,
                settings,
            } => ("update", window_id.clone(), settings.clone()),
            Self::Close(id) => ("close", id.clone(), Value::Null),
            Self::Resize {
                window_id,
                width,
                height,
            } => (
                "resize",
                window_id.clone(),
                json!({"width": width, "height": height}),
            ),
            Self::Move { window_id, x, y } => ("move", window_id.clone(), json!({"x": x, "y": y})),
            Self::Maximize {
                window_id,
                maximized,
            } => (
                "maximize",
                window_id.clone(),
                json!({"maximized": maximized}),
            ),
            Self::Minimize {
                window_id,
                minimized,
            } => (
                "minimize",
                window_id.clone(),
                json!({"minimized": minimized}),
            ),
            Self::SetMode { window_id, mode } => (
                "set_mode",
                window_id.clone(),
                json!({"mode": mode.to_string()}),
            ),
            Self::ToggleMaximize(id) => ("toggle_maximize", id.clone(), json!({})),
            Self::ToggleDecorations(id) => ("toggle_decorations", id.clone(), json!({})),
            Self::FocusWindow(id) => ("gain_focus", id.clone(), json!({})),
            Self::SetLevel { window_id, level } => (
                "set_level",
                window_id.clone(),
                json!({"level": level.to_string()}),
            ),
            Self::DragWindow(id) => ("drag", id.clone(), json!({})),
            Self::DragResize {
                window_id,
                direction,
            } => (
                "drag_resize",
                window_id.clone(),
                json!({"direction": direction}),
            ),
            Self::RequestAttention { window_id, urgency } => {
                let mut v = json!({});
                if let Some(u) = urgency {
                    v["urgency"] = json!(u);
                }
                ("request_attention", window_id.clone(), v)
            }
            Self::Screenshot { window_id, tag } => {
                ("screenshot", window_id.clone(), json!({"tag": tag}))
            }
            Self::SetResizable {
                window_id,
                resizable,
            } => (
                "set_resizable",
                window_id.clone(),
                json!({"resizable": resizable}),
            ),
            Self::SetMinSize {
                window_id,
                width,
                height,
            } => (
                "set_min_size",
                window_id.clone(),
                json!({"width": width, "height": height}),
            ),
            Self::SetMaxSize {
                window_id,
                width,
                height,
            } => (
                "set_max_size",
                window_id.clone(),
                json!({"width": width, "height": height}),
            ),
            Self::EnableMousePassthrough(id) => {
                ("mouse_passthrough", id.clone(), json!({"enabled": true}))
            }
            Self::DisableMousePassthrough(id) => {
                ("mouse_passthrough", id.clone(), json!({"enabled": false}))
            }
            Self::ShowSystemMenu(id) => ("show_system_menu", id.clone(), json!({})),
            Self::SetIcon {
                window_id,
                data,
                width,
                height,
            } => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                (
                    "set_icon",
                    window_id.clone(),
                    json!({"data": b64, "width": width, "height": height}),
                )
            }
            Self::SetResizeIncrements {
                window_id,
                width,
                height,
            } => (
                "set_resize_increments",
                window_id.clone(),
                json!({"width": width, "height": height}),
            ),
        }
    }

    /// Return the window ID this op targets, when one applies.
    pub fn window_id(&self) -> Option<&str> {
        match self {
            Self::Open { window_id, .. }
            | Self::Update { window_id, .. }
            | Self::Resize { window_id, .. }
            | Self::Move { window_id, .. }
            | Self::Maximize { window_id, .. }
            | Self::Minimize { window_id, .. }
            | Self::SetMode { window_id, .. }
            | Self::SetLevel { window_id, .. }
            | Self::DragResize { window_id, .. }
            | Self::RequestAttention { window_id, .. }
            | Self::Screenshot { window_id, .. }
            | Self::SetResizable { window_id, .. }
            | Self::SetMinSize { window_id, .. }
            | Self::SetMaxSize { window_id, .. }
            | Self::SetIcon { window_id, .. }
            | Self::SetResizeIncrements { window_id, .. } => Some(window_id),
            Self::Close(id)
            | Self::ToggleMaximize(id)
            | Self::ToggleDecorations(id)
            | Self::FocusWindow(id)
            | Self::DragWindow(id)
            | Self::EnableMousePassthrough(id)
            | Self::DisableMousePassthrough(id)
            | Self::ShowSystemMenu(id) => Some(id),
        }
    }
}

impl WindowQuery {
    /// Build a typed [`WindowQuery`] from the wire-protocol `{op,
    /// window_id, payload}` triple. Returns `None` for unrecognised
    /// op strings.
    pub fn from_wire(op: &str, window_id: &str, payload: &Value) -> Option<Self> {
        let wid = window_id.to_string();
        let tag = payload
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        match op {
            "get_size" => Some(Self::GetSize {
                window_id: wid,
                tag,
            }),
            "get_position" => Some(Self::GetPosition {
                window_id: wid,
                tag,
            }),
            "is_maximized" => Some(Self::IsMaximized {
                window_id: wid,
                tag,
            }),
            "is_minimized" => Some(Self::IsMinimized {
                window_id: wid,
                tag,
            }),
            "get_mode" => Some(Self::GetMode {
                window_id: wid,
                tag,
            }),
            "get_scale_factor" => Some(Self::GetScaleFactor {
                window_id: wid,
                tag,
            }),
            "monitor_size" => Some(Self::MonitorSize {
                window_id: wid,
                tag,
            }),
            "raw_id" => Some(Self::RawId {
                window_id: wid,
                tag,
            }),
            _ => None,
        }
    }

    /// Emit the wire-protocol `(op, window_id, payload)` triple.
    pub fn to_wire(&self) -> (&'static str, String, Value) {
        use serde_json::json;
        match self {
            Self::GetSize { window_id, tag } => {
                ("get_size", window_id.clone(), json!({"tag": tag}))
            }
            Self::GetPosition { window_id, tag } => {
                ("get_position", window_id.clone(), json!({"tag": tag}))
            }
            Self::IsMaximized { window_id, tag } => {
                ("is_maximized", window_id.clone(), json!({"tag": tag}))
            }
            Self::IsMinimized { window_id, tag } => {
                ("is_minimized", window_id.clone(), json!({"tag": tag}))
            }
            Self::GetMode { window_id, tag } => {
                ("get_mode", window_id.clone(), json!({"tag": tag}))
            }
            Self::GetScaleFactor { window_id, tag } => {
                ("get_scale_factor", window_id.clone(), json!({"tag": tag}))
            }
            Self::MonitorSize { window_id, tag } => {
                ("monitor_size", window_id.clone(), json!({"tag": tag}))
            }
            Self::RawId { window_id, tag } => ("raw_id", window_id.clone(), json!({"tag": tag})),
        }
    }

    /// Return the window ID this query targets.
    pub fn window_id(&self) -> &str {
        match self {
            Self::GetSize { window_id, .. }
            | Self::GetPosition { window_id, .. }
            | Self::IsMaximized { window_id, .. }
            | Self::IsMinimized { window_id, .. }
            | Self::GetMode { window_id, .. }
            | Self::GetScaleFactor { window_id, .. }
            | Self::MonitorSize { window_id, .. }
            | Self::RawId { window_id, .. } => window_id,
        }
    }
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
#[non_exhaustive]
pub enum SystemQuery {
    /// Query the current OS theme (light/dark).
    GetTheme {
        /// Correlation tag used for matching responses.
        tag: String,
    },
    /// Query system information (OS, renderer version, etc.).
    GetInfo {
        /// Correlation tag used for matching responses.
        tag: String,
    },
}

impl SystemOp {
    /// Build a typed [`SystemOp`] from the wire-protocol `(op, payload)`
    /// pair. Returns `None` for unrecognised ops.
    pub fn from_wire(op: &str, payload: &Value) -> Option<Self> {
        match op {
            "allow_automatic_tabbing" => {
                let enabled = payload
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                Some(Self::AllowAutomaticTabbing(enabled))
            }
            _ => None,
        }
    }

    /// Emit the wire-protocol `(op, payload)` pair.
    pub fn to_wire(&self) -> (&'static str, Value) {
        use serde_json::json;
        match self {
            Self::AllowAutomaticTabbing(enabled) => {
                ("allow_automatic_tabbing", json!({"enabled": enabled}))
            }
        }
    }
}

impl SystemQuery {
    /// Build a typed [`SystemQuery`] from the wire-protocol `(op, payload)`
    /// pair. Returns `None` for unrecognised ops.
    pub fn from_wire(op: &str, payload: &Value) -> Option<Self> {
        let tag = payload
            .get("tag")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        match op {
            "get_system_theme" => Some(Self::GetTheme { tag }),
            "get_system_info" => Some(Self::GetInfo { tag }),
            _ => None,
        }
    }

    /// Emit the wire-protocol `(op, payload)` pair.
    pub fn to_wire(&self) -> (&'static str, Value) {
        use serde_json::json;
        match self {
            Self::GetTheme { tag } => ("get_system_theme", json!({"tag": tag})),
            Self::GetInfo { tag } => ("get_system_info", json!({"tag": tag})),
        }
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/// A platform effect request.
#[derive(Debug)]
pub enum EffectRequest {
    /// File Open.
    FileOpen(FileDialogOpts),
    /// File Open Multiple.
    FileOpenMultiple(FileDialogOpts),
    /// File Save.
    FileSave(FileDialogOpts),
    /// Directory Select.
    DirectorySelect(FileDialogOpts),
    /// Directory Select Multiple.
    DirectorySelectMultiple(FileDialogOpts),
    /// Clipboard Read.
    ClipboardRead,
    /// Clipboard Write.
    ClipboardWrite(String),
    /// Clipboard Read Html.
    ClipboardReadHtml,
    /// Clipboard Write Html.
    ClipboardWriteHtml {
        /// HTML payload.
        html: String,
        /// Plain-text fallback for HTML clipboard writes.
        alt_text: Option<String>,
    },
    /// Clipboard Clear.
    ClipboardClear,
    /// Clipboard Read Primary.
    ClipboardReadPrimary,
    /// Clipboard Write Primary.
    ClipboardWritePrimary(String),
    /// Notification.
    Notification {
        /// Human-readable title.
        title: String,
        /// Human-readable body text.
        body: String,
        /// Per-operation options.
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

/// Returns true if `kind` is a built-in effect request kind.
pub fn is_known_effect_kind(kind: &str) -> bool {
    matches!(
        kind,
        "file_open"
            | "file_open_multiple"
            | "file_save"
            | "directory_select"
            | "directory_select_multiple"
            | "clipboard_read"
            | "clipboard_write"
            | "clipboard_read_html"
            | "clipboard_write_html"
            | "clipboard_clear"
            | "clipboard_read_primary"
            | "clipboard_write_primary"
            | "notification"
    )
}

/// Why a wire effect request could not be parsed safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectRequestValidationError {
    /// The effect kind is not built in.
    UnknownKind { kind: String },
    /// The payload is not a JSON object.
    InvalidPayload {
        kind: String,
        expected: &'static str,
    },
    /// A required field was absent.
    MissingField { kind: String, field: &'static str },
    /// A field was present with the wrong JSON type.
    InvalidFieldType {
        kind: String,
        field: &'static str,
        expected: &'static str,
    },
    /// A field had the right JSON type but not an accepted value.
    InvalidFieldValue {
        kind: String,
        field: &'static str,
        detail: String,
    },
}

impl fmt::Display for EffectRequestValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKind { kind } => write!(f, "unknown effect kind: {kind}"),
            Self::InvalidPayload { kind, expected } => {
                write!(f, "invalid payload for {kind}: expected {expected}")
            }
            Self::MissingField { kind, field } => {
                write!(f, "missing required field for {kind}: {field}")
            }
            Self::InvalidFieldType {
                kind,
                field,
                expected,
            } => write!(
                f,
                "invalid field type for {kind}.{field}: expected {expected}"
            ),
            Self::InvalidFieldValue {
                kind,
                field,
                detail,
            } => write!(f, "invalid field value for {kind}.{field}: {detail}"),
        }
    }
}

impl std::error::Error for EffectRequestValidationError {}

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
    /// Construct a new value.
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
    /// Construct a new value.
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
#[non_exhaustive]
pub enum ImageOp {
    /// Create an image from encoded bytes (PNG, JPEG, etc.).
    Create {
        /// Handle.
        handle: String,
        /// Raw bytes (pixels, font, etc.).
        data: Vec<u8>,
    },
    /// Create an image from raw RGBA pixel data.
    CreateRaw {
        /// Handle.
        handle: String,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Pixels.
        pixels: Vec<u8>,
    },
    /// Replace an existing image with new encoded bytes.
    Update {
        /// Handle.
        handle: String,
        /// Raw bytes (pixels, font, etc.).
        data: Vec<u8>,
    },
    /// Replace an existing image with new raw RGBA pixel data.
    UpdateRaw {
        /// Handle.
        handle: String,
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
        /// Pixels.
        pixels: Vec<u8>,
    },
    /// Delete an image by handle.
    Delete(String),
    /// List all loaded image handles.
    List {
        /// Correlation tag used for matching responses.
        tag: String,
    },
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
    /// are derived from the typed command via
    /// [`WidgetCommandEncode`](crate::WidgetCommandEncode).
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

/// Convert wire format `(kind, payload)` to an [`EffectRequest`],
/// rejecting unknown kinds and malformed payloads.
///
/// # Errors
///
/// Returns [`EffectRequestValidationError`] when the kind is unknown,
/// the payload is not an object, or required fields are missing or
/// malformed.
pub fn validate_effect_request_from_wire(
    kind: &str,
    payload: &Value,
) -> Result<EffectRequest, EffectRequestValidationError> {
    if !is_known_effect_kind(kind) {
        return Err(EffectRequestValidationError::UnknownKind {
            kind: kind.to_string(),
        });
    }
    let fields = payload_fields(kind, payload)?;
    match kind {
        "file_open" => Ok(EffectRequest::FileOpen(file_dialog_opts_from_fields(
            kind, fields,
        )?)),
        "file_open_multiple" => Ok(EffectRequest::FileOpenMultiple(
            file_dialog_opts_from_fields(kind, fields)?,
        )),
        "file_save" => Ok(EffectRequest::FileSave(file_dialog_opts_from_fields(
            kind, fields,
        )?)),
        "directory_select" => Ok(EffectRequest::DirectorySelect(
            file_dialog_opts_from_fields(kind, fields)?,
        )),
        "directory_select_multiple" => Ok(EffectRequest::DirectorySelectMultiple(
            file_dialog_opts_from_fields(kind, fields)?,
        )),
        "clipboard_read" => Ok(EffectRequest::ClipboardRead),
        "clipboard_write" => {
            let text = required_string_field(kind, fields, "text")?;
            Ok(EffectRequest::ClipboardWrite(text))
        }
        "clipboard_read_html" => Ok(EffectRequest::ClipboardReadHtml),
        "clipboard_write_html" => {
            let html = required_string_field(kind, fields, "html")?;
            let alt_text = optional_string_field(kind, fields, "alt_text")?;
            Ok(EffectRequest::ClipboardWriteHtml { html, alt_text })
        }
        "clipboard_clear" => Ok(EffectRequest::ClipboardClear),
        "clipboard_read_primary" => Ok(EffectRequest::ClipboardReadPrimary),
        "clipboard_write_primary" => {
            let text = required_string_field(kind, fields, "text")?;
            Ok(EffectRequest::ClipboardWritePrimary(text))
        }
        "notification" => {
            let title = required_string_field(kind, fields, "title")?;
            let body = required_string_field(kind, fields, "body")?;
            let opts = NotificationOpts {
                icon: optional_string_field(kind, fields, "icon")?,
                timeout: optional_u64_field(kind, fields, "timeout")?.map(Duration::from_millis),
                urgency: optional_urgency_field(kind, fields)?,
                sound: optional_string_field(kind, fields, "sound")?,
            };
            Ok(EffectRequest::Notification { title, body, opts })
        }
        _ => unreachable!("effect kind was checked before parsing"),
    }
}

/// Convert wire format `(kind, payload)` to an [`EffectRequest`].
///
/// Returns `None` for unrecognized kinds or invalid payloads.
pub fn effect_request_from_wire(kind: &str, payload: &Value) -> Option<EffectRequest> {
    validate_effect_request_from_wire(kind, payload).ok()
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

fn payload_fields<'a>(
    kind: &str,
    payload: &'a Value,
) -> Result<&'a Map<String, Value>, EffectRequestValidationError> {
    payload
        .as_object()
        .ok_or_else(|| EffectRequestValidationError::InvalidPayload {
            kind: kind.to_string(),
            expected: "object",
        })
}

fn required_string_field(
    kind: &str,
    fields: &Map<String, Value>,
    field: &'static str,
) -> Result<String, EffectRequestValidationError> {
    match fields.get(field) {
        Some(value) => value.as_str().map(ToString::to_string).ok_or_else(|| {
            EffectRequestValidationError::InvalidFieldType {
                kind: kind.to_string(),
                field,
                expected: "string",
            }
        }),
        None => Err(EffectRequestValidationError::MissingField {
            kind: kind.to_string(),
            field,
        }),
    }
}

fn optional_string_field(
    kind: &str,
    fields: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<String>, EffectRequestValidationError> {
    match fields.get(field) {
        Some(value) => value.as_str().map(|s| Some(s.to_string())).ok_or_else(|| {
            EffectRequestValidationError::InvalidFieldType {
                kind: kind.to_string(),
                field,
                expected: "string",
            }
        }),
        None => Ok(None),
    }
}

fn optional_u64_field(
    kind: &str,
    fields: &Map<String, Value>,
    field: &'static str,
) -> Result<Option<u64>, EffectRequestValidationError> {
    match fields.get(field) {
        Some(value) => {
            value
                .as_u64()
                .map(Some)
                .ok_or_else(|| EffectRequestValidationError::InvalidFieldType {
                    kind: kind.to_string(),
                    field,
                    expected: "unsigned integer",
                })
        }
        None => Ok(None),
    }
}

fn optional_urgency_field(
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<Option<NotificationUrgency>, EffectRequestValidationError> {
    let Some(value) = fields.get("urgency") else {
        return Ok(None);
    };
    let Some(urgency) = value.as_str() else {
        return Err(EffectRequestValidationError::InvalidFieldType {
            kind: kind.to_string(),
            field: "urgency",
            expected: "string",
        });
    };
    match urgency {
        "low" => Ok(Some(NotificationUrgency::Low)),
        "normal" => Ok(Some(NotificationUrgency::Normal)),
        "critical" => Ok(Some(NotificationUrgency::Critical)),
        _ => Err(EffectRequestValidationError::InvalidFieldValue {
            kind: kind.to_string(),
            field: "urgency",
            detail: "expected low, normal, or critical".to_string(),
        }),
    }
}

fn file_dialog_opts_from_fields(
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<FileDialogOpts, EffectRequestValidationError> {
    Ok(FileDialogOpts {
        title: optional_string_field(kind, fields, "title")?,
        directory: optional_string_field(kind, fields, "directory")?,
        default_name: optional_string_field(kind, fields, "default_name")?,
        filters: file_dialog_filters_from_fields(kind, fields)?,
    })
}

fn file_dialog_filters_from_fields(
    kind: &str,
    fields: &Map<String, Value>,
) -> Result<Vec<(String, Vec<String>)>, EffectRequestValidationError> {
    let Some(value) = fields.get("filters") else {
        return Ok(Vec::new());
    };
    let filters =
        value
            .as_array()
            .ok_or_else(|| EffectRequestValidationError::InvalidFieldType {
                kind: kind.to_string(),
                field: "filters",
                expected: "array",
            })?;
    let mut parsed = Vec::new();
    for filter in filters {
        let pair =
            filter
                .as_array()
                .ok_or_else(|| EffectRequestValidationError::InvalidFieldValue {
                    kind: kind.to_string(),
                    field: "filters",
                    detail: "each filter must be [name, extensions]".to_string(),
                })?;
        if pair.len() < 2 {
            return Err(EffectRequestValidationError::InvalidFieldValue {
                kind: kind.to_string(),
                field: "filters",
                detail: "each filter must include a name and extensions".to_string(),
            });
        }
        let name =
            pair[0]
                .as_str()
                .ok_or_else(|| EffectRequestValidationError::InvalidFieldValue {
                    kind: kind.to_string(),
                    field: "filters",
                    detail: "filter name must be a string".to_string(),
                })?;
        let ext =
            pair[1]
                .as_str()
                .ok_or_else(|| EffectRequestValidationError::InvalidFieldValue {
                    kind: kind.to_string(),
                    field: "filters",
                    detail: "filter extensions must be a string".to_string(),
                })?;
        let extensions: Vec<String> = ext
            .split(';')
            .map(|e| e.trim().trim_start_matches("*.").to_string())
            .collect();
        parsed.push((name.to_string(), extensions));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn effect_parser_rejects_missing_required_field() {
        let err = validate_effect_request_from_wire("clipboard_write", &json!({})).unwrap_err();

        assert_eq!(
            err,
            EffectRequestValidationError::MissingField {
                kind: "clipboard_write".to_string(),
                field: "text",
            }
        );
    }

    #[test]
    fn effect_parser_rejects_unknown_kind() {
        let err = validate_effect_request_from_wire("not_real", &json!({})).unwrap_err();

        assert_eq!(
            err,
            EffectRequestValidationError::UnknownKind {
                kind: "not_real".to_string(),
            }
        );
    }

    #[test]
    fn effect_parser_rejects_wrong_typed_required_field() {
        let err =
            validate_effect_request_from_wire("notification", &json!({"title": 1, "body": "hi"}))
                .unwrap_err();

        assert_eq!(
            err,
            EffectRequestValidationError::InvalidFieldType {
                kind: "notification".to_string(),
                field: "title",
                expected: "string",
            }
        );
    }

    #[test]
    fn effect_parser_rejects_wrong_typed_optional_field() {
        let err = validate_effect_request_from_wire(
            "clipboard_write_html",
            &json!({"html": "<b>hi</b>", "alt_text": false}),
        )
        .unwrap_err();

        assert_eq!(
            err,
            EffectRequestValidationError::InvalidFieldType {
                kind: "clipboard_write_html".to_string(),
                field: "alt_text",
                expected: "string",
            }
        );
    }

    #[test]
    fn effect_parser_rejects_invalid_file_dialog_filters() {
        let err = validate_effect_request_from_wire(
            "file_open",
            &json!({"filters": [{"name": "Images", "extensions": "png"}]}),
        )
        .unwrap_err();

        assert!(matches!(
            err,
            EffectRequestValidationError::InvalidFieldValue {
                kind,
                field: "filters",
                ..
            } if kind == "file_open"
        ));
    }

    #[test]
    fn effect_parser_parses_valid_required_fields() {
        let request = validate_effect_request_from_wire(
            "notification",
            &json!({
                "title": "Build done",
                "body": "All checks passed",
                "timeout": 1500,
                "urgency": "normal",
            }),
        )
        .unwrap();

        match request {
            EffectRequest::Notification { title, body, opts } => {
                assert_eq!(title, "Build done");
                assert_eq!(body, "All checks passed");
                assert_eq!(opts.timeout, Some(Duration::from_millis(1500)));
                assert_eq!(opts.urgency, Some(NotificationUrgency::Normal));
            }
            other => panic!("expected notification, got {other:?}"),
        }
    }

    #[test]
    fn effect_parser_round_trips_typed_requests() {
        let requests = vec![
            EffectRequest::FileOpen(
                FileDialogOpts::new()
                    .title("Open")
                    .filter("Images", &["png"]),
            ),
            EffectRequest::FileOpenMultiple(FileDialogOpts::new()),
            EffectRequest::FileSave(FileDialogOpts::new().default_name("note.txt")),
            EffectRequest::DirectorySelect(FileDialogOpts::new().directory("/tmp")),
            EffectRequest::DirectorySelectMultiple(FileDialogOpts::new()),
            EffectRequest::ClipboardRead,
            EffectRequest::ClipboardWrite("hello".to_string()),
            EffectRequest::ClipboardReadHtml,
            EffectRequest::ClipboardWriteHtml {
                html: "<b>hello</b>".to_string(),
                alt_text: Some("hello".to_string()),
            },
            EffectRequest::ClipboardClear,
            EffectRequest::ClipboardReadPrimary,
            EffectRequest::ClipboardWritePrimary("hello".to_string()),
            EffectRequest::Notification {
                title: "Done".to_string(),
                body: "Saved".to_string(),
                opts: NotificationOpts::new()
                    .icon("plushie")
                    .timeout(Duration::from_secs(1))
                    .urgency(NotificationUrgency::Low)
                    .sound("ding"),
            },
        ];

        for request in requests {
            let (kind, payload) = effect_request_to_wire(&request);
            let parsed = validate_effect_request_from_wire(kind, &payload)
                .unwrap_or_else(|err| panic!("{kind} failed to parse: {err}"));
            assert_eq!(parsed.kind(), kind);
        }
    }
}
