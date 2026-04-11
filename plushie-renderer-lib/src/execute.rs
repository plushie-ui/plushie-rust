//! Typed command execution for RendererOp.
//!
//! [`App::execute`] dispatches typed [`RendererOp`] variants directly
//! to iced operations. This is the primary entry point for direct mode
//! (zero serialization). Wire mode currently uses
//! [`App::apply`](crate::apply) with `IncomingMessage`.

use iced::Task;

use plushie_core::ops::*;
use plushie_widget_sdk::message::Message;

use crate::App;

impl App {
    /// Execute a typed renderer operation.
    ///
    /// Returns an iced Task for operations that need async completion
    /// (focus, scroll, effects, window queries).
    pub fn execute(&mut self, op: RendererOp) -> Task<Message> {
        use iced::widget::operation;
        use iced::widget::Id;

        match op {
            // -- Focus --
            RendererOp::Focus(id) => {
                if id.contains('/') {
                    // Canvas element focus
                    self.registry.handle_widget_op(&id, "focus", &serde_json::json!({}));
                    let parent = id.rsplit_once('/').map(|(p, _)| p).unwrap_or(&id);
                    operation::focus::<Message>(Id::from(parent.to_string()))
                } else {
                    operation::focus::<Message>(Id::from(id))
                }
            }
            RendererOp::FocusNext => operation::focus_next(),
            RendererOp::FocusPrevious => operation::focus_previous(),

            // -- Text operations --
            RendererOp::SelectAll(id) => operation::select_all(Id::from(id)),
            RendererOp::MoveCursorToFront(id) => operation::move_cursor_to_front(Id::from(id)),
            RendererOp::MoveCursorToEnd(id) => operation::move_cursor_to_end(Id::from(id)),
            RendererOp::MoveCursorTo { target, position } => {
                operation::move_cursor_to(Id::from(target), position)
            }
            RendererOp::SelectRange { target, start, end } => {
                operation::select_range(Id::from(target), start, end)
            }

            // -- Scroll --
            RendererOp::ScrollTo { target, x, y } => operation::scroll_to(
                Id::from(target),
                operation::AbsoluteOffset { x, y },
            ),
            RendererOp::ScrollBy { target, x, y } => operation::scroll_by(
                Id::from(target),
                operation::AbsoluteOffset { x, y },
            ),
            RendererOp::SnapTo { target, x, y } => operation::snap_to(
                Id::from(target),
                operation::RelativeOffset { x: Some(x), y: Some(y) },
            ),
            RendererOp::SnapToEnd(id) => operation::snap_to_end(Id::from(id)),

            // -- Accessibility --
            RendererOp::Announce(text) => iced::announce(text),

            // -- Window operations --
            RendererOp::Window(op) => self.execute_window_op(op),
            RendererOp::WindowQuery(query) => self.execute_window_query(query),

            // -- System --
            RendererOp::SystemOp(op) => self.execute_system_op(op),
            RendererOp::SystemQuery(query) => self.execute_system_query(query),

            // -- Effects --
            RendererOp::Effect { tag, request } => {
                if self.effect_handler.is_async(&request) {
                    let future = self.effect_handler.handle_async(tag, request);
                    let sink = self.emitter.sink();
                    Task::perform(future, move |response| {
                        let mut guard = sink.lock().unwrap_or_else(|e| e.into_inner());
                        if let Err(e) = guard.emit_effect_response(response) {
                            log::error!("effect response write error: {e}");
                        }
                        Message::NoOp
                    })
                } else if let Some(response) = self.effect_handler.handle_sync(&tag, &request) {
                    if let Err(e) = self.emitter.emit_effect_response(response) {
                        log::error!("effect response write error: {e}");
                        return iced::exit();
                    }
                    Task::none()
                } else {
                    Task::none()
                }
            }

            // -- Images --
            RendererOp::Image(op) => self.execute_image_op(op),

            // -- PaneGrid --
            RendererOp::PaneGrid(op) => self.execute_pane_grid_op(op),

            // -- Widget commands --
            RendererOp::WidgetCommand { node_id, op, payload } => {
                self.registry.handle_widget_op(&node_id, &op, &payload);
                Task::none()
            }
            RendererOp::WidgetCommands(commands) => {
                for cmd in commands {
                    self.registry.handle_widget_op(&cmd.node_id, &cmd.op, &cmd.payload);
                }
                Task::none()
            }

            // -- Font loading --
            RendererOp::LoadFont(data) => {
                iced::font::load(data).map(|_| Message::NoOp)
            }

            // -- Subscriptions --
            RendererOp::Subscribe { kind, tag, max_rate, window_id } => {
                use plushie_widget_sdk::protocol::IncomingMessage;
                self.core.apply(IncomingMessage::Subscribe {
                    kind, tag, window_id, max_rate,
                });
                Task::none()
            }
            RendererOp::Unsubscribe { kind, tag } => {
                use plushie_widget_sdk::protocol::IncomingMessage;
                self.core.apply(IncomingMessage::Unsubscribe {
                    kind, tag: Some(tag),
                });
                Task::none()
            }

            // -- Testing / debugging --
            RendererOp::TreeHash { tag } => {
                self.handle_widget_op("tree_hash", &serde_json::json!({"target": tag}))
            }
            RendererOp::FindFocused { tag } => {
                self.handle_widget_op("find_focused", &serde_json::json!({"target": tag}))
            }
            RendererOp::AdvanceFrame { timestamp } => {
                self.handle_widget_op("advance_frame", &serde_json::json!({"timestamp": timestamp}))
            }
        }
    }

