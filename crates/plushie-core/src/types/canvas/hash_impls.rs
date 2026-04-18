//! `Hash` implementations for canvas shape types.
//!
//! Floats are not `Hash` in std, so each `f32` is hashed via
//! [`f32::to_bits`]. This matches the pattern used by
//! `plushie_widget_sdk::shared_state::hash_json_value` for JSON numbers.
//!
//! These hashes feed per-layer tessellation caches in `CanvasEngine`;
//! they are process-local and never persisted. Bitwise equality of
//! f32s (including NaN payloads) is acceptable for that use case -
//! different bit patterns mean the cache should invalidate.

use std::hash::{Hash, Hasher};

use crate::types::{A11y, Angle, Color, Gradient, GradientStop, Radius};

use super::{
    CanvasFill, CanvasShape, CircleShape, ClipRect, Dash, DragAxis, DragBounds, FillRule,
    GroupShape, HitRect, ImageShape, LineCap, LineJoin, LineShape, PathCommand, PathShape,
    RectShape, ShapeStyle, Stroke, SvgShape, TextShape, Transform,
};

// ---------------------------------------------------------------------------
// Float helpers
// ---------------------------------------------------------------------------

fn hash_f32<H: Hasher>(v: f32, state: &mut H) {
    v.to_bits().hash(state);
}

fn hash_f32_opt<H: Hasher>(v: Option<f32>, state: &mut H) {
    match v {
        Some(f) => {
            1u8.hash(state);
            hash_f32(f, state);
        }
        None => 0u8.hash(state),
    }
}

// ---------------------------------------------------------------------------
// Core primitives (Angle, Color, Radius, Gradient, A11y)
// ---------------------------------------------------------------------------

impl Hash for Angle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash in a canonical unit so Angle::deg(180) and Angle::rad(PI)
        // collide for cache purposes (matches `PartialEq`'s approx_eq).
        // Use degrees as the canonical form to minimise precision drift
        // since degrees are the wire representation.
        hash_f32(self.degrees(), state);
    }
}

impl Hash for Color {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_hex().hash(state);
    }
}

impl Hash for Radius {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Radius::Uniform(r) => {
                0u8.hash(state);
                hash_f32(*r, state);
            }
            Radius::PerCorner {
                top_left,
                top_right,
                bottom_right,
                bottom_left,
            } => {
                1u8.hash(state);
                hash_f32(*top_left, state);
                hash_f32(*top_right, state);
                hash_f32(*bottom_right, state);
                hash_f32(*bottom_left, state);
            }
        }
    }
}

impl Hash for GradientStop {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32(self.offset, state);
        self.color.hash(state);
    }
}

impl Hash for Gradient {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32(self.start.0, state);
        hash_f32(self.start.1, state);
        hash_f32(self.end.0, state);
        hash_f32(self.end.1, state);
        self.stops.len().hash(state);
        for stop in &self.stops {
            stop.hash(state);
        }
    }
}

impl Hash for A11y {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.role.hash(state);
        self.label.hash(state);
        self.description.hash(state);
        self.hidden.hash(state);
        self.expanded.hash(state);
        self.required.hash(state);
        self.level.hash(state);
        // Live / Orientation / HasPopup all derive Hash via PlushieEnum.
        self.live.hash(state);
        self.busy.hash(state);
        self.invalid.hash(state);
        self.modal.hash(state);
        self.read_only.hash(state);
        self.mnemonic.hash(state);
        self.toggled.hash(state);
        self.selected.hash(state);
        self.value.hash(state);
        self.orientation.hash(state);
        self.disabled.hash(state);
        self.position_in_set.hash(state);
        self.size_of_set.hash(state);
        self.labelled_by.hash(state);
        self.described_by.hash(state);
        self.error_message.hash(state);
        self.active_descendant.hash(state);
        self.radio_group.hash(state);
        self.has_popup.hash(state);
        self.label_from.hash(state);
    }
}

// ---------------------------------------------------------------------------
// Canvas-specific sub-types
// ---------------------------------------------------------------------------

impl Hash for ClipRect {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        hash_f32(self.w, state);
        hash_f32(self.h, state);
    }
}

impl Hash for HitRect {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        hash_f32(self.w, state);
        hash_f32(self.h, state);
    }
}

impl Hash for DragBounds {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_f32_opt(self.min_x, state);
        hash_f32_opt(self.max_x, state);
        hash_f32_opt(self.min_y, state);
        hash_f32_opt(self.max_y, state);
    }
}

impl Hash for DragAxis {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self as u8).hash(state);
    }
}

impl Hash for FillRule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self as u8).hash(state);
    }
}

