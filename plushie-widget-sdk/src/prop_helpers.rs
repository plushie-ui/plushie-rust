//! Public prop extraction helpers for widget authors.
//!
//! These functions provide a convenient API for reading typed values from
//! a props map ([`Props`]). Widget authors use these in their `render()`
//! and `prepare()` implementations instead of manually traversing
//! `serde_json::Value`.
//!
//! ```ignore
//! let props = &node.props;
//! let label = prop_str(props, "label").unwrap_or_default();
//! let size = prop_f32(props, "size").unwrap_or(14.0);
//! ```

use iced::{Color, ContentFit, Length, alignment};
use plushie_core::protocol::Props;
use serde_json::Value;

use crate::theming::parse_hex_color;

// ---------------------------------------------------------------------------
// Props type alias (deprecated)
// ---------------------------------------------------------------------------

/// A borrowed reference to a JSON object's field map, or `None` when
/// the node has no props (e.g. `Value::Null`).
///
/// Deprecated: use `&Props` directly. Kept for backward compatibility
/// with external widget code that hasn't migrated yet.
#[deprecated(note = "use &Props directly")]
pub type JsonProps<'a> = Option<&'a serde_json::Map<String, Value>>;

/// Access the Wire-mode JSON map from `&Props`. Returns `None` for
/// `Props::Typed`. Use this for fallback paths that need raw map access.
pub fn wire_map(props: &Props) -> Option<&serde_json::Map<String, Value>> {
    props.as_object()
}

/// Safely narrow an f64 to f32, clamping values outside f32's range
/// instead of silently producing infinity.
///
/// For UI property values (dimensions, colors, text sizes) the f64 values
/// from JSON are always within f32 range, so this is pure defense-in-depth.
#[inline]
pub fn f64_to_f32(v: f64) -> f32 {
    v.clamp(f32::MIN as f64, f32::MAX as f64) as f32
}

// ---------------------------------------------------------------------------
// Core prop helpers
// ---------------------------------------------------------------------------

/// Get a string prop value.
pub fn prop_str(props: &Props, key: &str) -> Option<String> {
    if let Props::Typed(_) = props {
        return props.get_str(key).map(|s| s.to_string());
    }
    let val = props.as_object()?.get(key)?;
    match val.as_str() {
        Some(s) => Some(s.to_owned()),
        None => {
            log::trace!("prop '{}': expected string, got {:?}", key, val);
            None
        }
    }
}

/// Get an f32 prop value. Accepts both JSON numbers and numeric strings.
pub fn prop_f32(props: &Props, key: &str) -> Option<f32> {
    if let Props::Typed(_) = props {
        return props.get_f32(key);
    }
    let val = props.as_object()?.get(key)?;
    match val {
        Value::Number(n) => match n.as_f64() {
            Some(f) => Some(f64_to_f32(f)),
            None => {
                log::trace!("prop '{}': number failed f64 conversion: {:?}", key, val);
                None
            }
        },
        Value::String(s) => match s.trim().parse::<f32>().ok().filter(|f| f.is_finite()) {
            Some(f) => Some(f),
            None => {
                log::trace!("prop '{}': string not parseable as f32: {:?}", key, s);
                None
            }
        },
        _ => {
            log::trace!("prop '{}': expected f64, got {:?}", key, val);
            None
        }
    }
}

/// Get an f64 prop value. Accepts both JSON numbers and numeric strings.
pub fn prop_f64(props: &Props, key: &str) -> Option<f64> {
    if let Props::Typed(_) = props {
        return props.get_f64(key);
    }
    let val = props.as_object()?.get(key)?;
    match val {
        Value::Number(n) => match n.as_f64() {
            Some(f) => Some(f),
            None => {
                log::trace!("prop '{}': number failed f64 conversion: {:?}", key, val);
                None
            }
        },
        Value::String(s) => match s.trim().parse::<f64>().ok().filter(|f| f.is_finite()) {
            Some(f) => Some(f),
            None => {
                log::trace!("prop '{}': string not parseable as f64: {:?}", key, s);
                None
            }
        },
        _ => {
            log::trace!("prop '{}': expected f64, got {:?}", key, val);
            None
        }
    }
}

