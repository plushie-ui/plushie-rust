//! Builds the iced `Subscription` list based on which events the host has
//! registered for. Split into per-category builders (keyboard, mouse, touch,
//! IME, window, system).

use iced::{Subscription, event, system, window};

use plushie_widget_sdk::message::{KeyEventData, Message};

use crate::App;
use crate::constants::*;

impl App {
    /// Whether any host-registered subscription or any widget-scoped
    /// subscription is active for `kind`.
    ///
    /// The iced-source gating in [`renderer_subscriptions`] uses this
    /// so widget authors don't have to manually register a host
    /// subscription just to receive events. When a widget is the sole
    /// subscriber, iced still composes the underlying subscription and
    /// [`App::update`] fans the event out to the widget via
    /// [`WidgetRegistry::dispatch_widget_subscription`].
    fn has_host_or_widget_subscription(&self, kind: &str) -> bool {
        self.core.has_subscription(kind) || self.registry.has_widget_subscription(kind)
    }

    /// Build renderer subscriptions (everything except platform-specific
    /// input sources like stdin). The binary crate combines this with
    /// its own input subscription.
    pub fn renderer_subscriptions(&self) -> Subscription<Message> {
        let mut subs = vec![
            // Always listen for window close events so we can clean up maps.
            window::close_events().map(Message::WindowClosed),
        ];

        let has_on_event = self.has_host_or_widget_subscription(SUB_EVENT);

        self.keyboard_subscriptions(has_on_event, &mut subs);
        self.mouse_subscriptions(has_on_event, &mut subs);
        self.touch_subscriptions(has_on_event, &mut subs);
        self.ime_subscriptions(has_on_event, &mut subs);
        self.window_subscriptions(&mut subs);
        self.system_subscriptions(&mut subs);

        // -- Catch-all event subscription --
        // Subscribes to all keyboard, mouse, and touch events via a single
        // listener, re-using existing Message variants. Window events are
        // handled separately by on_window_event.
        //
        // To avoid duplicate event delivery when both on_event and a specific
        // subscription (e.g. on_key_press) are active, skip event categories
        // that already have a dedicated subscription listener above.
        if has_on_event {
            subs.push(event::listen_with(|evt, status, window| {
                let captured = status == iced::event::Status::Captured;
                match evt {
                    // Keyboard
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key,
                        modified_key,
                        physical_key,
                        location,
                        modifiers,
                        text,
                        repeat,
                    }) => Some(Message::KeyPressed(
                        KeyEventData {
                            key,
                            modified_key,
                            physical_key,
                            location,
                            modifiers,
                            text: text.map(|s| s.to_string()),
                            repeat,
                            captured,
                        },
                        window,
                    )),
                    iced::Event::Keyboard(iced::keyboard::Event::KeyReleased {
                        key,
                        modified_key,
                        physical_key,
                        location,
                        modifiers,
                    }) => Some(Message::KeyReleased(
                        KeyEventData {
                            key,
                            modified_key,
                            physical_key,
                            location,
                            modifiers,
                            text: None,
                            repeat: false,
                            captured,
                        },
                        window,
                    )),
                    iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                        Some(Message::ModifiersChanged(mods, window, captured))
                    }
                    // Mouse
                    iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                        Some(Message::CursorMoved(position, window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::CursorEntered) => {
                        Some(Message::CursorEntered(window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                        Some(Message::CursorLeft(window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)) => {
                        Some(Message::MouseButtonPressed(button, window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)) => {
                        Some(Message::MouseButtonReleased(button, window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                        Some(Message::WheelScrolled(delta, window, captured))
                    }
                    // Touch
                    iced::Event::Touch(iced::touch::Event::FingerPressed { id, position }) => {
                        Some(Message::FingerPressed(id, position, window, captured))
                    }
                    iced::Event::Touch(iced::touch::Event::FingerMoved { id, position }) => {
                        Some(Message::FingerMoved(id, position, window, captured))
                    }
                    iced::Event::Touch(iced::touch::Event::FingerLifted { id, position }) => {
                        Some(Message::FingerLifted(id, position, window, captured))
                    }
                    iced::Event::Touch(iced::touch::Event::FingerLost { id, position }) => {
                        Some(Message::FingerLost(id, position, window, captured))
                    }
                    // IME
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Opened) => {
                        Some(Message::ImeOpened(window, captured))
                    }
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Preedit(
                        text,
                        cursor,
                    )) => Some(Message::ImePreedit(text, cursor, window, captured)),
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Commit(text)) => {
                        Some(Message::ImeCommit(text, window, captured))
                    }
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Closed) => {
                        Some(Message::ImeClosed(window, captured))
                    }
                    // Window events handled by on_window_event
                    _ => None,
                }
            }));
        }

        Subscription::batch(subs)
    }

