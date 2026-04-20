//! Length type for widget sizing.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;

/// How a widget should be sized along an axis.
///
/// Wire format (strict, encoder-symmetric):
/// - `Fill`: the string `"fill"`
/// - `Shrink`: the string `"shrink"`
/// - `FillPortion(n)`: an object `{"fill_portion": n}` with a positive integer
/// - `Fixed(px)`: a non-negative number (logical pixels)
///
/// Any other shape (numeric strings, objects without `fill_portion`, etc.) is
/// rejected and the caller receives `None`. The decoder accepts exactly what
/// the encoder emits.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Length {
    /// Fill all available space.
    Fill,
    /// Take only the space needed by the content.
    #[default]
    Shrink,
    /// Fill a weighted portion of available space.
    FillPortion(u32),
    /// A fixed size in logical pixels.
    Fixed(f32),
}

impl PlushieType for Length {
    fn wire_decode(value: &Value) -> Option<Self> {
        match value {
            Value::Number(n) => n
                .as_f64()
                .map(|v| v as f32)
                .filter(|v| *v >= 0.0)
                .map(Self::Fixed),
            Value::String(s) => match s.as_str() {
                "fill" => Some(Self::Fill),
                "shrink" => Some(Self::Shrink),
                // Reject any other string, including numeric strings such as
                // "200". The encoder never emits numeric strings; accepting
                // them here would hide host-SDK bugs behind silent success.
                _ => None,
            },
            Value::Object(obj) => obj
                .get("fill_portion")
                .and_then(|v| v.as_u64())
                .map(|n| Self::FillPortion(u32::try_from(n).unwrap_or(u32::MAX).max(1))),
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Fill => PropValue::Str("fill".into()),
            Self::Shrink => PropValue::Str("shrink".into()),
            Self::FillPortion(n) => {
                assert!(*n >= 1, "length fill_portion must be >= 1, got {n}");
                let mut m = PropMap::new();
                m.insert("fill_portion", PropValue::U64(*n as u64));
                PropValue::Object(m)
            }
            Self::Fixed(f) => {
                assert!(*f >= 0.0, "length must be non-negative, got {f}");
                PropValue::F64(*f as f64)
            }
        }
    }

    fn type_name() -> &'static str {
        "length"
    }
}

impl From<f32> for Length {
    fn from(v: f32) -> Self {
        Self::Fixed(v)
    }
}

impl From<i32> for Length {
    fn from(v: i32) -> Self {
        Self::Fixed(v as f32)
    }
}

impl From<u32> for Length {
    fn from(v: u32) -> Self {
        Self::Fixed(v as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_is_shrink() {
        assert_eq!(Length::default(), Length::Shrink);
    }

    #[test]
    fn decode_accepts_canonical_encoder_shapes() {
        assert_eq!(Length::wire_decode(&json!("fill")), Some(Length::Fill));
        assert_eq!(Length::wire_decode(&json!("shrink")), Some(Length::Shrink));
        assert_eq!(Length::wire_decode(&json!(200)), Some(Length::Fixed(200.0)));
        assert_eq!(
            Length::wire_decode(&json!({"fill_portion": 3})),
            Some(Length::FillPortion(3))
        );
    }

    #[test]
    fn decode_rejects_numeric_strings() {
        assert_eq!(Length::wire_decode(&json!("200")), None);
        assert_eq!(Length::wire_decode(&json!("3.5")), None);
    }

    #[test]
    fn decode_rejects_unknown_objects() {
        assert_eq!(Length::wire_decode(&json!({"fill_porton": 3})), None);
        assert_eq!(Length::wire_decode(&json!({})), None);
        assert_eq!(Length::wire_decode(&json!({"foo": "bar"})), None);
    }

    #[test]
    fn decode_rejects_other_shapes() {
        assert_eq!(Length::wire_decode(&json!(null)), None);
        assert_eq!(Length::wire_decode(&json!(true)), None);
        assert_eq!(Length::wire_decode(&json!([1, 2])), None);
        assert_eq!(Length::wire_decode(&json!(-1.0)), None);
    }

    #[test]
    #[should_panic(expected = "length must be non-negative")]
    fn encode_rejects_negative_fixed() {
        Length::Fixed(-1.0).wire_encode();
    }

    #[test]
    #[should_panic(expected = "fill_portion must be >= 1")]
    fn encode_rejects_zero_fill_portion() {
        Length::FillPortion(0).wire_encode();
    }
}
