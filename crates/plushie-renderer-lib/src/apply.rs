//! Processes incoming protocol messages (snapshots, patches, settings,
//! widget commands) by delegating to Core and handling resulting effects.

use std::io;

use plushie_renderer_engine::CoreEffect;
use plushie_widget_sdk::protocol::IncomingMessage;

use crate::App;

impl App {
    pub fn apply(&mut self, message: IncomingMessage) -> io::Result<()> {
        // Widget commands bypass the normal tree update / diff / patch cycle.
        // Route through the unified widget registry.
        match &message {
            IncomingMessage::Command { id, family, value } => {
                // Route through execute_command which handles both
                // built-in ops (focus, scroll, text cursor) and
                // native widget commands (via registry fallback).
                let task = self.execute_command(id, family, value);
                self.pending_tasks.push(task);
                return Ok(());
            }
            IncomingMessage::Commands { commands } => {
                // Atomic batch: suppress outgoing events until all
                // commands have been applied, then flush in order.
                self.emitter.begin_batch();
                for cmd in commands {
                    let task = self.execute_command(&cmd.id, &cmd.family, &cmd.value);
                    self.pending_tasks.push(task);
                }
                self.pending_tasks.push(self.emitter.end_batch());
                return Ok(());
            }
            _ => {}
        }

        let is_snapshot = matches!(message, IncomingMessage::Snapshot { .. });
        let is_tree_change = matches!(
            message,
            IncomingMessage::Snapshot { .. } | IncomingMessage::Patch { .. }
        );
        let is_subscribe = matches!(message, IncomingMessage::Subscribe { .. });
        let is_unsubscribe = matches!(message, IncomingMessage::Unsubscribe { .. });
        let is_settings = matches!(message, IncomingMessage::Settings { .. });

        if is_snapshot {
            self.pending_tasks.push(self.emitter.flush());
            self.emitter.clear_widget_rates();
        }

        let effects = self.core.apply(message);

        if is_subscribe || is_settings {
            self.sync_subscription_rates();
        }
        if is_subscribe || is_unsubscribe {
            self.cleanup_subscription_rates();
        }
        for effect in effects {
            use plushie_renderer_engine::{Dispatch, Emit, StateChange};
            match effect {
                CoreEffect::Emit(Emit::Event(event)) => self.emitter.emit_event(event)?,
                CoreEffect::Emit(Emit::EffectResponse(response)) => {
                    self.emitter.emit_effect_response(response)?;
                }
                CoreEffect::Emit(Emit::StubAck(ack)) => {
                    let bytes = self.codec.encode(&ack).map_err(io::Error::other)?;
                    self.emitter.write_raw(&bytes)?;
                }
                CoreEffect::Dispatch(Dispatch::Effect {
                    request_id,
                    kind,
                    payload,
                }) => {
                    match plushie_core::ops::validate_effect_request_from_wire(&kind, &payload) {
                        Ok(request) => {
                            if self.effect_handler.is_async(&request) {
                                let future = self.effect_handler.handle_async(request_id, request);
                                let sink = self.emitter.sink();
                                let task = plushie_widget_sdk::iced::Task::perform(
                                    future,
                                    move |response| {
                                        // sink lock is the innermost; no
                                        // nested locks in this continuation.
                                        let mut guard = sink.lock();
                                        if let Err(e) = guard.emit_effect_response(response) {
                                            log::error!("effect response write error: {e}");
                                        }
                                        plushie_widget_sdk::runtime::Message::NoOp
                                    },
                                );
                                self.pending_tasks.push(task);
                            } else if let Some(response) =
                                self.effect_handler.handle_sync(&request_id, &request)
                            {
                                self.emitter.emit_effect_response(response)?;
                            }
                        }
                        Err(err) if request_id.is_empty() => {
                            log::warn!("invalid effect request without response id: {err}");
                        }
                        Err(err) => {
                            log::warn!("invalid effect request: {err}");
                            self.emitter.emit_effect_response(
                                plushie_widget_sdk::protocol::EffectResponse::error(
                                    request_id,
                                    err.to_string(),
                                ),
                            )?;
                        }
                    }
                }
                CoreEffect::Dispatch(Dispatch::WidgetOp { op, payload }) => {
                    let task = self.handle_widget_op(&op, &payload);
                    self.pending_tasks.push(task);
                }
                CoreEffect::Dispatch(Dispatch::Window(op)) => {
                    let task = self.dispatch_window_op(op);
                    self.pending_tasks.push(task);
                }
                CoreEffect::Dispatch(Dispatch::WindowQuery(q)) => {
                    let task = self.dispatch_window_query(q);
                    self.pending_tasks.push(task);
                }
                CoreEffect::Dispatch(Dispatch::System(op)) => {
                    let task = self.dispatch_system_op(op);
                    self.pending_tasks.push(task);
                }
                CoreEffect::Dispatch(Dispatch::SystemQuery(q)) => {
                    let task = self.dispatch_system_query(q);
                    self.pending_tasks.push(task);
                }
                CoreEffect::Dispatch(Dispatch::Image {
                    op,
                    handle,
                    data,
                    pixels,
                    width,
                    height,
                }) => {
                    self.handle_image_op(&op, &handle, data, pixels, width, height);
                }
                CoreEffect::StateChange(StateChange::SyncWindows) => {
                    let task = self.sync_windows();
                    self.pending_tasks.push(task);
                }
                CoreEffect::StateChange(StateChange::ThemeChanged(theme, chrome)) => {
                    self.theme = theme;
                    self.theme_chrome = chrome;
                    self.theme_follows_system = false;
                }
                CoreEffect::StateChange(StateChange::ThemeFollowsSystem) => {
                    self.theme_chrome = plushie_widget_sdk::runtime::ThemeChrome::default();
                    self.theme_follows_system = true;
                }
                CoreEffect::StateChange(StateChange::WidgetConfig(config)) => {
                    let ctx = plushie_widget_sdk::registry::InitCtx {
                        config: &config,
                        theme: &self.theme,
                        default_text_size: self.core.default_text_size,
                        default_font: self.core.default_font,
                    };
                    self.registry.init_all(&ctx);
                    for diag in self.registry.family_collision_diagnostics() {
                        self.emitter.emit_event(diag)?;
                    }
                }
                CoreEffect::StateChange(StateChange::ExitNodes(nodes)) => {
                    for (parent_id, index, node) in nodes {
                        self.transition_manager
                            .ghosts
                            .add_ghost(&parent_id, node, index);
                    }
                }
            }
        }

        if is_tree_change {
            self.windows.clear_theme_cache();
            let window_ids = self.core.tree.window_ids();
            log::debug!("window sync: {} windows", window_ids.len());
            for win_id in window_ids {
                if let Some(node) = self.core.tree.find_window(&win_id)
                    && let Some(theme_val) = node.props.get_value("theme")
                {
                    match plushie_widget_sdk::runtime::resolve_theme_resolution(&theme_val) {
                        plushie_widget_sdk::runtime::ThemeResolution::Theme(theme, chrome) => {
                            self.windows.set_theme(&win_id, theme, chrome);
                        }
                        plushie_widget_sdk::runtime::ThemeResolution::System => {
                            self.windows.set_theme_follows_system(&win_id);
                        }
                        plushie_widget_sdk::runtime::ThemeResolution::Invalid => {}
                    }
                }
            }

            if is_snapshot {
                self.transition_manager.clear();
            }
            // Single depth-first walk drives both the widget-prepare
            // pass and the animation-descriptor scan. Each concern is
            // isolated behind its own `TreeTransform`.
            let validate_props = self.core.is_validate_props_enabled();
            if let Some(root) = self.core.tree.root_mut() {
                self.registry.prepare_and_scan_with_validation(
                    root,
                    &mut self.core.caches,
                    &self.theme,
                    &mut self.transition_manager,
                    validate_props,
                );
            }
        }

        Ok(())
    }

    pub(crate) fn sync_subscription_rates(&mut self) {
        self.emitter.set_default_rate(self.core.default_event_rate);
        for (tag, rate) in self.core.subscription_rates() {
            self.emitter.set_subscription_rate(tag, rate);
        }
    }

    pub(crate) fn cleanup_subscription_rates(&mut self) {
        let active_rate_tags: std::collections::HashSet<String> = self
            .core
            .subscription_rate_tags()
            .map(|s| s.to_string())
            .collect();
        let emitter_keys: Vec<String> = self
            .emitter
            .subscription_rate_keys()
            .map(|s| s.to_string())
            .collect();
        for key in emitter_keys {
            if !active_rate_tags.contains(&key) {
                self.emitter.remove_subscription_rate(&key);
                let task = self
                    .emitter
                    .flush_key(&crate::emitter::CoalesceKey::Subscription(key));
                self.pending_tasks.push(task);
            }
        }
    }
}
