//! Declarative event subscriptions.
//!
//! Return subscriptions from [`App::subscribe`](crate::App::subscribe)
//! to receive events from the renderer. The runtime diffs the list
//! after each update and starts/stops subscriptions as needed.

use std::time::Duration;

/// Subscription op produced by the runtime when diffing the subscription
/// list returned by [`App::subscribe`](crate::App::subscribe).
///
/// Surfaced through [`TestSession::last_subscription_ops`](crate::test::TestSession::last_subscription_ops)
/// so tests can assert which subscriptions were started or stopped after
/// a model change.
pub use crate::runtime::subscriptions::SubOp;

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

impl SubscriptionKind {
    /// Canonical wire identifier for this kind. Single source of truth
    /// shared by the default tag assigned in [`Subscription::renderer`]
    /// and the wire kind reported by [`Subscription::kind`]; keeping
    /// both paths routed through this method prevents drift when a new
    /// variant is added.
    fn wire_str(&self) -> &'static str {
        match self {
            Self::Every(_) => "every",
            Self::OnKeyPress => "on_key_press",
            Self::OnKeyRelease => "on_key_release",
            Self::OnModifiersChanged => "on_modifiers_changed",
            Self::OnWindowClose => "on_window_close",
            Self::OnWindowEvent => "on_window_event",
            Self::OnWindowOpen => "on_window_open",
            Self::OnWindowResize => "on_window_resize",
            Self::OnWindowFocus => "on_window_focus",
            Self::OnWindowUnfocus => "on_window_unfocus",
            Self::OnWindowMove => "on_window_move",
            Self::OnPointerMove => "on_pointer_move",
            Self::OnPointerButton => "on_pointer_button",
            Self::OnPointerScroll => "on_pointer_scroll",
            Self::OnPointerTouch => "on_pointer_touch",
            Self::OnIme => "on_ime",
            Self::OnThemeChange => "on_theme_change",
            Self::OnAnimationFrame => "on_animation_frame",
            Self::OnFileDrop => "on_file_drop",
            Self::OnEvent => "on_event",
        }
    }
}

impl Subscription {
    /// Fire every `interval`. Delivers [`TimerEvent`](crate::event::TimerEvent).
    ///
    /// # Coalescing policy
    ///
    /// Ticks that would fire while a prior tick is already queued
    /// are coalesced: the runtime drops the extra tick rather than
    /// delivering a burst after slow `update()` cycles. That matches
    /// iced's `time::every` behaviour in direct mode and the
    /// tokio-interval spawn in wire mode. In practice: a 16 ms
    /// subscription does not deliver 100 `TimerEvent`s back-to-back
    /// when the app spends 1.6 s in a single `update`. It delivers
    /// at most one (the next scheduled tick).
    ///
    /// Apps that need catch-up or accurate per-tick timestamps
    /// should drive off the event's `timestamp` field rather than
    /// counting ticks.
    pub fn every(interval: Duration, tag: &str) -> Self {
        Self {
            kind: SubscriptionKind::Every(interval),
            tag: tag.to_string(),
            max_rate: None,
            window_id: None,
        }
    }

    fn renderer(kind: SubscriptionKind) -> Self {
        debug_assert!(
            !matches!(kind, SubscriptionKind::Every(_)),
            "SubscriptionKind::Every is constructed via `Subscription::every`, not `renderer`",
        );
        let tag = kind.wire_str().to_string();
        Self {
            kind,
            tag,
            max_rate: None,
            window_id: None,
        }
    }

