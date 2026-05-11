//! Canvas widget: 2D drawing surface with per-layer caching.
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
mod validation;

#[cfg(test)]
mod tests;

use iced::{Element, Point, Theme};

use plushie_core::types::{self as core_types, PlushieType};
use plushie_core::types::{CanvasShape, extract_canvas_layers};

use crate::PlushieRenderer;
use crate::canvas_engine::{CanvasLayerCaches, PreparedCanvasLayer};
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

// --- Public / pub(crate) re-exports ---

pub(crate) use interaction::{collect_interactive_elements, validate_interactive_elements};
pub(crate) use shapes::truncate_shapes;
pub(crate) use types::{InteractiveElement, TransformMatrix};
pub(crate) use validation::validate_canvas_shape_tree;

// Re-exports used only by tests and the canvas module itself.
// Guarded by cfg(test) to avoid unused-import warnings in production.
#[cfg(test)]
pub(crate) use shapes::{parse_canvas_fill, parse_canvas_stroke, resolve_color};
#[cfg(test)]
pub(crate) use types::{ArrowMode, CanvasState, DragAxis, HitRegion};

// --- canvas_layers_from_node ---

/// Extract canvas layer data from a node's children as typed shapes.
///
/// Canvas nodes carry shapes as tree children:
/// - `__layer__` children with a `name` prop and shape children (layered)
/// - Direct shape children without a layer wrapper (flat, treated as "default" layer)
///
/// Returns a BTreeMap so layer order is deterministic (alphabetical by name).
pub(crate) fn canvas_layers_from_node(
    node: &TreeNode,
) -> std::collections::BTreeMap<String, Vec<CanvasShape>> {
    // Check if children are __layer__ containers
    let has_layers = node.children.iter().any(|c| c.type_name == "__layer__");

    if has_layers {
        extract_canvas_layers(node)
    } else if !node.children.is_empty() {
        // Direct shape children (flat canvas, treated as "default" layer)
        let shapes: Vec<CanvasShape> = node
            .children
            .iter()
            .filter_map(CanvasShape::from_node)
            .collect();
        let mut map = std::collections::BTreeMap::new();
        map.insert("default".to_string(), shapes);
        map
    } else {
        std::collections::BTreeMap::new()
    }
}

// --- CanvasProps ---

struct CanvasProps {
    width: Option<core_types::Length>,
    height: Option<core_types::Length>,
    background: Option<core_types::Color>,
    alt: Option<String>,
    description: Option<String>,
    role: Option<String>,
    arrow_mode: Option<core_types::ArrowMode>,
}

impl CanvasProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: core_types::Length::extract(p, "width"),
            height: core_types::Length::extract(p, "height"),
            background: core_types::Color::extract(p, "background"),
            alt: String::extract(p, "alt"),
            description: String::extract(p, "description"),
            role: String::extract(p, "role"),
            arrow_mode: core_types::ArrowMode::extract(p, "arrow_mode"),
        }
    }
}

// --- render_canvas_with_state ---

/// Render a canvas with the provided cache state.
/// Called by CanvasWidget::render() with factory-owned state.
pub(crate) fn render_canvas_with_state<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
    node_caches: Option<&'a CanvasLayerCaches<R>>,
    interactive_elements: &'a [InteractiveElement],
    pending_focus: Option<String>,
    prepared_layers: Option<&'a [PreparedCanvasLayer]>,
) -> Element<'a, Message, Theme, R> {
    let cp = CanvasProps::from_node(node);
    let props = &node.props;

    let width = cp
        .width
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Fill);
    let height = cp
        .height
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Fixed(200.0));

    let fallback_layers;
    let layers = if let Some(prepared_layers) = prepared_layers {
        prepared_layers.to_vec()
    } else {
        fallback_layers = canvas_layers_from_node(node)
            .into_iter()
            .map(|(name, layer_shapes)| {
                let truncated = shapes::truncate_shapes(&name, layer_shapes);
                (name, truncated)
            })
            .collect::<Vec<_>>();
        fallback_layers
    };

    let background = cp.background.as_ref().map(iced_convert::color);

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
        arrow_mode: cp
            .arrow_mode
            .map(types::ArrowMode::from)
            .unwrap_or_default(),
        pending_focus,
    })
    .width(width)
    .height(height);

    c = c.id(iced::widget::Id::from(node.id.clone()));

    if let Some(alt) = cp.alt {
        c = c.alt(alt);
    }
    if let Some(desc) = cp.description {
        c = c.description(desc);
    }

    if let Some(ref role_str) = cp.role {
        if let Some(role) = crate::a11y::parse_role_str(role_str) {
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
    for (layer_name, shapes) in &layer_map {
        collect_interactive_elements(
            shapes,
            layer_name,
            TransformMatrix::identity(),
            None,
            None,
            "",
            &mut interactive_elements,
        );
    }

    interaction::find_hit_element(Point::new(x, y), &interactive_elements).map(|e| e.id.clone())
}

/// Check whether a canvas node contains an interactive element with the given ID.
///
/// Used by the scripting layer to verify that a canvas element ID refers
/// to a real interactive element before emitting events.
pub fn canvas_find_element_by_id(node: &crate::protocol::TreeNode, element_id: &str) -> bool {
    let layer_map = canvas_layers_from_node(node);

    let mut interactive_elements = Vec::new();
    for (layer_name, shapes) in &layer_map {
        collect_interactive_elements(
            shapes,
            layer_name,
            TransformMatrix::identity(),
            None,
            None,
            "",
            &mut interactive_elements,
        );
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
