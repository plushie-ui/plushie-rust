//! Padding type for widget spacing.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;

/// Spacing between a widget's border and its content.
///
/// Wire format: a plain number for uniform padding (all four sides equal),
/// or an object with `top`, `right`, `bottom`, `left` keys for per-side
/// values. Missing keys default to `0.0`.
///
/// Construct uniformly, by axis, or per-side:
///
/// ```
/// use plushie_core::types::Padding;
///
/// let uniform = Padding::from(16.0);
/// let axis = Padding::from((16.0, 8.0));
/// let full = Padding::new(16.0, 8.0, 16.0, 8.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Padding {
    /// Top padding in logical pixels.
    pub top: f32,
    /// Right padding in logical pixels.
    pub right: f32,
    /// Bottom padding in logical pixels.
    pub bottom: f32,
    /// Left padding in logical pixels.
    pub left: f32,
}

impl Padding {
    /// Create padding with all four sides specified.
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Create uniform padding on all sides.
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create padding with vertical and horizontal values.
    pub fn axes(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Padding on the top side only.
    pub fn top(value: f32) -> Self {
        Self {
            top: value,
            ..Self::default()
        }
    }

    /// Padding on the right side only.
    pub fn right(value: f32) -> Self {
        Self {
            right: value,
            ..Self::default()
        }
    }

    /// Padding on the bottom side only.
    pub fn bottom(value: f32) -> Self {
        Self {
            bottom: value,
            ..Self::default()
        }
    }

    /// Padding on the left side only.
    pub fn left(value: f32) -> Self {
        Self {
            left: value,
            ..Self::default()
        }
    }

    /// Padding on the vertical sides (top and bottom).
    pub fn vertical(value: f32) -> Self {
        Self::axes(value, 0.0)
    }

    /// Padding on the horizontal sides (left and right).
    pub fn horizontal(value: f32) -> Self {
        Self::axes(0.0, value)
    }
}

impl PlushieType for Padding {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => {
                let v = decode_non_negative_f32(n.as_f64()?)?;
                Some(Self::all(v))
            }
            Value::Object(obj) => {
                let top = optional_non_negative_f32(obj.get("top"))?;
                let right = optional_non_negative_f32(obj.get("right"))?;
                let bottom = optional_non_negative_f32(obj.get("bottom"))?;
                let left = optional_non_negative_f32(obj.get("left"))?;
                Some(Self::new(top, right, bottom, left))
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        assert!(
            self.top >= 0.0 && self.right >= 0.0 && self.bottom >= 0.0 && self.left >= 0.0,
            "padding must be non-negative, got top={} right={} bottom={} left={}",
            self.top,
            self.right,
            self.bottom,
            self.left
        );
        if self.top == self.right && self.right == self.bottom && self.bottom == self.left {
            PropValue::F64(self.top as f64)
        } else {
            let mut m = PropMap::new();
            m.insert("top", PropValue::F64(self.top as f64));
            m.insert("right", PropValue::F64(self.right as f64));
            m.insert("bottom", PropValue::F64(self.bottom as f64));
            m.insert("left", PropValue::F64(self.left as f64));
            PropValue::Object(m)
        }
    }

    fn type_name() -> &'static str {
        "padding"
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::all(0.0)
    }
}

impl From<f32> for Padding {
    fn from(v: f32) -> Self {
        Self::all(v)
    }
}

impl From<i32> for Padding {
    fn from(v: i32) -> Self {
        Self::all(v as f32)
    }
}

impl From<(f32, f32)> for Padding {
    fn from((v, h): (f32, f32)) -> Self {
        Self::axes(v, h)
    }
}

impl From<(f32, f32, f32, f32)> for Padding {
    fn from((t, r, b, l): (f32, f32, f32, f32)) -> Self {
        Self::new(t, r, b, l)
    }
}

fn optional_non_negative_f32(value: Option<&Value>) -> Option<f32> {
    match value {
        Some(value) => decode_non_negative_f32(value.as_f64()?),
        None => Some(0.0),
    }
}

fn decode_non_negative_f32(value: f64) -> Option<f32> {
    let value = value as f32;
    (value.is_finite() && value >= 0.0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let p = Padding::default();
        assert_eq!(p, Padding::all(0.0));
    }

    #[test]
    #[should_panic(expected = "padding must be non-negative")]
    fn encode_rejects_negative_padding() {
        Padding::new(-1.0, 0.0, 0.0, 0.0).wire_encode();
    }

    #[test]
    fn decode_rejects_negative_padding() {
        assert_eq!(Padding::wire_decode(&serde_json::json!(-1.0)), None);
        assert_eq!(
            Padding::wire_decode(&serde_json::json!({"top": -1.0})),
            None
        );
    }
}
