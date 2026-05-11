use plushie_core::protocol::{PropMap, PropValue};

use crate::protocol::TreeNode;

const MAX_CANVAS_COORDINATE: f64 = 1_000_000.0;

pub(crate) fn validate_canvas_shape_tree(canvas: &TreeNode) -> Vec<String> {
    let mut validator = ShapeValidator {
        warnings: Vec::new(),
    };

    for child in &canvas.children {
        validator.validate_canvas_child(child, "canvas");
    }

    validator.warnings
}

struct ShapeValidator {
    warnings: Vec<String>,
}

impl ShapeValidator {
    fn validate_canvas_child(&mut self, node: &TreeNode, parent: &str) {
        if node.type_name == "__layer__" {
            self.validate_layer(node, parent);
        } else {
            self.validate_shape_node(node, parent);
        }
    }

    fn validate_layer(&mut self, node: &TreeNode, parent: &str) {
        self.validate_allowed_props(node, &["id", "name"]);

        for child in &node.children {
            self.validate_shape_node(child, &format!("{parent} > layer {}", node.id));
        }
    }

    fn validate_shape_node(&mut self, node: &TreeNode, parent: &str) {
        match node.type_name.as_str() {
            "rect" => {
                self.validate_allowed_props(
                    node,
                    &[
                        "id",
                        "x",
                        "y",
                        "w",
                        "h",
                        "fill",
                        "stroke",
                        "opacity",
                        "fill_rule",
                        "radius",
                    ],
                );
                self.validate_coordinates(node, &["x", "y"]);
                self.validate_non_negative_fields(node, &["w", "h"]);
                self.validate_common_shape_fields(node);
                self.validate_radius(node, "radius");
            }
            "circle" => {
                self.validate_allowed_props(
                    node,
                    &[
                        "id",
                        "x",
                        "y",
                        "r",
                        "fill",
                        "stroke",
                        "opacity",
                        "fill_rule",
                    ],
                );
                self.validate_coordinates(node, &["x", "y"]);
                self.validate_non_negative_fields(node, &["r"]);
                self.validate_common_shape_fields(node);
            }
            "line" => {
                self.validate_allowed_props(
                    node,
                    &["id", "x1", "y1", "x2", "y2", "stroke", "opacity"],
                );
                self.validate_coordinates(node, &["x1", "y1", "x2", "y2"]);
                self.validate_common_shape_fields(node);
            }
            "path" => {
                self.validate_allowed_props(
                    node,
                    &["id", "commands", "fill", "stroke", "opacity", "fill_rule"],
                );
                self.validate_path_commands(node);
                self.validate_common_shape_fields(node);
            }
            "text" => {
                self.validate_allowed_props(
                    node,
                    &[
                        "id", "x", "y", "content", "fill", "size", "font", "align_x", "align_y",
                        "opacity",
                    ],
                );
                self.validate_coordinates(node, &["x", "y"]);
                self.validate_non_negative_fields(node, &["size"]);
                self.validate_common_shape_fields(node);
            }
            "image" => {
                self.validate_allowed_props(
                    node,
                    &["id", "source", "x", "y", "w", "h", "rotation", "opacity"],
                );
                self.validate_coordinates(node, &["x", "y", "rotation"]);
                self.validate_non_negative_fields(node, &["w", "h"]);
                self.validate_source(node, "image");
                self.validate_common_shape_fields(node);
            }
            "svg" => {
                self.validate_allowed_props(node, &["id", "source", "x", "y", "w", "h"]);
                self.validate_coordinates(node, &["x", "y"]);
                self.validate_non_negative_fields(node, &["w", "h"]);
                self.validate_source(node, "svg");
            }
            "group" => {
                self.validate_allowed_props(
                    node,
                    &[
                        "id",
                        "transforms",
                        "clip",
                        "on_click",
                        "on_hover",
                        "draggable",
                        "drag_axis",
                        "drag_bounds",
                        "cursor",
                        "hit_rect",
                        "tooltip",
                        "hover_style",
                        "pressed_style",
                        "focus_style",
                        "show_focus_ring",
                        "focus_ring_radius",
                        "focusable",
                        "a11y",
                    ],
                );
                self.validate_group_fields(node);
                for child in &node.children {
                    self.validate_shape_node(child, &format!("{parent} > group {}", node.id));
                }
            }
            _ => self.warn(format!(
                "{parent} child '{}' has unknown canvas shape type '{}'",
                node.id, node.type_name
            )),
        }
    }

