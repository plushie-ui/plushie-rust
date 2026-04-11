//! Border and radius types.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::color::Color;
use super::PlushieType;

/// Corner radius for a border: uniform or per-corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Radius {
    /// Same radius on all four corners.
    Uniform(f32),
    /// Individual radius for each corner.
    PerCorner {
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
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
                let br = obj.get("bottom_right").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let bl = obj.get("bottom_left").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                Some(Self::PerCorner { top_left: tl, top_right: tr, bottom_right: br, bottom_left: bl })
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Uniform(r) => PropValue::F64(*r as f64),
            Self::PerCorner { top_left, top_right, bottom_right, bottom_left } => {
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
#[derive(Debug, Clone, PartialEq)]
pub struct Border {
    pub color: Option<Color>,
    pub width: f32,
    pub radius: Radius,
}

impl Border {
    pub fn new() -> Self {
        Self { color: None, width: 0.0, radius: Radius::default() }
    }

    pub fn color(mut self, c: impl Into<Color>) -> Self {
        self.color = Some(c.into());
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.radius = Radius::Uniform(r);
        self
    }

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

        let color = obj
            .get("color")
            .and_then(Color::wire_decode);
        let width = obj
            .get("width")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        let radius = obj
            .get("radius")
            .and_then(Radius::wire_decode)
            .unwrap_or_default();

        Some(Self { color, width, radius })
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
