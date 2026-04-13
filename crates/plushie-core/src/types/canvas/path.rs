//! Typed path commands for canvas path shapes.

use serde_json::Value;

use crate::types::Angle;

/// A canvas path drawing command.
///
/// Angle fields use the [`Angle`] type. On the wire, angles are
/// transmitted in degrees (matching the cross-SDK convention).
#[derive(Debug, Clone, PartialEq)]
pub enum PathCommand {
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    BezierTo {
        cp1x: f32,
        cp1y: f32,
        cp2x: f32,
        cp2y: f32,
        x: f32,
        y: f32,
    },
    QuadraticTo {
        cpx: f32,
        cpy: f32,
        x: f32,
        y: f32,
    },
    Arc {
        cx: f32,
        cy: f32,
        radius: f32,
        start_angle: Angle,
        end_angle: Angle,
    },
    ArcTo {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        radius: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
        rotation: Angle,
        start_angle: Angle,
        end_angle: Angle,
    },
    RoundedRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
    },
    Close,
}

/// Decode an array of wire-format path commands into typed values.
///
/// Wire format: each command is either the string `"close"` or an array
/// `["command_name", arg1, arg2, ...]` where args are numbers.
pub fn decode_commands(value: &Value) -> Vec<PathCommand> {
    let arr = match value.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut result = Vec::with_capacity(arr.len());

    for cmd in arr {
        if let Some(s) = cmd.as_str() {
            if s == "close" {
                result.push(PathCommand::Close);
            }
            continue;
        }

        let parts = match cmd.as_array() {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };

        let cmd_name = match parts[0].as_str() {
            Some(n) => n,
            None => continue,
        };

        let f = |i: usize| -> f32 {
            parts
                .get(i)
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(0.0)
        };

        let parsed = match cmd_name {
            "move_to" => PathCommand::MoveTo { x: f(1), y: f(2) },
            "line_to" => PathCommand::LineTo { x: f(1), y: f(2) },
            "bezier_to" => PathCommand::BezierTo {
                cp1x: f(1),
                cp1y: f(2),
                cp2x: f(3),
                cp2y: f(4),
                x: f(5),
                y: f(6),
            },
            "quadratic_to" => PathCommand::QuadraticTo {
                cpx: f(1),
                cpy: f(2),
                x: f(3),
                y: f(4),
            },
            "arc" => PathCommand::Arc {
                cx: f(1),
                cy: f(2),
                radius: f(3),
                start_angle: Angle::deg(f(4)),
                end_angle: Angle::deg(f(5)),
            },
            "arc_to" => PathCommand::ArcTo {
                x1: f(1),
                y1: f(2),
                x2: f(3),
                y2: f(4),
                radius: f(5),
            },
            "ellipse" => PathCommand::Ellipse {
                cx: f(1),
                cy: f(2),
                rx: f(3),
                ry: f(4),
                rotation: Angle::deg(f(5)),
                start_angle: Angle::deg(f(6)),
                end_angle: Angle::deg(f(7)),
            },
            "rounded_rect" => PathCommand::RoundedRect {
                x: f(1),
                y: f(2),
                w: f(3),
                h: f(4),
                radius: f(5),
            },
            _ => continue,
        };

        result.push(parsed);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn decode_close_string() {
        let cmds = decode_commands(&json!(["close"]));
        assert_eq!(cmds, vec![PathCommand::Close]);
    }

    #[test]
    fn decode_move_and_line() {
        let cmds = decode_commands(&json!([["move_to", 10.0, 20.0], ["line_to", 30.0, 40.0]]));
        assert_eq!(
            cmds,
            vec![
                PathCommand::MoveTo { x: 10.0, y: 20.0 },
                PathCommand::LineTo { x: 30.0, y: 40.0 },
            ]
        );
    }

    #[test]
    fn decode_bezier() {
        let cmds = decode_commands(&json!([["bezier_to", 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]]));
        assert_eq!(
            cmds,
            vec![PathCommand::BezierTo {
                cp1x: 1.0,
                cp1y: 2.0,
                cp2x: 3.0,
                cp2y: 4.0,
                x: 5.0,
                y: 6.0,
            },]
        );
    }

    #[test]
    fn decode_quadratic() {
        let cmds = decode_commands(&json!([["quadratic_to", 1.0, 2.0, 3.0, 4.0]]));
        assert_eq!(
            cmds,
            vec![PathCommand::QuadraticTo {
                cpx: 1.0,
                cpy: 2.0,
                x: 3.0,
                y: 4.0
            },]
        );
    }

    #[test]
    fn decode_arc() {
        let cmds = decode_commands(&json!([["arc", 50.0, 50.0, 25.0, 0.0, 180.0]]));
        assert_eq!(
            cmds,
            vec![PathCommand::Arc {
                cx: 50.0,
                cy: 50.0,
                radius: 25.0,
                start_angle: Angle::deg(0.0),
                end_angle: Angle::deg(180.0),
            },]
        );
    }

    #[test]
    fn decode_arc_to() {
        let cmds = decode_commands(&json!([["arc_to", 1.0, 2.0, 3.0, 4.0, 5.0]]));
        assert_eq!(
            cmds,
            vec![PathCommand::ArcTo {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                radius: 5.0
            },]
        );
    }

    #[test]
    fn decode_ellipse() {
        let cmds = decode_commands(&json!([[
            "ellipse", 50.0, 50.0, 30.0, 20.0, 30.0, 0.0, 360.0
        ]]));
        assert_eq!(
            cmds,
            vec![PathCommand::Ellipse {
                cx: 50.0,
                cy: 50.0,
                rx: 30.0,
                ry: 20.0,
                rotation: Angle::deg(30.0),
                start_angle: Angle::deg(0.0),
                end_angle: Angle::deg(360.0),
            },]
        );
    }

    #[test]
    fn decode_rounded_rect() {
        let cmds = decode_commands(&json!([["rounded_rect", 10.0, 20.0, 100.0, 50.0, 8.0]]));
        assert_eq!(
            cmds,
            vec![PathCommand::RoundedRect {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 50.0,
                radius: 8.0
            },]
        );
    }

    #[test]
    fn decode_mixed_commands() {
        let cmds = decode_commands(&json!([
            ["move_to", 0, 0],
            ["line_to", 100, 0],
            ["line_to", 50, 80],
            "close"
        ]));
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[3], PathCommand::Close);
    }

    #[test]
    fn decode_unknown_skipped() {
        let cmds = decode_commands(&json!([["move_to", 0, 0], ["wibble", 1, 2, 3], "close"]));
        assert_eq!(
            cmds,
            vec![PathCommand::MoveTo { x: 0.0, y: 0.0 }, PathCommand::Close,]
        );
    }

    #[test]
    fn decode_non_array_returns_empty() {
        assert!(decode_commands(&json!("not an array")).is_empty());
        assert!(decode_commands(&json!(42)).is_empty());
    }
}
