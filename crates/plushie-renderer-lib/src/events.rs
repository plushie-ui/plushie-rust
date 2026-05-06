//! Subscription event handlers for keyboard, mouse, touch, IME, window
//! lifecycle, and pane grid events. Each handler checks whether the host
//! subscribed to the event type before emitting it.

use std::io;

use iced::{Point, Task, window};

use plushie_widget_sdk::protocol::OutgoingEvent;
use plushie_widget_sdk::protocol::OutgoingEventKeyExt;
use plushie_widget_sdk::runtime::{
    KeyEventData, Message, serialize_modifiers, serialize_mouse_button, serialize_scroll_delta,
};

use crate::App;
use crate::constants::*;

/// Convert a file path to a UTF-8 string, using lossy conversion if
/// the path contains non-UTF-8 bytes (rare on modern systems, but
/// possible on Linux with legacy filenames).
fn path_to_string(path: std::path::PathBuf) -> String {
    match path.to_str() {
        Some(s) => s.to_string(),
        None => {
            log::warn!(
                "file path contains non-UTF-8 bytes, using lossy conversion: {}",
                path.display()
            );
            path.to_string_lossy().into_owned()
        }
    }
}

impl App {
    /// Resolve an iced window::Id to a string window_id. Returns `None`
    /// for unresolved windows (e.g., late events after a window close).
    /// Callers should skip event emission when this returns `None`.
    fn resolve_window_id(&self, iced_id: &window::Id) -> Option<&str> {
        let resolved = self.windows.get_window_id(iced_id);
        if resolved.is_none() {
            log::debug!("event for unknown iced window {:?}, suppressing", iced_id);
        }
        resolved
    }

