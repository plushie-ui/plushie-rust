//! Easing curves for animations.
//!
//! 31 named easing curves plus custom cubic bezier support.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};
use crate::types::PlushieType;

/// An easing curve that controls animation timing.
///
/// Determines how values interpolate between start and end.
/// `Linear` is constant speed. `EaseIn*` starts slow and
/// accelerates. `EaseOut*` starts fast and decelerates.
/// `EaseInOut*` does both.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    /// Linear.
    Linear,

    // Sine
    /// Ease In.
    EaseIn,
    /// Ease Out.
    EaseOut,
    #[default]
    /// Ease In Out.
    EaseInOut,

    // Quad
    /// Ease In Quad.
    EaseInQuad,
    /// Ease Out Quad.
    EaseOutQuad,
    /// Ease In Out Quad.
    EaseInOutQuad,

    // Cubic
    /// Ease In Cubic.
    EaseInCubic,
    /// Ease Out Cubic.
    EaseOutCubic,
    /// Ease In Out Cubic.
    EaseInOutCubic,

    // Quart
    /// Ease In Quart.
    EaseInQuart,
    /// Ease Out Quart.
    EaseOutQuart,
    /// Ease In Out Quart.
    EaseInOutQuart,

    // Quint
    /// Ease In Quint.
    EaseInQuint,
    /// Ease Out Quint.
    EaseOutQuint,
    /// Ease In Out Quint.
    EaseInOutQuint,

    // Expo
    /// Ease In Expo.
    EaseInExpo,
    /// Ease Out Expo.
    EaseOutExpo,
    /// Ease In Out Expo.
    EaseInOutExpo,

    // Circ
    /// Ease In Circ.
    EaseInCirc,
    /// Ease Out Circ.
    EaseOutCirc,
    /// Ease In Out Circ.
    EaseInOutCirc,

    // Back
    /// Ease In Back.
    EaseInBack,
    /// Ease Out Back.
    EaseOutBack,
    /// Ease In Out Back.
    EaseInOutBack,

    // Elastic
    /// Ease In Elastic.
    EaseInElastic,
    /// Ease Out Elastic.
    EaseOutElastic,
    /// Ease In Out Elastic.
    EaseInOutElastic,

    // Bounce
    /// Ease In Bounce.
    EaseInBounce,
    /// Ease Out Bounce.
    EaseOutBounce,
    /// Ease In Out Bounce.
    EaseInOutBounce,

    /// Custom cubic bezier curve defined by two control points.
    CubicBezier(f32, f32, f32, f32),
}

impl PlushieType for Easing {
    fn wire_decode(value: &Value) -> Option<Self> {
        if let Some(s) = value.as_str() {
            return Self::from_snake_case(s);
        }
        // CubicBezier: {"cubic_bezier": [x1, y1, x2, y2]}
        let obj = value.as_object()?;
        let points = obj.get("cubic_bezier")?.as_array()?;
        if points.len() == 4 {
            let x1 = points[0].as_f64()? as f32;
            let y1 = points[1].as_f64()? as f32;
            let x2 = points[2].as_f64()? as f32;
            let y2 = points[3].as_f64()? as f32;
            Some(Self::CubicBezier(x1, y1, x2, y2))
        } else {
            None
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::CubicBezier(x1, y1, x2, y2) => {
                let mut map = PropMap::with_capacity(1);
                map.insert(
                    "cubic_bezier",
                    PropValue::Array(vec![
                        PropValue::F64(*x1 as f64),
                        PropValue::F64(*y1 as f64),
                        PropValue::F64(*x2 as f64),
                        PropValue::F64(*y2 as f64),
                    ]),
                );
                PropValue::Object(map)
            }
            other => PropValue::Str(other.to_snake_case().to_string()),
        }
    }

    fn type_name() -> &'static str {
        "easing"
    }
}

