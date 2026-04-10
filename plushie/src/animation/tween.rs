//! SDK-side frame-by-frame animation.
//!
//! [`Tween`] provides stateful interpolation driven by the app's
//! update loop. Use for animations that need to be tied to model
//! state (e.g., canvas-based custom rendering).
//!
//! For declarative property animations that the renderer handles
//! autonomously, use [`Transition`](super::Transition) or
//! [`Spring`](super::Spring) instead.

use super::{Easing, Repeat};

use std::f64::consts::PI;

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
    pub repeat: Option<Repeat>,
    pub auto_reverse: bool,
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
            repeat: None,
            auto_reverse: false,
            started_at: None,
            value: None,
            finished: false,
        }
    }

    pub fn easing(mut self, e: Easing) -> Self { self.easing = e; self }
    pub fn delay(mut self, ms: u64) -> Self { self.delay_ms = ms; self }
    pub fn repeat(mut self, n: u32) -> Self { self.repeat = Some(Repeat::Times(n)); self }
    pub fn repeat_forever(mut self) -> Self { self.repeat = Some(Repeat::Forever); self }
    pub fn auto_reverse(mut self, v: bool) -> Self { self.auto_reverse = v; self }

    /// Create a looping tween (repeat forever, auto-reverse).
    pub fn looping(from: f64, to: f64, duration_ms: u64) -> Self {
        Self::new(from, to, duration_ms)
            .repeat_forever()
            .auto_reverse(true)
    }

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
            self.handle_cycle_end();
            return;
        }

        let t = active_elapsed as f64 / self.duration_ms as f64;
        let eased_t = apply_easing(t, &self.easing);
        self.value = Some(self.from + (self.to - self.from) * eased_t);
    }

    /// Handle the end of one animation cycle: repeat, reverse, or finish.
    fn handle_cycle_end(&mut self) {
        match self.repeat {
            None => {
                self.value = Some(self.to);
                self.finished = true;
            }
            Some(Repeat::Forever) => {
                self.restart_cycle();
            }
            Some(Repeat::Times(n)) if n > 1 => {
                self.repeat = Some(Repeat::Times(n - 1));
                self.restart_cycle();
            }
            Some(Repeat::Times(_)) => {
                self.value = Some(self.to);
                self.finished = true;
            }
        }
    }

    /// Restart the cycle, swapping from/to if auto_reverse is set.
    fn restart_cycle(&mut self) {
        if let Some(started) = self.started_at {
            self.started_at = Some(started + self.duration_ms);
        }
        if self.auto_reverse {
            std::mem::swap(&mut self.from, &mut self.to);
        } else {
            self.value = Some(self.from);
        }
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

// ---------------------------------------------------------------------------
// Easing function dispatch
// ---------------------------------------------------------------------------

/// Maps an Easing variant to the actual easing curve function.
///
/// All curves match the standard CSS/easings.net definitions and
/// are consistent with `Plushie.Animation.Easing` on the Elixir side.
fn apply_easing(t: f64, easing: &Easing) -> f64 {
    // Back overshoot constants (standard CSS values)
    const C1: f64 = 1.70158;
    const C2: f64 = C1 * 1.525;
    const C3: f64 = C1 + 1.0;

    // Elastic constants
    const C4: f64 = 2.0 * PI / 3.0;
    const C5: f64 = 2.0 * PI / 4.5;

    // Bounce constants
    const N1: f64 = 7.5625;
    const D1: f64 = 2.75;

    match easing {
        // Linear
        Easing::Linear => t,

        // Sine
        Easing::EaseIn => 1.0 - (t * PI / 2.0).cos(),
        Easing::EaseOut => (t * PI / 2.0).sin(),
        Easing::EaseInOut => -(PI * t).cos().mul_add(0.5, -0.5),

        // Quadratic
        Easing::EaseInQuad => t * t,
        Easing::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        Easing::EaseInOutQuad => {
            if t < 0.5 { 2.0 * t * t }
            else { 1.0 - (-2.0 * t + 2.0).powi(2) / 2.0 }
        }

        // Cubic
        Easing::EaseInCubic => t * t * t,
        Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
        Easing::EaseInOutCubic => {
            if t < 0.5 { 4.0 * t * t * t }
            else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
        }

        // Quartic
        Easing::EaseInQuart => t * t * t * t,
        Easing::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
        Easing::EaseInOutQuart => {
            if t < 0.5 { 8.0 * t * t * t * t }
            else { 1.0 - (-2.0 * t + 2.0).powi(4) / 2.0 }
        }

        // Quintic
        Easing::EaseInQuint => t * t * t * t * t,
        Easing::EaseOutQuint => 1.0 - (1.0 - t).powi(5),
        Easing::EaseInOutQuint => {
            if t < 0.5 { 16.0 * t * t * t * t * t }
            else { 1.0 - (-2.0 * t + 2.0).powi(5) / 2.0 }
        }

        // Exponential
        Easing::EaseInExpo => {
            if t == 0.0 { 0.0 }
            else { 2.0_f64.powf(10.0 * t - 10.0) }
        }
        Easing::EaseOutExpo => {
            if t == 1.0 { 1.0 }
            else { 1.0 - 2.0_f64.powf(-10.0 * t) }
        }
        Easing::EaseInOutExpo => {
            if t == 0.0 { 0.0 }
            else if t == 1.0 { 1.0 }
            else if t < 0.5 { 2.0_f64.powf(20.0 * t - 10.0) / 2.0 }
            else { (2.0 - 2.0_f64.powf(-20.0 * t + 10.0)) / 2.0 }
        }

        // Circular
        Easing::EaseInCirc => 1.0 - (1.0 - t * t).sqrt(),
        Easing::EaseOutCirc => (1.0 - (t - 1.0) * (t - 1.0)).sqrt(),
        Easing::EaseInOutCirc => {
            if t < 0.5 { (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0 }
            else { (1.0 + (1.0 - (-2.0 * t + 2.0).powi(2)).sqrt()) / 2.0 }
        }

        // Back (overshoot)
        Easing::EaseInBack => C3 * t * t * t - C1 * t * t,
        Easing::EaseOutBack => {
            1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
        }
        Easing::EaseInOutBack => {
            if t < 0.5 {
                (2.0 * t).powi(2) * ((C2 + 1.0) * 2.0 * t - C2) / 2.0
            } else {
                ((2.0 * t - 2.0).powi(2) * ((C2 + 1.0) * (2.0 * t - 2.0) + C2) + 2.0) / 2.0
            }
        }

        // Elastic (oscillating overshoot)
        Easing::EaseInElastic => {
            if t == 0.0 { 0.0 }
            else if t == 1.0 { 1.0 }
            else { -(2.0_f64.powf(10.0 * t - 10.0) * ((10.0 * t - 10.75) * C4).sin()) }
        }
        Easing::EaseOutElastic => {
            if t == 0.0 { 0.0 }
            else if t == 1.0 { 1.0 }
            else { 2.0_f64.powf(-10.0 * t) * ((10.0 * t - 0.75) * C4).sin() + 1.0 }
        }
        Easing::EaseInOutElastic => {
            if t == 0.0 { 0.0 }
            else if t == 1.0 { 1.0 }
            else if t < 0.5 {
                -(2.0_f64.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * C5).sin()) / 2.0
            } else {
                2.0_f64.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * C5).sin() / 2.0 + 1.0
            }
        }

        // Bounce
        Easing::EaseInBounce => 1.0 - ease_out_bounce(1.0 - t, N1, D1),
        Easing::EaseOutBounce => ease_out_bounce(t, N1, D1),
        Easing::EaseInOutBounce => {
            if t < 0.5 {
                (1.0 - ease_out_bounce(1.0 - 2.0 * t, N1, D1)) / 2.0
            } else {
                (1.0 + ease_out_bounce(2.0 * t - 1.0, N1, D1)) / 2.0
            }
        }

        // Cubic bezier (Newton-Raphson solver)
        Easing::CubicBezier(x1, y1, x2, y2) => {
            cubic_bezier(t, *x1 as f64, *y1 as f64, *x2 as f64, *y2 as f64)
        }
    }
}

