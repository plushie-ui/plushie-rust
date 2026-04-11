//! Canvas fill types.

use serde_json::Value;

use crate::protocol::PropValue;

use super::super::{Color, Gradient, PlushieType};

/// Canvas fill: solid color or gradient.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasFill {
    Color(Color),
    Gradient(Gradient),
}

impl PlushieType for CanvasFill {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            // String -> Color (hex)
            Value::String(_) => Color::wire_decode(value).map(Self::Color),
            // Object with type "linear" -> Gradient
            Value::Object(obj) => {
                match obj.get("type").and_then(|v| v.as_str()) {
                    Some("linear") => Gradient::wire_decode(value).map(Self::Gradient),
                    _ => None,
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
        "canvas_fill"
    }
}

/// Fill rule for closed paths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

impl PlushieType for FillRule {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "non_zero" => Some(Self::NonZero),
            "even_odd" => Some(Self::EvenOdd),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Str(match self {
            Self::NonZero => "non_zero",
            Self::EvenOdd => "even_odd",
        }.into())
    }

    fn type_name() -> &'static str {
        "fill_rule"
    }
}

impl From<Color> for CanvasFill {
    fn from(c: Color) -> Self {
        Self::Color(c)
    }
}

impl From<Gradient> for CanvasFill {
    fn from(g: Gradient) -> Self {
        Self::Gradient(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fill_color_from_string() {
        let fill = CanvasFill::wire_decode(&json!("#ff0000")).unwrap();
        assert!(matches!(fill, CanvasFill::Color(_)));
    }

    #[test]
    fn fill_gradient_from_object() {
        let val = json!({
            "type": "linear",
            "start": [0.0, 0.0],
            "end": [1.0, 1.0],
            "stops": [[0.0, "#000000"], [1.0, "#ffffff"]]
        });
        let fill = CanvasFill::wire_decode(&val).unwrap();
        assert!(matches!(fill, CanvasFill::Gradient(_)));
    }

    #[test]
    fn fill_rule_round_trip() {
        let val = FillRule::NonZero.wire_encode();
        assert_eq!(val, PropValue::Str("non_zero".into()));

        let decoded = FillRule::wire_decode(&json!("even_odd")).unwrap();
        assert_eq!(decoded, FillRule::EvenOdd);
    }
}
