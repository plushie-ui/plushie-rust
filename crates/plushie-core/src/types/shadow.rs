//! Shadow type for drop shadow effects.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;
use super::color::Color;

/// A drop shadow effect.
///
/// ## Wire format
///
/// ```json
/// {"color": "#000000", "offset": [5.0, 10.0], "blur_radius": 3.0}
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// Shadow color.
    pub color: Color,
    /// Horizontal offset in logical pixels (positive = right).
    pub offset_x: f32,
    /// Vertical offset in logical pixels (positive = down).
    pub offset_y: f32,
    /// Blur radius in logical pixels. 0.0 produces a sharp shadow.
    pub blur_radius: f32,
}

impl Shadow {
    /// Construct a new value.
    pub fn new() -> Self {
        Self {
            color: Color::black(),
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
        }
    }

    /// Set or construct `color`.
    pub fn color(mut self, c: impl Into<Color>) -> Self {
        self.color = c.into();
        self
    }

    /// Set or construct `offset`.
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    /// Set or construct `blur_radius`.
    pub fn blur_radius(mut self, r: f32) -> Self {
        self.blur_radius = r;
        self
    }
}

impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}

impl PlushieType for Shadow {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        let color = obj
            .get("color")
            .and_then(Color::wire_decode)
            .unwrap_or_else(Color::black);

        let (offset_x, offset_y) = if let Some(arr) = obj.get("offset").and_then(|v| v.as_array()) {
            let x = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            (x, y)
        } else {
            let x = obj.get("offset_x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            let y = obj.get("offset_y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            (x, y)
        };

        let blur_radius = obj
            .get("blur_radius")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;

        Some(Self {
            color,
            offset_x,
            offset_y,
            blur_radius,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        m.insert("color", self.color.wire_encode());
        m.insert(
            "offset",
            PropValue::Array(vec![
                PropValue::F64(self.offset_x as f64),
                PropValue::F64(self.offset_y as f64),
            ]),
        );
        m.insert("blur_radius", PropValue::F64(self.blur_radius as f64));
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "shadow"
    }
}
