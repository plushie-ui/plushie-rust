//! Message dispatcher and stdin handler. Routes iced messages to event
//! handlers, emitters, or the apply pipeline.

use iced::{Task, Theme, window};

use plushie_widget_sdk::message::{Message, StdinEvent};
use plushie_widget_sdk::protocol::{IncomingMessage, OutgoingEvent};

use crate::App;
use crate::constants::*;
use crate::emitter::CoalesceKey;
use crate::emitters::{self, emit_event, emit_screenshot_response};

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Stdin(event) => self.handle_stdin(event),
            Message::NoOp => Task::none(),
            Message::FlushCoalesce => self.emitter.flush(),

            // Widget messages shared between daemon and headless modes.
            // The shared processor handles slider tracking, text editor
            // mutation and pane grid state.
            //
            // Redraw contract: iced::daemon rebuilds UIs after every
            // update() call regardless of the returned Task. Widgets
            // using canvas::Cache must clear caches themselves (see
            // GenerationCounter in registry.rs).
            msg @ (Message::Click(..)
            | Message::Input(..)
            | Message::Submit(..)
            | Message::Toggle(..)
            | Message::Select(..)
            | Message::Paste(..)
            | Message::OptionHovered(..)
            | Message::SensorResize(..)
            | Message::ScrollEvent(..)
            | Message::MouseAreaEvent(..)
            | Message::MouseAreaMove(..)
            | Message::MouseAreaScroll(..)
            | Message::CanvasEvent { .. }
            | Message::CanvasScroll { .. }
            | Message::CanvasElementEnter { .. }
            | Message::CanvasElementLeave { .. }
            | Message::CanvasElementClick { .. }
            | Message::CanvasElementKeyPress { .. }
            | Message::CanvasElementKeyRelease { .. }
            | Message::CanvasElementDrag { .. }
            | Message::CanvasElementDragEnd { .. }
            | Message::CanvasElementFocused { .. }
            | Message::CanvasElementBlurred { .. }
            | Message::CanvasElementFocusChanged { .. }
            | Message::CanvasFocused { .. }
            | Message::CanvasBlurred { .. }
            | Message::CanvasGroupFocused { .. }
            | Message::CanvasGroupBlurred { .. }
            | Message::Diagnostic { .. }
            | Message::Slide(..)
            | Message::SlideRelease(..)
            | Message::TextEditorAction(..)
            | Message::Event { .. }
            | Message::PaneFocusCycle(..)
            | Message::PaneResized(..)
            | Message::PaneDragged(..)
            | Message::PaneClicked(..)) => {
                let events = self.registry.process_message(&msg);
                let mut task = Task::none();
                for event in events {
                    let t = if event.coalesce.is_some() {
                        // Lazily cache event_rate from the widget's tree node.
                        if !event.id.is_empty()
                            && !self.emitter.has_widget_rate(&event.id)
                            && let Some(rate) = self.lookup_widget_event_rate(&event.id)
                        {
                            self.emitter.set_widget_rate(&event.id, rate);
                        }
                        let key = crate::emitter::widget_coalesce_key(&event);
                        self.emitter.coalesce(key, event)
                    } else {
                        self.emitter.emit_immediate(event)
                    };
                    task = Task::batch([task, t]);
                }
                task
            }
            // Status changes are emitted directly, not through the widget
            // message pipeline. This prevents them from being bundled into
            // interact_step event arrays during headless interactions.
            Message::StatusChanged(window_id, id, status) => {
                let event = plushie_widget_sdk::protocol::OutgoingEvent::generic(
                    "status".to_string(),
                    id,
                    Some(serde_json::json!(status)),
                )
                .with_window_id(window_id);
                self.emitter.emit_immediate(event)
            }

            Message::MarkdownUrl(url) => {
                log::debug!("markdown link clicked: {url}");
                Task::none()
            }

            // EffectResult is produced by the SDK's direct runner, not the
            // renderer binary. If it somehow arrives here, ignore it.
            Message::EffectResult { .. } => Task::none(),

            // -- Keyboard events --
            Message::KeyPressed(data, iced_id) => self.handle_key_pressed(data, iced_id),
            Message::KeyReleased(data, iced_id) => self.handle_key_released(data, iced_id),
            Message::ModifiersChanged(mods, iced_id, captured) => {
                self.handle_modifiers_changed(mods, iced_id, captured)
            }

            // -- Mouse events --
            Message::CursorMoved(pos, iced_id, captured) => {
                self.handle_cursor_moved(pos, iced_id, captured)
            }
            Message::CursorEntered(iced_id, captured) => {
                self.handle_cursor_entered(iced_id, captured)
            }
            Message::CursorLeft(iced_id, captured) => self.handle_cursor_left(iced_id, captured),
            Message::MouseButtonPressed(button, iced_id, captured) => {
                self.handle_mouse_button_pressed(button, iced_id, captured)
            }
            Message::MouseButtonReleased(button, iced_id, captured) => {
                self.handle_mouse_button_released(button, iced_id, captured)
            }
            Message::WheelScrolled(delta, iced_id, captured) => {
                self.handle_wheel_scrolled(delta, iced_id, captured)
            }

            // -- Touch events --
            Message::FingerPressed(finger, pos, iced_id, captured) => {
                self.handle_finger_pressed(finger, pos, iced_id, captured)
            }
            Message::FingerMoved(finger, pos, iced_id, captured) => {
                self.handle_finger_moved(finger, pos, iced_id, captured)
            }
            Message::FingerLifted(finger, pos, iced_id, captured) => {
                self.handle_finger_lifted(finger, pos, iced_id, captured)
            }
            Message::FingerLost(finger, pos, iced_id, captured) => {
                self.handle_finger_lost(finger, pos, iced_id, captured)
            }

            // -- IME events --
            Message::ImeOpened(iced_id, captured) => self.handle_ime_opened(iced_id, captured),
            Message::ImePreedit(text, cursor, iced_id, captured) => {
                self.handle_ime_preedit(text, cursor, iced_id, captured)
            }
            Message::ImeCommit(text, iced_id, captured) => {
                self.handle_ime_commit(text, iced_id, captured)
            }
            Message::ImeClosed(iced_id, captured) => self.handle_ime_closed(iced_id, captured),

            // -- Window lifecycle events --
            Message::WindowCloseRequested(iced_id) => {
                // Do NOT close the window or remove from maps here. The host
                // decides whether to close by sending a close_window command
                // or removing the window from the tree. Closing immediately
                // would bypass app-level confirmation dialogs.
                let window_id = self.windows.window_id_for(&iced_id);
                let wid = Some(window_id.as_str()).filter(|s| !s.is_empty());
                let entries = self.core.matching_entries(SUB_WINDOW_CLOSE, wid);
                if !entries.is_empty() {
                    let tasks: Vec<_> = entries
                        .into_iter()
                        .map(|entry| {
                            emitters::emit_or_exit(OutgoingEvent::window_close_requested(
                                entry.tag.clone(),
                                window_id.clone(),
                            ))
                        })
                        .collect();
                    Task::batch(tasks)
                } else {
                    Task::none()
                }
            }
            Message::WindowClosed(iced_id) => {
                if let Some(window_id) = self.windows.remove_by_iced(&iced_id) {
                    let wid = Some(window_id.as_str());
                    for entry in self.core.matching_entries(SUB_WINDOW_EVENT, wid) {
                        if let Err(e) = emit_event(OutgoingEvent::window_closed(
                            entry.tag.clone(),
                            window_id.clone(),
                        )) {
                            log::error!("write error: {e}");
                            return iced::exit();
                        }
                    }
                    log::info!("window closed: {window_id}");
                }
                // All managed windows gone -- notify the host.
                // The host can choose to exit, send a new Snapshot, or take other action.
                // We do NOT call iced::exit() here because the daemon should stay alive
                // to receive new tree snapshots (e.g. after a Reset or window re-creation).
                if self.windows.is_empty() && self.core.tree.root().is_some() {
                    log::info!("all windows closed -- notifying host");
                    return emitters::emit_or_exit(OutgoingEvent::generic(
                        "all_windows_closed".to_string(),
                        String::new(),
                        None,
                    ));
                }
                Task::none()
            }
            Message::WindowOpened(iced_id, window_id) => {
                log::info!("window opened: {window_id} -> {iced_id:?}");
                self.windows.insert(window_id, iced_id);
                Task::none()
            }
            Message::WindowEvent(iced_id, evt) => self.handle_window_event(iced_id, evt),

            // -- System / animation --
            Message::AnimationFrame(instant) => {
                // Advance renderer-side transitions on every frame tick
                let completions = self
                    .transition_manager
                    .advance_all(instant, &mut self.core.caches.interpolated_props);

                // Emit transition_complete events
                for c in completions {
                    let event = OutgoingEvent::generic(
                        "transition_complete",
                        c.widget_id.clone(),
                        Some(serde_json::json!({
                            "tag": c.tag,
                            "prop": c.prop_name,
                        })),
                    );
                    let _ = self.emitter.emit_immediate(event);
                }

                // Forward animation_frame to SDK if subscribed
                let entries = self.core.matching_entries(SUB_ANIMATION_FRAME, None);
                if let Some(entry) = entries.first() {
                    let epoch = *self.animation_epoch.get_or_insert(instant);
                    let millis = instant.duration_since(epoch).as_millis();
                    let event = OutgoingEvent::animation_frame(entry.tag.clone(), millis);
                    self.emitter.coalesce(
                        CoalesceKey::Subscription(SUB_ANIMATION_FRAME.to_string()),
                        event,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ThemeChanged(mode) => {
                // Track system theme so "system" theme value follows OS preference
                self.system_theme = match mode {
                    iced::theme::Mode::Light => Theme::Light,
                    iced::theme::Mode::Dark => Theme::Dark,
                    _ => Theme::Dark,
                };
                // Theme changes are global (not window-scoped), use first entry
                let entries = self.core.matching_entries(SUB_THEME_CHANGE, None);
                if let Some(entry) = entries.first() {
                    let mode_str = match mode {
                        iced::theme::Mode::Light => "light",
                        iced::theme::Mode::Dark => "dark",
                        _ => "system",
                    };
                    let event =
                        OutgoingEvent::theme_changed(entry.tag.clone(), mode_str.to_string());
                    self.emitter.coalesce(
                        CoalesceKey::Subscription(SUB_THEME_CHANGE.to_string()),
                        event,
                    )
                } else {
                    Task::none()
                }
            }
        }
    }

    pub fn handle_stdin(&mut self, event: StdinEvent) -> Task<Message> {
        match event {
            StdinEvent::Message(incoming) => {
                // Flush pending coalesced events on any incoming message.
                // This serves as a "host is ready" signal and provides
                // adaptive throughput matching.
                let _ = self.emitter.flush();
                // Handle scripting messages directly instead of passing
                // them to Core::apply. All other messages fall through.
                match incoming {
                    IncomingMessage::Query {
                        id,
                        target,
                        selector,
                    } => {
                        if let Err(e) =
                            crate::scripting::handle_query(&self.core, id, target, selector)
                        {
                            log::error!("write error: {e}");
                            return iced::exit();
                        }
                        Task::none()
                    }
                    IncomingMessage::Interact {
                        id,
                        action,
                        selector,
                        payload,
                    } => {
                        if let Err(e) = crate::scripting::handle_interact(
                            &self.core, id, action, selector, payload,
                        ) {
                            log::error!("write error: {e}");
                            return iced::exit();
                        }
                        Task::none()
                    }
                    IncomingMessage::Reset { id } => {
                        // Flush any pending coalesced events before reset.
                        let _ = self.emitter.flush();

                        // Reset core and emit the response.
                        if let Err(e) = crate::scripting::handle_reset(&mut self.core, id) {
                            log::error!("write error: {e}");
                            return iced::exit();
                        }

                        // Close all open windows and clear maps.
                        let close_tasks: Vec<Task<Message>> = self
                            .windows
                            .iced_ids()
                            .map(|&iced_id| window::close(iced_id))
                            .collect();
                        self.windows.clear();

                        // Reset remaining App-level state.
                        self.image_registry =
                            plushie_widget_sdk::image_registry::ImageRegistry::new();
                        self.theme = DEFAULT_THEME;
                        self.theme_follows_system = false;
                        self.scale_factor = 1.0;
                        self.pending_tasks.clear();
                        self.animation_epoch = None;
                        self.emitter = crate::emitter::EventEmitter::new();

                        Task::batch(close_tasks)
                    }
                    IncomingMessage::TreeHash { id, name, .. } => {
                        if let Err(e) = crate::scripting::handle_tree_hash(&self.core, id, name) {
                            log::error!("write error: {e}");
                            return iced::exit();
                        }
                        Task::none()
                    }
                    IncomingMessage::Screenshot { id, name, .. } => {
                        // Capture real GPU-rendered pixels via iced
                        if let Some((_, &iced_id)) = self.windows.iter().next() {
                            window::screenshot(iced_id).map(move |shot| {
                                use sha2::{Digest, Sha256};
                                let rgba: &[u8] = &shot.rgba;
                                let mut hasher = Sha256::new();
                                hasher.update(rgba);
                                let hash = format!("{:x}", hasher.finalize());
                                let w = shot.size.width;
                                let h = shot.size.height;
                                // Inside a Task callback -- log and continue;
                                // the next synchronous write will exit cleanly.
                                if let Err(e) =
                                    emit_screenshot_response(&id, &name, &hash, w, h, rgba)
                                {
                                    log::error!("write error in screenshot: {e}");
                                }
                                Message::NoOp
                            })
                        } else {
                            // No windows open -- return empty screenshot
                            if let Err(e) = emit_screenshot_response(&id, &name, "", 0, 0, &[]) {
                                log::error!("write error: {e}");
                                return iced::exit();
                            }
                            Task::none()
                        }
                    }
                    other => {
                        if let Err(e) = self.apply(other) {
                            log::error!("write error: {e}");
                            return iced::exit();
                        }
                        let tasks: Vec<Task<Message>> = self.pending_tasks.drain(..).collect();
                        Task::batch(tasks)
                    }
                }
            }
            StdinEvent::Warning(msg) => {
                log::warn!("stdin warning: {msg}");
                Task::none()
            }
            StdinEvent::Closed => {
                log::info!("stdin closed -- exiting");
                iced::exit()
            }
        }
    }
}
