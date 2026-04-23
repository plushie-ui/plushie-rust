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
    /// Advance the animation frame to the given timestamp.
    AdvanceFrame {
        /// Timestamp in milliseconds since the Unix epoch.
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
                let b64 = payload
                    .get("data")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let data = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .unwrap_or_default();
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
