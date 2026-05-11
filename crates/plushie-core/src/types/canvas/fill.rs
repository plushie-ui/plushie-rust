//! Canvas fill types.

use serde_json::Value;

use crate::PlushieEnum;
use crate::protocol::PropValue;

use super::super::{Color, Gradient, PlushieType};

/// Canvas fill: solid color or gradient.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasFill {
    /// Color.
    Color(Color),
    /// Gradient.
    Gradient(Gradient),
}

impl PlushieType for CanvasFill {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            // String -> Color (hex)
            Value::String(_) => Color::wire_decode(value).map(Self::Color),
            // Object with type "linear" -> Gradient
            Value::Object(obj) => match obj.get("type").and_then(|v| v.as_str()) {
                Some("linear") => Gradient::wire_decode(value).map(Self::Gradient),
                _ => None,
            },
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
#[derive(Debug, Clone, Copy, PartialEq, PlushieEnum)]
#[plushie_type(name = "fill_rule")]
pub enum FillRule {
    /// Non Zero.
    NonZero,
    /// Even Odd.
    EvenOdd,
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
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            let encoded = rule.wire_encode();
            let json_val: Value = encoded.into();
            let decoded = FillRule::wire_decode(&json_val).unwrap();
            assert_eq!(decoded, rule);
        }
    }
}