    fn validate_common_shape_fields(&mut self, node: &TreeNode) {
        self.validate_opacity(node);
        self.validate_stroke(node, "stroke");
    }

    fn validate_group_fields(&mut self, node: &TreeNode) {
        self.validate_transforms(node);
        self.validate_rect_object(node, "clip", true);
        self.validate_rect_object(node, "hit_rect", true);
        self.validate_drag_bounds(node);
        self.validate_non_negative_fields(node, &["focus_ring_radius"]);
        for key in ["hover_style", "pressed_style", "focus_style"] {
            self.validate_shape_style(node, key);
        }
    }

    fn validate_allowed_props(&mut self, node: &TreeNode, allowed: &[&str]) {
        for key in node.props.as_prop_map().keys() {
            if !allowed.contains(&key) {
                self.warn(format!(
                    "shape '{}' ({}) has unknown prop '{}'",
                    node.id, node.type_name, key
                ));
            }
        }
    }

    fn validate_coordinates(&mut self, node: &TreeNode, fields: &[&str]) {
        for field in fields {
            if let Some(value) = node.props.get(field) {
                self.validate_bounded_number(
                    value,
                    &format!("shape '{}' prop '{}'", node.id, field),
                );
            }
        }
    }

    fn validate_non_negative_fields(&mut self, node: &TreeNode, fields: &[&str]) {
        for field in fields {
            if let Some(value) = node.props.get(field) {
                self.validate_non_negative_number(
                    value,
                    &format!("shape '{}' prop '{}'", node.id, field),
                );
            }
        }
    }

    fn validate_opacity(&mut self, node: &TreeNode) {
        if let Some(value) = node.props.get("opacity") {
            self.validate_opacity_value(value, &format!("shape '{}' prop 'opacity'", node.id));
        }
    }

    fn validate_stroke(&mut self, node: &TreeNode, key: &str) {
        let Some(value) = node.props.get(key) else {
            return;
        };
        let Some(stroke) = value.as_object() else {
            return;
        };

        self.validate_stroke_object(node, key, stroke);
    }

    fn validate_stroke_object(&mut self, node: &TreeNode, key: &str, stroke: &PropMap) {
        self.validate_object_fields(
            stroke,
            &["color", "width", "cap", "join", "dash"],
            &format!("shape '{}' prop '{key}'", node.id),
        );

        if let Some(width) = stroke.get("width") {
            self.validate_non_negative_number(width, &format!("shape '{}' stroke width", node.id));
        }

        if let Some(dash) = stroke.get("dash") {
            self.validate_dash(node, dash);
        }
    }

    fn validate_dash(&mut self, node: &TreeNode, value: &PropValue) {
        let Some(dash) = value.as_object() else {
            return;
        };
        self.validate_object_fields(
            dash,
            &["segments", "offset"],
            &format!("shape '{}' stroke dash", node.id),
        );
        if let Some(offset) = dash.get("offset") {
            self.validate_bounded_number(
                offset,
                &format!("shape '{}' stroke dash offset", node.id),
            );
        }
        if let Some(segments) = dash.get("segments").and_then(PropValue::as_array) {
            for (idx, segment) in segments.iter().enumerate() {
                self.validate_non_negative_number(
                    segment,
                    &format!("shape '{}' stroke dash segment {idx}", node.id),
                );
            }
        }
    }

