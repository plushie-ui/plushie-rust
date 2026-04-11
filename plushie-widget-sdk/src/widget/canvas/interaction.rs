//! Canvas interactive element collection, hit testing, and validation.

use iced::{Point, Rectangle, mouse};

use plushie_core::types::canvas::{
    CanvasShape, GroupShape, Transform,
};

use super::types::*;
use crate::protocol::OutgoingEvent;

/// Test whether a point is inside a hit region.
///
/// Uses a small epsilon (0.5px) for boundary comparisons to handle
/// floating-point imprecision from transform matrix inversion. Without
/// this, points exactly on the boundary of a rotated element would
/// sometimes miss due to rounding errors.
pub(super) fn hit_test(point: Point, region: &HitRegion) -> bool {
    /// Half-pixel tolerance for boundary comparisons after transform.
    const EPS: f32 = 0.5;

    match *region {
        HitRegion::Rect { x, y, w, h } => {
            point.x >= x - EPS
                && point.x <= x + w + EPS
                && point.y >= y - EPS
                && point.y <= y + h + EPS
        }
        HitRegion::Circle { cx, cy, r } => {
            let dx = point.x - cx;
            let dy = point.y - cy;
            dx * dx + dy * dy <= r * r
        }
        HitRegion::Line {
            x1,
            y1,
            x2,
            y2,
            half_width,
        } => {
            // Distance from point to line segment.
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len_sq = dx * dx + dy * dy;
            if len_sq < f32::EPSILON {
                // Degenerate line (zero length) -- treat as point.
                let d = ((point.x - x1).powi(2) + (point.y - y1).powi(2)).sqrt();
                return d <= half_width;
            }
            // Project point onto line, clamped to segment.
            let t = ((point.x - x1) * dx + (point.y - y1) * dy) / len_sq;
            let t = t.clamp(0.0, 1.0);
            let proj_x = x1 + t * dx;
            let proj_y = y1 + t * dy;
            let dist_sq = (point.x - proj_x).powi(2) + (point.y - proj_y).powi(2);
            dist_sq <= half_width * half_width
        }
    }
}

/// Find the topmost interactive element under the given point.
///
/// Elements are tested in reverse order (last in list = topmost drawn = tested first).
/// The cursor point (in canvas space) is transformed into each element's
/// local coordinate space using the precomputed inverse transform matrix
/// before testing against the local-coordinate hit region.
///
/// Elements with a `clip_rect` are only hittable when the cursor is
/// inside the clip rectangle (tested in canvas space, before transform).
pub(super) fn find_hit_element(
    point: Point,
    elements: &[InteractiveElement],
) -> Option<&InteractiveElement> {
    elements.iter().rev().find(|e| {
        if !(e.on_click || e.on_hover || e.draggable) {
            return false;
        }
        // Clip test in canvas space.
        if let Some((cx, cy, cw, ch)) = e.clip_rect
            && (point.x < cx || point.x > cx + cw || point.y < cy || point.y > cy + ch)
        {
            return false;
        }
        // Transform cursor to element's local space.
        let local = match &e.inverse_transform {
            Some(inv) => {
                let (lx, ly) = inv.transform_point(point.x, point.y);
                Point::new(lx, ly)
            }
            // Singular transform -- element can't be hit.
            None => return false,
        };
        hit_test(local, &e.hit_region)
    })
}

