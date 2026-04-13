//! Line height type for text layout.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;

/// Line height for text: relative (multiplier) or absolute (pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    /// Relative to the font size (e.g. 1.5 = 150%).
    Relative(f32),
    /// Absolute height in logical pixels.
    Absolute(f32),
}

impl PlushieType for LineHeight {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => Some(Self::Relative(n.as_f64()? as f32)),
            Value::Object(obj) => {
                if let Some(n) = obj.get("relative").and_then(|v| v.as_f64()) {
                    Some(Self::Relative(n as f32))
                } else {
                    obj.get("absolute")
                        .and_then(|v| v.as_f64())
                        .map(|n| Self::Absolute(n as f32))
                }
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Relative(n) => PropValue::F64(*n as f64),
            Self::Absolute(n) => {
                let mut m = PropMap::new();
                m.insert("absolute", PropValue::F64(*n as f64));
                PropValue::Object(m)
            }
        }
    }

    fn type_name() -> &'static str {
        "line_height"
    }
}