    fn validate_radius(&mut self, node: &TreeNode, key: &str) {
        let Some(value) = node.props.get(key) else {
            return;
        };
        match value {
            PropValue::Object(radius) => {
                self.validate_object_fields(
                    radius,
                    &["top_left", "top_right", "bottom_right", "bottom_left"],
                    &format!("shape '{}' prop '{key}'", node.id),
                );
                for field in ["top_left", "top_right", "bottom_right", "bottom_left"] {
                    if let Some(value) = radius.get(field) {
                        self.validate_non_negative_number(
                            value,
                            &format!("shape '{}' radius {field}", node.id),
                        );
                    }
                }
            }
            _ => self
                .validate_non_negative_number(value, &format!("shape '{}' prop '{key}'", node.id)),
        }
    }

    fn validate_source(&mut self, node: &TreeNode, kind: &str) {
        let Some(source) = node.props.get_str("source") else {
            return;
        };
        if source.contains("://") {
            self.warn(format!(
                "shape '{}' {kind} source uses URL-like value '{source}'",
                node.id
            ));
        }
    }

    fn validate_transforms(&mut self, node: &TreeNode) {
        let Some(value) = node.props.get("transforms") else {
            return;
        };
        let Some(transforms) = value.as_array() else {
            self.warn(format!("shape '{}' transforms must be an array", node.id));
            return;
        };

        for (idx, transform) in transforms.iter().enumerate() {
            let context = format!("shape '{}' transform {idx}", node.id);
            let Some(obj) = transform.as_object() else {
                self.warn(format!("{context} must be an object"));
                continue;
            };
            let Some(kind) = obj.get("type").and_then(PropValue::as_str) else {
                self.warn(format!("{context} must have a string type"));
                continue;
            };

            match kind {
                "translate" => {
                    self.validate_object_fields(obj, &["type", "x", "y"], &context);
                    self.validate_object_numbers(obj, &["x", "y"], &context, false);
                }
                "rotate" => {
                    self.validate_object_fields(obj, &["type", "angle"], &context);
                    if let Some(angle) = obj.get("angle") {
                        self.validate_bounded_number(angle, &format!("{context} angle"));
                    } else {
                        self.warn(format!("{context} missing angle"));
                    }
                }
                "scale" => {
                    self.validate_object_fields(obj, &["type", "x", "y", "factor"], &context);
                    if let Some(factor) = obj.get("factor") {
                        self.validate_bounded_number(factor, &format!("{context} factor"));
                    } else if obj.get("x").is_some() || obj.get("y").is_some() {
                        self.validate_object_numbers(obj, &["x", "y"], &context, false);
                        for field in ["x", "y"] {
                            if obj.get(field).is_none() {
                                self.warn(format!("{context} missing {field}"));
                            }
                        }
                    } else {
                        self.warn(format!("{context} missing scale values"));
                    }
                }
                _ => self.warn(format!("{context} has unknown type '{kind}'")),
            }
        }
    }

    fn validate_path_commands(&mut self, node: &TreeNode) {
        let Some(value) = node.props.get("commands") else {
            return;
        };
        let Some(commands) = value.as_array() else {
            self.warn(format!("shape '{}' commands must be an array", node.id));
            return;
        };

        for (idx, command) in commands.iter().enumerate() {
            let context = format!("shape '{}' path command {idx}", node.id);
            match command {
                PropValue::Str(name) if name == "close" => {}
                PropValue::Array(parts) => self.validate_path_command_array(parts, &context),
                PropValue::Object(obj) => self.validate_path_command_object(obj, &context),
                _ => self.warn(format!("{context} has invalid shape")),
            }
        }
    }