/// Parse an [`InteractiveElement`] from a typed [`GroupShape`].
///
/// A group is interactive when it has an `id` field. Interactive
/// properties (`on_click`, `hover_style`, etc.) are typed fields on
/// the struct.
///
/// Returns `None` if the group has no id.
pub(super) fn parse_interactive_element(
    group: &GroupShape,
    layer_name: &str,
) -> Option<InteractiveElement> {
    let id = group.id.as_ref()?.clone();
    if id.is_empty() {
        return None;
    }

    // Warn on common mistakes.
    let draggable = group.draggable.unwrap_or(false);
    if !draggable && group.drag_bounds.is_some() {
        log::warn!("canvas element '{id}': drag_bounds set without draggable: true");
    }
    if !draggable && group.drag_axis.is_some() {
        log::warn!("canvas element '{id}': drag_axis set without draggable: true");
    }

    let hit_region = compute_hit_region(group)?;

    let drag_axis = match group.drag_axis {
        Some(plushie_core::types::canvas::DragAxis::X) => DragAxis::X,
        Some(plushie_core::types::canvas::DragAxis::Y) => DragAxis::Y,
        _ => DragAxis::Both,
    };

    let drag_bounds = group.drag_bounds.as_ref().map(|db| {
        let min_x = db.min_x.unwrap_or(f32::NEG_INFINITY);
        let max_x = db.max_x.unwrap_or(f32::INFINITY);
        let min_y = db.min_y.unwrap_or(f32::NEG_INFINITY);
        let max_y = db.max_y.unwrap_or(f32::INFINITY);
        DragBounds {
            min_x: min_x.min(max_x),
            max_x: min_x.max(max_x),
            min_y: min_y.min(max_y),
            max_y: min_y.max(max_y),
        }
    });

    Some(InteractiveElement {
        id,
        layer: layer_name.to_string(),
        hit_region,
        transform: TransformMatrix::identity(),
        inverse_transform: Some(TransformMatrix::identity()),
        clip_rect: None,
        on_click: group.on_click.unwrap_or(false),
        on_hover: group.on_hover.unwrap_or(false),
        draggable,
        drag_axis,
        drag_bounds,
        cursor: group.cursor.clone(),
        has_hover_style: group.hover_style.is_some(),
        has_pressed_style: group.pressed_style.is_some(),
        has_focus_style: group.focus_style.is_some(),
        show_focus_ring: group.show_focus_ring.unwrap_or(true),
        focus_ring_radius: group.focus_ring_radius,
        focusable: group.focusable.unwrap_or(false),
        parent_group: None,
        tooltip: group.tooltip.clone(),
        a11y: group.a11y.as_ref().map(crate::a11y::A11yOverrides::from_core),
    })
}

/// Compute the hit region for a group from its children's geometry.
///
/// Hit regions are in **local** coordinates (before transforms are applied).
/// Transform matrices are composed separately during hit testing.
///
/// An explicit `hit_rect` on the group overrides automatic inference.
/// `hit_rect` is in the group's local coordinate space.
fn compute_hit_region(group: &GroupShape) -> Option<HitRegion> {
    // Explicit hit_rect overrides geometric inference.
    if let Some(ref hr) = group.hit_rect {
        return Some(HitRegion::Rect {
            x: hr.x,
            y: hr.y,
            w: hr.w,
            h: hr.h,
        });
    }

    // Infer from children's bounding box.
    let (min_x, min_y, max_x, max_y) = children_bounds(&group.children)?;
    Some(HitRegion::Rect {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
    })
}

/// Parse a cursor name string into an iced mouse interaction.
pub(super) fn parse_cursor_interaction(cursor: &str) -> mouse::Interaction {
    match cursor {
        "pointer" => mouse::Interaction::Pointer,
        "grab" => mouse::Interaction::Grab,
        "grabbing" => mouse::Interaction::Grabbing,
        "crosshair" => mouse::Interaction::Crosshair,
        "move" => mouse::Interaction::Move,
        "text" => mouse::Interaction::Text,
        "not_allowed" | "not-allowed" => mouse::Interaction::NotAllowed,
        "no_drop" | "no-drop" => mouse::Interaction::NoDrop,
        "help" => mouse::Interaction::Help,
        "progress" => mouse::Interaction::Progress,
        "wait" => mouse::Interaction::Wait,
        "cell" => mouse::Interaction::Cell,
        "copy" => mouse::Interaction::Copy,
        "alias" => mouse::Interaction::Alias,
        "zoom_in" | "zoom-in" => mouse::Interaction::ZoomIn,
        "zoom_out" | "zoom-out" => mouse::Interaction::ZoomOut,
        "col_resize" | "col-resize" => mouse::Interaction::ResizingColumn,
        "row_resize" | "row-resize" => mouse::Interaction::ResizingRow,
        _ => mouse::Interaction::Pointer, // default for interactive elements
    }
}

