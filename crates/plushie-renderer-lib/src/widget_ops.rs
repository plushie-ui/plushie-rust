//! Widget operations: focus, scroll, cursor, pane grid, font loading,
//! tree hash queries, image management. Dispatched from the widget SDK's
//! `Dispatch::WidgetOp` core effect via the `op` string and JSON
//! `payload`.

use iced::Task;

use plushie_widget_sdk::protocol::OutgoingEvent;
use plushie_widget_sdk::runtime::Message;

use crate::App;

use crate::constants::{MAX_FONT_BYTES, MAX_LOADED_FONTS};

// ---------------------------------------------------------------------------
// Widget operations (impl App)
// ---------------------------------------------------------------------------

impl App {
    /// Dispatch a widget operation by name. Called when Core produces a
    /// `WidgetOp` effect. Returns an iced `Task` for operations that
    /// need async completion (focus, scroll, font load).
    pub fn handle_widget_op(&mut self, op: &str, payload: &serde_json::Value) -> Task<Message> {
        let get_target = || {
            payload
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };

        match op {
            "focus" => {
                let target = get_target();
                // Canvas element focus (target contains "/"): route through
                // the registry so CanvasWidget sets pending focus.
                if target.contains('/') {
                    self.registry.handle_widget_op(&target, "focus", payload);
                    // Use the registry's prefix walk to find the canvas widget ID.
                    let canvas_id = self
                        .registry
                        .get_for_node_id(&target)
                        .map(|(_, matched)| matched.to_string())
                        .unwrap_or(target);
                    iced::widget::operation::focus::<Message>(iced::widget::Id::from(canvas_id))
                } else {
                    iced::widget::operation::focus::<Message>(iced::widget::Id::from(target))
                }
            }
            "focus_next" => iced::widget::operation::focus_next(),
            "focus_previous" => iced::widget::operation::focus_previous(),
            "focus_next_within" => {
                let scope = payload
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                iced::widget::operation::focus_next_within(iced::widget::Id::from(scope))
            }
            "focus_previous_within" => {
                let scope = payload
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                iced::widget::operation::focus_previous_within(iced::widget::Id::from(scope))
            }
            "scroll_to" => {
                let target = get_target();
                let offset_x = payload
                    .get("offset_x")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                let offset_y = payload
                    .get("offset_y")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                iced::widget::operation::scroll_to(
                    iced::widget::Id::from(target),
                    iced::widget::operation::AbsoluteOffset {
                        x: offset_x.unwrap_or(0.0),
                        y: offset_y.unwrap_or(0.0),
                    },
                )
            }
            "scroll_by" => {
                let target = get_target();
                let offset_x = payload
                    .get("offset_x")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let offset_y = payload
                    .get("offset_y")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                iced::widget::operation::scroll_by(
                    iced::widget::Id::from(target),
                    iced::widget::operation::AbsoluteOffset {
                        x: offset_x,
                        y: offset_y,
                    },
                )
            }
            "snap_to" => {
                let target = get_target();
                let x = payload.get("x").and_then(|v| v.as_f64()).map(|v| v as f32);
                let y = payload.get("y").and_then(|v| v.as_f64()).map(|v| v as f32);
                iced::widget::operation::snap_to(
                    iced::widget::Id::from(target),
                    iced::widget::operation::RelativeOffset { x, y },
                )
            }
            "snap_to_end" => {
                let target = get_target();
                iced::widget::operation::snap_to_end(iced::widget::Id::from(target))
            }
            "select_all" => {
                iced::widget::operation::select_all(iced::widget::Id::from(get_target()))
            }
            "select_range" => {
                let target = get_target();
                let start = payload.get("start").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let end = payload.get("end").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                iced::widget::operation::select_range(iced::widget::Id::from(target), start, end)
            }
            "move_cursor_to_front" => {
                iced::widget::operation::move_cursor_to_front(iced::widget::Id::from(get_target()))
            }
            "move_cursor_to_end" => {
                iced::widget::operation::move_cursor_to_end(iced::widget::Id::from(get_target()))
            }
            "move_cursor_to" => {
                let target = get_target();
                let position = payload
                    .get("position")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                iced::widget::operation::move_cursor_to(iced::widget::Id::from(target), position)
            }
            "announce" => {
                // `politeness` is read off the wire for forward-compat
                // (the SDK always sends it) but the fork currently
                // treats all announcements as assertive. When the
                // fork grows a polite variant this branch can route
                // explicitly; until then the politeness hint is
                // logged for visibility but otherwise ignored.
                let text = payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                if let Some(p) = payload.get("politeness").and_then(|v| v.as_str()) {
                    log::trace!("announce politeness={p} (renderer routes to assertive)");
                }
                iced::announce(text)
            }
            "exit" => iced::exit(),
            // -- PaneGrid operations --
            // Routed through the registry to PaneGridWidget::handle_widget_op.
            "pane_split" | "pane_close" | "pane_swap" | "pane_maximize" | "pane_restore" => {
                let target = get_target();
                self.registry.handle_widget_op(&target, op, payload);
                Task::none()
            }
            "find_focused" => {
                let tag = payload
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .unwrap_or("find_focused")
                    .to_string();
                let sink = self.emitter.sink();
                iced::widget::operation::find_focused().map(move |maybe_id| {
                    let focused = maybe_id.map(|id| id.to_string());
                    // sink lock is the innermost; no nested locks.
                    let mut guard = sink.lock();
                    if let Err(e) = guard.emit_query_response(
                        "find_focused",
                        &tag,
                        &serde_json::json!({"focused": focused}),
                    ) {
                        log::error!("write error: {e}");
                    }
                    Message::NoOp
                })
            }
            // Load a font from base64-encoded data at runtime. Supports
            // TrueType (.ttf), OpenType (.otf), and TrueType Collections
            // (.ttc). Variable fonts are supported. Format detection is
            // handled by fontdb (via cosmic-text), so no explicit format
            // field is needed.
            "load_font" => {
                let family = payload
                    .get("family")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let data = payload
                    .get("data")
                    .and_then(crate::settings::decode_font_data)
                    .unwrap_or_default();
                if family.is_empty() {
                    log::warn!("load_font: missing family name");
                    Task::none()
                } else if data.is_empty() {
                    log::warn!("load_font: no font data provided for family {family}");
                    Task::none()
                } else if data.len() > MAX_FONT_BYTES {
                    log::warn!(
                        "load_font: font data for {family} ({} bytes) exceeds {} byte limit, rejecting",
                        data.len(),
                        MAX_FONT_BYTES
                    );
                    Task::none()
                } else if !crate::constants::try_reserve_font_slot() {
                    log::warn!(
                        "load_font: already loaded {MAX_LOADED_FONTS} fonts, \
                         rejecting to prevent unbounded memory growth"
                    );
                    Task::none()
                } else {
                    plushie_widget_sdk::fonts::register_loaded_family(family);
                    let family_for_log = family.to_string();
                    iced::font::load(data).map(move |result| {
                        match result {
                            Ok(()) => log::info!("font {family_for_log} loaded successfully"),
                            Err(e) => {
                                log::error!("font {family_for_log} load failed: {e:?}")
                            }
                        }
                        Message::NoOp
                    })
                }
            }
            "tree_hash" => {
                let tag = payload
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tree_hash")
                    .to_string();
                let hash = self.core.tree_hash();
                if let Err(e) = self.emitter.emit_query_response(
                    "tree_hash",
                    &tag,
                    &serde_json::json!({"hash": hash}),
                ) {
                    log::error!("write error: {e}");
                    return iced::exit();
                }
                Task::none()
            }
            "list_images" => {
                let tag = payload
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .unwrap_or("list_images")
                    .to_string();
                let handles: Vec<String> = self.image_registry.handle_names();
                if let Err(e) = self.emitter.emit_query_response(
                    "list_images",
                    &tag,
                    &serde_json::json!({"handles": handles}),
                ) {
                    log::error!("write error: {e}");
                    return iced::exit();
                }
                Task::none()
            }
            "clear_images" => {
                self.image_registry.clear();
                Task::none()
            }
            other => {
                log::warn!("unknown widget_op: {other}");
                Task::none()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Image operations
    // -----------------------------------------------------------------------

    /// Apply an image operation (create, update, remove) to the
    /// in-memory image registry. Emits an error event on failure.
    pub fn handle_image_op(
        &mut self,
        op: &str,
        handle: &str,
        data: Option<Vec<u8>>,
        pixels: Option<Vec<u8>>,
        width: Option<u32>,
        height: Option<u32>,
    ) {
        if let Err(error) = self
            .image_registry
            .apply_op(op, handle, data, pixels, width, height)
        {
            // Best-effort error notification. If stdout is broken the
            // next synchronous write in update() will exit cleanly.
            if let Err(e) = self.emitter.emit_event(OutgoingEvent::generic(
                "image_error".to_string(),
                handle.to_string(),
                Some(serde_json::json!({ "error": error })),
            )) {
                log::error!("write error: {e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PaneGrid helpers
// ---------------------------------------------------------------------------
