//! Window + system operations: typed dispatch for lifecycle (open,
//! close, update), state changes (resize, move, maximize, mode,
//! level, decorations, focus), queries (size, position, mode, scale
//! factor, monitor, raw_id), and window sync.
//!
//! Dispatched from `CoreEffect::WindowOp(WindowOp)` and siblings via
//! typed `match`. The renderer owns the `window_id -> iced::window::Id`
//! map in `self.windows`; handlers look up the iced id per op.
//!
//! ## Platform notes
//!
//! Several operations are no-ops on Wayland because the compositor owns
//! window positioning, focus, and icon management. When the renderer
//! detects Wayland (via `WAYLAND_DISPLAY`), it logs a debug warning for
//! these operations so SDK users can understand why their requests have
//! no visible effect.

use std::collections::HashSet;

use iced::{Point, Size, Task, window};

use plushie_core::ops::{
    NotificationUrgency, SystemOp, SystemQuery, WindowLevel, WindowMode, WindowOp, WindowQuery,
};
use plushie_widget_sdk::runtime::Message;

use crate::App;

/// Returns true if the current display server is Wayland.
///
/// Detected via the `WAYLAND_DISPLAY` environment variable, which is
/// set by Wayland compositors. Cached in a `OnceLock` so the env
/// lookup happens at most once per process.
/// On WASM, always returns false.
fn is_wayland() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static IS_WAYLAND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *IS_WAYLAND.get_or_init(|| std::env::var("WAYLAND_DISPLAY").is_ok())
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}

/// Log a debug warning when a window operation is a known no-op on Wayland.
fn warn_wayland_noop(op: &str) {
    if is_wayland() {
        log::debug!("{op}: no-op on Wayland (compositor-controlled)");
    }
}

// ---------------------------------------------------------------------------
// Window operations (impl App)
// ---------------------------------------------------------------------------