impl Hash for LineCap {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self as u8).hash(state);
    }
}

impl Hash for LineJoin {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self as u8).hash(state);
    }
}

impl Hash for Dash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.segments.len().hash(state);
        for s in &self.segments {
            hash_f32(*s, state);
        }
        hash_f32(self.offset, state);
    }
}

impl Hash for Stroke {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.color.hash(state);
        hash_f32(self.width, state);
        self.cap.hash(state);
        self.join.hash(state);
        self.dash.hash(state);
    }
}

impl Hash for ShapeStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fill.hash(state);
        self.stroke.hash(state);
        hash_f32_opt(self.opacity, state);
    }
}

impl Hash for CanvasFill {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            CanvasFill::Color(c) => {
                0u8.hash(state);
                c.hash(state);
            }
            CanvasFill::Gradient(g) => {
                1u8.hash(state);
                g.hash(state);
            }
        }
    }
}

impl Hash for Transform {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Transform::Translate { x, y } => {
                0u8.hash(state);
                hash_f32(*x, state);
                hash_f32(*y, state);
            }
            Transform::Rotate { angle } => {
                1u8.hash(state);
                angle.hash(state);
            }
            Transform::Scale { x, y } => {
                2u8.hash(state);
                hash_f32(*x, state);
                hash_f32(*y, state);
            }
            Transform::ScaleUniform { factor } => {
                3u8.hash(state);
                hash_f32(*factor, state);
            }
        }
    }
}

impl Hash for PathCommand {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            PathCommand::MoveTo { x, y } => {
                0u8.hash(state);
                hash_f32(*x, state);
                hash_f32(*y, state);
            }
            PathCommand::LineTo { x, y } => {
                1u8.hash(state);
                hash_f32(*x, state);
                hash_f32(*y, state);
            }
            PathCommand::BezierTo {
                cp1x,
                cp1y,
                cp2x,
                cp2y,
                x,
                y,
            } => {
                2u8.hash(state);
                hash_f32(*cp1x, state);
                hash_f32(*cp1y, state);
                hash_f32(*cp2x, state);
                hash_f32(*cp2y, state);
                hash_f32(*x, state);
                hash_f32(*y, state);
            }
            PathCommand::QuadraticTo { cpx, cpy, x, y } => {
                3u8.hash(state);
                hash_f32(*cpx, state);
                hash_f32(*cpy, state);
                hash_f32(*x, state);
                hash_f32(*y, state);
            }
            PathCommand::Arc {
                cx,
                cy,
                radius,
                start_angle,
                end_angle,
            } => {
                4u8.hash(state);
                hash_f32(*cx, state);
                hash_f32(*cy, state);
                hash_f32(*radius, state);
                start_angle.hash(state);
                end_angle.hash(state);
            }
            PathCommand::ArcTo {
                x1,
                y1,
                x2,
                y2,
                radius,
            } => {
                5u8.hash(state);
                hash_f32(*x1, state);
                hash_f32(*y1, state);
                hash_f32(*x2, state);
                hash_f32(*y2, state);
                hash_f32(*radius, state);
            }
            PathCommand::Ellipse {
                cx,
                cy,
                rx,
                ry,
                rotation,
                start_angle,
                end_angle,
            } => {
                6u8.hash(state);
                hash_f32(*cx, state);
                hash_f32(*cy, state);
                hash_f32(*rx, state);
                hash_f32(*ry, state);
                rotation.hash(state);
                start_angle.hash(state);
                end_angle.hash(state);
            }
            PathCommand::RoundedRect { x, y, w, h, radius } => {
                7u8.hash(state);
                hash_f32(*x, state);
                hash_f32(*y, state);
                hash_f32(*w, state);
                hash_f32(*h, state);
                radius.hash(state);
            }
            PathCommand::Close => {
                8u8.hash(state);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shape structs
// ---------------------------------------------------------------------------

impl Hash for RectShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        hash_f32(self.w, state);
        hash_f32(self.h, state);
        self.fill.hash(state);
        self.stroke.hash(state);
        hash_f32_opt(self.opacity, state);
        self.fill_rule.hash(state);
        self.radius.hash(state);
    }
}

impl Hash for CircleShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        hash_f32(self.r, state);
        self.fill.hash(state);
        self.stroke.hash(state);
        hash_f32_opt(self.opacity, state);
        self.fill_rule.hash(state);
    }
}

impl Hash for LineShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        hash_f32(self.x1, state);
        hash_f32(self.y1, state);
        hash_f32(self.x2, state);
        hash_f32(self.y2, state);
        self.stroke.hash(state);
        hash_f32_opt(self.opacity, state);
    }
}

