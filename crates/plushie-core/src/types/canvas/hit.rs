//! Canvas hit rectangle type.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue, Props};

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
        let x = obj
            .get("x")
            .and_then(value_to_f32)
            .map(clean_coordinate)
            .unwrap_or(0.0);
        let y = obj
            .get("y")
            .and_then(value_to_f32)
            .map(clean_coordinate)
            .unwrap_or(0.0);
        let w = clean_extent(value_to_f32(obj.get("w")?)?);
        let h = clean_extent(value_to_f32(obj.get("h")?)?);
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

    fn extract(props: &Props, key: &str) -> Option<Self> {
        let map = props.get(key)?.as_object()?;
        let x = map
            .get("x")
            .and_then(prop_value_to_f32)
            .map(clean_coordinate)
            .unwrap_or(0.0);
        let y = map
            .get("y")
            .and_then(prop_value_to_f32)
            .map(clean_coordinate)
            .unwrap_or(0.0);
        let w = clean_extent(prop_value_to_f32(map.get("w")?)?);
        let h = clean_extent(prop_value_to_f32(map.get("h")?)?);
        Some(Self { x, y, w, h })
    }
}

fn value_to_f32(value: &Value) -> Option<f32> {
    value.as_f64().map(|value| value as f32)
}

fn prop_value_to_f32(value: &PropValue) -> Option<f32> {
    match value {
        PropValue::F64(value) => Some(*value as f32),
        _ => value.as_f64().map(|value| value as f32),
    }
}

fn clean_coordinate(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn clean_extent(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
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

    #[test]
    fn hit_rect_normalizes_invalid_coordinates_and_dimensions() {
        let val = json!({"x": -5.0, "y": 1.0e39, "w": -20.0, "h": 1.0e39});
        let rect = HitRect::wire_decode(&val).unwrap();
        assert_eq!(
            rect,
            HitRect {
                x: -5.0,
                y: 0.0,
                w: 0.0,
                h: 0.0
            }
        );
    }

    #[test]
    fn hit_rect_extract_normalizes_non_finite_props() {
        let mut hit_rect = PropMap::new();
        hit_rect.insert("x", PropValue::F64(f64::NAN));
        hit_rect.insert("y", PropValue::F64(-7.0));
        hit_rect.insert("w", PropValue::F64(f64::INFINITY));
        hit_rect.insert("h", PropValue::F64(-3.0));

        let mut props = PropMap::new();
        props.insert("hit_rect", PropValue::Object(hit_rect));
        let rect = HitRect::extract(&Props::from(props), "hit_rect").unwrap();

        assert_eq!(
            rect,
            HitRect {
                x: 0.0,
                y: -7.0,
                w: 0.0,
                h: 0.0
            }
        );
    }
}
