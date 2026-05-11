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
            Value::Number(n) => decode_positive_f32(n.as_f64()?).map(Self::Relative),
            Value::Object(obj) => {
                if let Some(n) = obj.get("relative").and_then(|v| v.as_f64()) {
                    decode_positive_f32(n).map(Self::Relative)
                } else {
                    obj.get("absolute")
                        .and_then(|v| v.as_f64())
                        .and_then(decode_positive_f32)
                        .map(Self::Absolute)
                }
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Relative(n) => {
                assert!(is_positive_f32(*n), "line_height relative must be positive");
                PropValue::F64(*n as f64)
            }
            Self::Absolute(n) => {
                assert!(is_positive_f32(*n), "line_height absolute must be positive");
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

fn decode_positive_f32(value: f64) -> Option<f32> {
    let value = value as f32;
    is_positive_f32(value).then_some(value)
}

fn is_positive_f32(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn line_height_round_trip() {
        for original in [LineHeight::Relative(1.5), LineHeight::Absolute(24.0)] {
            let value = Value::from(original.wire_encode());
            assert_eq!(LineHeight::wire_decode(&value), Some(original));
        }
    }

    #[test]
    fn line_height_rejects_non_positive_values() {
        assert_eq!(LineHeight::wire_decode(&json!(0.0)), None);
        assert_eq!(LineHeight::wire_decode(&json!(-1.0)), None);
        assert_eq!(LineHeight::wire_decode(&json!({"relative": 0.0})), None);
        assert_eq!(LineHeight::wire_decode(&json!({"absolute": -1.0})), None);
    }

    #[test]
    fn line_height_rejects_infinite_after_f32_conversion() {
        assert_eq!(LineHeight::wire_decode(&json!(f64::MAX)), None);
    }

    #[test]
    #[should_panic(expected = "line_height relative must be positive")]
    fn line_height_encode_rejects_invalid_relative() {
        let _ = LineHeight::Relative(0.0).wire_encode();
    }

    #[test]
    #[should_panic(expected = "line_height absolute must be positive")]
    fn line_height_encode_rejects_invalid_absolute() {
        let _ = LineHeight::Absolute(f32::INFINITY).wire_encode();
    }
}