/// Convert a HitRegion to a bounding Rectangle for accessibility.
pub(super) fn hit_region_to_rect(region: &HitRegion) -> Rectangle {
    match *region {
        HitRegion::Rect { x, y, w, h } => Rectangle {
            x,
            y,
            width: w,
            height: h,
        },
        HitRegion::Circle { cx, cy, r } => Rectangle {
            x: cx - r,
            y: cy - r,
            width: r * 2.0,
            height: r * 2.0,
        },
        HitRegion::Line {
            x1,
            y1,
            x2,
            y2,
            half_width,
        } => {
            let min_x = x1.min(x2) - half_width;
            let min_y = y1.min(y2) - half_width;
            let max_x = x1.max(x2) + half_width;
            let max_y = y1.max(y2) + half_width;
            Rectangle {
                x: min_x,
                y: min_y,
                width: max_x - min_x,
                height: max_y - min_y,
            }
        }
    }
}

/// Compute the center point of a hit region.
pub(super) fn hit_region_center(region: &HitRegion) -> Point {
    match *region {
        HitRegion::Rect { x, y, w, h } => Point::new(x + w / 2.0, y + h / 2.0),
        HitRegion::Circle { cx, cy, .. } => Point::new(cx, cy),
        HitRegion::Line { x1, y1, x2, y2, .. } => Point::new((x1 + x2) / 2.0, (y1 + y2) / 2.0),
    }
}

