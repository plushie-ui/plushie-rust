use super::*;
use crate::protocol::TreeNode;
use crate::shared_state::hash_str;
use iced::widget::canvas;
use iced::{Color, Point, alignment};
use plushie_core::types::{CanvasShape, GroupShape};
use serde_json::Value;
use serde_json::json;

/// Convert a flat JSON shape ({"type":"group","id":"g","on_click":true,...})
/// into a TreeNode ({"id":"g","type":"group","props":{"on_click":true},...}).
fn flat_shape_to_tree_node(val: &Value) -> TreeNode {
    let obj = val.as_object().expect("expected JSON object");
    let type_name = obj
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("__auto__")
        .to_string();

    // Everything except type, id, children goes into props.
    let mut props = serde_json::Map::new();
    for (k, v) in obj {
        if k == "type" || k == "id" || k == "children" {
            continue;
        }
        props.insert(k.clone(), v.clone());
    }

    let children: Vec<TreeNode> = obj
        .get("children")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(flat_shape_to_tree_node).collect())
        .unwrap_or_default();

    TreeNode {
        id,
        type_name,
        props: plushie_core::protocol::Props::from_json(Value::Object(props)),
        children,
    }
}

/// Helper: convert a flat JSON group shape into a GroupShape via TreeNode.
fn group_from_json(val: &Value) -> GroupShape {
    let node = flat_shape_to_tree_node(val);
    GroupShape::from_node(&node)
}

/// Helper: convert a flat JSON shape array to Vec<CanvasShape> via TreeNodes.
fn shapes_from_json(shapes: &[Value]) -> Vec<CanvasShape> {
    shapes
        .iter()
        .filter_map(|v| {
            let node = flat_shape_to_tree_node(v);
            CanvasShape::from_node(&node)
        })
        .collect()
}

/// Helper: collect interactive elements from flat JSON shape values.
/// Wraps the conversion from JSON to typed shapes + the actual collection.
fn collect_interactive_from_json(
    shapes: &[Value],
    layer_name: &str,
    parent_transform: TransformMatrix,
    parent_clip: Option<(f32, f32, f32, f32)>,
    focusable_parent: Option<&str>,
    id_prefix: &str,
    out: &mut Vec<InteractiveElement>,
) {
    let typed = shapes_from_json(shapes);
    collect_interactive_elements(
        &typed,
        layer_name,
        parent_transform,
        parent_clip,
        focusable_parent,
        id_prefix,
        out,
    );
}

/// Helper: build a TreeNode with given children and optional props.
fn make_canvas_node(props: Value, children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: "test-canvas".to_string(),
        type_name: "canvas".to_string(),
        props: plushie_core::protocol::Props::from_json(props),
        children,
    }
}

fn make_layer_node(name: &str, shape_children: Vec<TreeNode>) -> TreeNode {
    TreeNode {
        id: format!("auto:layer:{name}"),
        type_name: "__layer__".to_string(),
        props: plushie_core::protocol::Props::from_json(json!({"name": name})),
        children: shape_children,
    }
}

fn make_shape_node(id: &str, type_name: &str, props: Value) -> TreeNode {
    TreeNode {
        id: id.to_string(),
        type_name: type_name.to_string(),
        props: plushie_core::protocol::Props::from_json(props),
        children: vec![],
    }
}

#[test]
fn canvas_layers_from_layer_children() {
    let node = make_canvas_node(
        json!({}),
        vec![
            make_layer_node(
                "background",
                vec![make_shape_node(
                    "auto:shape:bg:0",
                    "rect",
                    json!({"width": 100}),
                )],
            ),
            make_layer_node(
                "foreground",
                vec![make_shape_node(
                    "auto:shape:fg:0",
                    "circle",
                    json!({"radius": 50}),
                )],
            ),
        ],
    );
    let result = canvas_layers_from_node(&node);
    assert_eq!(result.len(), 2);
    assert!(result.contains_key("background"));
    assert!(result.contains_key("foreground"));
    let bg = result.get("background").unwrap();
    assert_eq!(bg.len(), 1);
    // Shape should be a Rect variant.
    assert!(matches!(&bg[0], plushie_core::types::CanvasShape::Rect(_)));
}

#[test]
fn canvas_flat_shape_children() {
    // Direct shape children without layer wrappers go into "default".
    let node = make_canvas_node(
        json!({}),
        vec![make_shape_node(
            "auto:shape:0",
            "line",
            json!({"x1": 0, "y1": 0, "x2": 100, "y2": 100}),
        )],
    );
    let result = canvas_layers_from_node(&node);
    assert_eq!(result.len(), 1);
    assert!(result.contains_key("default"));
}

#[test]
fn canvas_empty_children() {
    let node = make_canvas_node(json!({}), vec![]);
    let result = canvas_layers_from_node(&node);
    assert!(result.is_empty());
}

#[test]
fn canvas_hash_changes() {
    let hash_a = hash_str("[{\"type\":\"rect\"}]");
    let hash_b = hash_str("[{\"type\":\"circle\"}]");
    let hash_a2 = hash_str("[{\"type\":\"rect\"}]");

    // Same input produces same hash.
    assert_eq!(hash_a, hash_a2);
    // Different input produces different hash.
    assert_ne!(hash_a, hash_b);
}

#[test]
fn canvas_layer_sort_order() {
    let node = make_canvas_node(
        json!({}),
        vec![
            make_layer_node(
                "charlie",
                vec![make_shape_node("auto:shape:c:0", "rect", json!({}))],
            ),
            make_layer_node(
                "alpha",
                vec![make_shape_node("auto:shape:a:0", "circle", json!({}))],
            ),
            make_layer_node(
                "bravo",
                vec![make_shape_node("auto:shape:b:0", "line", json!({}))],
            ),
        ],
    );
    let result = canvas_layers_from_node(&node);
    let keys: Vec<&String> = result.keys().collect();
    assert_eq!(keys, vec!["alpha", "bravo", "charlie"]);
}

#[test]
fn canvas_path_commands_basic() {
    let shape = json!({
        "type": "path",
        "commands": [
            ["move_to", 10, 20],
            ["line_to", 30, 40],
            "close"
        ]
    });
    assert_eq!(shape.get("type").and_then(|v| v.as_str()), Some("path"));
    let commands = shape.get("commands").and_then(|v| v.as_array()).unwrap();
    assert_eq!(commands.len(), 3);
    // First command is an array starting with "move_to".
    let move_cmd = commands[0].as_array().unwrap();
    assert_eq!(move_cmd[0].as_str(), Some("move_to"));
    assert_eq!(move_cmd[1].as_f64(), Some(10.0));
    assert_eq!(move_cmd[2].as_f64(), Some(20.0));
    // Second command is an array starting with "line_to".
    let line_cmd = commands[1].as_array().unwrap();
    assert_eq!(line_cmd[0].as_str(), Some("line_to"));
    assert_eq!(line_cmd[1].as_f64(), Some(30.0));
    assert_eq!(line_cmd[2].as_f64(), Some(40.0));
    // Third command is the bare string "close".
    assert_eq!(commands[2].as_str(), Some("close"));
}