    fn keyboard_subscriptions(&self, has_on_event: bool, subs: &mut Vec<Subscription<Message>>) {
        // When on_event is active, its catch-all listener already covers keyboard,
        // mouse, touch, and IME events. Skip specific subscriptions to avoid
        // duplicate event delivery.
        if !has_on_event && self.has_host_or_widget_subscription(SUB_KEY_PRESS) {
            subs.push(event::listen_with(|evt, status, window| {
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key,
                    modified_key,
                    physical_key,
                    location,
                    modifiers,
                    text,
                    repeat,
                }) = evt
                {
                    Some(Message::KeyPressed(
                        KeyEventData {
                            key,
                            modified_key,
                            physical_key,
                            location,
                            modifiers,
                            text: text.map(|s| s.to_string()),
                            repeat,
                            captured: status == iced::event::Status::Captured,
                        },
                        window,
                    ))
                } else {
                    None
                }
            }));
        }

        if !has_on_event && self.has_host_or_widget_subscription(SUB_KEY_RELEASE) {
            subs.push(event::listen_with(|evt, status, window| {
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyReleased {
                    key,
                    modified_key,
                    physical_key,
                    location,
                    modifiers,
                }) = evt
                {
                    Some(Message::KeyReleased(
                        KeyEventData {
                            key,
                            modified_key,
                            physical_key,
                            location,
                            modifiers,
                            text: None,
                            repeat: false,
                            captured: status == iced::event::Status::Captured,
                        },
                        window,
                    ))
                } else {
                    None
                }
            }));
        }

        if !has_on_event && self.has_host_or_widget_subscription(SUB_MODIFIERS_CHANGED) {
            subs.push(event::listen_with(|evt, status, window| {
                if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) = evt {
                    Some(Message::ModifiersChanged(
                        mods,
                        window,
                        status == iced::event::Status::Captured,
                    ))
                } else {
                    None
                }
            }));
        }
    }

    fn mouse_subscriptions(&self, has_on_event: bool, subs: &mut Vec<Subscription<Message>>) {
        if !has_on_event && self.has_host_or_widget_subscription(SUB_POINTER_MOVE) {
            subs.push(event::listen_with(|evt, status, window| {
                let captured = status == iced::event::Status::Captured;
                match evt {
                    iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                        Some(Message::CursorMoved(position, window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::CursorEntered) => {
                        Some(Message::CursorEntered(window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                        Some(Message::CursorLeft(window, captured))
                    }
                    _ => None,
                }
            }));
        }

        if !has_on_event && self.has_host_or_widget_subscription(SUB_POINTER_BUTTON) {
            subs.push(event::listen_with(|evt, status, window| {
                let captured = status == iced::event::Status::Captured;
                match evt {
                    iced::Event::Mouse(iced::mouse::Event::ButtonPressed(button)) => {
                        Some(Message::MouseButtonPressed(button, window, captured))
                    }
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(button)) => {
                        Some(Message::MouseButtonReleased(button, window, captured))
                    }
                    _ => None,
                }
            }));
        }

        if !has_on_event && self.has_host_or_widget_subscription(SUB_POINTER_SCROLL) {
            subs.push(event::listen_with(|evt, status, window| {
                if let iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) = evt {
                    Some(Message::WheelScrolled(
                        delta,
                        window,
                        status == iced::event::Status::Captured,
                    ))
                } else {
                    None
                }
            }));
        }
    }

    fn touch_subscriptions(&self, has_on_event: bool, subs: &mut Vec<Subscription<Message>>) {
        if !has_on_event && self.has_host_or_widget_subscription(SUB_POINTER_TOUCH) {
            subs.push(event::listen_with(|evt, status, window| {
                let captured = status == iced::event::Status::Captured;
                match evt {
                    iced::Event::Touch(iced::touch::Event::FingerPressed { id, position }) => {
                        Some(Message::FingerPressed(id, position, window, captured))
                    }
                    iced::Event::Touch(iced::touch::Event::FingerMoved { id, position }) => {
                        Some(Message::FingerMoved(id, position, window, captured))
                    }
                    iced::Event::Touch(iced::touch::Event::FingerLifted { id, position }) => {
                        Some(Message::FingerLifted(id, position, window, captured))
                    }
                    iced::Event::Touch(iced::touch::Event::FingerLost { id, position }) => {
                        Some(Message::FingerLost(id, position, window, captured))
                    }
                    _ => None,
                }
            }));
        }
    }

    fn ime_subscriptions(&self, has_on_event: bool, subs: &mut Vec<Subscription<Message>>) {
        if !has_on_event && self.has_host_or_widget_subscription(SUB_IME) {
            subs.push(event::listen_with(|evt, status, window| {
                let captured = status == iced::event::Status::Captured;
                match evt {
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Opened) => {
                        Some(Message::ImeOpened(window, captured))
                    }
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Preedit(
                        text,
                        cursor,
                    )) => Some(Message::ImePreedit(text, cursor, window, captured)),
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Commit(text)) => {
                        Some(Message::ImeCommit(text, window, captured))
                    }
                    iced::Event::InputMethod(iced::advanced::input_method::Event::Closed) => {
                        Some(Message::ImeClosed(window, captured))
                    }
                    _ => None,
                }
            }));
        }
    }

    fn window_subscriptions(&self, subs: &mut Vec<Subscription<Message>>) {
        if self.has_any_subscription(&[
            SUB_WINDOW_EVENT,
            SUB_WINDOW_OPEN,
            SUB_WINDOW_MOVE,
            SUB_WINDOW_RESIZE,
            SUB_WINDOW_FOCUS,
            SUB_WINDOW_UNFOCUS,
            SUB_FILE_DROP,
        ]) {
            subs.push(window::events().map(|(id, evt)| Message::WindowEvent(id, evt)));
        }

        if self.has_host_or_widget_subscription(SUB_WINDOW_CLOSE) {
            subs.push(window::close_requests().map(Message::WindowCloseRequested));
        }

        // -- Animation frame subscription --
        // Active when the SDK subscribes to animation_frame, when any
        // widget has declared an animation-frame subscription, or when
        // the renderer has active transitions/springs (zero-traffic
        // animation).
        if self.has_host_or_widget_subscription(SUB_ANIMATION_FRAME)
            || self.transition_manager.has_active()
        {
            subs.push(window::frames().map(Message::AnimationFrame));
        }
    }

    fn system_subscriptions(&self, subs: &mut Vec<Subscription<Message>>) {
        // Track system theme changes when theme follows system OR when subscribed
        if self.theme_follows_system || self.has_host_or_widget_subscription(SUB_THEME_CHANGE) {
            subs.push(system::theme_changes().map(Message::ThemeChanged));
        }
    }

    /// Check if any of the given subscription keys are registered on
    /// the host side or by any widget.
    fn has_any_subscription(&self, keys: &[&str]) -> bool {
        keys.iter().any(|k| self.has_host_or_widget_subscription(k))
    }
}
