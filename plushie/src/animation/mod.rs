//! Declarative animation descriptors.
//!
//! Attach animations to widget properties in views. The renderer
//! interpolates values at full frame rate with zero wire traffic.
//!
//! ```ignore
//! text("value")
//!     .size(model.font_size)
//!     .transition("size", Transition::new(300).easing(Easing::EaseOut))
//! ```
//!
//! Three animation types:
//!
//! - [`Transition`]: Timed interpolation with easing curves.
//! - [`Spring`]: Physics-based spring animation.
//! - [`Sequence`]: Chain multiple animation steps.

mod easing;
mod tween;

pub use easing::Easing;
pub use tween::Tween;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------

/// A timed interpolation between values.
///
/// ```
/// use plushie::animation::Transition;
/// use plushie::animation::Easing;
///
/// let t = Transition::new(500)
///     .easing(Easing::EaseOutCubic)
///     .delay(100);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub duration: u64,
    pub easing: Easing,
    pub delay: u64,
    pub from: Option<serde_json::Value>,
    pub repeat: Option<Repeat>,
    pub auto_reverse: bool,
    pub on_complete: Option<String>,
}

impl Transition {
    /// Create a transition with the given duration in milliseconds.
    pub fn new(duration_ms: u64) -> Self {
        Self {
            duration: duration_ms,
            easing: Easing::EaseInOut,
            delay: 0,
            from: None,
            repeat: None,
            auto_reverse: false,
            on_complete: None,
        }
    }

    pub fn easing(mut self, e: Easing) -> Self { self.easing = e; self }
    pub fn delay(mut self, ms: u64) -> Self { self.delay = ms; self }
    pub fn from(mut self, v: impl Into<serde_json::Value>) -> Self {
        self.from = Some(v.into()); self
    }
    pub fn repeat(mut self, n: u32) -> Self { self.repeat = Some(Repeat::Times(n)); self }
    pub fn repeat_forever(mut self) -> Self { self.repeat = Some(Repeat::Forever); self }
    pub fn auto_reverse(mut self, v: bool) -> Self { self.auto_reverse = v; self }
    pub fn on_complete(mut self, tag: &str) -> Self { self.on_complete = Some(tag.into()); self }

    /// Create a looping transition (repeat forever, auto-reverse).
    pub fn looping(duration_ms: u64) -> Self {
        Self::new(duration_ms)
            .repeat_forever()
            .auto_reverse(true)
    }
}

/// How many times to repeat an animation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Repeat {
    Times(u32),
    Forever,
}

// ---------------------------------------------------------------------------
// Spring
// ---------------------------------------------------------------------------

/// A physics-based spring animation.
///
/// ```
/// use plushie::animation::Spring;
///
/// let s = Spring::new().stiffness(100.0).damping(10.0);
/// let bouncy = Spring::bouncy();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spring {
    pub stiffness: f64,
    pub damping: f64,
    pub mass: f64,
    pub velocity: f64,
    pub from: Option<serde_json::Value>,
    pub on_complete: Option<String>,
}

impl Spring {
    pub fn new() -> Self {
        Self {
            stiffness: 100.0,
            damping: 10.0,
            mass: 1.0,
            velocity: 0.0,
            from: None,
            on_complete: None,
        }
    }

    pub fn stiffness(mut self, s: f64) -> Self { self.stiffness = s; self }
    pub fn damping(mut self, d: f64) -> Self { self.damping = d; self }
    pub fn mass(mut self, m: f64) -> Self { self.mass = m; self }
    pub fn velocity(mut self, v: f64) -> Self { self.velocity = v; self }
    pub fn from(mut self, v: impl Into<serde_json::Value>) -> Self {
        self.from = Some(v.into()); self
    }
    pub fn on_complete(mut self, tag: &str) -> Self { self.on_complete = Some(tag.into()); self }

    // Named presets matching the Elixir SDK.
    pub fn gentle() -> Self { Self::new().stiffness(120.0).damping(14.0) }
    pub fn bouncy() -> Self { Self::new().stiffness(200.0).damping(12.0) }
    pub fn stiff() -> Self { Self::new().stiffness(300.0).damping(20.0) }
    pub fn snappy() -> Self { Self::new().stiffness(400.0).damping(30.0) }
    pub fn molasses() -> Self { Self::new().stiffness(50.0).damping(20.0) }
}

impl Default for Spring {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// Sequence
// ---------------------------------------------------------------------------

/// A chain of animation steps executed in order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sequence {
    pub steps: Vec<AnimationStep>,
    pub on_complete: Option<String>,
}

/// A single step in a sequence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimationStep {
    Transition(Transition),
    Spring(Spring),
}

impl Sequence {
    pub fn new(steps: Vec<AnimationStep>) -> Self {
        Self { steps, on_complete: None }
    }

    pub fn on_complete(mut self, tag: &str) -> Self {
        self.on_complete = Some(tag.into());
        self
    }
}

impl From<Transition> for AnimationStep {
    fn from(t: Transition) -> Self { AnimationStep::Transition(t) }
}

impl From<Spring> for AnimationStep {
    fn from(s: Spring) -> Self { AnimationStep::Spring(s) }
}