/// Get a u32 prop value. Accepts JSON numbers and numeric strings.
pub fn prop_u32(props: &Props, key: &str) -> Option<u32> {
    if let Props::Typed(_) = props {
        return props.get_u64(key).and_then(|v| u32::try_from(v).ok());
    }
    let val = props.as_object()?.get(key)?;
    match val {
        Value::Number(n) => match n.as_u64().and_then(|v| u32::try_from(v).ok()) {
            Some(u) => Some(u),
            None => {
                log::trace!("prop '{}': expected u32, got {:?}", key, val);
                None
            }
        },
        Value::String(s) => match s.trim().parse::<u32>() {
            Ok(u) => Some(u),
            Err(_) => {
                log::trace!("prop '{}': string not parseable as u32: {:?}", key, s);
                None
            }
        },
        _ => {
            log::trace!("prop '{}': expected u32, got {:?}", key, val);
            None
        }
    }
}

/// Get a u64 prop value. Accepts JSON numbers and numeric strings.
pub fn prop_u64(props: &Props, key: &str) -> Option<u64> {
    if let Props::Typed(_) = props {
        return props.get_u64(key);
    }
    let val = props.as_object()?.get(key)?;
    match val {
        Value::Number(n) => match n.as_u64() {
            Some(u) => Some(u),
            None => {
                log::trace!("prop '{}': expected u64, got {:?}", key, val);
                None
            }
        },
        Value::String(s) => match s.trim().parse::<u64>() {
            Ok(u) => Some(u),
            Err(_) => {
                log::trace!("prop '{}': string not parseable as u64: {:?}", key, s);
                None
            }
        },
        _ => {
            log::trace!("prop '{}': expected u64, got {:?}", key, val);
            None
        }
    }
}

/// Get a usize prop value. Accepts JSON numbers and numeric strings.
pub fn prop_usize(props: &Props, key: &str) -> Option<usize> {
    prop_u64(props, key).and_then(|v| usize::try_from(v).ok())
}

/// Get an i32 prop value. Accepts both JSON numbers and numeric strings.
pub fn prop_i32(props: &Props, key: &str) -> Option<i32> {
    if let Props::Typed(_) = props {
        return props.get_i64(key).and_then(|v| i32::try_from(v).ok());
    }
    let val = props.as_object()?.get(key)?;
    match val {
        Value::Number(n) => match n.as_i64().and_then(|v| i32::try_from(v).ok()) {
            Some(i) => Some(i),
            None => {
                log::trace!("prop '{}': expected i32, got {:?}", key, val);
                None
            }
        },
        Value::String(s) => match s.trim().parse::<i32>() {
            Ok(i) => Some(i),
            Err(_) => {
                log::trace!("prop '{}': string not parseable as i32: {:?}", key, s);
                None
            }
        },
        _ => {
            log::trace!("prop '{}': expected i32, got {:?}", key, val);
            None
        }
    }
}

/// Get an i64 prop value. Accepts JSON numbers and numeric strings.
pub fn prop_i64(props: &Props, key: &str) -> Option<i64> {
    if let Props::Typed(_) = props {
        return props.get_i64(key);
    }
    let val = props.as_object()?.get(key)?;
    match val {
        Value::Number(n) => match n.as_i64() {
            Some(i) => Some(i),
            None => {
                log::trace!("prop '{}': expected i64, got {:?}", key, val);
                None
            }
        },
        Value::String(s) => match s.trim().parse::<i64>() {
            Ok(i) => Some(i),
            Err(_) => {
                log::trace!("prop '{}': string not parseable as i64: {:?}", key, s);
                None
            }
        },
        _ => {
            log::trace!("prop '{}': expected i64, got {:?}", key, val);
            None
        }
    }
}

/// Get a boolean prop value.
pub fn prop_bool(props: &Props, key: &str) -> Option<bool> {
    if let Props::Typed(_) = props {
        return props.get_bool(key);
    }
    let val = props.as_object()?.get(key)?;
    match val.as_bool() {
        Some(b) => Some(b),
        None => {
            log::trace!("prop '{}': expected bool, got {:?}", key, val);
            None
        }
    }
}

/// Get a boolean prop value with a default.
pub fn prop_bool_default(props: &Props, key: &str, default: bool) -> bool {
    prop_bool(props, key).unwrap_or(default)
}

