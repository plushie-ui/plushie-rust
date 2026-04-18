//! Border and radius types.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;
use super::color::Color;

/// Corner radius for a border: uniform or per-corner.
///
/// ## Wire format
///
/// A plain number for uniform radius, or an object with per-corner keys:
///
/// ```json
/// 8
/// {"top_left": 8, "top_right": 4, "bottom_right": 8, "bottom_left": 4}
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Radius {
    /// Same radius on all four corners, in logical pixels.
    Uniform(f32),
    /// Individual radius for each corner, in logical pixels.
    PerCorner {
        /// Top-left corner radius in pixels.
        top_left: f32,
        /// Top-right corner radius in pixels.
        top_right: f32,
        /// Bottom-right corner radius in pixels.
        bottom_right: f32,
        /// Bottom-left corner radius in pixels.
        bottom_left: f32,
    },
}

impl Default for Radius {
    fn default() -> Self {
        Self::Uniform(0.0)
    }
}

impl From<f32> for Radius {
    fn from(r: f32) -> Self {
        Self::Uniform(r)
    }
}

impl PlushieType for Radius {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => Some(Self::Uniform(n.as_f64()? as f32)),
            Value::Object(obj) => {
                let tl = obj.get("top_left").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let tr = obj.get("top_right").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let br = obj
                    .get("bottom_right")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let bl = obj
                    .get("bottom_left")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                Some(Self::PerCorner {
                    top_left: tl,
                    top_right: tr,
                    bottom_right: br,
                    bottom_left: bl,
                })
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Uniform(r) => PropValue::F64(*r as f64),
            Self::PerCorner {
                top_left,
                top_right,
                bottom_right,
                bottom_left,
            } => {
                let mut m = PropMap::new();
                m.insert("top_left", PropValue::F64(*top_left as f64));
                m.insert("top_right", PropValue::F64(*top_right as f64));
                m.insert("bottom_right", PropValue::F64(*bottom_right as f64));
                m.insert("bottom_left", PropValue::F64(*bottom_left as f64));
                PropValue::Object(m)
            }
        }
    }

    fn type_name() -> &'static str {
        "radius"
    }
}

/// A widget border with color, width, and corner radius.
///
/// ## Wire format
///
/// ```json
/// {"color": "#rrggbb", "width": 2.0, "radius": 8}
/// ```
///
/// The `radius` value can be a number (uniform) or an object with
/// per-corner keys (see [`Radius`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Border {
    /// Border color. `None` means transparent.
    pub color: Option<Color>,
    /// Border width in logical pixels.
    pub width: f32,
    /// Corner radius (uniform or per-corner).
    pub radius: Radius,
}

impl Border {
    /// Construct a new value.
    pub fn new() -> Self {
        Self {
            color: None,
            width: 0.0,
            radius: Radius::default(),
        }
    }

    /// Set or construct `color`.
    pub fn color(mut self, c: impl Into<Color>) -> Self {
        self.color = Some(c.into());
        self
    }

    /// Set or construct `width`.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// Set or construct `radius`.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = Radius::Uniform(r);
        self
    }

    /// Set or construct `radius_corners`.
    pub fn radius_corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.radius = Radius::PerCorner {
            top_left: tl,
            top_right: tr,
            bottom_right: br,
            bottom_left: bl,
        };
        self
    }
}

impl Default for Border {
    fn default() -> Self {
        Self::new()
    }
}

impl PlushieType for Border {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        let color = obj.get("color").and_then(Color::wire_decode);
        let width = obj.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let radius = obj
            .get("radius")
            .and_then(Radius::wire_decode)
            .unwrap_or_default();

        Some(Self {
            color,
            width,
            radius,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        if let Some(ref color) = self.color {
            m.insert("color", color.wire_encode());
        }
        m.insert("width", PropValue::F64(self.width as f64));
        m.insert("radius", self.radius.wire_encode());
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "border"
    }
}
