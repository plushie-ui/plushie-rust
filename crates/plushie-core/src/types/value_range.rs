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
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

impl PlushieType for ValueRange {
    fn wire_decode(value: &Value) -> Option<Self> {
        let arr = value.as_array()?;
        let min = arr.first()?.as_f64()? as f32;
        let max = arr.get(1)?.as_f64()? as f32;
        Some(Self { min, max })
    }

    fn wire_encode(&self) -> PropValue {
        PropValue::Array(vec![
            PropValue::F64(self.min as f64),
            PropValue::F64(self.max as f64),
        ])
    }

    fn type_name() -> &'static str {
        "range"
    }
}