    fn execute_window_op(&mut self, op: WindowOp) -> Task<Message> {
        use serde_json::json;
        // Delegate to the existing string-based handler for now.
        // This will be refactored to use typed dispatch in a future phase.
        match op {
            WindowOp::Close(id) => self.handle_widget_op("close_window", &json!({"window_id": id})),
            WindowOp::Resize { window_id, width, height } => {
                self.handle_window_op("resize", &window_id, &json!({"width": width, "height": height}))
            }
            WindowOp::Move { window_id, x, y } => {
                self.handle_window_op("move", &window_id, &json!({"x": x, "y": y}))
            }
            WindowOp::Maximize { window_id, maximized } => {
                self.handle_window_op("maximize", &window_id, &json!({"maximized": maximized}))
            }
            WindowOp::Minimize { window_id, minimized } => {
                self.handle_window_op("minimize", &window_id, &json!({"minimized": minimized}))
            }
            WindowOp::SetMode { window_id, mode } => {
                self.handle_window_op("set_mode", &window_id, &json!({"mode": mode}))
            }
            WindowOp::ToggleMaximize(id) => {
                self.handle_window_op("toggle_maximize", &id, &json!({}))
            }
            WindowOp::ToggleDecorations(id) => {
                self.handle_window_op("toggle_decorations", &id, &json!({}))
            }
            WindowOp::FocusWindow(id) => {
                self.handle_window_op("gain_focus", &id, &json!({}))
            }
            WindowOp::SetLevel { window_id, level } => {
                self.handle_window_op("set_level", &window_id, &json!({"level": level}))
            }
            WindowOp::DragWindow(id) => {
                self.handle_window_op("drag", &id, &json!({}))
            }
            WindowOp::DragResize { window_id, direction } => {
                self.handle_window_op("drag_resize", &window_id, &json!({"direction": direction}))
            }
            WindowOp::RequestAttention { window_id, urgency } => {
                self.handle_window_op("request_attention", &window_id, &json!({"urgency": urgency}))
            }
            WindowOp::Screenshot { window_id, tag } => {
                self.handle_window_op("screenshot", &window_id, &json!({"tag": tag}))
            }
            WindowOp::SetResizable { window_id, resizable } => {
                self.handle_window_op("set_resizable", &window_id, &json!({"resizable": resizable}))
            }
            WindowOp::SetMinSize { window_id, width, height } => {
                self.handle_window_op("set_min_size", &window_id, &json!({"width": width, "height": height}))
            }
            WindowOp::SetMaxSize { window_id, width, height } => {
                self.handle_window_op("set_max_size", &window_id, &json!({"width": width, "height": height}))
            }
            WindowOp::EnableMousePassthrough(id) => {
                self.handle_window_op("mouse_passthrough", &id, &json!({"enabled": true}))
            }
            WindowOp::DisableMousePassthrough(id) => {
                self.handle_window_op("mouse_passthrough", &id, &json!({"enabled": false}))
            }
            WindowOp::ShowSystemMenu(id) => {
                self.handle_window_op("show_system_menu", &id, &json!({}))
            }
            WindowOp::SetIcon { window_id, data, width, height } => {
                self.handle_window_op("set_icon", &window_id, &json!({
                    "data": data, "width": width, "height": height
                }))
            }
            WindowOp::SetResizeIncrements { window_id, width, height } => {
                self.handle_window_op("set_resize_increments", &window_id, &json!({
                    "width": width, "height": height
                }))
            }
        }
    }