    fn validate_path_command_array(&mut self, parts: &[PropValue], context: &str) {
        let Some(name) = parts.first().and_then(PropValue::as_str) else {
            self.warn(format!("{context} must start with a command name"));
            return;
        };
        let numeric_indexes: &[usize] = match name {
            "move_to" | "line_to" => &[1, 2],
            "bezier_to" => &[1, 2, 3, 4, 5, 6],
            "quadratic_to" => &[1, 2, 3, 4],
            "arc" => &[1, 2, 3, 4, 5],
            "arc_to" => &[1, 2, 3, 4, 5],
            "ellipse" => &[1, 2, 3, 4, 5, 6, 7],
            "rounded_rect" => &[1, 2, 3, 4, 5],
            _ => {
                self.warn(format!("{context} has unknown command '{name}'"));
                return;
            }
        };
        for idx in numeric_indexes {
            if let Some(value) = parts.get(*idx) {
                let label = format!("{context} value {idx}");
                if radius_or_dimension_index(name, *idx) {
                    self.validate_non_negative_number(value, &label);
                } else {
                    self.validate_bounded_number(value, &label);
                }
            }
        }
    }

    fn validate_path_command_object(&mut self, obj: &PropMap, context: &str) {
        let Some(name) = obj.get("type").and_then(PropValue::as_str) else {
            self.warn(format!("{context} must have a string type"));
            return;
        };
        if name != "rounded_rect" {
            self.warn(format!("{context} has unknown command '{name}'"));
            return;
        }
        self.validate_object_fields(obj, &["type", "x", "y", "w", "h", "radius"], context);
        self.validate_object_numbers(obj, &["x", "y"], context, false);
        self.validate_object_numbers(obj, &["w", "h"], context, true);
        if let Some(radius) = obj.get("radius") {
            self.validate_radius_value(radius, &format!("{context} radius"));
        }
    }

    fn validate_shape_style(&mut self, node: &TreeNode, key: &str) {
        let Some(value) = node.props.get(key) else {
            return;
        };
        let Some(style) = value.as_object() else {
            return;
        };
        self.validate_object_fields(
            style,
            &["fill", "stroke", "opacity"],
            &format!("shape '{}' prop '{key}'", node.id),
        );
        if let Some(opacity) = style.get("opacity") {
            self.validate_opacity_value(opacity, &format!("shape '{}' {key} opacity", node.id));
        }
        if let Some(stroke) = style.get("stroke").and_then(PropValue::as_object) {
            self.validate_stroke_object(node, key, stroke);
        }
    }

    fn validate_rect_object(&mut self, node: &TreeNode, key: &str, dimensions_required: bool) {
        let Some(value) = node.props.get(key) else {
            return;
        };
        let Some(obj) = value.as_object() else {
            return;
        };
        self.validate_object_fields(
            obj,
            &["x", "y", "w", "h"],
            &format!("shape '{}' prop '{key}'", node.id),
        );
        self.validate_object_numbers(
            obj,
            &["x", "y"],
            &format!("shape '{}' {key}", node.id),
            false,
        );
        self.validate_object_numbers(
            obj,
            &["w", "h"],
            &format!("shape '{}' {key}", node.id),
            true,
        );
        if dimensions_required {
            for field in ["w", "h"] {
                if obj.get(field).is_none() {
                    self.warn(format!("shape '{}' {key} missing {field}", node.id));
                }
            }
        }
    }

    fn validate_drag_bounds(&mut self, node: &TreeNode) {
        let Some(value) = node.props.get("drag_bounds") else {
            return;
        };
        let Some(obj) = value.as_object() else {
            return;
        };
        self.validate_object_fields(
            obj,
            &["min_x", "max_x", "min_y", "max_y"],
            &format!("shape '{}' drag_bounds", node.id),
        );
        self.validate_object_numbers(
            obj,
            &["min_x", "max_x", "min_y", "max_y"],
            &format!("shape '{}' drag_bounds", node.id),
            false,
        );
        for (min_field, max_field) in [("min_x", "max_x"), ("min_y", "max_y")] {
            let min = obj.get(min_field).and_then(|v| v.as_f64());
            let max = obj.get(max_field).and_then(|v| v.as_f64());
            if let (Some(min), Some(max)) = (min, max)
                && min > max
            {
                self.warn(format!(
                    "shape '{}' drag_bounds {min_field} is greater than {max_field}",
                    node.id
                ));
            }
        }
    }

