//! Numeric value range type.

use serde_json::Value;

use crate::protocol::PropValue;

use super::PlushieType;

/// A numeric range with min and max bounds.
///
/// Used by sliders, progress bars, and scrollbars to describe a
/// bounded numeric domain. Distinct from [`std::ops::Range`]: this is
/// a closed `[min, max]` value range, not an iterator.
///
/// ## Wire format
///
/// ```json
/// [0.0, 100.0]
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueRange {
    /// Lower bound (inclusive).
    pub min: f32,
    /// Upper bound (inclusive).
    pub max: f32,
}

impl ValueRange {
    /// Construct a new value.
    ///
    /// # Panics
    ///
    /// Panics if either bound is not finite or if `min` is greater
    /// than `max`.
    pub fn new(min: f32, max: f32) -> Self {
        assert!(
            is_valid_range(min, max),
            "value range bounds must be finite and ordered"
        );
        Self { min, max }
    }
}

impl PlushieType for ValueRange {
    fn wire_decode(value: &Value) -> Option<Self> {
        let arr = value.as_array()?;
        if arr.len() != 2 {
            return None;
        }
        let min = decode_finite_f32(arr.first()?)?;
        let max = decode_finite_f32(arr.get(1)?)?;
        if min > max {
            return None;
        }
        Some(Self { min, max })
    }

    fn wire_encode(&self) -> PropValue {
        assert!(
            is_valid_range(self.min, self.max),
            "value range bounds must be finite and ordered"
        );
        PropValue::Array(vec![
            PropValue::F64(self.min as f64),
            PropValue::F64(self.max as f64),
        ])
    }

    fn type_name() -> &'static str {
        "range"
    }
}

fn decode_finite_f32(value: &Value) -> Option<f32> {
    let decoded = value.as_f64()?;
    let decoded = decoded as f32;
    decoded.is_finite().then_some(decoded)
}

fn is_valid_range(min: f32, max: f32) -> bool {
    min.is_finite() && max.is_finite() && min <= max
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decode_accepts_finite_range() {
        assert_eq!(
            ValueRange::wire_decode(&json!([0.0, 100.0])),
            Some(ValueRange {
                min: 0.0,
                max: 100.0
            })
        );
    }

    #[test]
    fn decode_rejects_non_pair_arrays() {
        assert_eq!(ValueRange::wire_decode(&json!([0.0])), None);
        assert_eq!(ValueRange::wire_decode(&json!([0.0, 1.0, 2.0])), None);
    }

    #[test]
    fn decode_rejects_non_numeric_values() {
        assert_eq!(ValueRange::wire_decode(&json!(["0", 1.0])), None);
        assert_eq!(ValueRange::wire_decode(&json!([0.0, null])), None);
    }

    #[test]
    fn decode_rejects_infinite_after_f32_conversion() {
        assert_eq!(ValueRange::wire_decode(&json!([0.0, f64::MAX])), None);
    }

    #[test]
    fn decode_rejects_min_above_max() {
        assert_eq!(ValueRange::wire_decode(&json!([100.0, 0.0])), None);
    }

    #[test]
    #[should_panic(expected = "value range bounds must be finite and ordered")]
    fn new_rejects_invalid_range() {
        let _ = ValueRange::new(100.0, 0.0);
    }

    #[test]
    #[should_panic(expected = "value range bounds must be finite and ordered")]
    fn encode_rejects_invalid_range() {
        let _ = ValueRange {
            min: 0.0,
            max: f32::INFINITY,
        }
        .wire_encode();
    }
}