    pub fn handle_key_pressed(&self, data: KeyEventData, iced_id: window::Id) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_KEY_PRESS, Some(window_id), data.captured, |tag| {
            OutgoingEvent::key_press(tag, &data)
        })
    }

    pub fn handle_key_released(&self, data: KeyEventData, iced_id: window::Id) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_KEY_RELEASE, Some(window_id), data.captured, |tag| {
            OutgoingEvent::key_release(tag, &data)
        })
    }

    pub fn handle_modifiers_changed(
        &mut self,
        mods: iced::keyboard::Modifiers,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        self.current_modifiers = mods;
        let Some(window_id) = self.windows.get_window_id(&iced_id) else {
            log::debug!("event for unknown iced window {:?}, suppressing", iced_id);
            return Task::none();
        };
        crate::app::coalesce_subscription_into(
            &self.core,
            &mut self.emitter,
            SUB_MODIFIERS_CHANGED,
            Some(window_id),
            captured,
            |tag| OutgoingEvent::modifiers_changed(tag, serialize_modifiers(mods)),
        )
    }

    pub fn handle_cursor_moved(
        &mut self,
        pos: Point,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.windows.get_window_id(&iced_id) else {
            log::debug!("event for unknown iced window {:?}, suppressing", iced_id);
            return Task::none();
        };
        crate::app::coalesce_subscription_into(
            &self.core,
            &mut self.emitter,
            SUB_POINTER_MOVE,
            Some(window_id),
            captured,
            |tag| OutgoingEvent::cursor_moved(tag, pos.x, pos.y),
        )
    }

    pub fn handle_cursor_entered(&self, iced_id: window::Id, captured: bool) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_MOVE, Some(window_id), captured, |tag| {
            OutgoingEvent::cursor_entered(tag)
        })
    }

    pub fn handle_cursor_left(&self, iced_id: window::Id, captured: bool) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_MOVE, Some(window_id), captured, |tag| {
            OutgoingEvent::cursor_left(tag)
        })
    }

    pub fn handle_mouse_button_pressed(
        &self,
        button: iced::mouse::Button,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_BUTTON, Some(window_id), captured, |tag| {
            OutgoingEvent::button_pressed(tag, serialize_mouse_button(&button))
        })
    }

    pub fn handle_mouse_button_released(
        &self,
        button: iced::mouse::Button,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_BUTTON, Some(window_id), captured, |tag| {
            OutgoingEvent::button_released(tag, serialize_mouse_button(&button))
        })
    }

    pub fn handle_wheel_scrolled(
        &mut self,
        delta: iced::mouse::ScrollDelta,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.windows.get_window_id(&iced_id) else {
            log::debug!("event for unknown iced window {:?}, suppressing", iced_id);
            return Task::none();
        };
        crate::app::coalesce_subscription_into(
            &self.core,
            &mut self.emitter,
            SUB_POINTER_SCROLL,
            Some(window_id),
            captured,
            |tag| {
                let (dx, dy, unit) = serialize_scroll_delta(&delta);
                OutgoingEvent::wheel_scrolled(tag, dx, dy, unit)
            },
        )
    }

    pub fn handle_finger_pressed(
        &self,
        finger: iced::touch::Finger,
        pos: Point,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_TOUCH, Some(window_id), captured, |tag| {
            OutgoingEvent::finger_pressed(tag, finger.0, pos.x, pos.y)
        })
    }

    pub fn handle_finger_moved(
        &mut self,
        finger: iced::touch::Finger,
        pos: Point,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.windows.get_window_id(&iced_id) else {
            log::debug!("event for unknown iced window {:?}, suppressing", iced_id);
            return Task::none();
        };
        crate::app::coalesce_subscription_into(
            &self.core,
            &mut self.emitter,
            SUB_POINTER_TOUCH,
            Some(window_id),
            captured,
            |tag| OutgoingEvent::finger_moved(tag, finger.0, pos.x, pos.y),
        )
    }

    pub fn handle_finger_lifted(
        &self,
        finger: iced::touch::Finger,
        pos: Point,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_TOUCH, Some(window_id), captured, |tag| {
            OutgoingEvent::finger_lifted(tag, finger.0, pos.x, pos.y)
        })
    }

    pub fn handle_finger_lost(
        &self,
        finger: iced::touch::Finger,
        pos: Point,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_POINTER_TOUCH, Some(window_id), captured, |tag| {
            OutgoingEvent::finger_lost(tag, finger.0, pos.x, pos.y)
        })
    }

    // IME (Input Method Editor) events for CJK and complex input.
    //
    // Platform support: Windows (Microsoft IME, Google Japanese, etc.),
    // macOS (built-in input methods), Linux/X11 (XIM/IBus), Linux/Wayland
    // (text-input-v3 protocol; compositor support varies). The preedit
    // cursor range may be None on some older X11 IME implementations.
    pub fn handle_ime_opened(&self, iced_id: window::Id, captured: bool) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_IME, Some(window_id), captured, |tag| {
            OutgoingEvent::ime_opened(tag)
        })
    }

    pub fn handle_ime_preedit(
        &self,
        text: String,
        cursor: Option<std::ops::Range<usize>>,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_IME, Some(window_id), captured, |tag| {
            OutgoingEvent::ime_preedit(tag, text.clone(), cursor.clone())
        })
    }

    pub fn handle_ime_commit(
        &self,
        text: String,
        iced_id: window::Id,
        captured: bool,
    ) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_IME, Some(window_id), captured, |tag| {
            OutgoingEvent::ime_commit(tag, text.clone())
        })
    }

    pub fn handle_ime_closed(&self, iced_id: window::Id, captured: bool) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id) else {
            return Task::none();
        };
        self.emit_subscription_for_window(SUB_IME, Some(window_id), captured, |tag| {
            OutgoingEvent::ime_closed(tag)
        })
    }

    /// Emit a window event to all matching entries across the catch-all
    /// window subscription and the event-specific subscription (if registered),
    /// filtered by window_id scope.
    fn emit_window_event(
        &self,
        specific_key: Option<&str>,
        event_fn: impl Fn(String, String) -> OutgoingEvent,
        window_id: String,
    ) -> io::Result<()> {
        let wid = Some(window_id.as_str());
        // Emit for catch-all SUB_WINDOW_EVENT entries
        for entry in self.core.matching_entries(SUB_WINDOW_EVENT, wid) {
            self.emitter
                .emit_event(event_fn(entry.tag.clone(), window_id.clone()))?;
        }
        // Emit for specific key entries (e.g. SUB_WINDOW_MOVE)
        if let Some(key) = specific_key {
            for entry in self.core.matching_entries(key, wid) {
                self.emitter
                    .emit_event(event_fn(entry.tag.clone(), window_id.clone()))?;
            }
        }
        Ok(())
    }

    pub fn handle_window_event(&self, iced_id: window::Id, evt: window::Event) -> Task<Message> {
        let Some(window_id) = self.resolve_window_id(&iced_id).map(str::to_string) else {
            return Task::none();
        };
        // Helper closure: emit and propagate errors uniformly.
        let result: io::Result<()> = (|| {
            match evt {
                window::Event::Opened {
                    position,
                    size,
                    scale_factor,
                } => {
                    let wid = Some(window_id.as_str());
                    let pos = position.map(|p| (p.x, p.y));
                    for entry in self.core.matching_entries(SUB_WINDOW_EVENT, wid) {
                        self.emitter.emit_event(OutgoingEvent::window_opened(
                            entry.tag.clone(),
                            window_id.clone(),
                            pos,
                            size.width,
                            size.height,
                            scale_factor,
                        ))?;
                    }
                    for entry in self.core.matching_entries(SUB_WINDOW_OPEN, wid) {
                        self.emitter.emit_event(OutgoingEvent::window_opened(
                            entry.tag.clone(),
                            window_id.clone(),
                            pos,
                            size.width,
                            size.height,
                            scale_factor,
                        ))?;
                    }
                }
                window::Event::Closed => {
                    let wid = Some(window_id.as_str());
                    for entry in self.core.matching_entries(SUB_WINDOW_EVENT, wid) {
                        self.emitter.emit_event(OutgoingEvent::window_closed(
                            entry.tag.clone(),
                            window_id.clone(),
                        ))?;
                    }
                }
                window::Event::Moved(point) => {
                    self.emit_window_event(
                        Some(SUB_WINDOW_MOVE),
                        |tag, jid| OutgoingEvent::window_moved(tag, jid, point.x, point.y),
                        window_id,
                    )?;
                }
                window::Event::Resized(size) => {
                    self.emit_window_event(
                        Some(SUB_WINDOW_RESIZE),
                        |tag, jid| OutgoingEvent::window_resized(tag, jid, size.width, size.height),
                        window_id,
                    )?;
                }
                window::Event::Rescaled(factor) => {
                    let wid = Some(window_id.as_str());
                    for entry in self.core.matching_entries(SUB_WINDOW_EVENT, wid) {
                        self.emitter.emit_event(OutgoingEvent::window_rescaled(
                            entry.tag.clone(),
                            window_id.clone(),
                            factor,
                        ))?;
                    }
                }
                window::Event::Focused => {
                    self.emit_window_event(
                        Some(SUB_WINDOW_FOCUS),
                        OutgoingEvent::window_focused,
                        window_id,
                    )?;
                }
                window::Event::Unfocused => {
                    self.emit_window_event(
                        Some(SUB_WINDOW_UNFOCUS),
                        OutgoingEvent::window_unfocused,
                        window_id,
                    )?;
                }
                window::Event::FileHovered(path) => {
                    let wid = Some(window_id.as_str());
                    let path_str = path_to_string(path);
                    for entry in self.core.matching_entries(SUB_FILE_DROP, wid) {
                        self.emitter.emit_event(OutgoingEvent::file_hovered(
                            entry.tag.clone(),
                            window_id.clone(),
                            path_str.clone(),
                        ))?;
                    }
                }
                window::Event::FileDropped(path) => {
                    let wid = Some(window_id.as_str());
                    let path_str = path_to_string(path);
                    for entry in self.core.matching_entries(SUB_FILE_DROP, wid) {
                        self.emitter.emit_event(OutgoingEvent::file_dropped(
                            entry.tag.clone(),
                            window_id.clone(),
                            path_str.clone(),
                        ))?;
                    }
                }
                window::Event::FilesHoveredLeft => {
                    let wid = Some(window_id.as_str());
                    for entry in self.core.matching_entries(SUB_FILE_DROP, wid) {
                        self.emitter.emit_event(OutgoingEvent::files_hovered_left(
                            entry.tag.clone(),
                            window_id.clone(),
                        ))?;
                    }
                }
                window::Event::CloseRequested => {
                    // Handled via close_requests() subscription separately.
                }
                window::Event::RedrawRequested(_) => {
                    // Handled via animation_frame subscription separately.
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            log::error!("write error: {e}");
            return iced::exit();
        }
        Task::none()
    }
}
