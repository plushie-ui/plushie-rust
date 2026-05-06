//! SDK-side frame-by-frame animation.
//!
//! [`Tween`] provides stateful interpolation driven by the app's
//! update loop. Use for animations that need to be tied to model
//! state (e.g., canvas-based custom rendering).
//!
//! Two modes are supported:
//!
//! - **Timed**: easing-curve interpolation over a fixed duration.
//! - **Spring**: damped harmonic oscillator physics simulation.
//!
//! For declarative property animations that the renderer handles
//! autonomously, use [`Transition`](super::Transition) or
//! [`Spring`](super::Spring) instead.

use super::{Easing, Repeat};

use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// SpringConfig
// ---------------------------------------------------------------------------

/// Physics parameters for spring-based animation.
///
/// ```
/// use plushie::animation::SpringConfig;
///
/// // Use a preset
/// let config = SpringConfig::bouncy();
///
/// // Or customize from defaults
/// let config = SpringConfig::default()
///     .stiffness(250.0)
///     .damping(18.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    /// Spring stiffness (force per unit displacement). Higher values
    /// produce faster, snappier motion. Default: 100.0.
    pub stiffness: f64,
    /// Damping coefficient (force per unit velocity). Higher values
    /// reduce overshoot and oscillation. Default: 10.0.
    pub damping: f64,
    /// Mass of the animated object. Higher values produce slower,
    /// heavier motion. Must be positive (zero causes division by
    /// zero in the physics simulation). Default: 1.0.
    pub mass: f64,
    /// Initial velocity at the start of the animation. Default: 0.0.
    pub initial_velocity: f64,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            initial_velocity: 0.0,
        }
    }
}

impl SpringConfig {
    /// Slow, smooth motion with no overshoot.
    pub fn gentle() -> Self {
        Self {
            stiffness: 120.0,
            damping: 14.0,
            ..Self::default()
        }
    }

    /// Quick motion with visible overshoot.
    pub fn bouncy() -> Self {
        Self {
            stiffness: 300.0,
            damping: 10.0,
            ..Self::default()
        }
    }

    /// Very quick, crisp stop with minimal overshoot.
    pub fn stiff() -> Self {
        Self {
            stiffness: 400.0,
            damping: 30.0,
            ..Self::default()
        }
    }

    /// Quick with minimal overshoot. A balanced default.
    pub fn snappy() -> Self {
        Self {
            stiffness: 200.0,
            damping: 20.0,
            ..Self::default()
        }
    }

    /// Slow, heavy, deliberate motion.
    pub fn molasses() -> Self {
        Self {
            stiffness: 60.0,
            damping: 12.0,
            ..Self::default()
        }
    }

    /// Override the stiffness value.
    pub fn stiffness(mut self, v: f64) -> Self {
        self.stiffness = v;
        self
    }
    /// Override the damping value.
    pub fn damping(mut self, v: f64) -> Self {
        self.damping = v;
        self
    }
    /// Override the mass value. Must be positive.
    pub fn mass(mut self, v: f64) -> Self {
        self.mass = v;
        self
    }
    /// Set the initial velocity for the animation.
    pub fn initial_velocity(mut self, v: f64) -> Self {
        self.initial_velocity = v;
        self
    }
}

// ---------------------------------------------------------------------------
// RedirectOpts
// ---------------------------------------------------------------------------

/// Optional parameter overrides for [`Tween::redirect_with`].
///
/// Only applies to timed tweens; ignored for spring tweens.
///
/// ```
/// use plushie::animation::{RedirectOpts, Easing};
///
/// let opts = RedirectOpts::default()
///     .easing(Easing::EaseOutCubic)
///     .duration(300);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RedirectOpts {
    /// Override the easing curve for the redirected animation.
    pub easing: Option<Easing>,
    /// Override the duration (ms) for the redirected animation.
    pub duration: Option<u64>,
}

impl RedirectOpts {
    /// Override the easing curve.
    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = Some(e);
        self
    }
    /// Override the duration in milliseconds.
    pub fn duration(mut self, d: u64) -> Self {
        self.duration = Some(d);
        self
    }
}

// ---------------------------------------------------------------------------
// Internal mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum TweenMode {
    Timed {
        duration_ms: u64,
        easing: Easing,
        delay_ms: u64,
    },
    Spring {
        config: SpringConfig,
        velocity: f64,
    },
}