/// Get a Length prop value, returning `fallback` when absent or unparseable.
pub fn prop_length(props: &Props, key: &str, fallback: Length) -> Length {
    props
        .get_value(key)
        .as_ref()
        .and_then(|v| match value_to_length(v) {
            Some(len) => Some(len),
            None => {
                log::trace!("prop '{}': expected length, got {:?}", key, v);
                None
            }
        })
        .unwrap_or(fallback)
}

/// Parse a "range" prop as `[min, max]` into an inclusive `f32` range.
pub fn prop_range_f32(props: &Props) -> std::ops::RangeInclusive<f32> {
    props
        .get_value("range")
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            let mut min = f64_to_f32(arr.first()?.as_f64()?);
            let mut max = f64_to_f32(arr.get(1)?.as_f64()?);
            if min > max {
                log::warn!("prop 'range': min ({min}) > max ({max}), swapping");
                std::mem::swap(&mut min, &mut max);
            }
            Some(min..=max)
        })
        .unwrap_or(0.0..=100.0)
}

/// Parse a "range" prop as `[min, max]` into an inclusive `f64` range.
pub fn prop_range_f64(props: &Props) -> std::ops::RangeInclusive<f64> {
    props
        .get_value("range")
        .as_ref()
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            let mut min = arr.first()?.as_f64()?;
            let mut max = arr.get(1)?.as_f64()?;
            if min > max {
                log::warn!("prop 'range': min ({min}) > max ({max}), swapping");
                std::mem::swap(&mut min, &mut max);
            }
            Some(min..=max)
        })
        .unwrap_or(0.0..=100.0)
}

/// Parse a color prop to `iced::Color`.
///
/// Accepts hex strings: `"#RRGGBB"` or `"#RRGGBBAA"`.
pub fn prop_color(props: &Props, key: &str) -> Option<Color> {
    let s = prop_str(props, key)?;
    match parse_hex_color(&s) {
        Some(c) => Some(c),
        None => {
            log::trace!("prop '{key}': invalid hex color: {s:?}");
            None
        }
    }
}

/// Get an array of f32 values from a prop.
/// Non-numeric elements are silently dropped with a warning.
pub fn prop_f32_array(props: &Props, key: &str) -> Option<Vec<f32>> {
    let val = props.get_value(key)?;
    match val.as_array() {
        Some(arr) => {
            let mut result = Vec::with_capacity(arr.len());
            for (i, v) in arr.iter().enumerate() {
                match v.as_f64() {
                    Some(f) => result.push(f64_to_f32(f)),
                    None => {
                        log::warn!(
                            "prop '{}': dropping non-numeric element at index {}: {:?}",
                            key,
                            i,
                            v
                        );
                    }
                }
            }
            Some(result)
        }
        None => {
            log::trace!("prop '{}': expected array, got {:?}", key, val);
            None
        }
    }
}

/// Parse a horizontal alignment prop.
pub fn prop_horizontal_alignment(props: &Props, key: &str) -> alignment::Horizontal {
    props
        .get_str(key)
        .and_then(value_to_horizontal_alignment)
        .unwrap_or(alignment::Horizontal::Left)
}

/// Parse a vertical alignment prop.
pub fn prop_vertical_alignment(props: &Props, key: &str) -> alignment::Vertical {
    props
        .get_str(key)
        .and_then(value_to_vertical_alignment)
        .unwrap_or(alignment::Vertical::Top)
}

