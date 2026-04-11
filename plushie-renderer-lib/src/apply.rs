//! Processes incoming protocol messages (snapshots, patches, settings,
//! widget commands) by delegating to Core and handling resulting effects.

use std::io;

use plushie_widget_sdk::engine::CoreEffect;
use plushie_widget_sdk::protocol::IncomingMessage;

use crate::App;
use crate::emitters::{emit_effect_response, emit_event};

impl App {
    pub fn apply(&mut self, message: IncomingMessage) -> io::Result<()> {
        // Widget commands bypass the normal tree update / diff / patch cycle.
        // Route through the unified widget registry.
        match &message {
            IncomingMessage::WidgetCommand {
                node_id,
                op,
                payload,
            } => {
                if let Some(events) = self.registry.handle_widget_op(node_id, op, payload) {
                    for ev in events {
                        emit_event(ev)?;
                    }
                }
                return Ok(());
            }
            IncomingMessage::WidgetCommands { commands } => {
                for cmd in commands {
                    if let Some(events) =
                        self.registry
                            .handle_widget_op(&cmd.node_id, &cmd.op, &cmd.payload)
                    {
                        for ev in events {
                            emit_event(ev)?;
                        }
                    }
                }
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
            let _ = self.emitter.flush();
            self.emitter.clear_widget_rates();
        }

        let effects = self.core.apply(message);

        if is_subscribe || is_settings {
            self.emitter.set_default_rate(self.core.default_event_rate);
            for (tag, rate) in self.core.subscription_rates() {
                self.emitter.set_subscription_rate(tag, rate);
            }
        }
        if is_subscribe || is_unsubscribe {
            // Collect tags that still have rates
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
                    self.emitter
                        .flush_key(&crate::emitter::CoalesceKey::Subscription(key));
                }
            }
        }
        for effect in effects {
            match effect {
                CoreEffect::SyncWindows => {
                    let task = self.sync_windows();
                    self.pending_tasks.push(task);
                }
                CoreEffect::EmitEvent(event) => emit_event(event)?,
                CoreEffect::EmitEffectResponse(response) => {
                    emit_effect_response(response)?;
                }
                CoreEffect::EmitStubAck(ack) => {
                    let codec = plushie_widget_sdk::codec::Codec::get_global();
                    let bytes = codec.encode(&ack).map_err(io::Error::other)?;
                    crate::emitters::write_output(&bytes)?;
                }
                CoreEffect::HandleEffect {
                    request_id,
                    kind,
                    payload,
                } => {
                    if let Some(request) = plushie_core::ops::effect_request_from_wire(&kind, &payload) {
                        if self.effect_handler.is_async(&request) {
                            let task = self.effect_handler.handle_async(request_id, request);
                            self.pending_tasks.push(task);
                        } else if let Some(response) =
                            self.effect_handler.handle_sync(&request_id, &request)
                        {
                            emit_effect_response(response)?;
                        }
                    } else {
                        log::warn!("unknown effect kind: {kind}");
                    }
                }
                CoreEffect::WidgetOp { op, payload } => {
                    let task = self.handle_widget_op(&op, &payload);
                    self.pending_tasks.push(task);
                }
                CoreEffect::WindowOp {
                    op,
                    window_id,
                    settings,
                } => {
                    let task = self.handle_window_op(&op, &window_id, &settings);
                    self.pending_tasks.push(task);
                }
                CoreEffect::SystemOp { op, settings } => {
                    let task = self.handle_system_op(&op, &settings);
                    self.pending_tasks.push(task);
                }
                CoreEffect::SystemQuery { op, settings } => {
                    let task = self.handle_system_query(&op, &settings);
                    self.pending_tasks.push(task);
                }
                CoreEffect::ThemeChanged(theme) => {
                    self.theme = theme;
                    self.theme_follows_system = false;
                }
                CoreEffect::ThemeFollowsSystem => {
                    self.theme_follows_system = true;
                }
                CoreEffect::ImageOp {
                    op,
                    handle,
                    data,
                    pixels,
                    width,
                    height,
                } => {
                    self.handle_image_op(&op, &handle, data, pixels, width, height);
                }
                CoreEffect::WidgetConfig(config) => {
                    let ctx = plushie_widget_sdk::registry::InitCtx {
                        config: &config,
                        theme: &self.theme,
                        default_text_size: self.core.default_text_size,
                        default_font: self.core.default_font,
                    };
                    self.registry.init_all(&ctx);
                }
                CoreEffect::ExitNodes(nodes) => {
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
                    && let Some(theme_val) = node.props.get("theme")
                    && let Some(theme) = plushie_widget_sdk::theming::resolve_theme_only(theme_val)
                {
                    self.windows.set_theme(&win_id, Some(theme));
                }
            }

            if is_snapshot {
                self.transition_manager.clear();
            }
            if let Some(root) = self.core.tree.root() {
                self.registry
                    .prepare_walk(root, &mut self.core.caches, &self.theme);
            }

            // Scan tree for animation descriptors and start/update animations.
            self.transition_manager.scan_tree(self.core.tree.root());
        }

        Ok(())
    }
}
