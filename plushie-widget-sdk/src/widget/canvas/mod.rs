//! Canvas widget -- 2D drawing surface with per-layer caching.
//!
//! Renders shapes from tree children onto an iced canvas. Supports:
//!
//! - **Shapes**: rect, circle, line, arc, path (with SVG-like commands),
//!   text, image
//! - **Layers**: multiple named layers with independent content-hash
//!   invalidation for efficient re-tessellation
//! - **Fills**: solid colors, linear/radial gradients, fill rules
//! - **Strokes**: color, width, line cap/join, dash patterns
//! - **Clipping**: push_clip/pop_clip regions for masked rendering
//! - **Events**: optional press, release, move, scroll handlers with
//!   canvas-local coordinates

mod interaction;
mod program;
mod shapes;
mod types;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use iced::widget::canvas;
use iced::{Color, Element, Length, Point, Theme};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

// --- Public / pub(crate) re-exports ---

pub(crate) use interaction::{collect_interactive_elements, validate_interactive_elements};
pub(crate) use types::{InteractiveElement, TransformMatrix};

// Re-exports used only by tests and the canvas module itself.
// Guarded by cfg(test) to avoid unused-import warnings in production.
#[cfg(test)]
pub(crate) use shapes::{parse_canvas_fill, parse_canvas_stroke};
#[cfg(test)]
pub(crate) use types::{ArrowMode, CanvasState, DragAxis, HitRegion};

// --- canvas_layers_from_node (moved from caches.rs) ---

/// Reconstruct a shape JSON Value from a tree node.
///
/// Shape nodes have `{id, type, props, children}`. This converts back to
/// the `{type: type, id: id, ...props, children: [...]}` format that the
/// canvas rendering and hit-testing code expects.
fn tree_node_to_shape_value(node: &TreeNode) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String(node.type_name.clone()));

    // Copy all props into the shape map. Uses as_value_cow() because
    // the canvas drawing pipeline works with JSON Values internally
    // (shapes are rendered from a flat list of Value maps).
    let props_cow = node.props.as_value_cow();
    if let Some(obj) = props_cow.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
    }

    // Recursively convert children (for group shapes)
    if !node.children.is_empty() {
        let child_shapes: Vec<Value> = node.children.iter().map(tree_node_to_shape_value).collect();
        map.insert("children".to_string(), Value::Array(child_shapes));
    }

    Value::Object(map)
}

/// Extract canvas layer data from a node's children. Returns owned Values
/// suitable for hashing and rendering.
///
/// Canvas nodes carry shapes as tree children:
/// - `__layer__` children with a `name` prop and shape children (layered)
/// - Direct shape children without a layer wrapper (flat, treated as "default" layer)
///
/// Returns a BTreeMap so layer order is deterministic (alphabetical by name).
pub(crate) fn canvas_layers_from_node(
    node: &TreeNode,
) -> std::collections::BTreeMap<String, Value> {
    let mut map = std::collections::BTreeMap::new();

    // Check if children are __layer__ containers
    let has_layers = node.children.iter().any(|c| c.type_name == "__layer__");

    if has_layers {
        for child in &node.children {
            if child.type_name == "__layer__" {
                let layer_name = child
                    .props
                    .as_object()
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                let shapes: Vec<Value> = child
                    .children
                    .iter()
                    .map(tree_node_to_shape_value)
                    .collect();
                map.insert(layer_name, Value::Array(shapes));
            }
        }
    } else if !node.children.is_empty() {
        // Direct shape children (flat canvas)
        let shapes: Vec<Value> = node.children.iter().map(tree_node_to_shape_value).collect();
        map.insert("default".to_string(), Value::Array(shapes));
    }

    map
}

// --- JSON helpers ---

/// Parse an f32 from a JSON value by key, defaulting to 0.
pub(crate) fn json_f32(val: &Value, key: &str) -> f32 {
    val.get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(0.0)
}

/// Parse a Color from a JSON "fill" field. Accepts "#rrggbb" hex strings;
/// defaults to white if missing or unparseable.
#[allow(dead_code)] // used by tests
pub(crate) fn json_color(val: &Value, key: &str) -> Color {
    val.get(key).and_then(parse_color).unwrap_or(Color::WHITE)
}

// --- render_canvas_with_state ---

