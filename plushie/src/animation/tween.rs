//! SDK-side frame-by-frame animation.
//!
//! [`Tween`] provides stateful interpolation driven by the app's
//! update loop. Use for animations that need to be tied to model
//! state (e.g., canvas-based custom rendering).
//!
//! For declarative property animations that the renderer handles
//! autonomously, use [`Transition`](super::Transition) or
//! [`Spring`](super::Spring) instead.

use super::Easing;

/// A stateful interpolator for frame-by-frame animation.
///
/// ```
/// use plushie::animation::{Tween, Easing};
///
/// let mut tween = Tween::new(0.0, 100.0, 500)
///     .easing(Easing::EaseOutCubic);
///
/// tween.start(0);
/// tween.advance(250);
/// assert!(tween.value().is_some());
/// assert!(!tween.finished());
///
/// tween.advance(500);
/// assert!(tween.finished());
/// ```
#[derive(Debug, Clone)]
pub struct Tween {
    pub from: f64,
    pub to: f64,
    pub duration_ms: u64,
    pub easing: Easing,
    pub delay_ms: u64,
    started_at: Option<u64>,
    value: Option<f64>,
    finished: bool,
}

impl Tween {
    /// Create a tween from `from` to `to` over `duration_ms` milliseconds.
    pub fn new(from: f64, to: f64, duration_ms: u64) -> Self {
        Self {
            from,
            to,
            duration_ms,
            easing: Easing::EaseInOut,
            delay_ms: 0,
            started_at: None,
            value: None,
            finished: false,
        }
    }

    pub fn easing(mut self, e: Easing) -> Self { self.easing = e; self }
    pub fn delay(mut self, ms: u64) -> Self { self.delay_ms = ms; self }

    /// Start the tween at the given timestamp (monotonic milliseconds).
    pub fn start(&mut self, timestamp: u64) {
        self.started_at = Some(timestamp);
        self.value = Some(self.from);
        self.finished = false;
    }

    /// Start only if not already started.
    pub fn start_once(&mut self, timestamp: u64) {
        if self.started_at.is_none() {
            self.start(timestamp);
        }
    }

    /// Advance the tween to the given timestamp.
    pub fn advance(&mut self, timestamp: u64) {
        let Some(started) = self.started_at else { return };
        if self.finished { return; }

        let elapsed = timestamp.saturating_sub(started);
        if elapsed < self.delay_ms {
            self.value = Some(self.from);
            return;
        }

        let active_elapsed = elapsed - self.delay_ms;
        if active_elapsed >= self.duration_ms {
            self.value = Some(self.to);
            self.finished = true;
            return;
        }

        let t = active_elapsed as f64 / self.duration_ms as f64;
        // For now, linear interpolation. Full easing function
        // dispatch will be implemented when easing.rs gets
        // interpolation functions.
        let eased_t = t; // TODO: apply self.easing
        self.value = Some(self.from + (self.to - self.from) * eased_t);
    }

    /// The current interpolated value, or `None` if not started.
    pub fn value(&self) -> Option<f64> {
        self.value
    }

    /// Whether the tween has reached its end value.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Whether the tween has been started and is not yet finished.
    pub fn running(&self) -> bool {
        self.started_at.is_some() && !self.finished
    }

    /// Redirect the tween to a new target from the current value.
    pub fn redirect(&mut self, to: f64, timestamp: u64) {
        let current = self.value.unwrap_or(self.from);
        self.from = current;
        self.to = to;
        self.start(timestamp);
    }
}