// ---------------------------------------------------------------------------
// Tween
// ---------------------------------------------------------------------------

/// A stateful interpolator for frame-by-frame animation.
///
/// ## Timed mode
///
/// Interpolates from `from` to `to` over a fixed duration using an
/// easing curve. Supports delay, repeat, and auto-reverse.
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
///
/// ## Spring mode
///
/// Uses a damped harmonic oscillator to animate toward the target.
/// Duration is implicit: the animation finishes when the spring
/// settles. Repeat and auto-reverse are not supported.
///
/// ```
/// use plushie::animation::{Tween, SpringConfig};
///
/// let mut tween = Tween::spring(0.0, 100.0, SpringConfig::bouncy());
///
/// tween.start(0);
/// tween.advance(500);
/// assert!(tween.running());
/// ```
#[derive(Debug, Clone)]
pub struct Tween {
    /// Starting value of the tween.
    pub from: f64,
    /// Target value at the end of the tween.
    pub to: f64,
    mode: TweenMode,
    /// Optional loop policy.
    pub repeat: Option<Repeat>,
    /// When true, each loop iteration reverses direction (ping-pong).
    pub auto_reverse: bool,
    started_at: Option<u64>,
    /// Timestamp of the last `advance` call. Only meaningful in spring
    /// mode; the timed mode reads `started_at` for elapsed-time math
    /// and does not update this field. `restart_cycle` therefore
    /// leaves `last_timestamp` alone in timed mode by design.
    last_timestamp: Option<u64>,
    value: Option<f64>,
    finished: bool,
}

impl Tween {
    // -- Timed constructors --------------------------------------------------

    /// Create a timed tween from `from` to `to` over `duration_ms` milliseconds.
    pub fn new(from: f64, to: f64, duration_ms: u64) -> Self {
        Self {
            from,
            to,
            mode: TweenMode::Timed {
                duration_ms,
                easing: Easing::EaseInOut,
                delay_ms: 0,
            },
            repeat: None,
            auto_reverse: false,
            started_at: None,
            last_timestamp: None,
            value: None,
            finished: false,
        }
    }

    /// Set the easing curve (timed mode only, no-op for spring).
    pub fn easing(mut self, e: Easing) -> Self {
        if let TweenMode::Timed { easing, .. } = &mut self.mode {
            *easing = e;
        }
        self
    }

    /// Set the delay before the animation starts (timed mode only).
    pub fn delay(mut self, ms: u64) -> Self {
        if let TweenMode::Timed { delay_ms, .. } = &mut self.mode {
            *delay_ms = ms;
        }
        self
    }

    /// Set the animation duration in milliseconds (timed mode only).
    pub fn duration(mut self, ms: u64) -> Self {
        if let TweenMode::Timed { duration_ms, .. } = &mut self.mode {
            *duration_ms = ms;
        }
        self
    }

    /// Set a finite repeat count (applies to timed mode only).
    pub fn repeat(mut self, n: u32) -> Self {
        self.repeat = Some(Repeat::Times(n));
        self
    }

    /// Repeat forever (applies to timed mode only).
    pub fn repeat_forever(mut self) -> Self {
        self.repeat = Some(Repeat::Forever);
        self
    }

    /// Reverse direction on each repeat (applies to timed mode only).
    pub fn auto_reverse(mut self, v: bool) -> Self {
        self.auto_reverse = v;
        self
    }

    /// Create a looping tween (repeat forever, auto-reverse).
    pub fn looping(from: f64, to: f64, duration_ms: u64) -> Self {
        Self::new(from, to, duration_ms)
            .repeat_forever()
            .auto_reverse(true)
    }

    // -- Spring constructor --------------------------------------------------

    /// Create a spring-physics tween from `from` toward `to`.
    ///
    /// The animation finishes when the spring settles (velocity and
    /// position error are both negligible). Duration is implicit.
    ///
    /// # Panics
    ///
    /// Panics if `config.mass` is zero, negative, NaN, or infinite.
    pub fn spring(from: f64, to: f64, config: SpringConfig) -> Self {
        assert!(
            config.mass.is_finite() && config.mass > 0.0,
            "spring mass must be a positive finite number, got {}",
            config.mass
        );

        let velocity = config.initial_velocity;
        Self {
            from,
            to,
            mode: TweenMode::Spring { config, velocity },
            repeat: None,
            auto_reverse: false,
            started_at: None,
            last_timestamp: None,
            value: None,
            finished: false,
        }
    }

