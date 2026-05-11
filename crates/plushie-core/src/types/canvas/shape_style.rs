//! Style overrides for interactive canvas shape states.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::super::PlushieType;
use super::Stroke;

/// Style overrides applied to canvas shapes in hover/pressed/focus states.
///
/// All fields are optional overrides that replace the shape's base
/// property for that state. An empty style is a no-op.
///
/// ## Wire format
///
/// ```json
/// {"fill": "#ff0000", "stroke": {"color": "#000", "width": 2}, "opacity": 0.5}
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeStyle {
    /// Fill color override as a hex string (e.g. `"#ff0000"`).
    pub fill: Option<String>,
    /// Stroke override (replaces the shape's base stroke for this state).
    pub stroke: Option<Stroke>,
    /// Opacity override, from 0.0 (transparent) to 1.0 (opaque).
    pub opacity: Option<f32>,
}

impl PlushieType for ShapeStyle {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let fill = obj.get("fill").and_then(|v| v.as_str()).map(String::from);
        let stroke = obj.get("stroke").and_then(Stroke::wire_decode);
        let opacity = obj
            .get("opacity")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        Some(Self {
            fill,
            stroke,
            opacity,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        if let Some(ref fill) = self.fill {
            m.insert("fill", PropValue::Str(fill.clone()));
        }
        if let Some(ref stroke) = self.stroke {
            m.insert("stroke", stroke.wire_encode());
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
    use super::super::super::Color;
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
            "stroke": {"color": "#000000", "width": 2.0},
            "opacity": 0.5
        });
        let style = ShapeStyle::wire_decode(&val).unwrap();
        assert_eq!(style.fill, Some("#00ff00".into()));
        let stroke = style.stroke.as_ref().unwrap();
        assert_eq!(stroke.color, Color::hex("#000000"));
        assert_eq!(stroke.width, 2.0);
        assert_eq!(style.opacity, Some(0.5));
    }

    #[test]
    fn shape_style_empty_is_no_op() {
        assert_eq!(
            ShapeStyle::wire_decode(&json!({})),
            Some(ShapeStyle {
                fill: None,
                stroke: None,
                opacity: None
            })
        );
    }

    #[test]
    fn shape_style_round_trip() {
        let original = ShapeStyle {
            fill: Some("#ff0000".into()),
            stroke: Some(Stroke {
                color: Color::hex("#0000ff"),
                width: 3.0,
                cap: None,
                join: None,
                dash: None,
            }),
            opacity: Some(0.75),
        };
        let encoded = original.wire_encode();
        let json_val: Value = encoded.into();
        let decoded = ShapeStyle::wire_decode(&json_val).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn empty_shape_style_round_trips() {
        let original = ShapeStyle {
            fill: None,
            stroke: None,
            opacity: None,
        };
        let encoded = original.wire_encode();
        let json_val: Value = encoded.into();
        let decoded = ShapeStyle::wire_decode(&json_val).unwrap();
        assert_eq!(original, decoded);
    }
}