/// Parse a content-fit prop.
pub fn prop_content_fit(props: &Props) -> Option<ContentFit> {
    let s = prop_str(props, "content_fit")?;
    match s.to_ascii_lowercase().as_str() {
        "contain" => Some(ContentFit::Contain),
        "cover" => Some(ContentFit::Cover),
        "fill" => Some(ContentFit::Fill),
        "none" => Some(ContentFit::None),
        "scale_down" => Some(ContentFit::ScaleDown),
        _ => {
            log::trace!("prop 'content_fit': unrecognized value: {:?}", s);
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Value conversion helpers (also public for advanced use)
// ---------------------------------------------------------------------------

/// Convert a JSON value to an iced Length.
pub fn value_to_length(val: &Value) -> Option<Length> {
    match val {
        Value::Number(n) => n
            .as_f64()
            .map(f64_to_f32)
            .filter(|v| *v >= 0.0)
            .map(Length::Fixed),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "fill" | "full" | "expand" | "stretch" => Some(Length::Fill),
            "shrink" | "auto" | "fit" => Some(Length::Shrink),
            other => other
                .parse::<f32>()
                .ok()
                .filter(|v| *v >= 0.0)
                .map(Length::Fixed),
        },
        Value::Object(obj) => {
            if let Some(n) = obj.get("fill_portion").and_then(|v| v.as_u64()) {
                Some(Length::FillPortion(u16::try_from(n).unwrap_or(1).max(1)))
            } else {
                Some(Length::Shrink)
            }
        }
        _ => None,
    }
}

pub fn value_to_horizontal_alignment(s: &str) -> Option<alignment::Horizontal> {
    match s.trim().to_ascii_lowercase().as_str() {
        "left" => Some(alignment::Horizontal::Left),
        "center" => Some(alignment::Horizontal::Center),
        "right" => Some(alignment::Horizontal::Right),
        other => {
            log::warn!("unknown horizontal alignment: {other:?}");
            None
        }
    }
}

pub fn value_to_vertical_alignment(s: &str) -> Option<alignment::Vertical> {
    match s.trim().to_ascii_lowercase().as_str() {
        "top" => Some(alignment::Vertical::Top),
        "center" => Some(alignment::Vertical::Center),
        "bottom" => Some(alignment::Vertical::Bottom),
        other => {
            log::warn!("unknown vertical alignment: {other:?}");
            None
        }
    }
}

/// Get an f64 prop value. Accepts JSON numbers and numeric strings.
/// Non-numeric elements are silently dropped with a warning.
pub fn prop_f64_array(props: &Props, key: &str) -> Option<Vec<f64>> {
    let val = props.get_value(key)?;
    match val.as_array() {
        Some(arr) => {
            let mut result = Vec::with_capacity(arr.len());
            for (i, v) in arr.iter().enumerate() {
                match v.as_f64() {
                    Some(f) => result.push(f),
                    None => {
                        log::warn!(
                            "prop '{}': dropping non-numeric element at index {}: {:?}",
                            key,
                            i,
                            v
                        );
                    }
                }
            }
            Some(result)
        }
        None => {
            log::trace!("prop '{}': expected array, got {:?}", key, val);
            None
        }
    }
}

/// Get an array of string values from a prop.
pub fn prop_str_array(props: &Props, key: &str) -> Option<Vec<String>> {
    let val = props.get_value(key)?;
    match val.as_array() {
        Some(arr) => Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
        ),
        None => {
            log::trace!("prop '{}': expected array, got {:?}", key, val);
            None
        }
    }
}

/// Get a reference to a JSON object prop (Wire variant only).
///
/// Returns `None` for `Props::Typed`. For code that needs to work
/// with both variants, use `props.get_value(key)` instead.
pub fn prop_object<'a>(props: &'a Props, key: &str) -> Option<&'a serde_json::Map<String, Value>> {
    let val = props.as_object()?.get(key)?;
    match val.as_object() {
        Some(obj) => Some(obj),
        None => {
            log::trace!("prop '{}': expected object, got {:?}", key, val);
            None
        }
    }
}

/// Get a reference to a raw JSON value prop (Wire variant only).
///
/// Returns `None` for `Props::Typed`. For code that needs to work
/// with both variants, use `props.get_value(key)` instead.
pub fn prop_value<'a>(props: &'a Props, key: &str) -> Option<&'a Value> {
    props.as_object()?.get(key)
}

/// Parse padding from a `"padding"` prop.
///
/// Supports three formats:
/// - `"padding": 10` -- uniform padding (all four sides)
/// - `"padding": {"top": 10, "right": 5, "bottom": 10, "left": 5}` -- per-side
/// - Individual `"padding_top"`, `"padding_right"`, etc. keys (legacy)
///
/// Returns `None` if no padding props are present, preserving iced defaults.
/// Negative values are clamped to `0.0` in the object and uniform formats.
pub fn prop_padding(props: &Props) -> Option<iced::Padding> {
    crate::widget::parse_padding_value(props)
}

// ---------------------------------------------------------------------------
// Animated prop helpers
// ---------------------------------------------------------------------------
// These check the interpolated_props cache (populated by the TransitionManager)
// before falling back to the tree's props. Use these in widget render functions
// for props that can be animated.