    // -- Lifecycle -----------------------------------------------------------

    /// Start the tween at the given timestamp (monotonic milliseconds).
    pub fn start(&mut self, timestamp: u64) {
        self.started_at = Some(timestamp);
        self.last_timestamp = Some(timestamp);
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
        if self.started_at.is_none() || self.finished {
            return;
        }

        match &self.mode {
            TweenMode::Timed { .. } => self.advance_timed(timestamp),
            TweenMode::Spring { .. } => self.advance_spring(timestamp),
        }
    }

    /// Redirect the tween to a new target from the current value.
    ///
    /// For spring tweens, velocity is preserved for natural momentum.
    /// For timed tweens, the animation restarts with the current
    /// easing and duration.
    pub fn redirect(&mut self, to: f64, timestamp: u64) {
        self.redirect_inner(to, timestamp, None);
    }

    /// Redirect with optional easing/duration overrides (timed only).
    ///
    /// Spring tweens ignore the options; velocity is always preserved.
    pub fn redirect_with(&mut self, to: f64, timestamp: u64, opts: RedirectOpts) {
        self.redirect_inner(to, timestamp, Some(opts));
    }

    fn redirect_inner(&mut self, to: f64, timestamp: u64, opts: Option<RedirectOpts>) {
        let current = self.value.unwrap_or(self.from);
        self.from = current;
        self.to = to;

        if let Some(opts) = opts
            && let TweenMode::Timed {
                easing,
                duration_ms,
                ..
            } = &mut self.mode
        {
            if let Some(e) = opts.easing {
                *easing = e;
            }
            if let Some(d) = opts.duration {
                *duration_ms = d;
            }
        }

        self.start(timestamp);
        // For springs, start() resets started_at and last_timestamp
        // but does NOT touch velocity (it lives in TweenMode::Spring).
    }

    // -- Queries -------------------------------------------------------------

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

    /// Whether this is a spring-mode tween.
    pub fn is_spring(&self) -> bool {
        matches!(self.mode, TweenMode::Spring { .. })
    }

    // -- Internal: timed advance ---------------------------------------------

    fn advance_timed(&mut self, timestamp: u64) {
        let started = self.started_at.unwrap();
        let TweenMode::Timed {
            duration_ms,
            easing,
            delay_ms,
        } = &self.mode
        else {
            return;
        };
        let duration_ms = *duration_ms;
        let delay_ms = *delay_ms;
        let easing = *easing;

        let elapsed = timestamp.saturating_sub(started);
        if elapsed < delay_ms {
            self.value = Some(self.from);
            return;
        }

        let active_elapsed = elapsed - delay_ms;
        if active_elapsed >= duration_ms {
            self.handle_cycle_end();
            return;
        }

        let t = active_elapsed as f64 / duration_ms as f64;
        let eased_t = apply_easing(t, &easing);
        self.value = Some(self.from + (self.to - self.from) * eased_t);
    }

    /// Handle the end of one timed animation cycle.
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

    fn restart_cycle(&mut self) {
        if let Some(started) = self.started_at
            && let TweenMode::Timed { duration_ms, .. } = &self.mode
        {
            self.started_at = Some(started + duration_ms);
        }
        if self.auto_reverse {
            std::mem::swap(&mut self.from, &mut self.to);
        } else {
            self.value = Some(self.from);
        }
    }

    // -- Internal: spring advance --------------------------------------------

