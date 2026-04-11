//! A value that can be either static or animated.
//!
//! Widget builder setters accept `impl Into<Animatable<T>>`, allowing
//! both static values and animation descriptors through the same method.
//!
//! # Wire format
//!
//! Static values encode directly as their underlying type (number, string,
//! etc.). Animation descriptors encode as JSON objects with a `"type"` field
//! (`"transition"`, `"spring"`, or `"sequence"`) that the renderer detects
//! during prop diffing. This means the same prop slot carries either a plain
//! value or an animation descriptor transparently.
//!
//! ```
//! use plushie_core::animation::{Transition, Spring, Easing};
//! use plushie_core::types::{Animatable, Color};
//!
//! // Static value (transparent, no overhead)
//! let size: Animatable<f32> = 24.0_f32.into();
//!
//! // Animated value (transition descriptor)
//! let animated: Animatable<f32> = Transition::new(300, 24.0_f32)
//!     .easing(Easing::EaseOut)
//!     .into();
//!
//! // Color animation
//! let color_anim: Animatable<Color> = Spring::bouncy(Color::red()).into();
//! ```

use crate::animation::{Sequence, Spring, Transition};
use crate::protocol::PropValue;
use crate::types::background::Background;
use crate::types::color::Color;
use crate::types::gradient::Gradient;
use crate::types::line_height::LineHeight;
use crate::types::PlushieType;

/// A value that can be either static or animated.
///
/// Widget builder setters accept `impl Into<Animatable<T>>`, allowing
/// both static values and animation descriptors through the same method.
///
/// Static values encode to their underlying wire type. Animation
/// descriptors encode as objects with a `"type"` discriminator field
/// that the renderer's animation system detects during prop diffing.
#[derive(Debug, Clone)]
pub enum Animatable<T: PlushieType> {
    /// A static value applied immediately.
    Value(T),
    /// A timed transition to the target value.
    Transition(Transition<T>),
    /// A spring physics animation to the target value.
    Spring(Spring<T>),
    /// A sequence of animation steps.
    Sequence(Sequence<T>),
}

impl<T: PlushieType> Animatable<T> {
    /// Encode to PropValue for wire transport.
    ///
    /// Static values encode directly. Animation descriptors encode
    /// as objects with a `"type"` field the renderer detects.
    pub fn wire_encode(&self) -> PropValue {
        match self {
            Self::Value(v) => v.wire_encode(),
            Self::Transition(t) => t.wire_encode(),
            Self::Spring(s) => s.wire_encode(),
            Self::Sequence(seq) => seq.wire_encode(),
        }
    }
}

/// Wrap a static value as [`Animatable::Value`].
impl<T: PlushieType> From<T> for Animatable<T> {
    fn from(v: T) -> Self {
        Self::Value(v)
    }
}

/// Wrap a transition descriptor as [`Animatable::Transition`].
impl<T: PlushieType> From<Transition<T>> for Animatable<T> {
    fn from(t: Transition<T>) -> Self {
        Self::Transition(t)
    }
}

/// Wrap a spring descriptor as [`Animatable::Spring`].
impl<T: PlushieType> From<Spring<T>> for Animatable<T> {
    fn from(s: Spring<T>) -> Self {
        Self::Spring(s)
    }
}

/// Wrap a sequence descriptor as [`Animatable::Sequence`].
impl<T: PlushieType> From<Sequence<T>> for Animatable<T> {
    fn from(seq: Sequence<T>) -> Self {
        Self::Sequence(seq)
    }
}

/// Convert a hex string to a static color value.
///
/// Convenience impl so color props accept `"#rrggbb"` directly
/// without requiring the intermediate `&str -> Color` conversion
/// that Rust's `Into` cannot chain.
impl From<&str> for Animatable<Color> {
    fn from(s: &str) -> Self {
        Self::Value(Color::from(s))
    }
}

/// Convert an owned hex string to a static color value.
impl From<String> for Animatable<Color> {
    fn from(s: String) -> Self {
        Self::Value(Color::from(s))
    }
}

/// Convert a bare f32 to a relative line height.
///
/// Allows `.line_height(1.5_f32)` without wrapping in `LineHeight::Relative`.
impl From<f32> for Animatable<LineHeight> {
    fn from(v: f32) -> Self {
        Self::Value(LineHeight::Relative(v))
    }
}

/// Convert a bare f64 to a relative line height.
///
/// Allows `.line_height(1.5)` (Rust defaults float literals to f64).
impl From<f64> for Animatable<LineHeight> {
    fn from(v: f64) -> Self {
        Self::Value(LineHeight::Relative(v as f32))
    }
}

/// Convert a solid color to a background.
///
/// Allows `.background(Color::red())` without wrapping in `Background::Color`.
impl From<Color> for Animatable<Background> {
    fn from(c: Color) -> Self {
        Self::Value(Background::Color(c))
    }
}

/// Convert a hex string to a solid-color background.
///
/// Allows `.background("#ff0000")`.
impl From<&str> for Animatable<Background> {
    fn from(s: &str) -> Self {
        Self::Value(Background::Color(Color::hex(s)))
    }
}

/// Convert a gradient to a background.
///
/// Allows `.background(gradient)` without wrapping in `Background::Gradient`.
impl From<Gradient> for Animatable<Background> {
    fn from(g: Gradient) -> Self {
        Self::Value(Background::Gradient(g))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::Easing;

    #[test]
    fn static_f32_value() {
        let a: Animatable<f32> = 42.0_f32.into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json, 42.0);
    }

    #[test]
    fn transition_f32() {
        let t: Transition<f32> = Transition::new(300, 24.0_f32).easing(Easing::EaseOut);
        let a: Animatable<f32> = t.into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "transition");
        assert_eq!(json["to"], 24.0);
    }

    #[test]
    fn spring_f32() {
        let s: Spring<f32> = Spring::bouncy(1.05_f32);
        let a: Animatable<f32> = s.into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "spring");
        // f32 1.05 encodes as f64 via PlushieType; precision is f32-level
        let to = json["to"].as_f64().unwrap();
        assert!((to - 1.05).abs() < 0.001);
    }

    #[test]
    fn sequence_f32() {
        let seq: Sequence<f32> = Sequence::new(vec![
            Transition::new(200, 1.0_f32).into(),
            Spring::new(0.0_f32).stiffness(200.0).into(),
        ]);
        let a: Animatable<f32> = seq.into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "sequence");
    }

    #[test]
    fn color_from_str_convenience() {
        let a: Animatable<Color> = "#ff0000".into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json, "#ff0000");
    }

    #[test]
    fn color_from_string_convenience() {
        let a: Animatable<Color> = String::from("#00ff00").into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json, "#00ff00");
    }

    #[test]
    fn color_static_value() {
        let a: Animatable<Color> = Color::red().into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json, "#ff0000");
    }

    #[test]
    fn color_transition() {
        let t: Transition<Color> = Transition::new(300, Color::blue());
        let a: Animatable<Color> = t.into();
        let encoded = a.wire_encode();
        let json = serde_json::Value::from(encoded);
        assert_eq!(json["type"], "transition");
        assert_eq!(json["to"], "#0000ff");
    }
}
