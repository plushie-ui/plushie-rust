//! Gradient types for background fills.

use serde_json::Value;

use crate::protocol::{PropMap, PropValue};

use super::PlushieType;
use super::color::Color;

/// A single stop in a gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    /// Position along the gradient axis, from 0.0 (start) to 1.0 (end).
    pub offset: f32,
    /// Color at this stop.
    pub color: Color,
}

impl GradientStop {
    /// Create a new gradient stop at the given offset with the given color.
    pub fn new(offset: f32, color: impl Into<Color>) -> Self {
        Self {
            offset,
            color: color.into(),
        }
    }
}

/// A linear gradient fill defined by start/end points and color stops.
///
/// ## Wire format
///
/// ```json
/// {"type": "linear", "start": [x, y], "end": [x, y], "stops": [[offset, "#hex"], ...]}
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Start point of the gradient axis as (x, y) in logical pixels.
    pub start: (f32, f32),
    /// End point of the gradient axis as (x, y) in logical pixels.
    pub end: (f32, f32),
    /// Color stops along the gradient, ordered by offset (0.0 to 1.0).
    pub stops: Vec<GradientStop>,
}

impl Gradient {
    /// Set or construct `linear`.
    pub fn linear(start: (f32, f32), end: (f32, f32), stops: Vec<(f32, Color)>) -> Self {
        Self {
            start,
            end,
            stops: stops
                .into_iter()
                .map(|(offset, color)| GradientStop { offset, color })
                .collect(),
        }
    }

    /// Construct a linear gradient from an angle (in degrees) and color stops.
    ///
    /// Matches the convention used by every other plushie SDK:
    ///
    /// - 0 deg = east, 90 = south, 180 = west, 270 = north
    /// - Start and end points are computed so the gradient axis passes
    ///   through the unit square centered at (0.5, 0.5)
    pub fn linear_from_angle(angle_degrees: f32, stops: Vec<(f32, Color)>) -> Self {
        let radians = angle_degrees * std::f32::consts::PI / 180.0;
        let dx = radians.cos();
        let dy = radians.sin();
        let half_len = dx.abs() / 2.0 + dy.abs() / 2.0;
        let start = (0.5 - dx * half_len, 0.5 - dy * half_len);
        let end = (0.5 + dx * half_len, 0.5 + dy * half_len);
        Self::linear(start, end, stops)
    }
}

impl PlushieType for Gradient {
    fn wire_decode(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;

        let start = decode_point(obj.get("start")?)?;
        let end = decode_point(obj.get("end")?)?;

        let stops_arr = obj.get("stops")?.as_array()?;
        let stops: Vec<GradientStop> = stops_arr
            .iter()
            .filter_map(|stop| {
                let arr = stop.as_array()?;
                let offset = arr.first()?.as_f64()? as f32;
                let color = Color::wire_decode(arr.get(1)?)?;
                Some(GradientStop { offset, color })
            })
            .collect();

        Some(Self { start, end, stops })
    }

    fn wire_encode(&self) -> PropValue {
        let mut m = PropMap::new();
        m.insert("type", PropValue::Str("linear".into()));
        m.insert(
            "start",
            PropValue::Array(vec![
                PropValue::F64(self.start.0 as f64),
                PropValue::F64(self.start.1 as f64),
            ]),
        );
        m.insert(
            "end",
            PropValue::Array(vec![
                PropValue::F64(self.end.0 as f64),
                PropValue::F64(self.end.1 as f64),
            ]),
        );
        m.insert(
            "stops",
            PropValue::Array(
                self.stops
                    .iter()
                    .map(|s| {
                        PropValue::Array(vec![
                            PropValue::F64(s.offset as f64),
                            s.color.wire_encode(),
                        ])
                    })
                    .collect(),
            ),
        );
        PropValue::Object(m)
    }

    fn type_name() -> &'static str {
        "gradient"
    }
}

fn decode_point(value: &Value) -> Option<(f32, f32)> {
    let arr = value.as_array()?;
    let x = arr.first()?.as_f64()? as f32;
    let y = arr.get(1)?.as_f64()? as f32;
    Some((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_stop_new() {
        let stop = GradientStop::new(0.5, Color::hex("#ff0000"));
        assert_eq!(stop.offset, 0.5);
        assert_eq!(stop.color, Color::hex("#ff0000"));
    }

    #[test]
    fn gradient_linear_from_angle_zero_is_east() {
        // 0 degrees -> east: start near (0, 0.5), end near (1, 0.5).
        let g = Gradient::linear_from_angle(0.0, vec![(0.0, Color::hex("#000000"))]);
        assert!((g.start.0 - 0.0).abs() < 1e-5);
        assert!((g.start.1 - 0.5).abs() < 1e-5);
        assert!((g.end.0 - 1.0).abs() < 1e-5);
        assert!((g.end.1 - 0.5).abs() < 1e-5);
    }

    #[test]
    fn gradient_linear_from_angle_ninety_is_south() {
        // 90 degrees -> south: start near (0.5, 0), end near (0.5, 1).
        let g = Gradient::linear_from_angle(90.0, vec![(0.0, Color::hex("#000000"))]);
        assert!((g.start.0 - 0.5).abs() < 1e-5);
        assert!((g.start.1 - 0.0).abs() < 1e-5);
        assert!((g.end.0 - 0.5).abs() < 1e-5);
        assert!((g.end.1 - 1.0).abs() < 1e-5);
    }
}
