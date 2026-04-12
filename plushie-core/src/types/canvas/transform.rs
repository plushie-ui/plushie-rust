//! Canvas transform types.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::super::PlushieType;

/// A 2D transform applied to canvas shapes or groups.
///
/// ## Wire format
///
/// An array of transform objects, applied in order:
///
/// ```json
/// [
///   {"type": "translate", "x": 10.0, "y": 20.0},
///   {"type": "rotate", "angle": 1.5708},
///   {"type": "scale", "x": 2.0, "y": 0.5},
///   {"type": "scale", "factor": 3.0}
/// ]
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Transform {
    /// Translation by (x, y) in logical pixels.
    Translate {
        /// Horizontal offset in pixels (positive = right).
        x: f32,
        /// Vertical offset in pixels (positive = down).
        y: f32,
    },
    /// Rotation around the origin.
    Rotate {
        /// Rotation angle in radians. Positive = clockwise.
        angle: f32,
    },
    /// Non-uniform scale.
    Scale {
        /// Horizontal scale factor.
        x: f32,
        /// Vertical scale factor.
        y: f32,
    },
    /// Uniform scale (same factor for both axes).
    ScaleUniform {
        /// Scale factor applied to both axes.
        factor: f32,
    },
}

impl PlushieType for Transform {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        match obj.get("type")?.as_str()? {
            "translate" => {
                let x = obj.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = obj.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                Some(Self::Translate { x, y })
            }
            "rotate" => {
                let angle = obj.get("angle").and_then(|v| v.as_f64())? as f32;
                Some(Self::Rotate { angle })
            }
            "scale" => {
                // Uniform: {type: "scale", factor: f32}
                // Non-uniform: {type: "scale", x: f32, y: f32}
                if let Some(factor) = obj.get("factor").and_then(|v| v.as_f64()) {
                    Some(Self::ScaleUniform {
                        factor: factor as f32,
                    })
                } else {
                    let x = obj.get("x").and_then(|v| v.as_f64())? as f32;
                    let y = obj.get("y").and_then(|v| v.as_f64())? as f32;
                    Some(Self::Scale { x, y })
                }
            }
            _ => None,
        }
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        match self {
            Self::Translate { x, y } => {
                m.insert("type", PropValue::Str("translate".into()));
                m.insert("x", PropValue::F64(*x as f64));
                m.insert("y", PropValue::F64(*y as f64));
            }
            Self::Rotate { angle } => {
                m.insert("type", PropValue::Str("rotate".into()));
                m.insert("angle", PropValue::F64(*angle as f64));
            }
            Self::Scale { x, y } => {
                m.insert("type", PropValue::Str("scale".into()));
                m.insert("x", PropValue::F64(*x as f64));
                m.insert("y", PropValue::F64(*y as f64));
            }
            Self::ScaleUniform { factor } => {
                m.insert("type", PropValue::Str("scale".into()));
                m.insert("factor", PropValue::F64(*factor as f64));
            }
        }
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "transform"
    }
}

/// Decode a list of transforms from a JSON array.
pub fn decode_transforms(value: &Value) -> Vec<Transform> {
    match value.as_array() {
        Some(arr) => arr.iter().filter_map(Transform::wire_decode).collect(),
        None => Vec::new(),
    }
}

/// Encode a list of transforms to a PropValue array.
pub fn encode_transforms(transforms: &[Transform]) -> PropValue {
    PropValue::Array(transforms.iter().map(|t| t.wire_encode()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translate() {
        let val = json!({"type": "translate", "x": 10.0, "y": 20.0});
        let t = Transform::wire_decode(&val).unwrap();
        assert_eq!(t, Transform::Translate { x: 10.0, y: 20.0 });
    }

    #[test]
    fn rotate() {
        let val = json!({"type": "rotate", "angle": std::f32::consts::FRAC_PI_2});
        let t = Transform::wire_decode(&val).unwrap();
        if let Transform::Rotate { angle } = t {
            assert!((angle - std::f32::consts::FRAC_PI_2).abs() < 0.001);
        } else {
            panic!("expected Rotate");
        }
    }

    #[test]
    fn scale_non_uniform() {
        let val = json!({"type": "scale", "x": 2.0, "y": 0.5});
        let t = Transform::wire_decode(&val).unwrap();
        assert_eq!(t, Transform::Scale { x: 2.0, y: 0.5 });
    }

    #[test]
    fn scale_uniform() {
        let val = json!({"type": "scale", "factor": 3.0});
        let t = Transform::wire_decode(&val).unwrap();
        assert_eq!(t, Transform::ScaleUniform { factor: 3.0 });
    }

    #[test]
    fn decode_transform_array() {
        let val = json!([
            {"type": "translate", "x": 5.0, "y": 10.0},
            {"type": "rotate", "angle": 0.5}
        ]);
        let transforms = decode_transforms(&val);
        assert_eq!(transforms.len(), 2);
    }

    #[test]
    fn decode_empty_and_invalid() {
        assert!(decode_transforms(&json!(null)).is_empty());
        assert!(decode_transforms(&json!([])).is_empty());
        // invalid entries are silently skipped
        let val = json!([{"type": "unknown"}, {"type": "rotate", "angle": 1.0}]);
        let transforms = decode_transforms(&val);
        assert_eq!(transforms.len(), 1);
    }
}