impl Hash for PathShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.commands.len().hash(state);
        for c in &self.commands {
            c.hash(state);
        }
        self.fill.hash(state);
        self.stroke.hash(state);
        hash_f32_opt(self.opacity, state);
        self.fill_rule.hash(state);
    }
}

impl Hash for TextShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        self.content.hash(state);
        self.fill.hash(state);
        hash_f32_opt(self.size, state);
        self.font.hash(state);
        self.align_x.hash(state);
        self.align_y.hash(state);
        hash_f32_opt(self.opacity, state);
    }
}

impl Hash for ImageShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        hash_f32(self.w, state);
        hash_f32(self.h, state);
        self.rotation.hash(state);
        hash_f32_opt(self.opacity, state);
    }
}

impl Hash for SvgShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        hash_f32(self.x, state);
        hash_f32(self.y, state);
        hash_f32(self.w, state);
        hash_f32(self.h, state);
    }
}

impl Hash for GroupShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.children.len().hash(state);
        for c in &self.children {
            c.hash(state);
        }
        self.transforms.len().hash(state);
        for t in &self.transforms {
            t.hash(state);
        }
        self.clip.hash(state);
        self.on_click.hash(state);
        self.on_hover.hash(state);
        self.draggable.hash(state);
        self.drag_axis.hash(state);
        self.drag_bounds.hash(state);
        self.cursor.hash(state);
        self.hit_rect.hash(state);
        self.tooltip.hash(state);
        self.hover_style.hash(state);
        self.pressed_style.hash(state);
        self.focus_style.hash(state);
        self.show_focus_ring.hash(state);
        hash_f32_opt(self.focus_ring_radius, state);
        self.focusable.hash(state);
        self.a11y.hash(state);
    }
}

impl Hash for CanvasShape {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            CanvasShape::Rect(r) => {
                0u8.hash(state);
                r.hash(state);
            }
            CanvasShape::Circle(c) => {
                1u8.hash(state);
                c.hash(state);
            }
            CanvasShape::Line(l) => {
                2u8.hash(state);
                l.hash(state);
            }
            CanvasShape::Path(p) => {
                3u8.hash(state);
                p.hash(state);
            }
            CanvasShape::Text(t) => {
                4u8.hash(state);
                t.hash(state);
            }
            CanvasShape::Image(i) => {
                5u8.hash(state);
                i.hash(state);
            }
            CanvasShape::Svg(s) => {
                6u8.hash(state);
                s.hash(state);
            }
            CanvasShape::Group(g) => {
                7u8.hash(state);
                g.hash(state);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use serde_json::json;

    use crate::protocol::TreeNode;
    use crate::types::canvas::CanvasShape;

    fn hash_shapes(shapes: &[CanvasShape]) -> u64 {
        let mut hasher = DefaultHasher::new();
        shapes.hash(&mut hasher);
        hasher.finish()
    }

    fn shape_from(type_name: &str, props: serde_json::Value) -> CanvasShape {
        let node: TreeNode = serde_json::from_value(json!({
            "id": "__auto__",
            "type": type_name,
            "props": props,
        }))
        .unwrap();
        CanvasShape::from_node(&node).unwrap()
    }

    #[test]
    fn identical_shape_sequences_hash_identically() {
        let a = vec![
            shape_from("rect", json!({"x": 1.0, "y": 2.0, "w": 3.0, "h": 4.0})),
            shape_from("circle", json!({"x": 5.0, "y": 6.0, "r": 7.0})),
        ];
        let b = vec![
            shape_from("rect", json!({"x": 1.0, "y": 2.0, "w": 3.0, "h": 4.0})),
            shape_from("circle", json!({"x": 5.0, "y": 6.0, "r": 7.0})),
        ];
        assert_eq!(hash_shapes(&a), hash_shapes(&b));
    }

    #[test]
    fn differing_shape_sequences_hash_differently() {
        let a = vec![shape_from(
            "rect",
            json!({"x": 1.0, "y": 2.0, "w": 3.0, "h": 4.0}),
        )];
        let b = vec![shape_from(
            "rect",
            json!({"x": 1.0, "y": 2.0, "w": 3.0, "h": 5.0}),
        )];
        assert_ne!(hash_shapes(&a), hash_shapes(&b));
    }

    #[test]
    fn variant_discriminant_separates_types() {
        // A rect and a circle with coincidentally-equal numeric fields
        // must not collide.
        let rect = vec![shape_from(
            "rect",
            json!({"x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0}),
        )];
        let circle = vec![shape_from("circle", json!({"x": 0.0, "y": 0.0, "r": 10.0}))];
        assert_ne!(hash_shapes(&rect), hash_shapes(&circle));
    }
}
