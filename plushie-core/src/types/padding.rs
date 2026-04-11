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
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    /// Create padding with all four sides specified.
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }

    /// Create uniform padding on all sides.
    pub fn all(value: f32) -> Self {
        Self { top: value, right: value, bottom: value, left: value }
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
}

impl PlushieType for Padding {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => {
                let v = n.as_f64()? as f32;
                Some(Self::all(v))
            }
            Value::Object(obj) => {
                let top = obj.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let right = obj.get("right").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let bottom = obj.get("bottom").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let left = obj.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                Some(Self::new(top, right, bottom, left))
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let p = Padding::default();
        assert_eq!(p, Padding::all(0.0));
    }
}