/// Compute the axis-aligned bounding box of a rectangle after applying
/// a transform, offset by a canvas position. Transforms all four corners
/// to handle rotation and shear correctly.
pub(super) fn transformed_bounds(
    rect: Rectangle,
    transform: &TransformMatrix,
    canvas_pos: iced::Point,
) -> Rectangle {
    let corners = [
        transform.transform_point(rect.x, rect.y),
        transform.transform_point(rect.x + rect.width, rect.y),
        transform.transform_point(rect.x, rect.y + rect.height),
        transform.transform_point(rect.x + rect.width, rect.y + rect.height),
    ];
    let min_x = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
    let min_y = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
    let max_x = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
    let max_y = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);
    Rectangle {
        x: canvas_pos.x + min_x,
        y: canvas_pos.y + min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

/// Serialize a mouse button for canvas events.
pub(super) fn serialize_mouse_button_for_canvas(button: &mouse::Button) -> String {
    match button {
        mouse::Button::Left => "left".to_string(),
        mouse::Button::Right => "right".to_string(),
        mouse::Button::Middle => "middle".to_string(),
        mouse::Button::Back => "back".to_string(),
        mouse::Button::Forward => "forward".to_string(),
        mouse::Button::Other(n) => format!("other_{n}"),
    }
}

/// Intersect two axis-aligned rectangles. Returns the intersection rect
/// as `(x, y, w, h)`. If the rectangles don't overlap, returns a
/// zero-area rect (w=0 or h=0).
pub(super) fn intersect_rects(
    a: (f32, f32, f32, f32),
    b: (f32, f32, f32, f32),
) -> (f32, f32, f32, f32) {
    let x = a.0.max(b.0);
    let y = a.1.max(b.1);
    let w = ((a.0 + a.2).min(b.0 + b.2) - x).max(0.0);
    let h = ((a.1 + a.3).min(b.1 + b.3) - y).max(0.0);
    (x, y, w, h)
}

/// Recursively collect interactive elements from a shape array, descending
/// into groups. The `parent_transform` accumulates the full 2D affine
/// transform from all ancestor groups, enabling correct hit testing for
/// rotated, scaled, and translated elements.
///
/// The optional `parent_clip` is the intersection of all ancestor clip
/// rectangles (in canvas space). Elements are only hittable when the
/// cursor falls within this clip region.
///
/// `focusable_parent` is the ID of the nearest ancestor focusable group.
///
/// `id_prefix` is the hierarchical path of ancestor interactive groups.
/// When a group "inner" is nested inside group "outer", its element_id
/// becomes "outer/inner". This produces scoped wire IDs like
/// "canvas/outer/inner" so the SDK's scope chain dispatch can walk
/// through nested canvas_widgets.
///
/// Children of a focusable group get `parent_group = Some(group_id)`,
/// which controls two-level keyboard navigation: Tab moves between
/// top-level entries, arrows navigate within a focused group's children.
///
/// Only groups with an `id` field are collected as interactive elements.
/// Non-group shapes are skipped.
pub(crate) fn collect_interactive_elements(
    shapes: &[CanvasShape],
    layer_name: &str,
    parent_transform: TransformMatrix,
    parent_clip: Option<(f32, f32, f32, f32)>,
    focusable_parent: Option<&str>,
    id_prefix: &str,
    out: &mut Vec<InteractiveElement>,
) {
    for shape in shapes {
        let group = match shape {
            CanvasShape::Group(g) => g,
            _ => continue,
        };

        // Compose this group's transforms with the parent's accumulated matrix.
        let group_matrix = if !group.transforms.is_empty() {
            let local = TransformMatrix::from_typed_transforms(&group.transforms);
            parent_transform.compose(&local)
        } else {
            parent_transform
        };

        // Intersect this group's clip (if any) with parent clip.
        let group_clip = if let Some(ref clip) = group.clip {
            let corners = [
                group_matrix.transform_point(clip.x, clip.y),
                group_matrix.transform_point(clip.x + clip.w, clip.y),
                group_matrix.transform_point(clip.x, clip.y + clip.h),
                group_matrix.transform_point(clip.x + clip.w, clip.y + clip.h),
            ];
            let min_x = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min);
            let min_y = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min);
            let max_x = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max);
            let max_y = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max);

            let clip_in_canvas = (min_x, min_y, max_x - min_x, max_y - min_y);

            match parent_clip {
                Some(pc) => Some(intersect_rects(pc, clip_in_canvas)),
                None => Some(clip_in_canvas),
            }
        } else {
            parent_clip
        };

        // Collect this group if it's interactive (has an id).
        let mut child_focusable_parent = focusable_parent;
        let mut focusable_group_id: Option<String> = None;
        let mut child_id_prefix = id_prefix.to_string();

        if let Some(mut element) = parse_interactive_element(group, layer_name) {
            if !id_prefix.is_empty() {
                element.id = format!("{}/{}", id_prefix, element.id);
            }

            element.transform = group_matrix;
            element.inverse_transform = group_matrix.inverse();
            element.clip_rect = group_clip;
            element.parent_group = focusable_parent.map(|s| s.to_string());

            if element.focusable {
                focusable_group_id = Some(element.id.clone());
            }

            child_id_prefix = element.id.clone();
            out.push(element);
        }

        if let Some(ref gid) = focusable_group_id {
            child_focusable_parent = Some(gid.as_str());
        }

        // Recurse into group children to find nested interactive elements.
        collect_interactive_elements(
            &group.children,
            layer_name,
            group_matrix,
            group_clip,
            child_focusable_parent,
            &child_id_prefix,
            out,
        );
    }
}