#[test]
fn canvas_stroke_parse() {
    let stroke_val = json!({
        "color": "#ff0000",
        "width": 3.0,
        "cap": "round",
        "join": "bevel"
    });
    let stroke = parse_canvas_stroke(&stroke_val);
    assert_eq!(
        stroke.style,
        canvas::Style::Solid(Color::from_rgb8(255, 0, 0))
    );
    assert_eq!(stroke.width, 3.0);
    // LineCap and LineJoin don't impl PartialEq, so use Debug format.
    assert_eq!(format!("{:?}", stroke.line_cap), "Round");
    assert_eq!(format!("{:?}", stroke.line_join), "Bevel");
}

#[test]
fn canvas_gradient_parse() {
    let fill_val = json!({
        "type": "linear",
        "start": [0.0, 0.0],
        "end": [100.0, 0.0],
        "stops": [
            [0.0, "#ff0000"],
            [1.0, "#0000ff"]
        ]
    });
    let shape = json!({"fill": fill_val.clone()});
    let fill = parse_canvas_fill(&fill_val, &shape);
    // The fill rule should be NonZero for gradient fills.
    assert_eq!(fill.rule, canvas::fill::Rule::NonZero);
    // The style should be a gradient, not a solid color.
    match &fill.style {
        canvas::Style::Gradient(canvas::Gradient::Linear(_)) => {}
        other => panic!("expected Gradient::Linear, got {other:?}"),
    }
}

#[test]
fn canvas_fill_rule_defaults_to_non_zero() {
    let fill_val = json!("#ff0000");
    let shape = json!({"fill": "#ff0000"});
    let fill = parse_canvas_fill(&fill_val, &shape);
    assert_eq!(fill.rule, canvas::fill::Rule::NonZero);
}

#[test]
fn canvas_fill_rule_even_odd() {
    let fill_val = json!("#00ff00");
    let shape = json!({"fill": "#00ff00", "fill_rule": "even_odd"});
    let fill = parse_canvas_fill(&fill_val, &shape);
    assert_eq!(fill.rule, canvas::fill::Rule::EvenOdd);
}

#[test]
fn canvas_fill_rule_explicit_non_zero() {
    let fill_val = json!("#0000ff");
    let shape = json!({"fill": "#0000ff", "fill_rule": "non_zero"});
    let fill = parse_canvas_fill(&fill_val, &shape);
    assert_eq!(fill.rule, canvas::fill::Rule::NonZero);
}

// Standalone clip tests removed; clips are now a group-level field.
// See draw_with_group_clip() and the "clip" field on group JSON.

// -- Text alignment tests --

#[test]
fn text_align_x_parses_left() {
    let v = json!("left");
    assert_eq!(
        format!("{:?}", shapes::parse_canvas_text_align_x(Some(&v))),
        "Left",
    );
}

#[test]
fn text_align_x_parses_center() {
    let v = json!("center");
    assert_eq!(
        format!("{:?}", shapes::parse_canvas_text_align_x(Some(&v))),
        "Center",
    );
}

#[test]
fn text_align_x_parses_right() {
    let v = json!("right");
    assert_eq!(
        format!("{:?}", shapes::parse_canvas_text_align_x(Some(&v))),
        "Right",
    );
}

#[test]
fn text_align_x_defaults_to_default() {
    assert_eq!(
        format!("{:?}", shapes::parse_canvas_text_align_x(None)),
        "Default",
    );
}

#[test]
fn text_align_y_parses_center() {
    let v = json!("center");
    assert_eq!(
        shapes::parse_canvas_text_align_y(Some(&v)),
        alignment::Vertical::Center
    );
}

#[test]
fn text_align_y_parses_bottom() {
    let v = json!("bottom");
    assert_eq!(
        shapes::parse_canvas_text_align_y(Some(&v)),
        alignment::Vertical::Bottom
    );
}

#[test]
fn text_align_y_defaults_to_top() {
    assert_eq!(
        shapes::parse_canvas_text_align_y(None),
        alignment::Vertical::Top,
    );
}

// -- Opacity tests --

#[test]
fn opacity_applied_to_fill() {
    let fill =
        shapes::apply_opacity_to_fill(Some(0.5), parse_canvas_fill(&json!("#ff0000"), &json!({})));
    match fill.style {
        canvas::Style::Solid(c) => {
            assert!(
                (c.a - 0.5).abs() < 0.001,
                "expected alpha ~0.5, got {}",
                c.a
            );
        }
        _ => panic!("expected solid fill"),
    }
}

#[test]
fn opacity_applied_to_stroke() {
    let stroke_val = json!({"color": "#00ff00", "width": 2.0});
    let stroke = shapes::apply_opacity_to_stroke(Some(0.25), parse_canvas_stroke(&stroke_val));
    match stroke.style {
        canvas::Style::Solid(c) => {
            assert!(
                (c.a - 0.25).abs() < 0.001,
                "expected alpha ~0.25, got {}",
                c.a
            );
        }
        _ => panic!("expected solid stroke"),
    }
}

#[test]
fn opacity_applied_to_color() {
    let color = shapes::apply_opacity_to_color(Some(0.75), Color::WHITE);
    assert!(
        (color.a - 0.75).abs() < 0.001,
        "expected alpha ~0.75, got {}",
        color.a
    );
}

#[test]
fn no_opacity_leaves_alpha_unchanged() {
    let fill =
        shapes::apply_opacity_to_fill(None, parse_canvas_fill(&json!("#ff0000"), &json!({})));
    match fill.style {
        canvas::Style::Solid(c) => {
            assert!(
                (c.a - 1.0).abs() < 0.001,
                "expected alpha ~1.0, got {}",
                c.a
            );
        }
        _ => panic!("expected solid fill"),
    }
}

// -- Hit testing --

#[test]
fn hit_test_rect_inside() {
    let region = HitRegion::Rect {
        x: 10.0,
        y: 20.0,
        w: 30.0,
        h: 40.0,
    };
    assert!(interaction::hit_test(Point::new(25.0, 40.0), &region));
}

#[test]
fn hit_test_rect_outside() {
    let region = HitRegion::Rect {
        x: 10.0,
        y: 20.0,
        w: 30.0,
        h: 40.0,
    };
    assert!(!interaction::hit_test(Point::new(5.0, 40.0), &region));
}

#[test]
fn hit_test_circle_inside() {
    let region = HitRegion::Circle {
        cx: 50.0,
        cy: 50.0,
        r: 20.0,
    };
    assert!(interaction::hit_test(Point::new(50.0, 50.0), &region));
    assert!(interaction::hit_test(Point::new(60.0, 50.0), &region));
}

#[test]
fn hit_test_circle_outside() {
    let region = HitRegion::Circle {
        cx: 50.0,
        cy: 50.0,
        r: 20.0,
    };
    assert!(!interaction::hit_test(Point::new(80.0, 50.0), &region));
}

#[test]
fn hit_test_line_near() {
    let region = HitRegion::Line {
        x1: 0.0,
        y1: 0.0,
        x2: 100.0,
        y2: 0.0,
        half_width: 5.0,
    };
    assert!(interaction::hit_test(Point::new(50.0, 3.0), &region));
    assert!(!interaction::hit_test(Point::new(50.0, 10.0), &region));
}