/// Bounce ease-out helper (used by all three bounce variants).
fn ease_out_bounce(t: f64, n1: f64, d1: f64) -> f64 {
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t2 = t - 1.5 / d1;
        n1 * t2 * t2 + 0.75
    } else if t < 2.5 / d1 {
        let t2 = t - 2.25 / d1;
        n1 * t2 * t2 + 0.9375
    } else {
        let t2 = t - 2.625 / d1;
        n1 * t2 * t2 + 0.984375
    }
}

/// Evaluate a cubic bezier easing curve using Newton-Raphson iteration.
fn cubic_bezier(t: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    if t <= 0.0 { return 0.0; }
    if t >= 1.0 { return 1.0; }

    // Solve for the bezier parameter `s` where bezier_x(s) == t
    let s = newton_raphson_solve(t, x1, x2, t, 8);
    bezier_eval(s, y1, y2)
}

/// Evaluates the cubic bezier polynomial for one axis.
/// B(s) = 3(1-s)^2*s*p1 + 3(1-s)*s^2*p2 + s^3
fn bezier_eval(s: f64, p1: f64, p2: f64) -> f64 {
    let s2 = s * s;
    let s3 = s2 * s;
    3.0 * (1.0 - s) * (1.0 - s) * s * p1 + 3.0 * (1.0 - s) * s2 * p2 + s3
}

