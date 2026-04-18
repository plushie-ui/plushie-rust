//! Canvas drag types.

use serde_json::Value;

use crate::PlushieEnum;
use crate::protocol::{PropMap, PropValue};

use super::super::PlushieType;

/// Bounding constraints for draggable canvas shapes.
///
/// Each bound is optional. Omitted bounds leave that axis unconstrained.
///
/// ## Wire format
///
/// ```json
/// {"min_x": 0.0, "max_x": 100.0, "min_y": -50.0, "max_y": 50.0}
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragBounds {
    /// Minimum x-coordinate in logical pixels. `None` means unbounded left.
    pub min_x: Option<f32>,
    /// Maximum x-coordinate in logical pixels. `None` means unbounded right.
    pub max_x: Option<f32>,
    /// Minimum y-coordinate in logical pixels. `None` means unbounded up.
    pub min_y: Option<f32>,
    /// Maximum y-coordinate in logical pixels. `None` means unbounded down.
    pub max_y: Option<f32>,
}

impl PlushieType for DragBounds {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let min_x = obj.get("min_x").and_then(|v| v.as_f64()).map(|f| f as f32);
        let max_x = obj.get("max_x").and_then(|v| v.as_f64()).map(|f| f as f32);
        let min_y = obj.get("min_y").and_then(|v| v.as_f64()).map(|f| f as f32);
        let max_y = obj.get("max_y").and_then(|v| v.as_f64()).map(|f| f as f32);
        Some(Self {
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        if let Some(v) = self.min_x {
            m.insert("min_x", PropValue::F64(v as f64));
        }
        if let Some(v) = self.max_x {
            m.insert("max_x", PropValue::F64(v as f64));
        }
        if let Some(v) = self.min_y {
            m.insert("min_y", PropValue::F64(v as f64));
        }
        if let Some(v) = self.max_y {
            m.insert("max_y", PropValue::F64(v as f64));
        }
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "drag_bounds"
    }
}

/// Axis constraint for drag movement.
#[derive(Debug, Clone, Copy, PartialEq, PlushieEnum)]
#[plushie_type(name = "drag_axis")]
pub enum DragAxis {
    /// Both.
    Both,
    /// X.
    X,
    /// Y.
    Y,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn drag_bounds_full() {
        let val = json!({"min_x": 0.0, "max_x": 100.0, "min_y": -50.0, "max_y": 50.0});
        let bounds = DragBounds::wire_decode(&val).unwrap();
        assert_eq!(bounds.min_x, Some(0.0));
        assert_eq!(bounds.max_x, Some(100.0));
    }

    #[test]
    fn drag_bounds_partial() {
        let val = json!({"min_x": 10.0});
        let bounds = DragBounds::wire_decode(&val).unwrap();
        assert_eq!(bounds.min_x, Some(10.0));
        assert!(bounds.max_x.is_none());
    }

    #[test]
    fn drag_axis_round_trip() {
        for (axis, s) in [
            (DragAxis::Both, "both"),
            (DragAxis::X, "x"),
            (DragAxis::Y, "y"),
        ] {
            let encoded = axis.wire_encode();
            assert_eq!(encoded, PropValue::Str(s.into()));
            let decoded = DragAxis::wire_decode(&json!(s)).unwrap();
            assert_eq!(decoded, axis);
        }
    }
}