/// Get an f32 prop value, checking the animation cache first.
///
/// If the TransitionManager is actively interpolating this prop, returns
/// the current animated value. Otherwise falls back to the tree prop.
pub fn prop_animated_f32(
    interpolated: &std::collections::HashMap<String, serde_json::Map<String, Value>>,
    node_id: &str,
    props: &Props,
    key: &str,
) -> Option<f32> {
    // Check interpolated cache first
    if let Some(overrides) = interpolated.get(node_id)
        && let Some(val) = overrides.get(key)
    {
        return val.as_f64().map(f64_to_f32);
    }
    // Fall back to tree props (skip descriptor maps)
    if let Some(val) = props.get(key) {
        if val.is_object() {
            // This is likely an animation descriptor -- the renderer hasn't
            // started animating yet (first frame) or it just completed.
            // Return None so the widget uses its default.
            return None;
        }
    }
    prop_f32(props, key)
}

/// Get a color prop value, checking the animation cache first.
pub fn prop_animated_color(
    interpolated: &std::collections::HashMap<String, serde_json::Map<String, Value>>,
    node_id: &str,
    props: &Props,
    key: &str,
) -> Option<Color> {
    if let Some(overrides) = interpolated.get(node_id)
        && let Some(val) = overrides.get(key)
    {
        return val.as_str().and_then(parse_hex_color);
    }
    if let Some(val) = props.get(key) {
        if val.is_object() {
            return None;
        }
    }
    prop_color(props, key)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_props(val: Value) -> Props {
        Props::Wire(val)
    }

    #[test]
    fn test_prop_str() {
        let p = make_props(json!({"label": "hello"}));
        assert_eq!(prop_str(&p, "label"), Some("hello".to_string()));
        assert_eq!(prop_str(&p, "missing"), None);
    }

    #[test]
    fn test_prop_str_non_string() {
        let p = make_props(json!({"num": 42}));
        assert_eq!(prop_str(&p, "num"), None);
    }

    #[test]
    fn test_prop_f32_number() {
        let p = make_props(json!({"size": 14.5}));
        let v = prop_f32(&p, "size").unwrap();
        assert!((v - 14.5).abs() < 0.001);
    }

    #[test]
    fn test_prop_f32_string() {
        let p = make_props(json!({"size": "14.5"}));
        let v = prop_f32(&p, "size").unwrap();
        assert!((v - 14.5).abs() < 0.001);
    }

    #[test]
    fn test_prop_f32_missing() {
        let p = make_props(json!({}));
        assert!(prop_f32(&p, "size").is_none());
    }

    #[test]
    fn test_prop_f32_wrong_type() {
        let p = make_props(json!({"size": true}));
        assert!(prop_f32(&p, "size").is_none());
    }

    #[test]
    fn test_prop_f64_number() {
        let p = make_props(json!({"value": 99.9}));
        let v = prop_f64(&p, "value").unwrap();
        assert!((v - 99.9).abs() < 0.0001);
    }

    #[test]
    fn test_prop_f64_string() {
        let p = make_props(json!({"value": "99.9"}));
        let v = prop_f64(&p, "value").unwrap();
        assert!((v - 99.9).abs() < 0.0001);
    }

    // -- prop_u32 --

    #[test]
    fn test_prop_u32_number() {
        let p = make_props(json!({"count": 42}));
        assert_eq!(prop_u32(&p, "count"), Some(42));
    }

    #[test]
    fn test_prop_u32_string() {
        let p = make_props(json!({"count": "123"}));
        assert_eq!(prop_u32(&p, "count"), Some(123));
    }

    #[test]
    fn test_prop_u32_missing() {
        let p = make_props(json!({}));
        assert_eq!(prop_u32(&p, "count"), None);
    }

    #[test]
    fn test_prop_u32_negative() {
        let p = make_props(json!({"count": -1}));
        assert_eq!(prop_u32(&p, "count"), None);
    }

    #[test]
    fn test_prop_u32_overflow() {
        let p = make_props(json!({"count": 5_000_000_000u64}));
        assert_eq!(prop_u32(&p, "count"), None);
    }

    // -- prop_u64 --

    #[test]
    fn test_prop_u64_number() {
        let p = make_props(json!({"big": 9_000_000_000u64}));
        assert_eq!(prop_u64(&p, "big"), Some(9_000_000_000));
    }

    #[test]
    fn test_prop_u64_string() {
        let p = make_props(json!({"big": "999"}));
        assert_eq!(prop_u64(&p, "big"), Some(999));
    }

    #[test]
    fn test_prop_u64_missing() {
        let p = make_props(json!({}));
        assert_eq!(prop_u64(&p, "big"), None);
    }

    #[test]
    fn test_prop_u64_negative() {
        let p = make_props(json!({"big": -1}));
        assert_eq!(prop_u64(&p, "big"), None);
    }

    // -- prop_usize --

    #[test]
    fn test_prop_usize_number() {
        let p = make_props(json!({"idx": 7}));
        assert_eq!(prop_usize(&p, "idx"), Some(7));
    }

    #[test]
    fn test_prop_usize_string() {
        let p = make_props(json!({"idx": "42"}));
        assert_eq!(prop_usize(&p, "idx"), Some(42));
    }

    #[test]
    fn test_prop_usize_missing() {
        let p = make_props(json!({}));
        assert_eq!(prop_usize(&p, "idx"), None);
    }

    // -- prop_i32 --

    #[test]
    fn test_prop_i32_positive() {
        let p = make_props(json!({"x": 42}));
        assert_eq!(prop_i32(&p, "x"), Some(42));
    }

    #[test]
    fn test_prop_i32_negative() {
        let p = make_props(json!({"x": -100}));
        assert_eq!(prop_i32(&p, "x"), Some(-100));
    }

    #[test]
    fn test_prop_i32_string() {
        let p = make_props(json!({"x": "-7"}));
        assert_eq!(prop_i32(&p, "x"), Some(-7));
    }

    #[test]
    fn test_prop_i32_missing() {
        let p = make_props(json!({}));
        assert_eq!(prop_i32(&p, "x"), None);
    }

    #[test]
    fn test_prop_i32_overflow() {
        let p = make_props(json!({"x": 5_000_000_000i64}));
        assert_eq!(prop_i32(&p, "x"), None);
    }

    // -- prop_i64 --

    #[test]
    fn test_prop_i64_positive() {
        let p = make_props(json!({"offset": 100}));
        assert_eq!(prop_i64(&p, "offset"), Some(100));
    }

    #[test]
    fn test_prop_i64_negative() {
        let p = make_props(json!({"offset": -50}));
        assert_eq!(prop_i64(&p, "offset"), Some(-50));
    }

    #[test]
    fn test_prop_i64_string() {
        let p = make_props(json!({"offset": "-99"}));
        assert_eq!(prop_i64(&p, "offset"), Some(-99));
    }

    #[test]
    fn test_prop_i64_missing() {
        let p = make_props(json!({}));
        assert_eq!(prop_i64(&p, "offset"), None);
    }

    #[test]
    fn test_prop_bool() {
        let p = make_props(json!({"disabled": true}));
        assert_eq!(prop_bool(&p, "disabled"), Some(true));
        assert_eq!(prop_bool(&p, "missing"), None);
    }

    #[test]
    fn test_prop_bool_default() {
        let p = make_props(json!({"disabled": true}));
        assert!(prop_bool_default(&p, "disabled", false));
        assert!(!prop_bool_default(&p, "missing", false));
        assert!(prop_bool_default(&p, "missing", true));
    }

    #[test]
    fn test_prop_bool_wrong_type() {
        let p = make_props(json!({"disabled": "yes"}));
        assert_eq!(prop_bool(&p, "disabled"), None);
    }

    #[test]
    fn test_prop_length_fixed() {
        let p = make_props(json!({"width": 100}));
        let len = prop_length(&p, "width", Length::Shrink);
        assert!(matches!(len, Length::Fixed(v) if (v - 100.0).abs() < 0.001));
    }

    #[test]
    fn test_prop_length_fill() {
        let p = make_props(json!({"width": "fill"}));
        let len = prop_length(&p, "width", Length::Shrink);
        assert!(matches!(len, Length::Fill));
    }

    #[test]
    fn test_prop_length_fallback() {
        let p = make_props(json!({}));
        let len = prop_length(&p, "width", Length::Shrink);
        assert!(matches!(len, Length::Shrink));
    }

    #[test]
    fn test_prop_range_f32_present() {
        let p = make_props(json!({"range": [10.0, 50.0]}));
        let r = prop_range_f32(&p);
        assert_eq!(*r.start(), 10.0);
        assert_eq!(*r.end(), 50.0);
    }

    #[test]
    fn test_prop_range_f32_default() {
        let p = make_props(json!({}));
        let r = prop_range_f32(&p);
        assert_eq!(*r.start(), 0.0);
        assert_eq!(*r.end(), 100.0);
    }

    #[test]
    fn test_prop_range_f64_present() {
        let p = make_props(json!({"range": [1.0, 2.0]}));
        let r = prop_range_f64(&p);
        assert_eq!(*r.start(), 1.0);
        assert_eq!(*r.end(), 2.0);
    }

    #[test]
    fn test_prop_color_valid() {
        let p = make_props(json!({"bg": "#ff0000"}));
        let c = prop_color(&p, "bg").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
        assert!(c.b.abs() < 0.01);
    }

    #[test]
    fn test_prop_color_with_alpha() {
        let p = make_props(json!({"bg": "#ff000080"}));
        let c = prop_color(&p, "bg").unwrap();
        assert!((c.a - 0.502).abs() < 0.01);
    }

    #[test]
    fn test_prop_color_invalid() {
        let p = make_props(json!({"bg": "not-a-color"}));
        assert!(prop_color(&p, "bg").is_none());
    }

    #[test]
    fn test_prop_color_rejects_object() {
        let p = make_props(json!({"color": {"r": 1.0, "g": 0.0, "b": 0.0}}));
        assert_eq!(prop_color(&p, "color"), None);
    }

    #[test]
    fn test_prop_color_missing() {
        let p = make_props(json!({}));
        assert!(prop_color(&p, "bg").is_none());
    }

    #[test]
    fn test_prop_f32_array() {
        let p = make_props(json!({"data": [1.0, 2.5, 3.0]}));
        let arr = prop_f32_array(&p, "data").unwrap();
        assert_eq!(arr.len(), 3);
        assert!((arr[0] - 1.0).abs() < 0.001);
        assert!((arr[1] - 2.5).abs() < 0.001);
        assert!((arr[2] - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_prop_f32_array_empty() {
        let p = make_props(json!({"data": []}));
        let arr = prop_f32_array(&p, "data").unwrap();
        assert!(arr.is_empty());
    }

    #[test]
    fn test_prop_f32_array_missing() {
        let p = make_props(json!({}));
        assert!(prop_f32_array(&p, "data").is_none());
    }

    #[test]
    fn test_prop_f32_array_not_array() {
        let p = make_props(json!({"data": "nope"}));
        assert!(prop_f32_array(&p, "data").is_none());
    }

    #[test]
    fn test_prop_horizontal_alignment() {
        let p = make_props(json!({"align": "center"}));
        assert!(matches!(
            prop_horizontal_alignment(&p, "align"),
            alignment::Horizontal::Center
        ));
    }

    #[test]
    fn test_prop_horizontal_alignment_default() {
        let p = make_props(json!({}));
        assert!(matches!(
            prop_horizontal_alignment(&p, "align"),
            alignment::Horizontal::Left
        ));
    }

    #[test]
    fn test_prop_vertical_alignment() {
        let p = make_props(json!({"valign": "bottom"}));
        assert!(matches!(
            prop_vertical_alignment(&p, "valign"),
            alignment::Vertical::Bottom
        ));
    }

    #[test]
    fn test_prop_content_fit() {
        let p = make_props(json!({"content_fit": "cover"}));
        assert_eq!(prop_content_fit(&p), Some(ContentFit::Cover));
    }

    #[test]
    fn test_prop_content_fit_missing() {
        let p = make_props(json!({}));
        assert_eq!(prop_content_fit(&p), None);
    }

    #[test]
    fn test_value_to_length_fill_portion() {
        let val = json!({"fill_portion": 3});
        let len = value_to_length(&val).unwrap();
        assert!(matches!(len, Length::FillPortion(3)));
    }

    #[test]
    fn test_prop_f32_string_nan() {
        // "NaN" strings are rejected -- non-finite values return None.
        let p = make_props(json!({"size": "NaN"}));
        assert!(prop_f32(&p, "size").is_none());
    }

    #[test]
    fn test_prop_f32_string_infinity() {
        // "Infinity" strings are rejected -- non-finite values return None.
        let p = make_props(json!({"size": "Infinity"}));
        assert!(prop_f32(&p, "size").is_none());
    }

    #[test]
    fn test_prop_f32_empty_string() {
        let p = make_props(json!({"size": ""}));
        assert!(prop_f32(&p, "size").is_none());
    }

    #[test]
    fn test_prop_u32_non_numeric_string() {
        let p = make_props(json!({"count": "not_a_number"}));
        assert_eq!(prop_u32(&p, "count"), None);
    }

    #[test]
    fn test_empty_props() {
        let p = Props::default();
        assert!(prop_str(&p, "anything").is_none());
        assert!(prop_f32(&p, "anything").is_none());
        assert!(prop_bool(&p, "anything").is_none());
    }

    // -- prop_f64_array --

    #[test]
    fn test_prop_f64_array() {
        let p = make_props(json!({"data": [1.0, 2.5, 3.0]}));
        let arr = prop_f64_array(&p, "data").unwrap();
        assert_eq!(arr.len(), 3);
        assert!((arr[0] - 1.0).abs() < 0.0001);
        assert!((arr[1] - 2.5).abs() < 0.0001);
        assert!((arr[2] - 3.0).abs() < 0.0001);
    }

    #[test]
    fn test_prop_f64_array_skips_non_numeric() {
        let p = make_props(json!({"data": [1.0, "nope", 3.0]}));
        let arr = prop_f64_array(&p, "data").unwrap();
        assert_eq!(arr.len(), 2);
        assert!((arr[0] - 1.0).abs() < 0.0001);
        assert!((arr[1] - 3.0).abs() < 0.0001);
    }

    #[test]
    fn test_prop_f64_array_missing() {
        let p = make_props(json!({}));
        assert!(prop_f64_array(&p, "data").is_none());
    }

    // -- prop_str_array --

    #[test]
    fn test_prop_str_array() {
        let p = make_props(json!({"tags": ["a", "b", "c"]}));
        let arr = prop_str_array(&p, "tags").unwrap();
        assert_eq!(arr, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_prop_str_array_skips_non_string() {
        let p = make_props(json!({"tags": ["a", 42, "c"]}));
        let arr = prop_str_array(&p, "tags").unwrap();
        assert_eq!(arr, vec!["a", "c"]);
    }

    #[test]
    fn test_prop_str_array_missing() {
        let p = make_props(json!({}));
        assert!(prop_str_array(&p, "tags").is_none());
    }

    // -- prop_object --

    #[test]
    fn test_prop_object() {
        let p = make_props(json!({"style": {"color": "red"}}));
        let obj = prop_object(&p, "style").unwrap();
        assert_eq!(obj.get("color").and_then(|v| v.as_str()), Some("red"));
    }

    #[test]
    fn test_prop_object_missing() {
        let p = make_props(json!({}));
        assert!(prop_object(&p, "style").is_none());
    }

    #[test]
    fn test_prop_object_wrong_type() {
        let p = make_props(json!({"style": "not an object"}));
        assert!(prop_object(&p, "style").is_none());
    }

    // -- prop_value --

    #[test]
    fn test_prop_value_string() {
        let p = make_props(json!({"x": "hello"}));
        let v = prop_value(&p, "x").unwrap();
        assert_eq!(v.as_str(), Some("hello"));
    }

    #[test]
    fn test_prop_value_number() {
        let p = make_props(json!({"x": 42}));
        let v = prop_value(&p, "x").unwrap();
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn test_prop_value_missing() {
        let p = make_props(json!({}));
        assert!(prop_value(&p, "x").is_none());
    }

    // -- Property-based tests -------------------------------------------------

    mod proptest_prop_helpers {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// prop_f32 should never panic for any f64 value and should
            /// return Some for all finite inputs.
            #[test]
            fn prop_f32_never_panics(val: f64) {
                let p = make_props(json!({"v": val}));
                let result = prop_f32(&p, "v");
                if val.is_finite() {
                    prop_assert!(result.is_some(), "expected Some for finite {val}");
                    let f = result.unwrap();
                    prop_assert!(f.is_finite(), "expected finite f32 for finite input {val}");
                } else {
                    // NaN and Infinity become JSON null via serde_json,
                    // so prop_f32 returns None -- that's correct.
                }
            }
        }
    }
}