    fn validate_object_fields(&mut self, obj: &PropMap, allowed: &[&str], context: &str) {
        for key in obj.keys() {
            if !allowed.contains(&key) {
                self.warn(format!("{context} has unknown field '{key}'"));
            }
        }
    }

    fn validate_object_numbers(
        &mut self,
        obj: &PropMap,
        fields: &[&str],
        context: &str,
        non_negative: bool,
    ) {
        for field in fields {
            if let Some(value) = obj.get(field) {
                let label = format!("{context} field '{field}'");
                if non_negative {
                    self.validate_non_negative_number(value, &label);
                } else {
                    self.validate_bounded_number(value, &label);
                }
            }
        }
    }

    fn validate_radius_value(&mut self, value: &PropValue, context: &str) {
        match value {
            PropValue::Object(radius) => {
                self.validate_object_fields(
                    radius,
                    &["top_left", "top_right", "bottom_right", "bottom_left"],
                    context,
                );
                for field in ["top_left", "top_right", "bottom_right", "bottom_left"] {
                    if let Some(value) = radius.get(field) {
                        self.validate_non_negative_number(value, &format!("{context} {field}"));
                    }
                }
            }
            _ => self.validate_non_negative_number(value, context),
        }
    }

    fn validate_opacity_value(&mut self, value: &PropValue, context: &str) {
        if let Some(number) = self.number(value, context)
            && !(0.0..=1.0).contains(&number)
        {
            self.warn(format!("{context} must be between 0 and 1"));
        }
    }

    fn validate_non_negative_number(&mut self, value: &PropValue, context: &str) {
        if let Some(number) = self.number(value, context) {
            if number < 0.0 {
                self.warn(format!("{context} must be non-negative"));
            }
            if number.abs() > MAX_CANVAS_COORDINATE {
                self.warn(format!("{context} is outside the canvas coordinate limit"));
            }
        }
    }

    fn validate_bounded_number(&mut self, value: &PropValue, context: &str) {
        if let Some(number) = self.number(value, context)
            && number.abs() > MAX_CANVAS_COORDINATE
        {
            self.warn(format!("{context} is outside the canvas coordinate limit"));
        }
    }

    fn number(&mut self, value: &PropValue, context: &str) -> Option<f64> {
        let number = match value {
            PropValue::F64(value) => *value,
            PropValue::I64(value) => *value as f64,
            PropValue::U64(value) => *value as f64,
            _ => {
                self.warn(format!("{context} must be a finite number"));
                return None;
            }
        };

        if !number.is_finite() {
            self.warn(format!("{context} must be a finite number"));
            return None;
        }

        Some(number)
    }

    fn warn(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

fn radius_or_dimension_index(command: &str, idx: usize) -> bool {
    matches!(
        (command, idx),
        ("arc", 3)
            | ("arc_to", 5)
            | ("ellipse", 3)
            | ("ellipse", 4)
            | ("rounded_rect", 3)
            | ("rounded_rect", 4)
            | ("rounded_rect", 5)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie_core::protocol::Props;
    use serde_json::{Value, json};

    fn node(id: &str, type_name: &str, props: Value, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: type_name.to_string(),
            props: Props::from_json(props),
            children,
        }
    }

    fn shape(id: &str, type_name: &str, props: Value) -> TreeNode {
        node(id, type_name, props, Vec::new())
    }

    fn canvas(children: Vec<TreeNode>) -> TreeNode {
        node("canvas", "canvas", json!({}), children)
    }

    fn warnings_for(child: TreeNode) -> Vec<String> {
        validate_canvas_shape_tree(&canvas(vec![child]))
    }

    #[test]
    fn canvas_shape_validation_valid_direct_rect_and_layered_group() {
        let direct = canvas(vec![shape(
            "r",
            "rect",
            json!({"x": 1, "y": 2, "w": 10, "h": 20, "fill": "#ff0000"}),
        )]);
        assert!(validate_canvas_shape_tree(&direct).is_empty());

        let layered = canvas(vec![node(
            "layer",
            "__layer__",
            json!({"name": "main"}),
            vec![node(
                "g",
                "group",
                json!({"transforms": [{"type": "translate", "x": 5, "y": 6}]}),
                vec![shape("c", "circle", json!({"x": 1, "y": 2, "r": 3}))],
            )],
        )]);
        assert!(validate_canvas_shape_tree(&layered).is_empty());
    }

    #[test]
    fn canvas_shape_validation_unknown_shape_prop_warns() {
        let warnings = warnings_for(shape(
            "r",
            "rect",
            json!({"x": 0, "w": 1, "h": 1, "extra": true}),
        ));
        assert!(warnings.iter().any(|w| w.contains("unknown prop 'extra'")));
    }

    #[test]
    fn canvas_shape_validation_huge_coordinate_warns() {
        let warnings = warnings_for(shape(
            "r",
            "rect",
            json!({"x": 1_000_001, "y": 0, "w": 1, "h": 1}),
        ));
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("outside the canvas coordinate limit"))
        );
    }

