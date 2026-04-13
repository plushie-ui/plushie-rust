//! Canvas hit rectangle type.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::super::PlushieType;

/// Custom hit-test rectangle for interactive canvas shapes.
///
/// Overrides the shape's geometry for pointer hit detection,
/// allowing larger or smaller interactive areas.
///
/// ## Wire format
///
/// ```json
/// {"x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0}
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitRect {
    /// Left edge x-coordinate in logical pixels.
    pub x: f32,
    /// Top edge y-coordinate in logical pixels.
    pub y: f32,
    /// Width of the hit area in logical pixels.
    pub w: f32,
    /// Height of the hit area in logical pixels.
    pub h: f32,
}

impl PlushieType for HitRect {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let x = obj.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let y = obj.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let w = obj.get("w").and_then(|v| v.as_f64())? as f32;
        let h = obj.get("h").and_then(|v| v.as_f64())? as f32;
        Some(Self { x, y, w, h })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        m.insert("x", PropValue::F64(self.x as f64));
        m.insert("y", PropValue::F64(self.y as f64));
        m.insert("w", PropValue::F64(self.w as f64));
        m.insert("h", PropValue::F64(self.h as f64));
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "hit_rect"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hit_rect_decode() {
        let val = json!({"x": 5.0, "y": 10.0, "w": 200.0, "h": 100.0});
        let rect = HitRect::wire_decode(&val).unwrap();
        assert_eq!(
            rect,
            HitRect {
                x: 5.0,
                y: 10.0,
                w: 200.0,
                h: 100.0
            }
        );
    }

    #[test]
    fn hit_rect_requires_dimensions() {
        assert!(HitRect::wire_decode(&json!({"x": 0.0})).is_none());
    }
}
