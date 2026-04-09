//! Declarative event subscriptions.
//!
//! Return subscriptions from [`App::subscribe`](crate::App::subscribe)
//! to receive events from the renderer. The runtime diffs the list
//! after each update and starts/stops subscriptions as needed.

use std::time::Duration;

/// A subscription to an event source.
///
/// Construct via the named constructors (`every`, `on_key_press`, etc.).
/// Chain `.for_window()` to scope to a specific window, and
/// `.max_rate()` to limit event frequency.
///
/// ```ignore
/// fn subscribe(model: &Self) -> Vec<Subscription> {
///     vec![
///         Subscription::every(Duration::from_millis(16), "tick"),
///         Subscription::on_key_press().for_window("main"),
///         Subscription::on_pointer_move().max_rate(60),
///     ]
/// }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields read by runners during subscription management
pub struct Subscription {
    pub(crate) kind: SubscriptionKind,
    pub(crate) tag: String,
    pub(crate) max_rate: Option<u32>,
    pub(crate) window_id: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Variants used by subscription diffing in runners
pub(crate) enum SubscriptionKind {
    Every(Duration),
    OnKeyPress,
    OnKeyRelease,
    OnModifiersChanged,
    OnWindowClose,
    OnWindowEvent,
    OnWindowOpen,
    OnWindowResize,
    OnWindowFocus,
    OnWindowUnfocus,
    OnWindowMove,
    OnPointerMove,
    OnPointerButton,
    OnPointerScroll,
    OnPointerTouch,
    OnIme,
    OnThemeChange,
    OnAnimationFrame,
    OnFileDrop,
    OnEvent,
}

impl Subscription {
    /// Fire every `interval`. Delivers [`TimerEvent`](crate::event::TimerEvent).
    pub fn every(interval: Duration, tag: &str) -> Self {
        Self {
            kind: SubscriptionKind::Every(interval),
            tag: tag.to_string(),
            max_rate: None,
            window_id: None,
        }
    }

    fn renderer(kind: SubscriptionKind) -> Self {
        let tag = match &kind {
            SubscriptionKind::Every(_) => unreachable!(),
            SubscriptionKind::OnKeyPress => "on_key_press",
            SubscriptionKind::OnKeyRelease => "on_key_release",
            SubscriptionKind::OnModifiersChanged => "on_modifiers_changed",
            SubscriptionKind::OnWindowClose => "on_window_close",
            SubscriptionKind::OnWindowEvent => "on_window_event",
            SubscriptionKind::OnWindowOpen => "on_window_open",
            SubscriptionKind::OnWindowResize => "on_window_resize",
            SubscriptionKind::OnWindowFocus => "on_window_focus",
            SubscriptionKind::OnWindowUnfocus => "on_window_unfocus",
            SubscriptionKind::OnWindowMove => "on_window_move",
            SubscriptionKind::OnPointerMove => "on_pointer_move",
            SubscriptionKind::OnPointerButton => "on_pointer_button",
            SubscriptionKind::OnPointerScroll => "on_pointer_scroll",
            SubscriptionKind::OnPointerTouch => "on_pointer_touch",
            SubscriptionKind::OnIme => "on_ime",
            SubscriptionKind::OnThemeChange => "on_theme_change",
            SubscriptionKind::OnAnimationFrame => "on_animation_frame",
            SubscriptionKind::OnFileDrop => "on_file_drop",
            SubscriptionKind::OnEvent => "on_event",
        };
        Self {
            kind,
            tag: tag.to_string(),
            max_rate: None,
            window_id: None,
        }
    }

    /// Delivers [`KeyEvent`](crate::event::KeyEvent) on key press.
    pub fn on_key_press() -> Self { Self::renderer(SubscriptionKind::OnKeyPress) }
    /// Delivers [`KeyEvent`](crate::event::KeyEvent) on key release.
    pub fn on_key_release() -> Self { Self::renderer(SubscriptionKind::OnKeyRelease) }
    /// Delivers [`ModifiersEvent`](crate::event::ModifiersEvent) when modifier keys change.
    pub fn on_modifiers_changed() -> Self { Self::renderer(SubscriptionKind::OnModifiersChanged) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window close is requested.
    pub fn on_window_close() -> Self { Self::renderer(SubscriptionKind::OnWindowClose) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) for all window lifecycle events.
    pub fn on_window_event() -> Self { Self::renderer(SubscriptionKind::OnWindowEvent) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window opens.
    pub fn on_window_open() -> Self { Self::renderer(SubscriptionKind::OnWindowOpen) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window is resized.
    pub fn on_window_resize() -> Self { Self::renderer(SubscriptionKind::OnWindowResize) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window gains focus.
    pub fn on_window_focus() -> Self { Self::renderer(SubscriptionKind::OnWindowFocus) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window loses focus.
    pub fn on_window_unfocus() -> Self { Self::renderer(SubscriptionKind::OnWindowUnfocus) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window is moved.
    pub fn on_window_move() -> Self { Self::renderer(SubscriptionKind::OnWindowMove) }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on pointer/mouse movement.
    pub fn on_pointer_move() -> Self { Self::renderer(SubscriptionKind::OnPointerMove) }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on pointer/mouse button press or release.
    pub fn on_pointer_button() -> Self { Self::renderer(SubscriptionKind::OnPointerButton) }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on pointer/mouse scroll.
    pub fn on_pointer_scroll() -> Self { Self::renderer(SubscriptionKind::OnPointerScroll) }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on touch input.
    pub fn on_pointer_touch() -> Self { Self::renderer(SubscriptionKind::OnPointerTouch) }
    /// Delivers [`ImeEvent`](crate::event::ImeEvent) for input method editor events.
    pub fn on_ime() -> Self { Self::renderer(SubscriptionKind::OnIme) }
    /// Delivers [`SystemEvent`](crate::event::SystemEvent) when the OS theme changes.
    pub fn on_theme_change() -> Self { Self::renderer(SubscriptionKind::OnThemeChange) }
    /// Delivers [`SystemEvent`](crate::event::SystemEvent) on each animation frame.
    pub fn on_animation_frame() -> Self { Self::renderer(SubscriptionKind::OnAnimationFrame) }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when files are dropped on a window.
    pub fn on_file_drop() -> Self { Self::renderer(SubscriptionKind::OnFileDrop) }
    /// Delivers all renderer events (catch-all subscription).
    pub fn on_event() -> Self { Self::renderer(SubscriptionKind::OnEvent) }

    /// Scope this subscription to a specific window.
    pub fn for_window(mut self, window_id: &str) -> Self {
        self.window_id = Some(window_id.to_string());
        self
    }

    /// Limit the maximum event rate (events per second).
    pub fn max_rate(mut self, rate: u32) -> Self {
        self.max_rate = Some(rate);
        self
    }
}