impl App {
    /// Dispatch a typed [`WindowOp`].
    ///
    /// Each variant maps to the appropriate iced window operation. The
    /// window id is looked up in `self.windows`; unknown ids are logged
    /// and produce `Task::none()`.
    pub fn dispatch_window_op(&mut self, op: WindowOp) -> Task<Message> {
        match op {
            WindowOp::Open {
                window_id,
                settings,
            } => {
                if self.windows.contains_window(&window_id) {
                    log::warn!("window_op open: {window_id} already open, skipping");
                    return Task::none();
                }
                let win_settings = parse_window_settings(&settings);
                let initial_decorations = win_settings.decorations;
                let scale_factor = parse_scale_factor(&settings);
                let (iced_id, open_task) = window::open(win_settings);

                self.windows.insert(window_id.clone(), iced_id);
                self.windows.set_decorated(&window_id, initial_decorations);
                self.windows.set_scale_factor(&window_id, scale_factor);

                open_task.map(move |id| Message::WindowOpened(id, window_id.clone()))
            }
            WindowOp::Update {
                window_id,
                settings,
            } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    log::warn!("window_op update: unknown window_id: {window_id}");
                    return Task::none();
                };
                let mut tasks: Vec<Task<Message>> = Vec::new();
                let Some(obj) = settings.as_object() else {
                    return Task::none();
                };

                // Window titles are tree-owned. Updating the window node title
                // changes `title_for_window`; the update op itself has no
                // separate iced task for `title`.
                let _ = obj.get("title").and_then(|v| v.as_str());

                if obj.contains_key("width") || obj.contains_key("height") {
                    let w = obj.get("width").and_then(|v| v.as_f64()).unwrap_or(800.0) as f32;
                    let h = obj.get("height").and_then(|v| v.as_f64()).unwrap_or(600.0) as f32;
                    tasks.push(window::resize(iced_id, Size::new(w, h)));
                }
                if let Some(maximized) = obj.get("maximized").and_then(|v| v.as_bool()) {
                    tasks.push(window::maximize(iced_id, maximized));
                }
                if let Some(resizable) = obj.get("resizable").and_then(|v| v.as_bool()) {
                    tasks.push(window::set_resizable(iced_id, resizable));
                }
                // Note: visible and fullscreen both call set_mode. If both are
                // present, the last one wins. Hosts should not set both.
                if let Some(visible) = obj.get("visible").and_then(|v| v.as_bool()) {
                    let mode = if visible {
                        window::Mode::Windowed
                    } else {
                        window::Mode::Hidden
                    };
                    tasks.push(window::set_mode(iced_id, mode));
                }
                if let Some(fullscreen) = obj.get("fullscreen").and_then(|v| v.as_bool()) {
                    let mode = if fullscreen {
                        window::Mode::Fullscreen
                    } else {
                        window::Mode::Windowed
                    };
                    tasks.push(window::set_mode(iced_id, mode));
                }
                if obj.contains_key("min_size") {
                    let sz = parse_optional_size(
                        obj.get("min_size").unwrap_or(&serde_json::Value::Null),
                    );
                    tasks.push(window::set_min_size(iced_id, sz));
                }
                if obj.contains_key("max_size") {
                    let sz = parse_optional_size(
                        obj.get("max_size").unwrap_or(&serde_json::Value::Null),
                    );
                    tasks.push(window::set_max_size(iced_id, sz));
                }
                if obj.contains_key("level") {
                    let level = parse_window_level_str(
                        obj.get("level")
                            .and_then(|v| v.as_str())
                            .unwrap_or("normal"),
                    );
                    tasks.push(window::set_level(iced_id, level));
                }
                if let Some(desired) = obj.get("decorations").and_then(|v| v.as_bool()) {
                    let current = self.windows.is_decorated(&window_id);
                    if desired != current {
                        self.windows.set_decorated(&window_id, desired);
                        tasks.push(window::toggle_decorations(iced_id));
                    }
                }
                if obj.contains_key("scale_factor") {
                    let sf = parse_scale_factor(&serde_json::Value::Object(obj.clone()));
                    self.windows.set_scale_factor(&window_id, sf);
                }

                Task::batch(tasks)
            }
            WindowOp::Close(window_id) => {
                if let Some(iced_id) = self.windows.remove_by_window(&window_id) {
                    window::close(iced_id)
                } else {
                    log::warn!("window_op close: unknown window_id: {window_id}");
                    Task::none()
                }
            }
            WindowOp::Resize {
                window_id,
                width,
                height,
            } => self.with_iced(&window_id, |id| {
                window::resize(id, Size::new(width, height))
            }),
            WindowOp::Move { window_id, x, y } => {
                warn_wayland_noop("move");
                self.with_iced(&window_id, |id| window::move_to(id, Point::new(x, y)))
            }
            WindowOp::Maximize {
                window_id,
                maximized,
            } => self.with_iced(&window_id, |id| window::maximize(id, maximized)),
            WindowOp::Minimize {
                window_id,
                minimized,
            } => self.with_iced(&window_id, |id| window::minimize(id, minimized)),
            WindowOp::SetMode { window_id, mode } => {
                let iced_mode = match mode {
                    WindowMode::Fullscreen => window::Mode::Fullscreen,
                    WindowMode::Windowed => window::Mode::Windowed,
                };
                self.with_iced(&window_id, |id| window::set_mode(id, iced_mode))
            }
            WindowOp::ToggleMaximize(window_id) => {
                self.with_iced(&window_id, window::toggle_maximize)
            }
            WindowOp::ToggleDecorations(window_id) => {
                let current = self.windows.is_decorated(&window_id);
                self.windows.set_decorated(&window_id, !current);
                self.with_iced(&window_id, window::toggle_decorations)
            }
            WindowOp::FocusWindow(window_id) => {
                warn_wayland_noop("gain_focus");
                self.with_iced(&window_id, window::gain_focus)
            }
            WindowOp::SetLevel { window_id, level } => {
                let iced_level = match level {
                    WindowLevel::Normal => window::Level::Normal,
                    WindowLevel::AlwaysOnTop => window::Level::AlwaysOnTop,
                    WindowLevel::AlwaysOnBottom => window::Level::AlwaysOnBottom,
                };
                self.with_iced(&window_id, |id| window::set_level(id, iced_level))
            }
            WindowOp::DragWindow(window_id) => self.with_iced(&window_id, window::drag),
            WindowOp::DragResize {
                window_id,
                direction,
            } => {
                #[cfg(target_os = "macos")]
                log::warn!("drag_resize is not supported on macOS");
                let Some(dir) = parse_direction(&direction) else {
                    log::warn!("drag_resize: invalid direction '{direction}', rejecting");
                    return Task::none();
                };
                self.with_iced(&window_id, |id| window::drag_resize(id, dir))
            }
            WindowOp::RequestAttention { window_id, urgency } => {
                let attention = urgency.map(|u| match u {
                    NotificationUrgency::Critical => window::UserAttention::Critical,
                    NotificationUrgency::Normal | NotificationUrgency::Low => {
                        window::UserAttention::Informational
                    }
                });
                self.with_iced(&window_id, |id| {
                    window::request_user_attention(id, attention)
                })
            }
            WindowOp::Screenshot { window_id, tag } => self.screenshot_task(&window_id, &tag),
            WindowOp::SetResizable {
                window_id,
                resizable,
            } => self.with_iced(&window_id, |id| window::set_resizable(id, resizable)),
            WindowOp::SetMinSize {
                window_id,
                width,
                height,
            } => self.with_iced(&window_id, |id| {
                window::set_min_size(id, Some(Size::new(width, height)))
            }),
            WindowOp::SetMaxSize {
                window_id,
                width,
                height,
            } => self.with_iced(&window_id, |id| {
                window::set_max_size(id, Some(Size::new(width, height)))
            }),
            WindowOp::EnableMousePassthrough(window_id) => {
                self.with_iced(&window_id, window::enable_mouse_passthrough)
            }
            WindowOp::DisableMousePassthrough(window_id) => {
                self.with_iced(&window_id, window::disable_mouse_passthrough)
            }
            WindowOp::ShowSystemMenu(window_id) => {
                #[cfg(not(target_os = "windows"))]
                log::warn!("show_system_menu is only supported on Windows");
                self.with_iced(&window_id, window::show_system_menu)
            }
            WindowOp::SetIcon {
                window_id,
                data,
                width,
                height,
            } => {
                warn_wayland_noop("set_icon");
                self.dispatch_set_icon(&window_id, data, width, height)
            }
            WindowOp::SetResizeIncrements {
                window_id,
                width,
                height,
            } => self.with_iced(&window_id, |id| {
                window::set_resize_increments(id, Some(Size::new(width, height)))
            }),
            _ => {
                log::warn!("unhandled WindowOp variant");
                Task::none()
            }
        }
    }

    /// Dispatch a typed [`WindowQuery`]. Each variant produces a
    /// response event via the emitter sink when the underlying iced
    /// task resolves.
    pub fn dispatch_window_query(&mut self, q: WindowQuery) -> Task<Message> {
        let sink = self.emitter.sink();
        match q {
            WindowQuery::GetSize { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::size(iced_id).map(move |size| {
                    let data = serde_json::json!({
                        "width": size.width,
                        "height": size.height,
                        "op": "get_size",
                        "request_id": tag,
                    });
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::GetPosition { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::position(iced_id).map(move |pos| {
                    let data = match pos {
                        Some(p) => serde_json::json!({
                            "x": p.x,
                            "y": p.y,
                            "op": "get_position",
                            "request_id": tag,
                        }),
                        None => serde_json::json!({
                            "op": "get_position",
                            "request_id": tag,
                        }),
                    };
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::GetMode { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::mode(iced_id).map(move |mode| {
                    let mode_str = match mode {
                        window::Mode::Windowed => "windowed",
                        window::Mode::Fullscreen => "fullscreen",
                        window::Mode::Hidden => "hidden",
                    };
                    let data = serde_json::json!({
                        "mode": mode_str,
                        "op": "get_mode",
                        "request_id": tag,
                    });
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::GetScaleFactor { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::scale_factor(iced_id).map(move |factor| {
                    let data = serde_json::json!({
                        "scale_factor": factor,
                        "op": "get_scale_factor",
                        "request_id": tag,
                    });
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::IsMaximized { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::is_maximized(iced_id).map(move |val| {
                    let data = serde_json::json!({
                        "maximized": val,
                        "op": "is_maximized",
                        "request_id": tag,
                    });
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::IsMinimized { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::is_minimized(iced_id).map(move |val| {
                    let data = serde_json::json!({
                        "minimized": val,
                        "op": "is_minimized",
                        "request_id": tag,
                    });
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::MonitorSize { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::monitor_size(iced_id).map(move |size_opt| {
                    let data = match size_opt {
                        Some(size) => serde_json::json!({
                            "width": size.width,
                            "height": size.height,
                            "op": "monitor_size",
                            "request_id": tag,
                        }),
                        None => serde_json::json!({
                            "op": "monitor_size",
                            "request_id": tag,
                        }),
                    };
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            WindowQuery::RawId { window_id, tag } => {
                let Some(&iced_id) = self.windows.get_iced(&window_id) else {
                    return Task::none();
                };
                let wid = window_id.clone();
                window::raw_id::<Message>(iced_id).map(move |raw| {
                    let data = serde_json::json!({
                        "raw_id": raw,
                        "op": "raw_id",
                        "platform": std::env::consts::OS,
                        "request_id": tag,
                    });
                    let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
                    if let Err(e) = sink.lock().emit_effect_response(resp) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            _ => {
                log::warn!("unhandled WindowQuery variant");
                Task::none()
            }
        }
    }

    /// Dispatch a typed [`SystemOp`].
    pub fn dispatch_system_op(&mut self, op: SystemOp) -> Task<Message> {
        match op {
            SystemOp::AllowAutomaticTabbing(enabled) => window::allow_automatic_tabbing(enabled),
        }
    }

    /// Dispatch a typed [`SystemQuery`]. Responses are emitted via the
    /// sink when the underlying iced task resolves.
    pub fn dispatch_system_query(&mut self, q: SystemQuery) -> Task<Message> {
        let sink = self.emitter.sink();
        match q {
            SystemQuery::GetTheme { tag } => iced::system::theme().map(move |mode| {
                let mode_str = match mode {
                    iced::theme::Mode::Light => "light",
                    iced::theme::Mode::Dark => "dark",
                    iced::theme::Mode::None => "none",
                };
                let mut guard = sink.lock();
                if let Err(e) =
                    guard.emit_query_response("system_theme", &tag, &serde_json::json!(mode_str))
                {
                    log::error!("write error: {e}");
                }
                Message::NoOp
            }),
            #[cfg(not(target_arch = "wasm32"))]
            SystemQuery::GetInfo { tag } => iced::system::information().map(move |info| {
                let data = serde_json::json!({
                    "system_name": info.system_name,
                    "system_kernel": info.system_kernel,
                    "system_version": info.system_version,
                    "system_short_version": info.system_short_version,
                    "cpu_brand": info.cpu_brand,
                    "cpu_cores": info.cpu_cores,
                    "memory_total": info.memory_total,
                    "memory_used": info.memory_used,
                    "graphics_backend": info.graphics_backend,
                    "graphics_adapter": info.graphics_adapter,
                });
                let mut guard = sink.lock();
                if let Err(e) = guard.emit_query_response("system_info", &tag, &data) {
                    log::error!("write error: {e}");
                }
                Message::NoOp
            }),
            #[cfg(target_arch = "wasm32")]
            SystemQuery::GetInfo { .. } => Task::none(),
            _ => {
                log::warn!("unhandled SystemQuery variant");
                Task::none()
            }
        }
    }

    fn with_iced(
        &self,
        window_id: &str,
        f: impl FnOnce(window::Id) -> Task<Message>,
    ) -> Task<Message> {
        match self.windows.get_iced(window_id) {
            Some(&id) => f(id),
            None => {
                log::warn!("window_op: unknown window_id: {window_id}");
                Task::none()
            }
        }
    }

    fn screenshot_task(&self, window_id: &str, tag: &str) -> Task<Message> {
        let Some(&iced_id) = self.windows.get_iced(window_id) else {
            return Task::none();
        };
        use base64::Engine as _;
        let sink = self.emitter.sink();
        let wid = window_id.to_string();
        let tag = tag.to_string();
        window::screenshot(iced_id).map(move |screenshot| {
            let rgba_b64 = base64::engine::general_purpose::STANDARD.encode(&screenshot.rgba);
            let data = serde_json::json!({
                "width": screenshot.size.width,
                "height": screenshot.size.height,
                "bytes_len": screenshot.rgba.len(),
                "rgba": rgba_b64,
                "op": "screenshot",
                "request_id": tag,
            });
            let resp = plushie_widget_sdk::protocol::EffectResponse::ok(wid.clone(), data);
            if let Err(e) = sink.lock().emit_effect_response(resp) {
                log::error!("write error: {e}");
            }
            Message::NoOp
        })
    }

    fn dispatch_set_icon(
        &self,
        window_id: &str,
        data: Vec<u8>,
        width: u32,
        height: u32,
    ) -> Task<Message> {
        const MAX_ICON_DIMENSION: u32 = 1024;
        let Some(&iced_id) = self.windows.get_iced(window_id) else {
            return Task::none();
        };
        if width == 0 || height == 0 {
            log::error!("set_icon: zero dimension ({width}x{height})");
            return Task::none();
        }
        if width > MAX_ICON_DIMENSION || height > MAX_ICON_DIMENSION {
            log::error!(
                "set_icon: dimensions {width}x{height} exceed maximum {MAX_ICON_DIMENSION}"
            );
            return Task::none();
        }
        if width != height {
            log::warn!(
                "set_icon: non-square icon ({width}x{height}); some platforms may render poorly"
            );
        }
        let expected_len = match (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(4))
        {
            Some(len) => len,
            None => {
                log::error!("set_icon: dimensions {width}x{height} would overflow");
                return Task::none();
            }
        };
        if data.len() != expected_len {
            log::error!(
                "set_icon: expected {expected_len} bytes ({width}x{height}x4), got {}",
                data.len()
            );
            return Task::none();
        }
        match window::icon::from_rgba(data, width, height) {
            Ok(icon) => window::set_icon(iced_id, icon),
            Err(e) => {
                log::error!("set_icon: icon creation failed: {e}");
                Task::none()
            }
        }
    }

    /// Compare the set of window nodes in the tree against the currently open
    /// windows and open/close as needed.
    pub fn sync_windows(&mut self) -> Task<Message> {
        let tree_windows: HashSet<String> = self.core.tree.window_ids().into_iter().collect();
        let open_windows: HashSet<String> = self.windows.window_ids().cloned().collect();

        let mut tasks = Vec::new();

        // Open windows that exist in the tree but are not yet open.
        for win_id in &tree_windows {
            if !open_windows.contains(win_id) {
                let scale_factor = self
                    .core
                    .tree
                    .find_window(win_id)
                    .and_then(|n| parse_scale_factor(&n.props.to_value()));
                let settings = self.window_settings_for(win_id);
                let initial_decorations = settings.decorations;
                let (iced_id, open_task) = window::open(settings);
                self.windows.insert(win_id.clone(), iced_id);
                self.windows.set_decorated(win_id, initial_decorations);
                self.windows.set_scale_factor(win_id, scale_factor);

                let wid = win_id.clone();
                tasks.push(open_task.map(move |id| Message::WindowOpened(id, wid.clone())));
            }
        }

        // Close windows that are open but no longer in the tree.
        for win_id in &open_windows {
            if !tree_windows.contains(win_id)
                && let Some(iced_id) = self.windows.remove_by_window(win_id)
            {
                tasks.push(window::close(iced_id));
            }
        }

        Task::batch(tasks)
    }

    /// Build window::Settings from a window node's props.
    pub fn window_settings_for(&self, window_id: &str) -> window::Settings {
        if let Some(node) = self.core.tree.find_window(window_id) {
            parse_window_settings(&node.props.to_value())
        } else {
            window::Settings {
                size: Size::new(800.0, 600.0),
                ..window::Settings::default()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Settings / enum parsing helpers
// ---------------------------------------------------------------------------

/// Maximum window dimension in logical pixels.
const MAX_WINDOW_DIM: f32 = 16384.0;

/// Parse a full `window::Settings` from a JSON value (node props or op settings).
pub fn parse_window_settings(v: &serde_json::Value) -> window::Settings {
    let mut width = v.get("width").and_then(|v| v.as_f64()).unwrap_or(800.0) as f32;
    let mut height = v.get("height").and_then(|v| v.as_f64()).unwrap_or(600.0) as f32;
    if !(1.0..=MAX_WINDOW_DIM).contains(&width) {
        log::warn!("window width {width} out of range, clamping to 1.0..={MAX_WINDOW_DIM}");
        width = width.clamp(1.0, MAX_WINDOW_DIM);
    }
    if !(1.0..=MAX_WINDOW_DIM).contains(&height) {
        log::warn!("window height {height} out of range, clamping to 1.0..={MAX_WINDOW_DIM}");
        height = height.clamp(1.0, MAX_WINDOW_DIM);
    }

    let maximized = v
        .get("maximized")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let fullscreen = v
        .get("fullscreen")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let visible = v.get("visible").and_then(|v| v.as_bool()).unwrap_or(true);
    let resizable = v.get("resizable").and_then(|v| v.as_bool()).unwrap_or(true);
    let closeable = v.get("closeable").and_then(|v| v.as_bool()).unwrap_or(true);
    let minimizable = v
        .get("minimizable")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let decorations = v
        .get("decorations")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let transparent = v
        .get("transparent")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let blur = v.get("blur").and_then(|v| v.as_bool()).unwrap_or(false);
    let exit_on_close_request = v
        .get("exit_on_close_request")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let position = match v.get("position") {
        Some(serde_json::Value::String(s)) if s == "centered" => window::Position::Centered,
        Some(obj) if obj.is_object() => {
            const MAX_POS: f32 = 32768.0;
            let mut x = obj.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let mut y = obj.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            if !(-MAX_POS..=MAX_POS).contains(&x) {
                log::warn!(
                    "window position x={x} out of range, clamping to -{MAX_POS}..={MAX_POS}"
                );
                x = x.clamp(-MAX_POS, MAX_POS);
            }
            if !(-MAX_POS..=MAX_POS).contains(&y) {
                log::warn!(
                    "window position y={y} out of range, clamping to -{MAX_POS}..={MAX_POS}"
                );
                y = y.clamp(-MAX_POS, MAX_POS);
            }
            warn_wayland_noop("position");
            window::Position::Specific(Point::new(x, y))
        }
        _ => window::Position::default(),
    };

    let min_size = parse_optional_size(v.get("min_size").unwrap_or(&serde_json::Value::Null));
    let max_size = parse_optional_size(v.get("max_size").unwrap_or(&serde_json::Value::Null));

    let level = parse_window_level_str(v.get("level").and_then(|v| v.as_str()).unwrap_or("normal"));

    window::Settings {
        size: Size::new(width, height),
        maximized,
        fullscreen,
        position,
        min_size,
        max_size,
        visible,
        resizable,
        closeable,
        minimizable,
        decorations,
        transparent,
        blur,
        level,
        exit_on_close_request,
        ..window::Settings::default()
    }
}

/// Extract an optional per-window scale_factor from a JSON value.
/// Returns `None` when absent (meaning "use global default"), or
/// `Some(validated)` when present.
fn parse_scale_factor(v: &serde_json::Value) -> Option<f32> {
    v.get("scale_factor")
        .and_then(|v| v.as_f64())
        .map(|v| crate::app::validate_scale_factor(v as f32))
}

fn parse_optional_size(v: &serde_json::Value) -> Option<Size> {
    let w = v.get("width").and_then(|v| v.as_f64())? as f32;
    let h = v.get("height").and_then(|v| v.as_f64())? as f32;
    Some(Size::new(w, h))
}

fn parse_window_level_str(s: &str) -> window::Level {
    match s {
        "always_on_top" => window::Level::AlwaysOnTop,
        "always_on_bottom" => window::Level::AlwaysOnBottom,
        _ => window::Level::Normal,
    }
}

fn parse_direction(s: &str) -> Option<window::Direction> {
    match s {
        "north" => Some(window::Direction::North),
        "south" => Some(window::Direction::South),
        "east" => Some(window::Direction::East),
        "west" => Some(window::Direction::West),
        "north_east" => Some(window::Direction::NorthEast),
        "north_west" => Some(window::Direction::NorthWest),
        "south_east" => Some(window::Direction::SouthEast),
        "south_west" => Some(window::Direction::SouthWest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{Size, window};
    use serde_json::json;

    #[test]
    fn parse_window_settings_defaults() {
        let settings = parse_window_settings(&json!({}));
        assert_eq!(settings.size, Size::new(800.0, 600.0));
        assert!(settings.visible);
        assert!(settings.resizable);
        assert!(settings.decorations);
        assert!(!settings.maximized);
        assert!(!settings.fullscreen);
        assert!(!settings.transparent);
    }

    #[test]
    fn parse_window_settings_custom_size() {
        let settings = parse_window_settings(&json!({"width": 1024, "height": 768}));
        assert_eq!(settings.size, Size::new(1024.0, 768.0));
    }

    #[test]
    fn parse_window_settings_centered_position() {
        let settings = parse_window_settings(&json!({"position": "centered"}));
        assert!(matches!(settings.position, window::Position::Centered));
    }

    #[test]
    fn parse_window_settings_specific_position() {
        let settings = parse_window_settings(&json!({"position": {"x": 100, "y": 200}}));
        match settings.position {
            window::Position::Specific(p) => {
                assert_eq!(p.x, 100.0);
                assert_eq!(p.y, 200.0);
            }
            _ => panic!("expected Specific position"),
        }
    }

    #[test]
    fn parse_window_settings_boolean_flags() {
        let settings = parse_window_settings(&json!({
            "maximized": true,
            "transparent": true,
            "decorations": false,
            "resizable": false,
        }));
        assert!(settings.maximized);
        assert!(settings.transparent);
        assert!(!settings.decorations);
        assert!(!settings.resizable);
    }

    #[test]
    fn parse_optional_size_from_object() {
        let sz = parse_optional_size(&json!({"width": 100, "height": 200}));
        assert_eq!(sz, Some(Size::new(100.0, 200.0)));
    }

    #[test]
    fn parse_optional_size_null() {
        let sz = parse_optional_size(&json!(null));
        assert_eq!(sz, None);
    }

    #[test]
    fn parse_window_level_variants() {
        assert!(matches!(
            parse_window_level_str("always_on_top"),
            window::Level::AlwaysOnTop
        ));
        assert!(matches!(
            parse_window_level_str("always_on_bottom"),
            window::Level::AlwaysOnBottom
        ));
        assert!(matches!(
            parse_window_level_str("normal"),
            window::Level::Normal
        ));
        assert!(matches!(
            parse_window_level_str("unknown"),
            window::Level::Normal
        ));
    }

    #[test]
    fn parse_direction_variants() {
        assert!(matches!(
            parse_direction("north"),
            Some(window::Direction::North)
        ));
        assert!(matches!(
            parse_direction("south_west"),
            Some(window::Direction::SouthWest)
        ));
        assert!(parse_direction("invalid").is_none());
    }
}
