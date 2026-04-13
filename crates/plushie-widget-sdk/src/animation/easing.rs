//! Easing resolution: maps wire-format strings to lilt::Easing variants.
//!
//! Cubic bezier is handled separately in `timed.rs`; this module only
//! resolves named easings.

use iced::animation::Easing;

/// Resolves a wire-format easing value to a lilt Easing.
///
/// Accepts a string (named easing). Cubic bezier objects are handled
/// by the caller (parse_timed extracts the control points).
pub fn resolve(value: &serde_json::Value) -> Easing {
    match value {
        serde_json::Value::String(s) => from_str(s),
        _ => Easing::EaseInOut,
    }
}

/// Maps a snake_case easing name to the corresponding lilt variant.
pub fn from_str(s: &str) -> Easing {
    match s {
        "linear" => Easing::Linear,
        "ease_in" => Easing::EaseIn,
        "ease_out" => Easing::EaseOut,
        "ease_in_out" => Easing::EaseInOut,
        "ease_in_quad" => Easing::EaseInQuad,
        "ease_out_quad" => Easing::EaseOutQuad,
        "ease_in_out_quad" => Easing::EaseInOutQuad,
        "ease_in_cubic" => Easing::EaseInCubic,
        "ease_out_cubic" => Easing::EaseOutCubic,
        "ease_in_out_cubic" => Easing::EaseInOutCubic,
        "ease_in_quart" => Easing::EaseInQuart,
        "ease_out_quart" => Easing::EaseOutQuart,
        "ease_in_out_quart" => Easing::EaseInOutQuart,
        "ease_in_quint" => Easing::EaseInQuint,
        "ease_out_quint" => Easing::EaseOutQuint,
        "ease_in_out_quint" => Easing::EaseInOutQuint,
        "ease_in_expo" => Easing::EaseInExpo,
        "ease_out_expo" => Easing::EaseOutExpo,
        "ease_in_out_expo" => Easing::EaseInOutExpo,
        "ease_in_circ" => Easing::EaseInCirc,
        "ease_out_circ" => Easing::EaseOutCirc,
        "ease_in_out_circ" => Easing::EaseInOutCirc,
        "ease_in_back" => Easing::EaseInBack,
        "ease_out_back" => Easing::EaseOutBack,
        "ease_in_out_back" => Easing::EaseInOutBack,
        "ease_in_elastic" => Easing::EaseInElastic,
        "ease_out_elastic" => Easing::EaseOutElastic,
        "ease_in_out_elastic" => Easing::EaseInOutElastic,
        "ease_in_bounce" => Easing::EaseInBounce,
        "ease_out_bounce" => Easing::EaseOutBounce,
        "ease_in_out_bounce" => Easing::EaseInOutBounce,
        _ => Easing::EaseInOut,
    }
}