/// Validate interactive elements and return diagnostic events for common
/// accessibility issues. Called once per tree snapshot/patch.
pub(crate) fn validate_interactive_elements(
    canvas_id: &str,
    elements: &[InteractiveElement],
) -> Vec<OutgoingEvent> {
    let mut diagnostics = Vec::new();

    for element in elements {
        // Interactive element without a11y metadata.
        if element.a11y.is_none() {
            diagnostics.push(OutgoingEvent::diagnostic(
                canvas_id.to_string(),
                Some(element.id.clone()),
                "warning",
                "canvas_no_a11y",
                &format!(
                    "interactive element '{}' has no a11y metadata; \
                     focusable but invisible to screen readers",
                    element.id,
                ),
            ));
        }

        if let Some(ref a11y) = element.a11y {
            use iced::advanced::widget::operation::accessible as acc;
            // Switch without toggled state.
            if a11y.role() == Some(acc::Role::Switch) && a11y.toggled().is_none() {
                diagnostics.push(OutgoingEvent::diagnostic(
                    canvas_id.to_string(),
                    Some(element.id.clone()),
                    "warning",
                    "canvas_switch_no_toggled",
                    &format!(
                        "element '{}' has role 'switch' without 'toggled' state",
                        element.id,
                    ),
                ));
            }
            // Radio without selected state.
            if a11y.role() == Some(acc::Role::RadioButton) && a11y.selected().is_none() {
                diagnostics.push(OutgoingEvent::diagnostic(
                    canvas_id.to_string(),
                    Some(element.id.clone()),
                    "warning",
                    "canvas_radio_no_selected",
                    &format!(
                        "element '{}' has role 'radio' without 'selected' state",
                        element.id,
                    ),
                ));
            }
            // Checkbox without toggled state.
            if a11y.role() == Some(acc::Role::CheckBox) && a11y.toggled().is_none() {
                diagnostics.push(OutgoingEvent::diagnostic(
                    canvas_id.to_string(),
                    Some(element.id.clone()),
                    "warning",
                    "canvas_checkbox_no_toggled",
                    &format!(
                        "element '{}' has role 'check_box' without 'toggled' state",
                        element.id,
                    ),
                ));
            }
        }
    }

    // Multiple elements without position_in_set.
    let interactive_count = elements.iter().filter(|e| e.parent_group.is_none()).count();
    if interactive_count > 1 {
        let missing_position = elements
            .iter()
            .filter(|e| e.parent_group.is_none())
            .filter(|e| {
                e.a11y
                    .as_ref()
                    .map(|a| a.position_in_set().is_none())
                    .unwrap_or(true)
            })
            .count();
        if missing_position > 0 && missing_position < interactive_count {
            // Partial annotation: some elements have position_in_set but
            // others don't. This is likely an oversight and produces an
            // inconsistent screen reader experience.
            diagnostics.push(OutgoingEvent::diagnostic(
                canvas_id.to_string(),
                None,
                "warning",
                "canvas_partial_set_position",
                &format!(
                    "{} of {} interactive elements missing position_in_set/size_of_set; \
                     annotate all elements in the set for consistent screen reader context",
                    missing_position, interactive_count,
                ),
            ));
        } else if missing_position == interactive_count {
            diagnostics.push(OutgoingEvent::diagnostic(
                canvas_id.to_string(),
                None,
                "info",
                "canvas_no_set_position",
                &format!(
                    "{} interactive elements without position_in_set/size_of_set; \
                     consider adding set position for screen reader context",
                    interactive_count,
                ),
            ));
        }
    }

    diagnostics
}

// ---------------------------------------------------------------------------
// Bounds helpers (used by compute_hit_region)
// ---------------------------------------------------------------------------

/// Extract the net translation from a group's `"transforms"` array.
///
/// Scans for `{"type": "translate", "x": ..., "y": ...}` entries and
/// sums their x/y components. Non-translate transforms (rotate, scale)
/// are ignored for this purpose -- they affect hit testing via the
/// transform matrix in Phase 2.5, not via this simple offset.
/// Compute the translation offset from a group's transforms.
fn group_translation(transforms: &[Transform]) -> (f32, f32) {
    let mut tx = 0.0f32;
    let mut ty = 0.0f32;
    for t in transforms {
        if let Transform::Translate { x, y } = t {
            tx += x;
            ty += y;
        }
    }
    (tx, ty)
}

