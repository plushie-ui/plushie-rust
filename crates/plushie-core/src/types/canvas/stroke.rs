//! Canvas stroke types.

use serde_json::Value;

use crate::PlushieEnum;
use crate::protocol::{PropMap, PropValue};

use super::super::{Color, PlushieType};

/// Stroke style for canvas shapes.
///
/// ## Wire format
///
/// ```json
/// {"color": "#000000", "width": 2.0, "cap": "round", "join": "bevel", "dash": {"segments": [5, 3], "offset": 0}}
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// Stroke color.
    pub color: Color,
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Line cap style for stroke endpoints. `None` uses the renderer default (butt).
    pub cap: Option<LineCap>,
    /// Line join style for stroke corners. `None` uses the renderer default (miter).
    pub join: Option<LineJoin>,
    /// Dash pattern. `None` draws a solid line.
    pub dash: Option<Dash>,
}

/// Line cap style for stroke endpoints.
#[derive(Debug, Clone, Copy, PartialEq, PlushieEnum)]
#[plushie_type(name = "line_cap")]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

/// Line join style for stroke corners.
#[derive(Debug, Clone, Copy, PartialEq, PlushieEnum)]
#[plushie_type(name = "line_join")]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

/// Dash pattern for strokes.
///
/// ## Wire format
///
/// ```json
/// {"segments": [5.0, 3.0], "offset": 0.0}
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Dash {
    /// Alternating dash/gap lengths in logical pixels. Must be non-empty.
    pub segments: Vec<f32>,
    /// Starting offset into the dash pattern, in logical pixels.
    pub offset: f32,
}

impl PlushieType for Dash {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let segments: Vec<f32> = obj
            .get("segments")?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        if segments.is_empty() {
            return None;
        }
        let offset = obj.get("offset").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        Some(Self { segments, offset })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        m.insert(
            "segments",
            PropValue::Array(
                self.segments
                    .iter()
                    .map(|s| PropValue::F64(*s as f64))
                    .collect(),
            ),
        );
        m.insert("offset", PropValue::F64(self.offset as f64));
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "dash"
    }
}

impl PlushieType for Stroke {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let color = Color::wire_decode(obj.get("color")?)?;
        let width = obj.get("width").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let cap = obj.get("cap").and_then(LineCap::wire_decode);
        let join = obj.get("join").and_then(LineJoin::wire_decode);
        let dash = obj.get("dash").and_then(Dash::wire_decode);
        Some(Self {
            color,
            width,
            cap,
            join,
            dash,
        })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        m.insert("color", self.color.wire_encode());
        m.insert("width", PropValue::F64(self.width as f64));
        if let Some(ref cap) = self.cap {
            m.insert("cap", cap.wire_encode());
        }
        if let Some(ref join) = self.join {
            m.insert("join", join.wire_encode());
        }
        if let Some(ref dash) = self.dash {
            m.insert("dash", dash.wire_encode());
        }
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "stroke"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stroke_minimal() {
        let val = json!({"color": "#ff0000", "width": 2.0});
        let stroke = Stroke::wire_decode(&val).unwrap();
        assert_eq!(stroke.width, 2.0);
        assert!(stroke.cap.is_none());
        assert!(stroke.join.is_none());
        assert!(stroke.dash.is_none());
    }

    #[test]
    fn stroke_full() {
        let val = json!({
            "color": "#000000",
            "width": 3.0,
            "cap": "round",
            "join": "bevel",
            "dash": {"segments": [5.0, 3.0], "offset": 1.0}
        });
        let stroke = Stroke::wire_decode(&val).unwrap();
        assert_eq!(stroke.cap, Some(LineCap::Round));
        assert_eq!(stroke.join, Some(LineJoin::Bevel));
        let dash = stroke.dash.as_ref().unwrap();
        assert_eq!(dash.segments, vec![5.0, 3.0]);
        assert_eq!(dash.offset, 1.0);
    }

    #[test]
    fn dash_empty_segments_returns_none() {
        let val = json!({"segments": [], "offset": 0.0});
        assert!(Dash::wire_decode(&val).is_none());
    }

    #[test]
    fn stroke_round_trip() {
        let original = Stroke {
            color: Color::hex("#abcdef"),
            width: 2.5,
            cap: Some(LineCap::Square),
            join: Some(LineJoin::Miter),
            dash: None,
        };
        let encoded = original.wire_encode();
        let json_val: Value = encoded.into();
        let decoded = Stroke::wire_decode(&json_val).unwrap();
        assert_eq!(original, decoded);
    }
}