    #[test]
    fn canvas_shape_validation_negative_dimension_or_radius_warns() {
        let rect_warnings =
            warnings_for(shape("r", "rect", json!({"x": 0, "y": 0, "w": -1, "h": 1})));
        assert!(
            rect_warnings
                .iter()
                .any(|w| w.contains("must be non-negative"))
        );

        let circle_warnings = warnings_for(shape("c", "circle", json!({"x": 0, "y": 0, "r": -1})));
        assert!(
            circle_warnings
                .iter()
                .any(|w| w.contains("must be non-negative"))
        );
    }

    #[test]
    fn canvas_shape_validation_reversed_drag_bounds_warns() {
        let warnings = warnings_for(node(
            "g",
            "group",
            json!({
                "draggable": true,
                "drag_bounds": {
                    "min_x": 20,
                    "max_x": 10,
                    "min_y": 0,
                    "max_y": -1
                }
            }),
            Vec::new(),
        ));

        assert!(
            warnings
                .iter()
                .any(|w| w.contains("min_x is greater than max_x"))
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("min_y is greater than max_y"))
        );
    }

    #[test]
    fn canvas_shape_validation_invalid_transform_object_warns() {
        let warnings = warnings_for(node(
            "g",
            "group",
            json!({"transforms": [{"type": "skew", "x": 1}, {"type": "scale"}]}),
            Vec::new(),
        ));
        assert!(warnings.iter().any(|w| w.contains("unknown type 'skew'")));
        assert!(warnings.iter().any(|w| w.contains("missing scale values")));
    }

    #[test]
    fn canvas_shape_validation_image_or_svg_url_source_warns() {
        let image_warnings = warnings_for(shape(
            "i",
            "image",
            json!({"source": "https://example.invalid/image.png", "x": 0, "y": 0, "w": 1, "h": 1}),
        ));
        assert!(image_warnings.iter().any(|w| w.contains("URL-like value")));

        let svg_warnings = warnings_for(shape(
            "s",
            "svg",
            json!({"source": "file://tmp/icon.svg", "x": 0, "y": 0, "w": 1, "h": 1}),
        ));
        assert!(svg_warnings.iter().any(|w| w.contains("URL-like value")));
    }

    #[test]
    fn canvas_shape_validation_unknown_child_type_warns_under_canvas_layer_and_group() {
        let warnings = validate_canvas_shape_tree(&canvas(vec![
            shape("bad-canvas", "not_shape", json!({})),
            node(
                "layer",
                "__layer__",
                json!({"name": "main"}),
                vec![shape("bad-layer", "not_shape", json!({}))],
            ),
            node(
                "g",
                "group",
                json!({}),
                vec![shape("bad-group", "not_shape", json!({}))],
            ),
        ]));

        assert!(warnings.iter().any(|w| w.contains("bad-canvas")));
        assert!(warnings.iter().any(|w| w.contains("bad-layer")));
        assert!(warnings.iter().any(|w| w.contains("bad-group")));
    }
}
