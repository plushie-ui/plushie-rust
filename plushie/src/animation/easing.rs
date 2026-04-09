//! Easing curves for animations.
//!
//! 31 named easing curves plus custom cubic bezier support.

use serde::{Deserialize, Serialize};

/// An easing curve that controls animation timing.
///
/// Determines how values interpolate between start and end.
/// `Linear` is constant speed. `EaseIn*` starts slow and
/// accelerates. `EaseOut*` starts fast and decelerates.
/// `EaseInOut*` does both.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Easing {
    Linear,

    // Sine
    EaseIn,
    EaseOut,
    EaseInOut,

    // Quad
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,

    // Cubic
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,

    // Quart
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,

    // Quint
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,

    // Expo
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,

    // Circ
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,

    // Back
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,

    // Elastic
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,

    // Bounce
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,

    /// Custom cubic bezier curve defined by two control points.
    CubicBezier(f32, f32, f32, f32),
}

impl Default for Easing {
    fn default() -> Self {
        Self::EaseInOut
    }
}