    fn advance_spring(&mut self, timestamp: u64) {
        let last = self.last_timestamp.unwrap_or(timestamp);
        let elapsed_ms = timestamp.saturating_sub(last);
        self.last_timestamp = Some(timestamp);

        if elapsed_ms == 0 {
            return;
        }

        let TweenMode::Spring { config, velocity } = &mut self.mode else {
            return;
        };

        let mut pos = self.value.unwrap_or(self.from);
        let mut vel = *velocity;

        // Fixed 1ms timestep Euler integration for numerical stability.
        // Capped at 1000 steps per advance to prevent runaway loops
        // on large time deltas (matches Elixir implementation).
        let dt: f64 = 0.001;
        let steps = elapsed_ms.min(1000);
        for _ in 0..steps {
            let force = -config.stiffness * (pos - self.to) - config.damping * vel;
            let acc = force / config.mass;
            vel += acc * dt;
            pos += vel * dt;

            if !pos.is_finite() || !vel.is_finite() {
                pos = self.to;
                vel = 0.0;
                self.finished = true;
                break;
            }
        }

        // Convergence: spring has settled when both velocity and
        // position error are negligible.
        if vel.abs() < 0.01 && (pos - self.to).abs() < 0.001 {
            pos = self.to;
            vel = 0.0;
            self.finished = true;
        }

        self.value = Some(pos);
        *velocity = vel;
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
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }

