//! Length type for widget sizing.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;

/// How a widget should be sized along an axis.
///
/// Wire format:
/// - `Fill`: the string `"fill"`
/// - `Shrink`: the string `"shrink"`
/// - `FillPortion(n)`: an object `{"fill_portion": n}`
/// - `Fixed(px)`: a non-negative number (logical pixels)
///
/// A numeric string (e.g. `"200"`) is also accepted as `Fixed`.
#[derive(Debug, Clone, PartialEq)]
pub enum Length {
    /// Fill all available space.
    Fill,
    /// Take only the space needed by the content.
    Shrink,
    /// Fill a weighted portion of available space.
    FillPortion(u16),
    /// A fixed size in logical pixels.
    Fixed(f32),
}

impl Default for Length {
    fn default() -> Self {
        Self::Shrink
    }
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
                other => other
                    .parse::<f32>()
                    .ok()
                    .filter(|v| *v >= 0.0)
                    .map(Self::Fixed),
            },
            Value::Object(obj) => {
                if let Some(n) = obj.get("fill_portion").and_then(|v| v.as_u64()) {
                    Some(Self::FillPortion(u16::try_from(n).unwrap_or(1).max(1)))
                } else {
                    Some(Self::Shrink)
                }
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        match self {
            Self::Fill => PropValue::Str("fill".into()),
            Self::Shrink => PropValue::Str("shrink".into()),
            Self::FillPortion(n) => {
                let mut m = PropMap::new();
                m.insert("fill_portion", PropValue::U64(*n as u64));
                PropValue::Object(m)
            }
            Self::Fixed(f) => PropValue::F64(*f as f64),
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

    #[test]
    fn default_is_shrink() {
        assert_eq!(Length::default(), Length::Shrink);
    }
}
