//! Style overrides for interactive canvas shape states.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::super::PlushieType;

/// Style overrides applied to canvas shapes in hover/pressed/focus states.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeStyle {
    /// Fill override (color hex string or gradient ref).
    pub fill: Option<String>,
    /// Stroke override as raw JSON value.
    pub stroke: Option<Value>,
    /// Opacity override.
    pub opacity: Option<f32>,
}

impl PlushieType for ShapeStyle {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let fill = obj.get("fill").and_then(|v| v.as_str()).map(String::from);
        let stroke = obj.get("stroke").cloned();
        let opacity = obj.get("opacity").and_then(|v| v.as_f64()).map(|f| f as f32);
        // At least one field must be present to be a valid style.
        if fill.is_none() && stroke.is_none() && opacity.is_none() {
            return None;
        }
        Some(Self { fill, stroke, opacity })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        if let Some(ref fill) = self.fill {
            m.insert("fill", PropValue::Str(fill.clone()));
        }
        if let Some(ref stroke) = self.stroke {
            m.insert("stroke", PropValue::from(stroke.clone()));
        }
        if let Some(opacity) = self.opacity {
            m.insert("opacity", PropValue::F64(opacity as f64));
        }
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "shape_style"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shape_style_fill_only() {
        let val = json!({"fill": "#ff0000"});
        let style = ShapeStyle::wire_decode(&val).unwrap();
        assert_eq!(style.fill, Some("#ff0000".into()));
        assert!(style.stroke.is_none());
        assert!(style.opacity.is_none());
    }

    #[test]
    fn shape_style_all_fields() {
        let val = json!({
            "fill": "#00ff00",
            "stroke": {"color": "#000", "width": 2.0},
            "opacity": 0.5
        });
        let style = ShapeStyle::wire_decode(&val).unwrap();
        assert_eq!(style.fill, Some("#00ff00".into()));
        assert!(style.stroke.is_some());
        assert_eq!(style.opacity, Some(0.5));
    }

    #[test]
    fn shape_style_empty_is_none() {
        assert!(ShapeStyle::wire_decode(&json!({})).is_none());
    }
}