/// Render a canvas with the provided cache state.
/// Called by CanvasWidget::render() with factory-owned state.
pub(crate) fn render_canvas_with_state<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
    node_caches: Option<&'a HashMap<String, (u64, canvas::Cache<R>)>>,
    interactive_elements: &'a [InteractiveElement],
    pending_focus: Option<String>,
) -> Element<'a, Message, Theme, R> {
    let props = &node.props;
    let width = prop_length(props, "width", Length::Fill);
    let height = prop_length(props, "height", Length::Fixed(200.0));

    // Build sorted layer data from children (shapes as tree nodes).
    let layer_map = canvas_layers_from_node(node);
    let layers: Vec<(String, Vec<Value>)> = layer_map
        .into_iter()
        .map(|(name, val)| {
            let layer_shapes = val.as_array().cloned().unwrap_or_default();
            (name.clone(), shapes::truncate_shapes(&name, layer_shapes))
        })
        .collect();

    let bg_val = props.get_value("background");
    let background = bg_val.as_ref().and_then(parse_color);

    let on_press = prop_bool_default(props, "on_press", false);
    let on_release = prop_bool_default(props, "on_release", false);
    let on_move = prop_bool_default(props, "on_move", false);
    let on_scroll = prop_bool_default(props, "on_scroll", false);
    let interactive = prop_bool_default(props, "interactive", false);
    let has_interactive_elements = !interactive_elements.is_empty();

    let mut c = iced::widget::Canvas::<_, Message, iced::Theme, R>::new(program::CanvasProgram {
        layers,
        caches: node_caches,
        background,
        window_id: ctx.window_id.to_string(),
        id: node.id.clone(),
        on_press: on_press || interactive || has_interactive_elements,
        on_release: on_release || interactive || has_interactive_elements,
        on_move: on_move || interactive || has_interactive_elements,
        on_scroll: on_scroll || interactive,
        images: ctx.images,
        interactive_elements,
        arrow_mode: prop_str(props, "arrow_mode")
            .map(|s| types::ArrowMode::from_str(&s))
            .unwrap_or_default(),
        pending_focus,
    })
    .width(width)
    .height(height);

    c = c.id(iced::widget::Id::from(node.id.clone()));

    if let Some(alt) = prop_str(props, "alt") {
        c = c.alt(alt);
    }
    if let Some(desc) = prop_str(props, "description") {
        c = c.description(desc);
    }

    if let Some(role_str) = prop_str(props, "role") {
        if let Some(role) = crate::a11y::parse_role_str(&role_str) {
            c = c.role(role);
        } else {
            log::warn!("canvas '{}': unknown role '{role_str}'", node.id);
        }
    } else if has_interactive_elements {
        c = c.role(iced::advanced::widget::operation::accessible::Role::Group);
    }

    c.into()
}

// --- Public query functions ---

/// Hit-test a canvas node at a canvas-relative point.
///
/// Parses interactive elements from the canvas node's layers, then
/// checks if the given point hits any interactive element. Returns
/// the element ID if a hit is found.
///
/// Used by the headless interact handler for `canvas_press` actions
/// where coordinates are canvas-relative and we need to determine
/// which element (if any) was clicked without going through iced's
/// mouse event system.
pub fn canvas_hit_test(node: &crate::protocol::TreeNode, x: f32, y: f32) -> Option<String> {
    let layer_map = canvas_layers_from_node(node);

    let mut interactive_elements = Vec::new();
    for (layer_name, shapes_val) in &layer_map {
        if let Some(shapes_arr) = shapes_val.as_array() {
            collect_interactive_elements(
                shapes_arr,
                layer_name,
                TransformMatrix::identity(),
                None,
                None,
                "",
                &mut interactive_elements,
            );
        }
    }

    interaction::find_hit_element(Point::new(x, y), &interactive_elements).map(|e| e.id.clone())
}

/// Check whether a canvas node contains an interactive element with the given ID.
///
/// Used by the scripting layer to verify that a scoped canvas element ID
/// (e.g. "my-canvas/save-button") refers to a real interactive element
/// before emitting a click event.
pub fn canvas_find_element_by_id(node: &crate::protocol::TreeNode, element_id: &str) -> bool {
    let layer_map = canvas_layers_from_node(node);

    let mut interactive_elements = Vec::new();
    for (layer_name, shapes_val) in &layer_map {
        if let Some(shapes_arr) = shapes_val.as_array() {
            collect_interactive_elements(
                shapes_arr,
                layer_name,
                TransformMatrix::identity(),
                None,
                None,
                "",
                &mut interactive_elements,
            );
        }
    }

    interactive_elements.iter().any(|e| e.id == element_id)
}

/// Check whether a canvas node has `on_press` enabled.
pub fn canvas_has_on_press(node: &crate::protocol::TreeNode) -> bool {
    node.props
        .get("on_press")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}