        // Cubic
        Easing::EaseInCubic => t * t * t,
        Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
        Easing::EaseInOutCubic => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }

        // Quartic
        Easing::EaseInQuart => t * t * t * t,
        Easing::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
        Easing::EaseInOutQuart => {
            if t < 0.5 {
                8.0 * t * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
            }
        }

        // Quintic
        Easing::EaseInQuint => t * t * t * t * t,
        Easing::EaseOutQuint => 1.0 - (1.0 - t).powi(5),
        Easing::EaseInOutQuint => {
            if t < 0.5 {
                16.0 * t * t * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(5) / 2.0
            }
        }

        // Exponential
        Easing::EaseInExpo => {
            if t == 0.0 {
                0.0
            } else {
                2.0_f64.powf(10.0 * t - 10.0)
            }
        }
        Easing::EaseOutExpo => {
            if t == 1.0 {
                1.0
            } else {
                1.0 - 2.0_f64.powf(-10.0 * t)
            }
        }
        Easing::EaseInOutExpo => {
            if t == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else if t < 0.5 {
                2.0_f64.powf(20.0 * t - 10.0) / 2.0
            } else {
                (2.0 - 2.0_f64.powf(-20.0 * t + 10.0)) / 2.0
            }
        }

        // Circular
        Easing::EaseInCirc => 1.0 - (1.0 - t * t).sqrt(),
        Easing::EaseOutCirc => (1.0 - (t - 1.0) * (t - 1.0)).sqrt(),
        Easing::EaseInOutCirc => {
            if t < 0.5 {
                (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
            } else {
                (1.0 + (1.0 - (-2.0 * t + 2.0).powi(2)).sqrt()) / 2.0
            }
        }

        // Back (overshoot)
        Easing::EaseInBack => C3 * t * t * t - C1 * t * t,
        Easing::EaseOutBack => 1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2),
        Easing::EaseInOutBack => {
            if t < 0.5 {
                (2.0 * t).powi(2) * ((C2 + 1.0) * 2.0 * t - C2) / 2.0
            } else {
                ((2.0 * t - 2.0).powi(2) * ((C2 + 1.0) * (2.0 * t - 2.0) + C2) + 2.0) / 2.0
            }
        }

        // Elastic (oscillating overshoot)
        Easing::EaseInElastic => {
            if t == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else {
                -(2.0_f64.powf(10.0 * t - 10.0) * ((10.0 * t - 10.75) * C4).sin())
            }
        }
        Easing::EaseOutElastic => {
            if t == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else {
                2.0_f64.powf(-10.0 * t) * ((10.0 * t - 0.75) * C4).sin() + 1.0
            }
        }
        Easing::EaseInOutElastic => {
            if t == 0.0 {
                0.0
            } else if t == 1.0 {
                1.0
            } else if t < 0.5 {
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
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }

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
    3.0 * (1.0 - s) * (1.0 - s) * p1 + 6.0 * (1.0 - s) * s * (p2 - p1) + 3.0 * s * s * (1.0 - p2)
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

    // -- Easing tests --------------------------------------------------------

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
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
            Easing::EaseInCubic,
            Easing::EaseOutCubic,
            Easing::EaseInOutCubic,
            Easing::EaseInQuart,
            Easing::EaseOutQuart,
            Easing::EaseInOutQuart,
            Easing::EaseInQuint,
            Easing::EaseOutQuint,
            Easing::EaseInOutQuint,
            Easing::EaseInExpo,
            Easing::EaseOutExpo,
            Easing::EaseInOutExpo,
            Easing::EaseInCirc,
            Easing::EaseOutCirc,
            Easing::EaseInOutCirc,
            Easing::EaseInBounce,
            Easing::EaseOutBounce,
            Easing::EaseInOutBounce,
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

    // -- Timed advance tests -------------------------------------------------

    #[test]
    fn advance_uses_easing() {
        let mut tween = Tween::new(0.0, 100.0, 1000).easing(Easing::EaseInQuad);
        tween.start(0);
        tween.advance(500);
        // EaseInQuad at t=0.5: 0.25, so value should be ~25
        let v = tween.value().unwrap();
        assert!((v - 25.0).abs() < 0.01, "got: {}", v);
    }

    #[test]
    fn repeat_restarts() {
        let mut tween = Tween::new(0.0, 100.0, 100).easing(Easing::Linear).repeat(2);
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

    // -- Spring tests --------------------------------------------------------

    #[test]
    fn spring_config_presets() {
        let gentle = SpringConfig::gentle();
        assert_eq!(gentle.stiffness, 120.0);
        assert_eq!(gentle.damping, 14.0);

        let bouncy = SpringConfig::bouncy();
        assert_eq!(bouncy.stiffness, 300.0);
        assert_eq!(bouncy.damping, 10.0);

        let stiff = SpringConfig::stiff();
        assert_eq!(stiff.stiffness, 400.0);
        assert_eq!(stiff.damping, 30.0);

        let snappy = SpringConfig::snappy();
        assert_eq!(snappy.stiffness, 200.0);
        assert_eq!(snappy.damping, 20.0);

        let molasses = SpringConfig::molasses();
        assert_eq!(molasses.stiffness, 60.0);
        assert_eq!(molasses.damping, 12.0);
    }

    #[test]
    fn spring_config_builder() {
        let config = SpringConfig::default()
            .stiffness(250.0)
            .damping(18.0)
            .mass(2.0)
            .initial_velocity(5.0);
        assert_eq!(config.stiffness, 250.0);
        assert_eq!(config.damping, 18.0);
        assert_eq!(config.mass, 2.0);
        assert_eq!(config.initial_velocity, 5.0);
    }

    #[test]
    fn spring_approaches_target() {
        let mut tween = Tween::spring(0.0, 100.0, SpringConfig::default());
        tween.start(0);

        tween.advance(100);
        let v = tween.value().unwrap();
        assert!(v > 0.0, "spring should move toward target, got: {}", v);
        assert!(
            v < 100.0,
            "spring shouldn't overshoot this early with default damping"
        );
        assert!(!tween.finished());
    }

    #[test]
    fn spring_settles_at_target() {
        let mut tween = Tween::spring(0.0, 100.0, SpringConfig::stiff());
        tween.start(0);

        // Advance enough for the spring to settle
        for t in (100..=5000).step_by(100) {
            tween.advance(t);
            if tween.finished() {
                break;
            }
        }

        assert!(tween.finished(), "stiff spring should settle within 5s");
        assert!(
            (tween.value().unwrap() - 100.0).abs() < 0.01,
            "settled value should be at target"
        );
    }

    #[test]
    fn bouncy_spring_overshoots() {
        let mut tween = Tween::spring(0.0, 100.0, SpringConfig::bouncy());
        tween.start(0);

        let mut max_value: f64 = 0.0;
        for t in (1..=2000).step_by(1) {
            tween.advance(t);
            if let Some(v) = tween.value() {
                max_value = max_value.max(v);
            }
        }

        assert!(
            max_value > 100.0,
            "bouncy spring should overshoot target, max was: {}",
            max_value
        );
    }

    #[test]
    fn spring_with_initial_velocity() {
        let config = SpringConfig::default().initial_velocity(50.0);
        let mut tween = Tween::spring(0.0, 100.0, config);
        tween.start(0);

        tween.advance(50);
        let with_velocity = tween.value().unwrap();

        let mut tween2 = Tween::spring(0.0, 100.0, SpringConfig::default());
        tween2.start(0);
        tween2.advance(50);
        let without_velocity = tween2.value().unwrap();

        assert!(
            with_velocity > without_velocity,
            "initial velocity should make the spring move faster: {} vs {}",
            with_velocity,
            without_velocity
        );
    }

    #[test]
    #[should_panic(expected = "spring mass must be a positive finite number")]
    fn spring_rejects_zero_mass() {
        let _ = Tween::spring(0.0, 100.0, SpringConfig::default().mass(0.0));
    }

    #[test]
    #[should_panic(expected = "spring mass must be a positive finite number")]
    fn spring_rejects_negative_mass() {
        let _ = Tween::spring(0.0, 100.0, SpringConfig::default().mass(-1.0));
    }

    #[test]
    #[should_panic(expected = "spring mass must be a positive finite number")]
    fn spring_rejects_infinite_mass() {
        let _ = Tween::spring(0.0, 100.0, SpringConfig::default().mass(f64::INFINITY));
    }

    #[test]
    #[should_panic(expected = "spring mass must be a positive finite number")]
    fn spring_rejects_nan_mass() {
        let _ = Tween::spring(0.0, 100.0, SpringConfig::default().mass(f64::NAN));
    }

    #[test]
    fn spring_is_spring() {
        let tween = Tween::spring(0.0, 100.0, SpringConfig::default());
        assert!(tween.is_spring());

        let tween2 = Tween::new(0.0, 100.0, 500);
        assert!(!tween2.is_spring());
    }

    // -- Redirect tests ------------------------------------------------------

    #[test]
    fn redirect_changes_target() {
        let mut tween = Tween::new(0.0, 100.0, 1000).easing(Easing::Linear);
        tween.start(0);
        tween.advance(500);
        let mid = tween.value().unwrap();

        tween.redirect(50.0, 500);
        assert_eq!(tween.from, mid);
        assert_eq!(tween.to, 50.0);
        assert!(tween.running());
    }

    #[test]
    fn redirect_with_easing_override() {
        let mut tween = Tween::new(0.0, 100.0, 1000).easing(Easing::Linear);
        tween.start(0);
        tween.advance(500);

        tween.redirect_with(
            200.0,
            500,
            RedirectOpts::default()
                .easing(Easing::EaseInQuad)
                .duration(2000),
        );

        // After redirect, advance halfway through new duration
        tween.advance(1500);
        let v = tween.value().unwrap();
        // EaseInQuad at t=0.5: 0.25. Range is 50..200 (from mid to 200), so
        // value should be ~50 + 0.25 * 150 = ~87.5
        assert!((v - 87.5).abs() < 1.0, "got: {}", v);
    }

    #[test]
    fn spring_redirect_preserves_velocity() {
        let mut tween = Tween::spring(0.0, 100.0, SpringConfig::stiff());
        tween.start(0);

        // Let the spring build up velocity
        tween.advance(50);
        let value_before = tween.value().unwrap();
        assert!(value_before > 0.0);

        // Redirect to a different target
        tween.redirect(200.0, 50);
        assert_eq!(tween.from, value_before);
        assert_eq!(tween.to, 200.0);

        // The spring should continue moving (velocity preserved)
        tween.advance(51);
        let value_after = tween.value().unwrap();
        assert!(
            value_after > value_before,
            "spring should keep moving after redirect: {} vs {}",
            value_after,
            value_before
        );
    }

    #[test]
    fn spring_large_time_delta_is_capped() {
        // With the 1000-step cap, a huge time delta should not loop
        // for millions of iterations. The value should still be finite
        // and reasonable (not NaN or infinity).
        let mut tween = Tween::spring(0.0, 100.0, SpringConfig::stiff());
        tween.start(0);
        tween.advance(1_000_000); // 1 million ms, but capped to 1000 steps
        let v = tween.value().unwrap();
        assert!(
            v.is_finite(),
            "value should be finite after large delta: {}",
            v
        );
    }

    #[test]
    fn spring_overflow_stops_with_finite_value() {
        let config = SpringConfig::default()
            .stiffness(f64::MAX)
            .damping(0.0)
            .mass(f64::MIN_POSITIVE);
        let mut tween = Tween::spring(0.0, 100.0, config);
        tween.start(0);
        tween.advance(1);

        assert!(tween.finished());
        assert_eq!(tween.value(), Some(100.0));
    }
}