/// Derivative of the bezier polynomial for one axis.
fn bezier_derivative(s: f64, p1: f64, p2: f64) -> f64 {
    3.0 * (1.0 - s) * (1.0 - s) * p1
        + 6.0 * (1.0 - s) * s * (p2 - p1)
        + 3.0 * s * s * (1.0 - p2)
}

fn newton_raphson_solve(target_x: f64, x1: f64, x2: f64, mut guess: f64, max_iter: u32) -> f64 {
    for _ in 0..max_iter {
        let x = bezier_eval(guess, x1, x2);
        let dx = bezier_derivative(guess, x1, x2);

        if (x - target_x).abs() < 1.0e-7 || dx.abs() < 1.0e-7 {
            break;
        }
        guess = (guess - (x - target_x) / dx).clamp(0.0, 1.0);
    }
    guess
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_easing_is_identity() {
        for i in 0..=10 {
            let t = i as f64 / 10.0;
            let result = apply_easing(t, &Easing::Linear);
            assert!((result - t).abs() < 1.0e-10);
        }
    }

    #[test]
    fn ease_in_out_boundaries() {
        assert!((apply_easing(0.0, &Easing::EaseInOut)).abs() < 1.0e-10);
        assert!((apply_easing(1.0, &Easing::EaseInOut) - 1.0).abs() < 1.0e-10);
    }

    #[test]
    fn all_easings_reach_endpoints() {
        let easings = [
            Easing::Linear,
            Easing::EaseIn, Easing::EaseOut, Easing::EaseInOut,
            Easing::EaseInQuad, Easing::EaseOutQuad, Easing::EaseInOutQuad,
            Easing::EaseInCubic, Easing::EaseOutCubic, Easing::EaseInOutCubic,
            Easing::EaseInQuart, Easing::EaseOutQuart, Easing::EaseInOutQuart,
            Easing::EaseInQuint, Easing::EaseOutQuint, Easing::EaseInOutQuint,
            Easing::EaseInExpo, Easing::EaseOutExpo, Easing::EaseInOutExpo,
            Easing::EaseInCirc, Easing::EaseOutCirc, Easing::EaseInOutCirc,
            Easing::EaseInBounce, Easing::EaseOutBounce, Easing::EaseInOutBounce,
        ];
        for e in &easings {
            let at_zero = apply_easing(0.0, e);
            let at_one = apply_easing(1.0, e);
            assert!(at_zero.abs() < 1.0e-10, "{:?} at 0: {}", e, at_zero);
            assert!((at_one - 1.0).abs() < 1.0e-10, "{:?} at 1: {}", e, at_one);
        }
    }

    #[test]
    fn cubic_bezier_easing_linear() {
        // A linear cubic bezier: (0.0, 0.0, 1.0, 1.0)
        let e = Easing::CubicBezier(0.0, 0.0, 1.0, 1.0);
        for i in 0..=10 {
            let t = i as f64 / 10.0;
            let result = apply_easing(t, &e);
            assert!((result - t).abs() < 0.01, "t={}: {}", t, result);
        }
    }

    #[test]
    fn advance_uses_easing() {
        let mut tween = Tween::new(0.0, 100.0, 1000)
            .easing(Easing::EaseInQuad);
        tween.start(0);
        tween.advance(500);
        // EaseInQuad at t=0.5: 0.25, so value should be ~25
        let v = tween.value().unwrap();
        assert!((v - 25.0).abs() < 0.01, "got: {}", v);
    }

    #[test]
    fn repeat_restarts() {
        let mut tween = Tween::new(0.0, 100.0, 100)
            .easing(Easing::Linear)
            .repeat(2);
        tween.start(0);
        tween.advance(100); // first cycle ends
        assert!(!tween.finished());
        assert_eq!(tween.repeat, Some(Repeat::Times(1)));
        tween.advance(200); // second cycle ends
        assert!(tween.finished());
    }

    #[test]
    fn auto_reverse_swaps_direction() {
        let mut tween = Tween::new(0.0, 100.0, 100)
            .easing(Easing::Linear)
            .repeat_forever()
            .auto_reverse(true);
        tween.start(0);
        tween.advance(100); // first cycle ends, should reverse
        assert_eq!(tween.from, 100.0);
        assert_eq!(tween.to, 0.0);
        tween.advance(150); // halfway through reverse
        let v = tween.value().unwrap();
        assert!((v - 50.0).abs() < 1.0, "got: {}", v);
    }
}