/// Compute the bounding box of a single shape in its parent's coordinate
/// system. Returns `(min_x, min_y, max_x, max_y)` or `None` if bounds
/// can't be determined for this shape type.
fn child_bounds(child: &CanvasShape) -> Option<(f32, f32, f32, f32)> {
    match child {
        CanvasShape::Rect(r) => Some((r.x, r.y, r.x + r.w, r.y + r.h)),
        CanvasShape::Circle(c) => Some((c.x - c.r, c.y - c.r, c.x + c.r, c.y + c.r)),
        CanvasShape::Line(l) => Some((
            l.x1.min(l.x2),
            l.y1.min(l.y2),
            l.x1.max(l.x2),
            l.y1.max(l.y2),
        )),
        CanvasShape::Text(t) => {
            let size = t.size.unwrap_or(16.0);
            let est_w = t.content.chars().count() as f32 * size * 0.6;
            Some((t.x, t.y - size, t.x + est_w, t.y))
        }
        CanvasShape::Image(i) => Some((i.x, i.y, i.x + i.w, i.y + i.h)),
        CanvasShape::Svg(s) => Some((s.x, s.y, s.x + s.w, s.y + s.h)),
        CanvasShape::Group(g) => {
            let (gx, gy) = group_translation(&g.transforms);
            let (min_x, min_y, max_x, max_y) = children_bounds(&g.children)?;
            Some((gx + min_x, gy + min_y, gx + max_x, gy + max_y))
        }
        CanvasShape::Path(p) => path_bounds(&p.commands),
    }
}

/// Compute bounding box of a path from its commands.
/// Examines move_to, line_to, and arc endpoints. Bezier control points
/// are included conservatively (they bound the curve).
fn path_bounds(commands: &[plushie_core::types::canvas::PathCommand]) -> Option<(f32, f32, f32, f32)> {
    use plushie_core::types::canvas::PathCommand;

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut has_point = false;

    let mut update = |x: f32, y: f32| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        has_point = true;
    };

    for cmd in commands {
        match cmd {
            PathCommand::MoveTo { x, y } | PathCommand::LineTo { x, y } => {
                update(*x, *y);
            }
            PathCommand::BezierTo { cp1x, cp1y, cp2x, cp2y, x, y } => {
                update(*cp1x, *cp1y);
                update(*cp2x, *cp2y);
                update(*x, *y);
            }
            PathCommand::QuadraticTo { cpx, cpy, x, y } => {
                update(*cpx, *cpy);
                update(*x, *y);
            }
            PathCommand::Arc { cx, cy, radius, .. } => {
                update(cx - radius, cy - radius);
                update(cx + radius, cy + radius);
            }
            PathCommand::ArcTo { x1, y1, x2, y2, .. } => {
                update(*x1, *y1);
                update(*x2, *y2);
            }
            PathCommand::Ellipse { cx, cy, rx, ry, .. } => {
                update(cx - rx, cy - ry);
                update(cx + rx, cy + ry);
            }
            PathCommand::RoundedRect { x, y, w, h, .. } => {
                update(*x, *y);
                update(x + w, y + h);
            }
            PathCommand::Close => {}
        }
    }

    has_point.then_some((min_x, min_y, max_x, max_y))
}

/// Compute the union bounding box of a list of child shapes.
/// Returns `(min_x, min_y, max_x, max_y)` or `None` if no children
/// have computable bounds.
fn children_bounds(children: &[CanvasShape]) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut has_bounds = false;
    for child in children {
        if let Some((cx0, cy0, cx1, cy1)) = child_bounds(child) {
            min_x = min_x.min(cx0);
            min_y = min_y.min(cy0);
            max_x = max_x.max(cx1);
            max_y = max_y.max(cy1);
            has_bounds = true;
        }
    }
    has_bounds.then_some((min_x, min_y, max_x, max_y))
}