    fn execute_window_query(&mut self, query: WindowQuery) -> Task<Message> {
        use serde_json::json;
        match query {
            WindowQuery::GetSize { window_id, tag } => {
                self.handle_window_op("get_size", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::GetPosition { window_id, tag } => {
                self.handle_window_op("get_position", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::IsMaximized { window_id, tag } => {
                self.handle_window_op("is_maximized", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::IsMinimized { window_id, tag } => {
                self.handle_window_op("is_minimized", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::GetMode { window_id, tag } => {
                self.handle_window_op("get_mode", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::GetScaleFactor { window_id, tag } => {
                self.handle_window_op("get_scale_factor", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::MonitorSize { window_id, tag } => {
                self.handle_window_op("monitor_size", &window_id, &json!({"tag": tag}))
            }
            WindowQuery::RawId { window_id, tag } => {
                self.handle_window_op("raw_id", &window_id, &json!({"tag": tag}))
            }
        }
    }

    fn execute_system_op(&mut self, op: SystemOp) -> Task<Message> {
        match op {
            SystemOp::AllowAutomaticTabbing(enabled) => {
                self.handle_system_op("allow_automatic_tabbing", &serde_json::json!({"enabled": enabled}))
            }
        }
    }

    fn execute_system_query(&mut self, query: SystemQuery) -> Task<Message> {
        match query {
            SystemQuery::GetTheme { tag } => {
                self.handle_system_query("get_system_theme", &serde_json::json!({"tag": tag}))
            }
            SystemQuery::GetInfo { tag } => {
                self.handle_system_query("get_system_info", &serde_json::json!({"tag": tag}))
            }
        }
    }

    fn execute_image_op(&mut self, op: ImageOp) -> Task<Message> {
        match op {
            ImageOp::Create { handle, data } => {
                self.handle_image_op("create_from_bytes", &handle, Some(data), None, None, None);
                Task::none()
            }
            ImageOp::CreateRaw { handle, width, height, pixels } => {
                self.handle_image_op("create_from_rgba", &handle, None, Some(pixels), Some(width), Some(height));
                Task::none()
            }
            ImageOp::Update { handle, data } => {
                self.handle_image_op("create_from_bytes", &handle, Some(data), None, None, None);
                Task::none()
            }
            ImageOp::UpdateRaw { handle, width, height, pixels } => {
                self.handle_image_op("create_from_rgba", &handle, None, Some(pixels), Some(width), Some(height));
                Task::none()
            }
            ImageOp::Delete(handle) => {
                self.handle_image_op("delete", &handle, None, None, None, None);
                Task::none()
            }
            ImageOp::List { tag } => {
                self.handle_widget_op("list_images", &serde_json::json!({"target": tag}))
            }
            ImageOp::Clear => {
                self.handle_widget_op("clear_images", &serde_json::json!({}))
            }
        }
    }

    fn execute_pane_grid_op(&mut self, op: PaneGridOp) -> Task<Message> {
        use serde_json::json;
        match op {
            PaneGridOp::Split { target, pane, axis, new_pane } => {
                self.handle_widget_op("pane_split", &json!({
                    "target": target, "pane": pane, "axis": axis, "new_pane": new_pane
                }))
            }
            PaneGridOp::Close { target, pane } => {
                self.handle_widget_op("pane_close", &json!({"target": target, "pane": pane}))
            }
            PaneGridOp::Swap { target, a, b } => {
                self.handle_widget_op("pane_swap", &json!({"target": target, "a": a, "b": b}))
            }
            PaneGridOp::Maximize { target, pane } => {
                self.handle_widget_op("pane_maximize", &json!({"target": target, "pane": pane}))
            }
            PaneGridOp::Restore(target) => {
                self.handle_widget_op("pane_restore", &json!({"target": target}))
            }
        }
    }
}
