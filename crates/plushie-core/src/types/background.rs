//! Background fill type.

use serde_json::Value;

use crate::protocol::PropValue;

use super::PlushieType;
use super::color::Color;
use super::gradient::Gradient;

/// A background fill: either a solid color or a gradient.
#[derive(Debug, Clone, PartialEq)]
pub enum Background {
    /// A solid color fill.
    Color(Color),
    /// A gradient fill.
    Gradient(Gradient),
}

impl PlushieType for Background {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::String(_) => Color::wire_decode(value).map(Self::Color),
            Value::Object(obj) => {
                if obj.get("type").and_then(|v| v.as_str()) == Some("linear") {
                    Gradient::wire_decode(value).map(Self::Gradient)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Color(c) => c.wire_encode(),
            Self::Gradient(g) => g.wire_encode(),
        }
    }

    fn type_name() -> &'static str {
        "background"
    }
}

impl From<Color> for Background {
    fn from(c: Color) -> Self {
        Self::Color(c)
    }
}

impl From<Gradient> for Background {
    fn from(g: Gradient) -> Self {
        Self::Gradient(g)
    }
}

impl From<&str> for Background {
    fn from(s: &str) -> Self {
        Self::Color(Color::from(s))
    }
}

impl From<String> for Background {
    fn from(s: String) -> Self {
        Self::Color(Color::from(s.as_str()))
    }
}
