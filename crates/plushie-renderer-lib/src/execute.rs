//! Typed command execution for RendererOp.
//!
//! [`App::execute`] dispatches typed [`RendererOp`] variants directly
//! to iced operations. This is the primary entry point for direct mode
//! (zero serialization). Wire mode currently uses
//! [`App::apply`](crate::apply) with `IncomingMessage`.

use iced::Task;

use plushie_core::ops::*;
use plushie_widget_sdk::runtime::Message;

use crate::App;

impl App {
    /// Execute a typed renderer operation.
    ///
    /// Returns an iced Task for operations that need async completion
    /// (focus, scroll, effects, window queries).
    pub fn execute(&mut self, op: RendererOp) -> Task<Message> {
        use iced::widget::operation;

        match op {
            // -- Widget-targeted command (unified) --
            RendererOp::Command {
                ref id,
                ref family,
                ref value,
            } => self.execute_command(id, family, value),
            RendererOp::Commands(commands) => {
                // Atomic batch: buffer outgoing events so observers
                // see a single consistent state after all commands
                // commit.
                self.emitter.begin_batch();
                let tasks: Vec<_> = commands
                    .iter()
                    .map(|cmd| self.execute_command(&cmd.id, &cmd.family, &cmd.value))
                    .collect();
                Task::batch([Task::batch(tasks), self.emitter.end_batch()])
            }

            // -- Global focus (no target widget) --
            RendererOp::FocusNext => operation::focus_next(),
            RendererOp::FocusPrevious => operation::focus_previous(),
            RendererOp::FocusNextWithin { scope } => {
                operation::focus_next_within(iced::advanced::widget::Id::from(scope))
            }
            RendererOp::FocusPreviousWithin { scope } => {
                operation::focus_previous_within(iced::advanced::widget::Id::from(scope))
            }

            // -- Accessibility --
            //
            // Politeness is carried on the wire but collapsed to the
            // fork's assertive `announce` for now. Per-politeness
            // routing requires fork-level additions; the SDK-level
            // API is future-proofed so app code can specify politeness
            // today and the renderer picks it up when the fork grows
            // the `announce_polite` variant.
            RendererOp::Announce { text, .. } => iced::announce(text),

            // -- Window operations --
            RendererOp::Window(op) => self.dispatch_window_op(op),
            RendererOp::WindowQuery(query) => self.dispatch_window_query(query),

            // -- System --
            RendererOp::SystemOp(op) => self.dispatch_system_op(op),
            RendererOp::SystemQuery(query) => self.dispatch_system_query(query),

            // -- Effects --
            RendererOp::Effect { tag, request, .. } => {
                if self.effect_handler.is_async(&request) {
                    let future = self.effect_handler.handle_async(tag, request);
                    let sink = self.emitter.sink();
                    Task::perform(future, move |response| {
                        // sink lock is the innermost; no nested locks
                        // here, and iced's async continuation must
                        // keep it that way.
                        let mut guard = sink.lock();
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

            // -- Font loading --
            RendererOp::LoadFont { family, bytes } => {
                if !crate::constants::try_reserve_runtime_font_load(&family, bytes.len()) {
                    return Task::none();
                }
                plushie_widget_sdk::fonts::register_loaded_family(&family);
                iced::font::load(bytes).map(|_| Message::NoOp)
            }

            // -- Subscriptions --
            RendererOp::Subscribe {
                kind,
                tag,
                max_rate,
                window_id,
            } => {
                use plushie_widget_sdk::protocol::IncomingMessage;
                self.core.apply(IncomingMessage::Subscribe {
                    kind,
                    tag,
                    window_id,
                    max_rate,
                });
                self.sync_subscription_rates();
                self.cleanup_subscription_rates();
                Task::none()
            }
            RendererOp::Unsubscribe { kind, tag } => {
                use plushie_widget_sdk::protocol::IncomingMessage;
                self.core.apply(IncomingMessage::Unsubscribe { kind, tag });
                self.sync_subscription_rates();
                self.cleanup_subscription_rates();
                Task::none()
            }

            // -- Testing / debugging --
            RendererOp::TreeHash { tag } => {
                self.handle_widget_op("tree_hash", &serde_json::json!({"target": tag}))
            }
            RendererOp::FindFocused { tag } => {
                self.handle_widget_op("find_focused", &serde_json::json!({"target": tag}))
            }
            RendererOp::AdvanceFrame { timestamp } => self.handle_widget_op(
                "advance_frame",
                &serde_json::json!({"timestamp": timestamp}),
            ),
            _ => Task::none(),
        }
    }

    fn execute_image_op(&mut self, op: ImageOp) -> Task<Message> {
        match op {
            ImageOp::Create { handle, data } => {
                self.handle_image_op("create_image", &handle, Some(data), None, None, None);
                Task::none()
            }
            ImageOp::CreateRaw {
                handle,
                width,
                height,
                pixels,
            } => {
                self.handle_image_op(
                    "create_image",
                    &handle,
                    None,
                    Some(pixels),
                    Some(width),
                    Some(height),
                );
                Task::none()
            }
            ImageOp::Update { handle, data } => {
                self.handle_image_op("update_image", &handle, Some(data), None, None, None);
                Task::none()
            }
            ImageOp::UpdateRaw {
                handle,
                width,
                height,
                pixels,
            } => {
                self.handle_image_op(
                    "update_image",
                    &handle,
                    None,
                    Some(pixels),
                    Some(width),
                    Some(height),
                );
                Task::none()
            }
            ImageOp::Delete(handle) => {
                self.handle_image_op("delete_image", &handle, None, None, None, None);
                Task::none()
            }
            ImageOp::List { tag } => {
                self.handle_widget_op("list_images", &serde_json::json!({"tag": tag}))
            }
            ImageOp::Clear => self.handle_widget_op("clear_images", &serde_json::json!({})),
            _ => Task::none(),
        }
    }

    /// Dispatch a widget-targeted command by family.
    ///
    /// Built-in operations (focus, scroll, text cursor) return iced Tasks.
    /// Everything else routes to the widget registry.
    pub(crate) fn execute_command(
        &mut self,
        id: &str,
        family: &str,
        value: &serde_json::Value,
    ) -> Task<Message> {
        use iced::widget::Id as WId;
        use iced::widget::operation;

        match family {
            "focus" => {
                if id.contains('/') {
                    self.registry
                        .handle_widget_op(id, "focus", &serde_json::json!({}));
                    let canvas_id = self
                        .registry
                        .get_for_node_id(id)
                        .map(|(_, matched)| matched.to_string())
                        .unwrap_or_else(|| id.to_string());
                    operation::focus::<Message>(WId::from(canvas_id))
                } else {
                    operation::focus::<Message>(WId::from(id.to_string()))
                }
            }
            "select_all" => operation::select_all(WId::from(id.to_string())),
            "move_cursor_to_front" => operation::move_cursor_to_front(WId::from(id.to_string())),
            "move_cursor_to_end" => operation::move_cursor_to_end(WId::from(id.to_string())),
            "move_cursor_to" => {
                let pos = value.as_u64().unwrap_or(0) as usize;
                operation::move_cursor_to(WId::from(id.to_string()), pos)
            }
            "select_range" => {
                let start = value.get("start_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let end = value.get("end_pos").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                operation::select_range(WId::from(id.to_string()), start, end)
            }
            "scroll_to" => {
                let x = value.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = value.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                operation::scroll_to(
                    WId::from(id.to_string()),
                    operation::AbsoluteOffset { x, y },
                )
            }
            "scroll_by" => {
                let x = value.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = value.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                operation::scroll_by(
                    WId::from(id.to_string()),
                    operation::AbsoluteOffset { x, y },
                )
            }
            "snap_to" => {
                let x = value.get("x").and_then(|v| v.as_f64()).map(|v| v as f32);
                let y = value.get("y").and_then(|v| v.as_f64()).map(|v| v as f32);
                operation::snap_to(
                    WId::from(id.to_string()),
                    operation::RelativeOffset { x, y },
                )
            }
            "snap_to_end" => operation::snap_to_end(WId::from(id.to_string())),
            // Everything else routes to the widget registry (native widgets,
            // pane grid ops, etc.)
            _ => {
                self.registry.handle_widget_op(id, family, value);
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;

    use parking_lot::Mutex;
    use plushie_core::ops::EffectRequest;
    use plushie_widget_sdk::protocol::{DiagnosticMessage, EffectResponse, OutgoingEvent};

    struct NullEffectHandler;

    impl crate::effects::EffectHandler for NullEffectHandler {
        fn handle_sync(&self, _: &str, _: &EffectRequest) -> Option<EffectResponse> {
            None
        }

        fn handle_async(
            &self,
            _: String,
            _: EffectRequest,
        ) -> Pin<Box<dyn Future<Output = EffectResponse> + Send>> {
            Box::pin(async { unreachable!() })
        }

        fn is_async(&self, _: &EffectRequest) -> bool {
            false
        }
    }

    struct NullSink;

    impl crate::emitters::EventSink for NullSink {
        fn emit_event(&mut self, _: OutgoingEvent) -> std::io::Result<()> {
            Ok(())
        }

        fn emit_effect_response(&mut self, _: EffectResponse) -> std::io::Result<()> {
            Ok(())
        }

        fn emit_query_response(
            &mut self,
            _: &str,
            _: &str,
            _: &serde_json::Value,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn emit_screenshot_response(
            &mut self,
            _: &str,
            _: &str,
            _: &str,
            _: u32,
            _: u32,
            _: &[u8],
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn emit_hello(
            &mut self,
            _: &str,
            _: &str,
            _: &[&str],
            _: &[&str],
            _: &str,
        ) -> std::io::Result<()> {
            Ok(())
        }

        fn emit_diagnostic(&mut self, _: DiagnosticMessage) -> std::io::Result<()> {
            Ok(())
        }

        fn write_raw(&mut self, _: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn test_app() -> App {
        let sink = Arc::new(Mutex::new(
            Box::new(NullSink) as Box<dyn crate::emitters::EventSink>
        ));
        App::new(
            plushie_widget_sdk::registry::WidgetRegistry::new(),
            Box::new(NullEffectHandler),
            sink,
        )
    }

    #[test]
    fn execute_subscribe_updates_emitter_rate() {
        let mut app = test_app();

        let _ = app.execute(RendererOp::Subscribe {
            kind: "on_pointer_move".to_string(),
            tag: "on_pointer_move".to_string(),
            max_rate: Some(30),
            window_id: None,
        });

        assert_eq!(
            app.emitter.subscription_rate_for("on_pointer_move"),
            Some(30)
        );
    }

    #[test]
    fn execute_unsubscribe_removes_emitter_rate() {
        let mut app = test_app();

        let _ = app.execute(RendererOp::Subscribe {
            kind: "on_pointer_move".to_string(),
            tag: "on_pointer_move".to_string(),
            max_rate: Some(30),
            window_id: None,
        });
        let _ = app.execute(RendererOp::Unsubscribe {
            kind: "on_pointer_move".to_string(),
            tag: "on_pointer_move".to_string(),
        });

        assert_eq!(app.emitter.subscription_rate_for("on_pointer_move"), None);
    }

    #[test]
    fn execute_subscribe_without_rate_removes_existing_rate() {
        let mut app = test_app();

        let _ = app.execute(RendererOp::Subscribe {
            kind: "on_pointer_move".to_string(),
            tag: "on_pointer_move".to_string(),
            max_rate: Some(30),
            window_id: None,
        });
        let _ = app.execute(RendererOp::Subscribe {
            kind: "on_pointer_move".to_string(),
            tag: "on_pointer_move".to_string(),
            max_rate: None,
            window_id: None,
        });

        assert_eq!(app.emitter.subscription_rate_for("on_pointer_move"), None);
    }
}