impl Easing {
    fn to_snake_case(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::EaseIn => "ease_in",
            Self::EaseOut => "ease_out",
            Self::EaseInOut => "ease_in_out",
            Self::EaseInQuad => "ease_in_quad",
            Self::EaseOutQuad => "ease_out_quad",
            Self::EaseInOutQuad => "ease_in_out_quad",
            Self::EaseInCubic => "ease_in_cubic",
            Self::EaseOutCubic => "ease_out_cubic",
            Self::EaseInOutCubic => "ease_in_out_cubic",
            Self::EaseInQuart => "ease_in_quart",
            Self::EaseOutQuart => "ease_out_quart",
            Self::EaseInOutQuart => "ease_in_out_quart",
            Self::EaseInQuint => "ease_in_quint",
            Self::EaseOutQuint => "ease_out_quint",
            Self::EaseInOutQuint => "ease_in_out_quint",
            Self::EaseInExpo => "ease_in_expo",
            Self::EaseOutExpo => "ease_out_expo",
            Self::EaseInOutExpo => "ease_in_out_expo",
            Self::EaseInCirc => "ease_in_circ",
            Self::EaseOutCirc => "ease_out_circ",
            Self::EaseInOutCirc => "ease_in_out_circ",
            Self::EaseInBack => "ease_in_back",
            Self::EaseOutBack => "ease_out_back",
            Self::EaseInOutBack => "ease_in_out_back",
            Self::EaseInElastic => "ease_in_elastic",
            Self::EaseOutElastic => "ease_out_elastic",
            Self::EaseInOutElastic => "ease_in_out_elastic",
            Self::EaseInBounce => "ease_in_bounce",
            Self::EaseOutBounce => "ease_out_bounce",
            Self::EaseInOutBounce => "ease_in_out_bounce",
            Self::CubicBezier(..) => "cubic_bezier",
        }
    }

    fn from_snake_case(s: &str) -> Option<Self> {
        match s {
            "linear" => Some(Self::Linear),
            "ease_in" => Some(Self::EaseIn),
            "ease_out" => Some(Self::EaseOut),
            "ease_in_out" => Some(Self::EaseInOut),
            "ease_in_quad" => Some(Self::EaseInQuad),
            "ease_out_quad" => Some(Self::EaseOutQuad),
            "ease_in_out_quad" => Some(Self::EaseInOutQuad),
            "ease_in_cubic" => Some(Self::EaseInCubic),
            "ease_out_cubic" => Some(Self::EaseOutCubic),
            "ease_in_out_cubic" => Some(Self::EaseInOutCubic),
            "ease_in_quart" => Some(Self::EaseInQuart),
            "ease_out_quart" => Some(Self::EaseOutQuart),
            "ease_in_out_quart" => Some(Self::EaseInOutQuart),
            "ease_in_quint" => Some(Self::EaseInQuint),
            "ease_out_quint" => Some(Self::EaseOutQuint),
            "ease_in_out_quint" => Some(Self::EaseInOutQuint),
            "ease_in_expo" => Some(Self::EaseInExpo),
            "ease_out_expo" => Some(Self::EaseOutExpo),
            "ease_in_out_expo" => Some(Self::EaseInOutExpo),
            "ease_in_circ" => Some(Self::EaseInCirc),
            "ease_out_circ" => Some(Self::EaseOutCirc),
            "ease_in_out_circ" => Some(Self::EaseInOutCirc),
            "ease_in_back" => Some(Self::EaseInBack),
            "ease_out_back" => Some(Self::EaseOutBack),
            "ease_in_out_back" => Some(Self::EaseInOutBack),
            "ease_in_elastic" => Some(Self::EaseInElastic),
            "ease_out_elastic" => Some(Self::EaseOutElastic),
            "ease_in_out_elastic" => Some(Self::EaseInOutElastic),
            "ease_in_bounce" => Some(Self::EaseInBounce),
            "ease_out_bounce" => Some(Self::EaseOutBounce),
            "ease_in_out_bounce" => Some(Self::EaseInOutBounce),
            _ => None,
        }
    }
}