    /// Delivers [`KeyEvent`](crate::event::KeyEvent) on key press.
    pub fn on_key_press() -> Self {
        Self::renderer(SubscriptionKind::OnKeyPress)
    }
    /// Delivers [`KeyEvent`](crate::event::KeyEvent) on key release.
    pub fn on_key_release() -> Self {
        Self::renderer(SubscriptionKind::OnKeyRelease)
    }
    /// Delivers [`ModifiersEvent`](crate::event::ModifiersEvent) when modifier keys change.
    pub fn on_modifiers_changed() -> Self {
        Self::renderer(SubscriptionKind::OnModifiersChanged)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window close is requested.
    pub fn on_window_close() -> Self {
        Self::renderer(SubscriptionKind::OnWindowClose)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) for all window lifecycle events.
    pub fn on_window_event() -> Self {
        Self::renderer(SubscriptionKind::OnWindowEvent)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window opens.
    pub fn on_window_open() -> Self {
        Self::renderer(SubscriptionKind::OnWindowOpen)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window is resized.
    pub fn on_window_resize() -> Self {
        Self::renderer(SubscriptionKind::OnWindowResize)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window gains focus.
    pub fn on_window_focus() -> Self {
        Self::renderer(SubscriptionKind::OnWindowFocus)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window loses focus.
    pub fn on_window_unfocus() -> Self {
        Self::renderer(SubscriptionKind::OnWindowUnfocus)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when a window is moved.
    pub fn on_window_move() -> Self {
        Self::renderer(SubscriptionKind::OnWindowMove)
    }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on pointer/mouse movement.
    pub fn on_pointer_move() -> Self {
        Self::renderer(SubscriptionKind::OnPointerMove)
    }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on pointer/mouse button press or release.
    pub fn on_pointer_button() -> Self {
        Self::renderer(SubscriptionKind::OnPointerButton)
    }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on pointer/mouse scroll.
    pub fn on_pointer_scroll() -> Self {
        Self::renderer(SubscriptionKind::OnPointerScroll)
    }
    /// Delivers [`WidgetEvent`](crate::event::WidgetEvent) on touch input.
    pub fn on_pointer_touch() -> Self {
        Self::renderer(SubscriptionKind::OnPointerTouch)
    }
    /// Delivers [`ImeEvent`](crate::event::ImeEvent) for input method editor events.
    pub fn on_ime() -> Self {
        Self::renderer(SubscriptionKind::OnIme)
    }
    /// Delivers [`SystemEvent`](crate::event::SystemEvent) when the OS theme changes.
    pub fn on_theme_change() -> Self {
        Self::renderer(SubscriptionKind::OnThemeChange)
    }
    /// Delivers [`SystemEvent`](crate::event::SystemEvent) on each animation frame.
    pub fn on_animation_frame() -> Self {
        Self::renderer(SubscriptionKind::OnAnimationFrame)
    }
    /// Delivers [`WindowEvent`](crate::event::WindowEvent) when files are dropped on a window.
    pub fn on_file_drop() -> Self {
        Self::renderer(SubscriptionKind::OnFileDrop)
    }
    /// Delivers all renderer events (catch-all subscription).
    pub fn on_event() -> Self {
        Self::renderer(SubscriptionKind::OnEvent)
    }

    /// Scope this subscription to a specific window.
    ///
    /// Use as a chained method on a single subscription. For grouping
    /// multiple subscriptions under a window, see
    /// [`Subscription::window_group`](Self::window_group).
    pub fn for_window(mut self, window_id: &str) -> Self {
        self.window_id = Some(window_id.to_string());
        if !matches!(self.kind, SubscriptionKind::Every(_)) {
            self.tag = format!("{window_id}#{}", self.kind.wire_str());
        }
        self
    }

    /// Scope a group of subscriptions to a specific window.
    ///
    /// Equivalent to calling `.for_window(window_id)` on each
    /// subscription individually. Handy when returning multiple
    /// per-window subscriptions from [`App::subscribe`](crate::App::subscribe):
    ///
    /// ```ignore
    /// fn subscribe(model: &Self) -> Vec<Subscription> {
    ///     Subscription::window_group("main", vec![
    ///         Subscription::on_key_press(),
    ///         Subscription::on_pointer_move(),
    ///     ])
    /// }
    /// ```
    pub fn window_group(
        window_id: &str,
        subs: impl IntoIterator<Item = Subscription>,
    ) -> Vec<Subscription> {
        subs.into_iter().map(|s| s.for_window(window_id)).collect()
    }

    /// Limit the maximum event rate (events per second).
    pub fn max_rate(mut self, rate: u32) -> Self {
        self.max_rate = Some(rate);
        self
    }

    /// The tag identifying this subscription.
    ///
    /// Useful for inspecting the active subscription list in tests
    /// (see [`TestSession::active_subscriptions`](crate::test::TestSession::active_subscriptions)).
    /// Window-scoped renderer subscriptions use a window-qualified tag
    /// so per-window subscriptions do not collide with global ones.
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// The wire kind string for this subscription.
    ///
    /// Stable, lowercase identifier (e.g. `"every"`, `"on_key_press"`).
    /// Exposed for use in test assertions and subscription diffing.
    pub fn kind(&self) -> &'static str {
        self.kind.wire_str()
    }

    /// Event-rate cap, if one was configured via
    /// [`max_rate`](Self::max_rate).
    pub fn max_rate_hint(&self) -> Option<u32> {
        self.max_rate
    }

    /// The window scope, if set via [`for_window`](Self::for_window).
    pub fn window_id(&self) -> Option<&str> {
        self.window_id.as_deref()
    }

    /// The timer interval for [`Subscription::every`] subscriptions,
    /// or `None` for renderer-side subscriptions.
    pub fn interval(&self) -> Option<Duration> {
        match self.kind {
            SubscriptionKind::Every(d) => Some(d),
            _ => None,
        }
    }

    /// Unique key for diffing: `(kind, tag)`.
    pub(crate) fn diff_key(&self) -> (&'static str, &str) {
        (self.kind(), &self.tag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_group_scopes_each_subscription() {
        let subs = Subscription::window_group(
            "secondary",
            vec![
                Subscription::on_key_press(),
                Subscription::on_pointer_move(),
            ],
        );
        assert_eq!(subs.len(), 2);
        for sub in &subs {
            assert_eq!(sub.window_id.as_deref(), Some("secondary"));
            assert!(sub.tag.starts_with("secondary#"));
        }
    }

    #[test]
    fn for_window_chains_on_single_subscription() {
        let sub = Subscription::on_key_press().for_window("main");
        assert_eq!(sub.window_id.as_deref(), Some("main"));
        assert_eq!(sub.tag(), "main#on_key_press");
    }

    #[test]
    fn for_window_keeps_timer_tag() {
        let sub = Subscription::every(Duration::from_millis(16), "tick").for_window("main");
        assert_eq!(sub.window_id.as_deref(), Some("main"));
        assert_eq!(sub.tag(), "tick");
    }
}