#[test]
fn hit_test_line_endpoint() {
    let region = HitRegion::Line {
        x1: 10.0,
        y1: 10.0,
        x2: 10.0,
        y2: 10.0,
        half_width: 5.0,
    };
    // Degenerate line (zero length): treated as point.
    assert!(interaction::hit_test(Point::new(12.0, 10.0), &region));
    assert!(!interaction::hit_test(Point::new(20.0, 10.0), &region));
}

// -- Interactive shape parsing --

#[test]
fn parse_interactive_group_basic() {
    let shape = json!({
        "type": "group",
        "id": "bar-1",
        "on_click": true,
        "on_hover": true,
        "cursor": "pointer",
        "tooltip": "Bar 1: 200 units",
        "children": [
            {"type": "rect", "x": 10, "y": 20, "w": 30, "h": 40, "fill": "#ff0000"}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    assert_eq!(result.id, "bar-1");
    assert!(result.on_click);
    assert!(result.on_hover);
    assert_eq!(result.cursor.as_deref(), Some("pointer"));
    assert_eq!(result.tooltip.as_deref(), Some("Bar 1: 200 units"));
    assert!(matches!(result.hit_region, HitRegion::Rect { .. }));
}

#[test]
fn parse_interactive_group_with_drag() {
    let shape = json!({
        "type": "group",
        "id": "dot-1",
        "on_click": true,
        "draggable": true,
        "drag_axis": "x",
        "children": [
            {"type": "circle", "x": 50, "y": 50, "r": 20}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "layer1").unwrap();
    assert_eq!(result.id, "dot-1");
    assert!(result.draggable);
    assert_eq!(result.drag_axis, DragAxis::X);
    // Group hit region is a bounding box (Rect), not Circle.
    assert!(matches!(result.hit_region, HitRegion::Rect { .. }));
}

#[test]
fn parse_interactive_group_with_hit_rect() {
    let shape = json!({
        "type": "group",
        "id": "path-group",
        "on_click": true,
        "hit_rect": {"x": 0, "y": 0, "w": 100, "h": 100},
        "children": [
            {"type": "path", "commands": [["move_to", 0, 0], ["line_to", 100, 100]]}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    assert_eq!(result.id, "path-group");
    assert!(matches!(result.hit_region, HitRegion::Rect { .. }));
}

#[test]
fn parse_interactive_missing_id_returns_none() {
    // Group without an id is not interactive.
    let shape = json!({
        "type": "group",
        "on_click": true,
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 10, "h": 10}]
    });
    assert!(interaction::parse_interactive_element(&group_from_json(&shape), "default").is_none());
}

// parse_interactive_non_group_returns_none: removed.
// Non-group filtering is now enforced by the type system (only
// GroupShape is accepted by parse_interactive_element).

// -- Hit region to rect --

#[test]
fn hit_region_to_rect_circle() {
    let rect = interaction::hit_region_to_rect(&HitRegion::Circle {
        cx: 50.0,
        cy: 50.0,
        r: 20.0,
    });
    assert!((rect.x - 30.0).abs() < 0.01);
    assert!((rect.y - 30.0).abs() < 0.01);
    assert!((rect.width - 40.0).abs() < 0.01);
    assert!((rect.height - 40.0).abs() < 0.01);
}

// -- Style merging --

#[test]
fn merge_shape_style_overrides_fill() {
    let shape = json!({"type": "rect", "fill": "#ff0000", "stroke": {"color": "#000"}});
    let overrides = json!({"fill": "#00ff00"});
    let merged = program::merge_shape_style(&shape, &overrides);
    assert_eq!(merged["fill"], "#00ff00");
    // Non-overridden fields preserved.
    assert_eq!(merged["stroke"]["color"], "#000");
}

// -- Group shape tests --

#[test]
fn compute_hit_region_group_with_rect_children() {
    // Hit regions are in LOCAL coordinates (no group offset applied).
    let shape = json!({
        "type": "group",
        "id": "grp1", "on_click": true,
        "children": [
            {"type": "rect", "x": 0, "y": 0, "w": 100, "h": 40},
            {"type": "rect", "x": 10, "y": 50, "w": 80, "h": 20}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    // Bounding box of children in local space: x=0..100, y=0..70.
    match result.hit_region {
        HitRegion::Rect { x, y, w, h } => {
            assert!((x - 0.0).abs() < 0.01);
            assert!((y - 0.0).abs() < 0.01);
            assert!((w - 100.0).abs() < 0.01);
            assert!((h - 70.0).abs() < 0.01);
        }
        other => panic!("expected Rect, got {other:?}"),
    }
}

#[test]
fn compute_hit_region_group_with_mixed_children() {
    let shape = json!({
        "type": "group",
        "id": "grp2", "on_click": true,
        "children": [
            {"type": "rect", "x": 0, "y": 0, "w": 50, "h": 30},
            {"type": "circle", "x": 80, "y": 15, "r": 10}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    // Rect: 0..50, 0..30; Circle: 70..90, 5..25
    // Union in local space: 0..90, 0..30
    match result.hit_region {
        HitRegion::Rect { x, y, w, h } => {
            assert!((x - 0.0).abs() < 0.01);
            assert!((y - 0.0).abs() < 0.01);
            assert!((w - 90.0).abs() < 0.01);
            assert!((h - 30.0).abs() < 0.01);
        }
        other => panic!("expected Rect, got {other:?}"),
    }
}

#[test]
fn compute_hit_region_group_no_children() {
    let shape = json!({
        "type": "group",
        "id": "empty", "on_click": true,
        "children": []
    });
    assert!(interaction::parse_interactive_element(&group_from_json(&shape), "default").is_none());
}

#[test]
fn parse_interactive_group() {
    // New format: interactive fields at top level, no nested "interactive".
    let shape = json!({
        "type": "group",
        "id": "btn",
        "on_click": true,
        "on_hover": true,
        "cursor": "pointer",
        "a11y": {"role": "button", "label": "Save"},
        "children": [
            {"type": "rect", "x": 0, "y": 0, "w": 100, "h": 40, "fill": "#3498db"},
            {"type": "text", "x": 30, "y": 25, "content": "Save", "fill": "#ccc"}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    assert_eq!(result.id, "btn");
    assert!(result.on_click);
    assert!(result.on_hover);
    assert_eq!(result.cursor.as_deref(), Some("pointer"));
    assert!(result.a11y.is_some());
    assert!(result.show_focus_ring); // default true
    assert!(!result.focusable); // default false
    match result.hit_region {
        HitRegion::Rect { x, y, w, h } => {
            // Bounding box in local space. Union of:
            //   rect child: (0, 0) -> (100, 40)
            //   text child: (30, 9) -> (60.4, 25) (estimated)
            // Result: (0, 0) -> (100, 40)
            assert!((x - 0.0).abs() < 0.01, "x={x}");
            assert!((y - 0.0).abs() < 0.01, "y={y}");
            assert!((w - 100.0).abs() < 0.01, "w={w}");
            assert!((h - 40.0).abs() < 0.01, "h={h}");
        }
        other => panic!("expected Rect, got {other:?}"),
    }
}

#[test]
fn parse_interactive_element_skips_non_groups() {
    let shape = json!({
        "type": "rect", "x": 0, "y": 0, "w": 100, "h": 40,
        "id": "rect-btn", "on_click": true
    });
    assert!(interaction::parse_interactive_element(&group_from_json(&shape), "default").is_none());
}

#[test]
fn parse_interactive_group_with_new_fields() {
    let shape = json!({
        "type": "group",
        "id": "toggle",
        "on_click": true,
        "focus_style": {"stroke": {"color": "#3b82f6", "width": 2.0}},
        "show_focus_ring": false,
        "focusable": true,
        "children": [
            {"type": "rect", "x": 0, "y": 0, "w": 60, "h": 30}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    assert_eq!(result.id, "toggle");
    assert!(result.has_focus_style);
    assert!(!result.show_focus_ring);
    assert!(result.focusable);
}

#[test]
fn group_translation_from_transforms() {
    // Test via collect: collect a group with transforms and check the
    // element's transform matrix translation component.
    let shapes = vec![json!({
        "type": "group",
        "id": "g",
        "on_click": true,
        "transforms": [
            {"type": "translate", "x": 50.0, "y": 30.0},
            {"type": "rotate", "angle": 0.5},
            {"type": "translate", "x": 10.0, "y": 0.0}
        ],
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 10, "h": 10}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    // The transform should have the composed result.
    assert_eq!(elements.len(), 1);
}

#[test]
fn group_translation_no_transforms() {
    let shapes = vec![json!({
        "type": "group",
        "id": "notrans",
        "on_click": true,
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 10, "h": 10}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 1);
    let (tx, ty) = elements[0].transform.transform_point(0.0, 0.0);
    assert_eq!(tx, 0.0);
    assert_eq!(ty, 0.0);
}

#[test]
fn collect_interactive_elements_recurses_into_groups() {
    let shapes = vec![
        // Non-group shapes are skipped, even with an id.
        json!({
            "type": "rect", "x": 0, "y": 0, "w": 10, "h": 10,
            "id": "top-rect", "on_click": true
        }),
        // Interactive group with a nested interactive group.
        json!({
            "type": "group",
            "id": "grp", "on_click": true,
            "children": [
                {"type": "rect", "x": 0, "y": 0, "w": 50, "h": 50},
                {
                    "type": "group",
                    "transforms": [{"type": "translate", "x": 10, "y": 10}],
                    "id": "nested-grp", "on_click": true,
                    "children": [
                        {"type": "circle", "x": 5, "y": 5, "r": 5}
                    ]
                }
            ]
        }),
    ];
    let mut result = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut result,
    );
    let ids: Vec<&str> = result.iter().map(|s| s.id.as_str()).collect();
    // Non-group "top-rect" is NOT collected.
    assert!(!ids.contains(&"top-rect"));
    // Both groups are collected. Nested groups get hierarchical IDs.
    assert!(ids.contains(&"grp"));
    assert!(ids.contains(&"grp/nested-grp"));
}

#[test]
fn path_bounds_computes_from_commands() {
    // Test path bounds indirectly through hit region computation.
    let shape = json!({
        "type": "group",
        "id": "star", "on_click": true,
        "children": [{
            "type": "path",
            "commands": [
                ["move_to", 0.0, -12.0],
                ["line_to", 11.4, -3.7],
                ["line_to", 7.0, 9.7],
                ["line_to", -7.0, 9.7],
                ["line_to", -11.4, -3.7],
                "close"
            ],
            "fill": "#ff0000"
        }]
    });
    let result = interaction::parse_interactive_element(&group_from_json(&shape), "default");
    assert!(
        result.is_some(),
        "group with path child should have a hit region"
    );
}

#[test]
fn interactive_group_with_path_child_gets_hit_region() {
    let shape = json!({
        "type": "group",
        "id": "star", "on_click": true,
        "children": [
            {
                "type": "path",
                "commands": [
                    ["move_to", 0.0, -12.0],
                    ["line_to", 11.4, -3.7],
                    ["line_to", 7.0, 9.7],
                    ["line_to", -7.0, 9.7],
                    ["line_to", -11.4, -3.7],
                    "close"
                ],
                "fill": "#ff0000"
            }
        ]
    });
    let result = interaction::parse_interactive_element(&group_from_json(&shape), "default");
    assert!(
        result.is_some(),
        "group with path child should have a hit region"
    );
}

#[test]
fn hit_rect_on_group_is_local_coordinates() {
    // hit_rect is in local coordinates: no transform offset applied.
    // Transform offsets are handled by the transform matrix during hit testing.
    let shape = json!({
        "type": "group",
        "id": "star", "on_click": true,
        "hit_rect": {"x": -12.0, "y": -12.0, "w": 28.0, "h": 28.0},
        "children": [
            {"type": "rect", "x": 0, "y": 0, "w": 10, "h": 10}
        ]
    });
    let result =
        interaction::parse_interactive_element(&group_from_json(&shape), "default").unwrap();
    match result.hit_region {
        HitRegion::Rect { x, y, w, h } => {
            // Local coordinates, no offset.
            assert!((x - (-12.0)).abs() < 0.01, "x should be -12, got {x}");
            assert!((y - (-12.0)).abs() < 0.01, "y should be -12, got {y}");
            assert!((w - 28.0).abs() < 0.01);
            assert!((h - 28.0).abs() < 0.01);
        }
        other => panic!("expected Rect, got {other:?}"),
    }
}

// -- TransformMatrix tests --

#[test]
fn transform_identity() {
    let m = TransformMatrix::identity();
    let (x, y) = m.transform_point(10.0, 20.0);
    assert!((x - 10.0).abs() < 0.001);
    assert!((y - 20.0).abs() < 0.001);
}

#[test]
fn transform_translate() {
    let m = TransformMatrix::identity().translate(50.0, 30.0);
    let (x, y) = m.transform_point(10.0, 20.0);
    assert!((x - 60.0).abs() < 0.001);
    assert!((y - 50.0).abs() < 0.001);
}

#[test]
fn transform_rotate_90() {
    let m = TransformMatrix::identity().rotate(std::f32::consts::FRAC_PI_2);
    // (10, 0) rotated 90 degrees CCW -> (0, 10)
    let (x, y) = m.transform_point(10.0, 0.0);
    assert!(x.abs() < 0.001, "x should be ~0, got {x}");
    assert!((y - 10.0).abs() < 0.001, "y should be ~10, got {y}");
}

#[test]
fn transform_scale() {
    let m = TransformMatrix::identity().scale(2.0, 3.0);
    let (x, y) = m.transform_point(10.0, 20.0);
    assert!((x - 20.0).abs() < 0.001);
    assert!((y - 60.0).abs() < 0.001);
}

#[test]
fn transform_inverse_roundtrip() {
    let m = TransformMatrix::identity()
        .translate(50.0, 30.0)
        .rotate(0.5)
        .scale(2.0, 1.5);
    let inv = m.inverse().unwrap();
    // Forward then inverse should return to original.
    let (fx, fy) = m.transform_point(10.0, 20.0);
    let (rx, ry) = inv.transform_point(fx, fy);
    assert!(
        (rx - 10.0).abs() < 0.01,
        "roundtrip x: expected 10, got {rx}"
    );
    assert!(
        (ry - 20.0).abs() < 0.01,
        "roundtrip y: expected 20, got {ry}"
    );
}

#[test]
fn transform_singular_has_no_inverse() {
    // Scale to zero on one axis -> singular.
    let m = TransformMatrix::identity().scale(0.0, 1.0);
    assert!(m.inverse().is_none());
}

#[test]
fn transform_from_json() {
    let transforms = vec![
        json!({"type": "translate", "x": 50.0, "y": 30.0}),
        json!({"type": "rotate", "angle": 0.0}),
        json!({"type": "scale", "factor": 2.0}),
    ];
    let m = TransformMatrix::from_transforms(&transforms);
    // translate(50,30) then scale(2) -> point (10,20) becomes (50+10*2, 30+20*2) = (70, 70)
    let (x, y) = m.transform_point(10.0, 20.0);
    assert!((x - 70.0).abs() < 0.01, "x={x}");
    assert!((y - 70.0).abs() < 0.01, "y={y}");
}

#[test]
fn find_hit_element_with_transform() {
    // Group at (100, 100) with a 50x50 rect child.
    // Cursor at canvas (125, 125) should hit (local 25, 25).
    let shapes = vec![json!({
        "type": "group",
        "id": "btn",
        "on_click": true,
        "transforms": [{"type": "translate", "x": 100, "y": 100}],
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 50, "h": 50}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 1);

    // Hit inside.
    let hit = interaction::find_hit_element(Point::new(125.0, 125.0), &elements);
    assert!(hit.is_some(), "should hit at (125, 125)");

    // Miss outside.
    let miss = interaction::find_hit_element(Point::new(50.0, 50.0), &elements);
    assert!(miss.is_none(), "should miss at (50, 50)");
}

#[test]
fn find_hit_element_with_rotation() {
    // Group rotated 45 degrees, with a 100x20 rect at origin.
    // The rect covers a diagonal strip in canvas space.
    let shapes = vec![json!({
        "type": "group",
        "id": "rotated",
        "on_click": true,
        "transforms": [{"type": "rotate", "angle": 45.0}],
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 100, "h": 20}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );

    // Canvas point (35, 35) -> local via inverse rotate(-45deg):
    //   local_x = cos(-45)*35 + (-sin(-45))*35 = 0.707*35 + 0.707*35 ~ 49.5
    //   local_y = sin(-45)*35 + cos(-45)*35 = -0.707*35 + 0.707*35 ~ 0
    // local (49.5, 0) is inside rect (0,0,100,20). Should hit.
    let hit = interaction::find_hit_element(Point::new(35.0, 35.0), &elements);
    assert!(hit.is_some(), "should hit along the rotated diagonal");

    // Canvas point (0, 80) -> local:
    //   local_x = 0.707*0 + 0.707*80 ~ 56.6
    //   local_y = -0.707*0 + 0.707*80 ~ 56.6
    // local (56.6, 56.6) is outside rect height 20. Should miss.
    let miss = interaction::find_hit_element(Point::new(0.0, 80.0), &elements);
    assert!(miss.is_none(), "should miss far from diagonal");
}

#[test]
fn find_hit_element_respects_clip() {
    // Group at (0,0) with a 100x100 rect, but clipped to a 50x50 region.
    let shapes = vec![json!({
        "type": "group",
        "id": "clipped",
        "on_click": true,
        "clip": {"x": 0, "y": 0, "w": 50, "h": 50},
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 100, "h": 100}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );

    // Inside clip region.
    let hit = interaction::find_hit_element(Point::new(25.0, 25.0), &elements);
    assert!(hit.is_some(), "should hit inside clip");

    // Inside hit region but outside clip.
    let miss = interaction::find_hit_element(Point::new(75.0, 75.0), &elements);
    assert!(
        miss.is_none(),
        "should miss outside clip despite being in hit region"
    );
}

#[test]
fn intersect_rects_overlap() {
    let r = interaction::intersect_rects((0.0, 0.0, 100.0, 100.0), (50.0, 50.0, 100.0, 100.0));
    assert!((r.0 - 50.0).abs() < 0.01);
    assert!((r.1 - 50.0).abs() < 0.01);
    assert!((r.2 - 50.0).abs() < 0.01);
    assert!((r.3 - 50.0).abs() < 0.01);
}

#[test]
fn intersect_rects_no_overlap() {
    let r = interaction::intersect_rects((0.0, 0.0, 10.0, 10.0), (20.0, 20.0, 10.0, 10.0));
    assert!(r.2 == 0.0 || r.3 == 0.0, "no-overlap should have zero area");
}

#[test]
fn collect_nested_transform_accumulates() {
    // Outer group translated (100, 0), inner group translated (0, 50).
    // Inner element should have accumulated transform (100, 50).
    let shapes = vec![json!({
        "type": "group",
        "transforms": [{"type": "translate", "x": 100, "y": 0}],
        "children": [{
            "type": "group",
            "id": "inner",
            "on_click": true,
            "transforms": [{"type": "translate", "x": 0, "y": 50}],
            "children": [{"type": "rect", "x": 0, "y": 0, "w": 10, "h": 10}]
        }]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 1);
    let e = &elements[0];
    // The transform should map (0,0) to (100, 50).
    let (tx, ty) = e.transform.transform_point(0.0, 0.0);
    assert!((tx - 100.0).abs() < 0.01, "tx={tx}");
    assert!((ty - 50.0).abs() < 0.01, "ty={ty}");
}

// -- TransformMatrix::compose tests --

#[test]
fn compose_identity_is_noop() {
    let m = TransformMatrix::identity().translate(10.0, 20.0);
    let id = TransformMatrix::identity();
    let composed = m.compose(&id);
    let (x, y) = composed.transform_point(0.0, 0.0);
    assert!((x - 10.0).abs() < 0.001);
    assert!((y - 20.0).abs() < 0.001);
}

#[test]
fn compose_translate_then_scale() {
    let parent = TransformMatrix::identity().translate(100.0, 0.0);
    let local = TransformMatrix::identity().scale(2.0, 2.0);
    let composed = parent.compose(&local);
    // Point (5, 5) -> scale(2,2) -> (10, 10) -> translate(100, 0) -> (110, 10)
    let (x, y) = composed.transform_point(5.0, 5.0);
    assert!((x - 110.0).abs() < 0.01, "x={x}");
    assert!((y - 10.0).abs() < 0.01, "y={y}");
}

#[test]
fn compose_matches_chained_transforms() {
    // compose(parent, local) should equal parent.translate().rotate()
    // when local = translate then rotate
    let parent = TransformMatrix::identity().translate(50.0, 0.0);
    let local = TransformMatrix::identity()
        .translate(10.0, 0.0)
        .rotate(std::f32::consts::FRAC_PI_2);
    let composed = parent.compose(&local);
    let chained = TransformMatrix::identity()
        .translate(50.0, 0.0)
        .translate(10.0, 0.0)
        .rotate(std::f32::consts::FRAC_PI_2);
    // Both should transform (1, 0) the same way.
    let (cx, cy) = composed.transform_point(1.0, 0.0);
    let (sx, sy) = chained.transform_point(1.0, 0.0);
    assert!((cx - sx).abs() < 0.01, "x: composed={cx}, chained={sx}");
    assert!((cy - sy).abs() < 0.01, "y: composed={cy}, chained={sy}");
}

// -- hit_test epsilon tolerance --

#[test]
fn hit_test_rect_boundary_with_epsilon() {
    // Point exactly on the boundary should hit (within epsilon).
    let region = HitRegion::Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 50.0,
    };
    assert!(interaction::hit_test(Point::new(0.0, 0.0), &region));
    assert!(interaction::hit_test(Point::new(100.0, 50.0), &region));
    // Slightly outside but within epsilon (0.5px).
    assert!(interaction::hit_test(Point::new(-0.3, -0.3), &region));
    assert!(interaction::hit_test(Point::new(100.3, 50.3), &region));
    // Beyond epsilon.
    assert!(!interaction::hit_test(Point::new(-1.0, 0.0), &region));
    assert!(!interaction::hit_test(Point::new(0.0, -1.0), &region));
}

// -- Clip inheritance through nested groups --

#[test]
fn nested_clip_is_intersected() {
    let shapes = vec![json!({
        "type": "group",
        "clip": {"x": 0, "y": 0, "w": 100, "h": 100},
        "children": [{
            "type": "group",
            "id": "inner",
            "on_click": true,
            "clip": {"x": 50, "y": 50, "w": 100, "h": 100},
            "children": [{"type": "rect", "x": 0, "y": 0, "w": 200, "h": 200}]
        }]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 1);
    let clip = elements[0].clip_rect.unwrap();
    // Intersection of (0,0,100,100) and (50,50,100,100) = (50,50,50,50).
    assert!((clip.0 - 50.0).abs() < 0.01, "clip x={}", clip.0);
    assert!((clip.1 - 50.0).abs() < 0.01, "clip y={}", clip.1);
    assert!((clip.2 - 50.0).abs() < 0.01, "clip w={}", clip.2);
    assert!((clip.3 - 50.0).abs() < 0.01, "clip h={}", clip.3);
}

#[test]
fn clip_from_parent_propagates_to_child() {
    let shapes = vec![json!({
        "type": "group",
        "clip": {"x": 10, "y": 10, "w": 80, "h": 80},
        "children": [{
            "type": "group",
            "id": "child",
            "on_click": true,
            "children": [{"type": "rect", "x": 0, "y": 0, "w": 100, "h": 100}]
        }]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 1);
    let clip = elements[0].clip_rect.unwrap();
    assert!((clip.0 - 10.0).abs() < 0.01);
    assert!((clip.1 - 10.0).abs() < 0.01);
    assert!((clip.2 - 80.0).abs() < 0.01);
    assert!((clip.3 - 80.0).abs() < 0.01);
}

#[test]
fn no_clip_means_no_clip_rect() {
    let shapes = vec![json!({
        "type": "group",
        "id": "noclip",
        "on_click": true,
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 50, "h": 50}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert!(elements[0].clip_rect.is_none());
}

// -- set_focus and resolve_focus_index --

/// Helper to build a minimal CanvasProgram for state-machine tests.
fn test_program(elements: &[InteractiveElement]) -> program::CanvasProgram<'_> {
    static IMAGES: std::sync::LazyLock<crate::image_registry::ImageRegistry> =
        std::sync::LazyLock::new(crate::image_registry::ImageRegistry::new);
    program::CanvasProgram {
        layers: vec![],
        caches: None,
        background: None,
        window_id: "test-window".to_string(),
        id: "test-canvas".to_string(),
        on_press: false,
        on_release: false,
        on_move: false,
        on_scroll: false,
        images: &IMAGES,
        interactive_elements: elements,
        arrow_mode: ArrowMode::Wrap,
        pending_focus: None,
    }
}

/// Helper to build a minimal InteractiveElement.
fn test_element(id: &str) -> InteractiveElement {
    InteractiveElement {
        id: id.to_string(),
        layer: "default".to_string(),
        hit_region: HitRegion::Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        },
        transform: TransformMatrix::identity(),
        inverse_transform: Some(TransformMatrix::identity()),
        clip_rect: None,
        on_click: true,
        on_hover: false,
        draggable: false,
        drag_axis: DragAxis::Both,
        drag_bounds: None,
        cursor: None,
        has_hover_style: false,
        has_pressed_style: false,
        has_focus_style: false,
        show_focus_ring: true,
        focus_ring_radius: None,
        focusable: false,
        parent_group: None,
        tooltip: None,
        a11y: None,
    }
}

#[test]
fn resolve_focus_index_finds_element() {
    let elements = vec![test_element("a"), test_element("b"), test_element("c")];
    let program = test_program(&elements);
    let state = CanvasState {
        focused_id: Some("b".to_string()),
        ..Default::default()
    };
    assert_eq!(program.resolve_focus_index(&state), Some(1));
}

#[test]
fn resolve_focus_index_returns_none_for_missing() {
    let elements = vec![test_element("a"), test_element("b")];
    let program = test_program(&elements);
    let state = CanvasState {
        focused_id: Some("deleted".to_string()),
        ..Default::default()
    };
    assert_eq!(program.resolve_focus_index(&state), None);
}

#[test]
fn resolve_focus_index_returns_none_when_unfocused() {
    let elements = vec![test_element("a")];
    let program = test_program(&elements);
    let state = CanvasState::default();
    assert_eq!(program.resolve_focus_index(&state), None);
}

#[test]
fn set_focus_from_none_to_element() {
    let elements = vec![test_element("a"), test_element("b")];
    let program = test_program(&elements);
    let mut state = CanvasState::default();
    let msg = program.set_focus(&mut state, Some(1));
    assert!(msg.is_some());
    assert_eq!(state.focused_id, Some("b".to_string()));
    match msg.unwrap() {
        crate::message::Message::CanvasElementFocusChanged {
            old_element_id,
            new_element_id,
            ..
        } => {
            assert_eq!(old_element_id, None);
            assert_eq!(new_element_id, Some("b".to_string()));
        }
        other => panic!("expected FocusChanged, got {other:?}"),
    }
}

#[test]
fn set_focus_between_elements() {
    let elements = vec![test_element("a"), test_element("b")];
    let program = test_program(&elements);
    let mut state = CanvasState {
        focused_id: Some("a".to_string()),
        ..Default::default()
    };
    let msg = program.set_focus(&mut state, Some(1));
    assert!(msg.is_some());
    assert_eq!(state.focused_id, Some("b".to_string()));
    match msg.unwrap() {
        crate::message::Message::CanvasElementFocusChanged {
            old_element_id,
            new_element_id,
            ..
        } => {
            assert_eq!(old_element_id, Some("a".to_string()));
            assert_eq!(new_element_id, Some("b".to_string()));
        }
        other => panic!("expected FocusChanged, got {other:?}"),
    }
}

#[test]
fn set_focus_to_same_is_noop() {
    let elements = vec![test_element("a"), test_element("b")];
    let program = test_program(&elements);
    let mut state = CanvasState {
        focused_id: Some("a".to_string()),
        ..Default::default()
    };
    let msg = program.set_focus(&mut state, Some(0));
    assert!(msg.is_none());
    assert_eq!(state.focused_id, Some("a".to_string()));
}

#[test]
fn set_focus_clear() {
    let elements = vec![test_element("a")];
    let program = test_program(&elements);
    let mut state = CanvasState {
        focused_id: Some("a".to_string()),
        ..Default::default()
    };
    let msg = program.set_focus(&mut state, None);
    assert!(msg.is_some());
    assert_eq!(state.focused_id, None);
    match msg.unwrap() {
        crate::message::Message::CanvasElementFocusChanged {
            old_element_id,
            new_element_id,
            ..
        } => {
            assert_eq!(old_element_id, Some("a".to_string()));
            assert_eq!(new_element_id, None);
        }
        other => panic!("expected FocusChanged, got {other:?}"),
    }
}

#[test]
fn set_focus_clear_when_already_none() {
    let elements = vec![test_element("a")];
    let program = test_program(&elements);
    let mut state = CanvasState::default();
    let msg = program.set_focus(&mut state, None);
    assert!(msg.is_none());
}

#[test]
fn set_focus_out_of_bounds_clears() {
    let elements = vec![test_element("a")];
    let program = test_program(&elements);
    let mut state = CanvasState {
        focused_id: Some("a".to_string()),
        ..Default::default()
    };
    // Index 99 is out of bounds -> new_id becomes None -> blur.
    let msg = program.set_focus(&mut state, Some(99));
    assert!(msg.is_some());
    assert_eq!(state.focused_id, None);
}

// -- layers_with_active_interaction --

#[test]
fn layers_active_hover_style() {
    let mut elements = vec![test_element("btn")];
    elements[0].layer = "ui".to_string();
    elements[0].has_hover_style = true;
    let program = test_program(&elements);
    let state = CanvasState {
        hovered_element: Some("btn".to_string()),
        ..Default::default()
    };
    let layers = program.layers_with_active_interaction(&state);
    assert_eq!(layers, vec!["ui"]);
}

#[test]
fn layers_active_focus_style() {
    let mut elements = vec![test_element("btn")];
    elements[0].layer = "ui".to_string();
    elements[0].has_focus_style = true;
    let program = test_program(&elements);
    let state = CanvasState {
        focused_id: Some("btn".to_string()),
        canvas_focused: true,
        focus_visible: true,
        ..Default::default()
    };
    let layers = program.layers_with_active_interaction(&state);
    assert_eq!(layers, vec!["ui"]);
}

#[test]
fn layers_active_hover_and_focus_on_different_layers() {
    let mut hover_elem = test_element("hover-btn");
    hover_elem.layer = "layer-a".to_string();
    hover_elem.has_hover_style = true;
    let mut focus_elem = test_element("focus-btn");
    focus_elem.layer = "layer-b".to_string();
    focus_elem.has_focus_style = true;
    let elements = vec![hover_elem, focus_elem];
    let program = test_program(&elements);
    let state = CanvasState {
        hovered_element: Some("hover-btn".to_string()),
        focused_id: Some("focus-btn".to_string()),
        canvas_focused: true,
        focus_visible: true,
        ..Default::default()
    };
    let layers = program.layers_with_active_interaction(&state);
    assert_eq!(layers.len(), 2);
    assert!(layers.contains(&"layer-a".to_string()));
    assert!(layers.contains(&"layer-b".to_string()));
}

#[test]
fn layers_active_hover_and_focus_on_same_layer_no_dupe() {
    let mut hover_elem = test_element("hover-btn");
    hover_elem.layer = "ui".to_string();
    hover_elem.has_hover_style = true;
    let mut focus_elem = test_element("focus-btn");
    focus_elem.layer = "ui".to_string();
    focus_elem.has_focus_style = true;
    let elements = vec![hover_elem, focus_elem];
    let program = test_program(&elements);
    let state = CanvasState {
        hovered_element: Some("hover-btn".to_string()),
        focused_id: Some("focus-btn".to_string()),
        canvas_focused: true,
        focus_visible: true,
        ..Default::default()
    };
    let layers = program.layers_with_active_interaction(&state);
    // Should not duplicate "ui".
    assert_eq!(layers, vec!["ui"]);
}

#[test]
fn layers_active_no_style_returns_empty() {
    let elements = vec![test_element("btn")]; // no style flags
    let program = test_program(&elements);
    let state = CanvasState {
        hovered_element: Some("btn".to_string()),
        ..Default::default()
    };
    let layers = program.layers_with_active_interaction(&state);
    assert!(layers.is_empty());
}

// -- find_hit_element edge cases --

#[test]
fn find_hit_element_empty_list() {
    assert!(interaction::find_hit_element(Point::new(0.0, 0.0), &[]).is_none());
}

#[test]
fn find_hit_element_singular_transform_not_hittable() {
    let shapes = vec![json!({
        "type": "group",
        "id": "collapsed",
        "on_click": true,
        "transforms": [{"type": "scale", "x": 0, "y": 1}],
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 100, "h": 100}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 1);
    assert!(elements[0].inverse_transform.is_none());
    // Can't hit an element with a singular transform.
    let hit = interaction::find_hit_element(Point::new(50.0, 50.0), &elements);
    assert!(hit.is_none());
}

#[test]
fn find_hit_element_topmost_wins() {
    // Two overlapping elements. Last in list = topmost = tested first.
    let mut a = test_element("bottom");
    a.hit_region = HitRegion::Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    let mut b = test_element("top");
    b.hit_region = HitRegion::Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    let elements = vec![a, b];
    let hit = interaction::find_hit_element(Point::new(50.0, 50.0), &elements).unwrap();
    assert_eq!(hit.id, "top");
}

#[test]
fn find_hit_element_skips_non_interactive() {
    // Element with on_click=false, on_hover=false, draggable=false.
    let mut elem = test_element("passive");
    elem.on_click = false;
    elem.hit_region = HitRegion::Rect {
        x: 0.0,
        y: 0.0,
        w: 100.0,
        h: 100.0,
    };
    let elements = vec![elem];
    let hit = interaction::find_hit_element(Point::new(50.0, 50.0), &elements);
    assert!(hit.is_none());
}

// -- Transformed clip test --

#[test]
fn clip_transformed_by_group_matrix() {
    let shapes = vec![json!({
        "type": "group",
        "id": "shifted-clip",
        "on_click": true,
        "transforms": [{"type": "translate", "x": 100, "y": 100}],
        "clip": {"x": 0, "y": 0, "w": 50, "h": 50},
        "children": [{"type": "rect", "x": 0, "y": 0, "w": 100, "h": 100}]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    let clip = elements[0].clip_rect.unwrap();
    assert!((clip.0 - 100.0).abs() < 0.01, "clip x={}", clip.0);
    assert!((clip.1 - 100.0).abs() < 0.01, "clip y={}", clip.1);
    assert!((clip.2 - 50.0).abs() < 0.01, "clip w={}", clip.2);
    assert!((clip.3 - 50.0).abs() < 0.01, "clip h={}", clip.3);

    // Hit inside clip (canvas 125, 125 -> local 25, 25 -> in rect).
    let hit = interaction::find_hit_element(Point::new(125.0, 125.0), &elements);
    assert!(hit.is_some());

    // Hit outside clip but inside transformed rect.
    let miss = interaction::find_hit_element(Point::new(175.0, 175.0), &elements);
    assert!(miss.is_none());
}

// -- TransformMatrix::decompose tests --

#[test]
fn decompose_identity() {
    let (tx, ty, angle, sx, sy) = TransformMatrix::identity().decompose();
    assert!((tx).abs() < 0.001);
    assert!((ty).abs() < 0.001);
    assert!((angle).abs() < 0.001);
    assert!((sx - 1.0).abs() < 0.001);
    assert!((sy - 1.0).abs() < 0.001);
}

#[test]
fn decompose_translate_only() {
    let m = TransformMatrix::identity().translate(42.0, -17.0);
    let (tx, ty, angle, sx, sy) = m.decompose();
    assert!((tx - 42.0).abs() < 0.001);
    assert!((ty - (-17.0)).abs() < 0.001);
    assert!((angle).abs() < 0.001);
    assert!((sx - 1.0).abs() < 0.001);
    assert!((sy - 1.0).abs() < 0.001);
}

#[test]
fn decompose_rotate_only() {
    let angle_in = 0.7;
    let m = TransformMatrix::identity().rotate(angle_in);
    let (tx, ty, angle, sx, sy) = m.decompose();
    assert!((tx).abs() < 0.001);
    assert!((ty).abs() < 0.001);
    assert!((angle - angle_in).abs() < 0.001, "angle={angle}");
    assert!((sx - 1.0).abs() < 0.001);
    assert!((sy - 1.0).abs() < 0.001);
}

#[test]
fn decompose_scale_only() {
    let m = TransformMatrix::identity().scale(3.0, 0.5);
    let (tx, ty, angle, sx, sy) = m.decompose();
    assert!((tx).abs() < 0.001);
    assert!((ty).abs() < 0.001);
    assert!((angle).abs() < 0.001);
    assert!((sx - 3.0).abs() < 0.001, "sx={sx}");
    assert!((sy - 0.5).abs() < 0.001, "sy={sy}");
}

#[test]
fn decompose_roundtrip() {
    let original = TransformMatrix::identity()
        .translate(50.0, 30.0)
        .rotate(0.5)
        .scale(2.0, 1.5);
    let (tx, ty, angle, sx, sy) = original.decompose();
    let rebuilt = TransformMatrix::identity()
        .translate(tx, ty)
        .rotate(angle)
        .scale(sx, sy);

    // Test several points.
    for &(px, py) in &[(0.0, 0.0), (10.0, 20.0), (-5.0, 15.0)] {
        let (ox, oy) = original.transform_point(px, py);
        let (rx, ry) = rebuilt.transform_point(px, py);
        assert!(
            (ox - rx).abs() < 0.1 && (oy - ry).abs() < 0.1,
            "point ({px},{py}): original=({ox},{oy}), rebuilt=({rx},{ry})"
        );
    }
}

// -- ArrowMode tests --

#[test]
fn arrow_mode_from_str_known_values() {
    assert_eq!(ArrowMode::from_str("wrap"), ArrowMode::Wrap);
    assert_eq!(ArrowMode::from_str("clamp"), ArrowMode::Clamp);
    assert_eq!(ArrowMode::from_str("linear"), ArrowMode::Linear);
    assert_eq!(ArrowMode::from_str("none"), ArrowMode::None);
}

#[test]
fn arrow_mode_from_str_unknown_defaults_to_wrap() {
    assert_eq!(ArrowMode::from_str("invalid"), ArrowMode::Wrap);
    assert_eq!(ArrowMode::from_str(""), ArrowMode::Wrap);
}

#[test]
fn arrow_mode_default_is_wrap() {
    assert_eq!(ArrowMode::default(), ArrowMode::Wrap);
}

// -- Focusable groups / two-level navigation --

#[test]
fn parent_group_set_for_children_of_focusable_group() {
    let shapes = vec![json!({
        "type": "group",
        "id": "toolbar",
        "focusable": true,
        "on_click": true,
        "children": [
            {
                "type": "group",
                "id": "btn-a",
                "on_click": true,
                "children": [{"type": "rect", "x": 0, "y": 0, "w": 30, "h": 30}]
            },
            {
                "type": "group",
                "id": "btn-b",
                "on_click": true,
                "children": [{"type": "rect", "x": 40, "y": 0, "w": 30, "h": 30}]
            }
        ]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    // Should collect: toolbar, btn-a, btn-b
    assert_eq!(elements.len(), 3);
    assert_eq!(elements[0].id, "toolbar");
    assert!(elements[0].focusable);
    assert_eq!(elements[0].parent_group, None); // top-level
    assert_eq!(elements[1].id, "toolbar/btn-a");
    assert_eq!(elements[1].parent_group, Some("toolbar".to_string()));
    assert_eq!(elements[2].id, "toolbar/btn-b");
    assert_eq!(elements[2].parent_group, Some("toolbar".to_string()));
}

#[test]
fn parent_group_none_without_focusable() {
    let shapes = vec![json!({
        "type": "group",
        "id": "container",
        "on_click": true,
        "children": [
            {
                "type": "group",
                "id": "child",
                "on_click": true,
                "children": [{"type": "rect", "x": 0, "y": 0, "w": 10, "h": 10}]
            }
        ]
    })];
    let mut elements = Vec::new();
    collect_interactive_from_json(
        &shapes,
        "default",
        TransformMatrix::identity(),
        None,
        None,
        "",
        &mut elements,
    );
    assert_eq!(elements.len(), 2);
    assert_eq!(elements[0].parent_group, None);
    assert_eq!(elements[1].parent_group, None);
}

#[test]
fn top_level_indices_excludes_group_children() {
    let mut toolbar = test_element("toolbar");
    toolbar.focusable = true;
    let mut btn_a = test_element("btn-a");
    btn_a.parent_group = Some("toolbar".to_string());
    let mut btn_b = test_element("btn-b");
    btn_b.parent_group = Some("toolbar".to_string());
    let standalone = test_element("standalone");
    let elements = vec![toolbar, btn_a, btn_b, standalone];
    let program = test_program(&elements);
    let top = program.top_level_indices();
    // Only "toolbar" (idx 0) and "standalone" (idx 3) are top-level.
    assert_eq!(top, vec![0, 3]);
}

#[test]
fn group_child_indices_returns_children() {
    let mut toolbar = test_element("toolbar");
    toolbar.focusable = true;
    let mut btn_a = test_element("btn-a");
    btn_a.parent_group = Some("toolbar".to_string());
    let mut btn_b = test_element("btn-b");
    btn_b.parent_group = Some("toolbar".to_string());
    let standalone = test_element("standalone");
    let elements = vec![toolbar, btn_a, btn_b, standalone];
    let program = test_program(&elements);
    let children = program.group_child_indices("toolbar");
    assert_eq!(children, vec![1, 2]);
}

#[test]
fn group_child_indices_empty_for_nonexistent_group() {
    let elements = vec![test_element("a"), test_element("b")];
    let program = test_program(&elements);
    assert!(program.group_child_indices("nonexistent").is_empty());
}
