//! Canvas shape types and extraction from the wire tree.

use std::collections::BTreeMap;

use crate::protocol::TreeNode;

use super::super::{Angle, PlushieType, Radius};
use super::clip::ClipRect;
use super::drag::{DragAxis, DragBounds};
use super::fill::{CanvasFill, FillRule};
use super::hit::HitRect;
use super::path::{PathCommand, decode_commands};
use super::shape_style::ShapeStyle;
use super::stroke::Stroke;
use super::transform::{Transform, decode_transforms};

/// A canvas shape node, decoded from the wire tree.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum CanvasShape {
    /// Rect.
    Rect(RectShape),
    /// Circle.
    Circle(CircleShape),
    /// Line.
    Line(LineShape),
    /// Path.
    Path(PathShape),
    /// Text.
    Text(TextShape),
    /// Image.
    Image(ImageShape),
    /// Svg.
    Svg(SvgShape),
    /// Group.
    Group(GroupShape),
}

impl CanvasShape {
    /// Decode a TreeNode into a CanvasShape based on its type_name.
    ///
    /// Returns `None` for unrecognized types (layers, non-shape nodes).
    pub fn from_node(node: &TreeNode) -> Option<Self> {
        match node.type_name.as_str() {
            "rect" => Some(Self::Rect(RectShape::from_node(node))),
            "circle" => Some(Self::Circle(CircleShape::from_node(node))),
            "line" => Some(Self::Line(LineShape::from_node(node))),
            "path" => Some(Self::Path(PathShape::from_node(node))),
            "text" => Some(Self::Text(TextShape::from_node(node))),
            "image" => Some(Self::Image(ImageShape::from_node(node))),
            "svg" => Some(Self::Svg(SvgShape::from_node(node))),
            "group" => Some(Self::Group(GroupShape::from_node(node))),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// RectShape
// ---------------------------------------------------------------------------
/// Canvas rect shape descriptor.

#[derive(Debug, Clone, PartialEq)]
pub struct RectShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// W.
    pub w: f32,
    /// H.
    pub h: f32,
    /// Fill.
    pub fill: Option<CanvasFill>,
    /// Stroke.
    pub stroke: Option<Stroke>,
    /// Alpha multiplier (0..=1).
    pub opacity: Option<f32>,
    /// Fill rule.
    pub fill_rule: Option<FillRule>,
    /// Corner or drop radius.
    pub radius: Option<Radius>,
}

impl RectShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            id: id_from_node(node),
            x: f32::extract(p, "x").unwrap_or(0.0),
            y: f32::extract(p, "y").unwrap_or(0.0),
            w: f32::extract(p, "w").unwrap_or(0.0),
            h: f32::extract(p, "h").unwrap_or(0.0),
            fill: CanvasFill::extract(p, "fill"),
            stroke: Stroke::extract(p, "stroke"),
            opacity: f32::extract(p, "opacity"),
            fill_rule: FillRule::extract(p, "fill_rule"),
            radius: Radius::extract(p, "radius"),
        }
    }
}

// ---------------------------------------------------------------------------
// CircleShape
// ---------------------------------------------------------------------------
/// Canvas circle shape descriptor.

#[derive(Debug, Clone, PartialEq)]
pub struct CircleShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// R.
    pub r: f32,
    /// Fill.
    pub fill: Option<CanvasFill>,
    /// Stroke.
    pub stroke: Option<Stroke>,
    /// Alpha multiplier (0..=1).
    pub opacity: Option<f32>,
    /// Fill rule.
    pub fill_rule: Option<FillRule>,
}

impl CircleShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            id: id_from_node(node),
            x: f32::extract(p, "x").unwrap_or(0.0),
            y: f32::extract(p, "y").unwrap_or(0.0),
            r: f32::extract(p, "r").unwrap_or(0.0),
            fill: CanvasFill::extract(p, "fill"),
            stroke: Stroke::extract(p, "stroke"),
            opacity: f32::extract(p, "opacity"),
            fill_rule: FillRule::extract(p, "fill_rule"),
        }
    }
}

// ---------------------------------------------------------------------------
// LineShape
// ---------------------------------------------------------------------------
/// Canvas line shape descriptor.

#[derive(Debug, Clone, PartialEq)]
pub struct LineShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// X1.
    pub x1: f32,
    /// Y1.
    pub y1: f32,
    /// X2.
    pub x2: f32,
    /// Y2.
    pub y2: f32,
    /// Stroke.
    pub stroke: Option<Stroke>,
    /// Alpha multiplier (0..=1).
    pub opacity: Option<f32>,
}

impl LineShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            id: id_from_node(node),
            x1: f32::extract(p, "x1").unwrap_or(0.0),
            y1: f32::extract(p, "y1").unwrap_or(0.0),
            x2: f32::extract(p, "x2").unwrap_or(0.0),
            y2: f32::extract(p, "y2").unwrap_or(0.0),
            stroke: Stroke::extract(p, "stroke"),
            opacity: f32::extract(p, "opacity"),
        }
    }
}

// ---------------------------------------------------------------------------
// PathShape
// ---------------------------------------------------------------------------

/// Canvas arbitrary path built from drawing commands.
#[derive(Debug, Clone, PartialEq)]
pub struct PathShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// Commands.
    pub commands: Vec<PathCommand>,
    /// Fill.
    pub fill: Option<CanvasFill>,
    /// Stroke.
    pub stroke: Option<Stroke>,
    /// Alpha multiplier (0..=1).
    pub opacity: Option<f32>,
    /// Fill rule.
    pub fill_rule: Option<FillRule>,
}

impl PathShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        let commands = p
            .get_value("commands")
            .map(|v| decode_commands(&v))
            .unwrap_or_default();
        Self {
            id: id_from_node(node),
            commands,
            fill: CanvasFill::extract(p, "fill"),
            stroke: Stroke::extract(p, "stroke"),
            opacity: f32::extract(p, "opacity"),
            fill_rule: FillRule::extract(p, "fill_rule"),
        }
    }
}

// ---------------------------------------------------------------------------
// TextShape
// ---------------------------------------------------------------------------
/// Canvas text shape descriptor.

#[derive(Debug, Clone, PartialEq)]
pub struct TextShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// Content.
    pub content: String,
    /// Fill.
    pub fill: Option<CanvasFill>,
    /// Size in pixels.
    pub size: Option<f32>,
    /// Font specifier.
    pub font: Option<String>,
    /// Horizontal alignment.
    pub align_x: Option<String>,
    /// Vertical alignment.
    pub align_y: Option<String>,
    /// Alpha multiplier (0..=1).
    pub opacity: Option<f32>,
}

impl TextShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            id: id_from_node(node),
            x: f32::extract(p, "x").unwrap_or(0.0),
            y: f32::extract(p, "y").unwrap_or(0.0),
            content: String::extract(p, "content").unwrap_or_default(),
            fill: CanvasFill::extract(p, "fill"),
            size: f32::extract(p, "size"),
            font: String::extract(p, "font"),
            align_x: String::extract(p, "align_x"),
            align_y: String::extract(p, "align_y"),
            opacity: f32::extract(p, "opacity"),
        }
    }
}

// ---------------------------------------------------------------------------
// ImageShape
// ---------------------------------------------------------------------------
/// Canvas image shape descriptor.

#[derive(Debug, Clone, PartialEq)]
pub struct ImageShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// Source identifier.
    pub source: String,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// W.
    pub w: f32,
    /// H.
    pub h: f32,
    /// Rotation angle. Wire format is degrees.
    pub rotation: Option<Angle>,
    /// Alpha multiplier (0..=1).
    pub opacity: Option<f32>,
}

impl ImageShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            id: id_from_node(node),
            source: String::extract(p, "source").unwrap_or_default(),
            x: f32::extract(p, "x").unwrap_or(0.0),
            y: f32::extract(p, "y").unwrap_or(0.0),
            w: f32::extract(p, "w").unwrap_or(0.0),
            h: f32::extract(p, "h").unwrap_or(0.0),
            rotation: Angle::extract(p, "rotation"),
            opacity: f32::extract(p, "opacity"),
        }
    }
}

// ---------------------------------------------------------------------------
// SvgShape
// ---------------------------------------------------------------------------
/// Canvas svg shape descriptor.

#[derive(Debug, Clone, PartialEq)]
pub struct SvgShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// Source identifier.
    pub source: String,
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
    /// W.
    pub w: f32,
    /// H.
    pub h: f32,
}

impl SvgShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            id: id_from_node(node),
            source: String::extract(p, "source").unwrap_or_default(),
            x: f32::extract(p, "x").unwrap_or(0.0),
            y: f32::extract(p, "y").unwrap_or(0.0),
            w: f32::extract(p, "w").unwrap_or(0.0),
            h: f32::extract(p, "h").unwrap_or(0.0),
        }
    }
}

// ---------------------------------------------------------------------------
// GroupShape
// ---------------------------------------------------------------------------

/// A canvas group with optional transforms, clip, interactivity, and children.
///
/// On the wire, both structural `group` and `interactive` elements use
/// type "group". The presence of interactive fields (on_click, draggable,
/// etc.) distinguishes them.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupShape {
    /// Target widget ID.
    pub id: Option<String>,
    /// Child nodes.
    pub children: Vec<CanvasShape>,
    /// Transforms.
    pub transforms: Vec<Transform>,
    /// Clip region.
    pub clip: Option<ClipRect>,
    // Interactive fields
    /// Click handler hook.
    pub on_click: Option<bool>,
    /// On hover.
    pub on_hover: Option<bool>,
    /// Draggable.
    pub draggable: Option<bool>,
    /// Drag axis.
    pub drag_axis: Option<DragAxis>,
    /// Drag bounds.
    pub drag_bounds: Option<DragBounds>,
    /// Cursor.
    pub cursor: Option<String>,
    /// Hit rect.
    pub hit_rect: Option<HitRect>,
    /// Tooltip.
    pub tooltip: Option<String>,
    /// Hover style.
    pub hover_style: Option<ShapeStyle>,
    /// Pressed style.
    pub pressed_style: Option<ShapeStyle>,
    /// Focus style.
    pub focus_style: Option<ShapeStyle>,
    /// Show focus ring.
    pub show_focus_ring: Option<bool>,
    /// Focus ring radius.
    pub focus_ring_radius: Option<f32>,
    /// Whether this item accepts keyboard focus.
    pub focusable: Option<bool>,
    /// Accessibility annotations for interactive canvas shapes.
    pub a11y: Option<super::super::A11y>,
}

impl GroupShape {
    /// Construct from a node.
    pub fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;

        let transforms = p
            .get_value("transforms")
            .as_ref()
            .map(decode_transforms)
            .unwrap_or_default();

        let children = node
            .children
            .iter()
            .filter_map(CanvasShape::from_node)
            .collect();

        Self {
            id: id_from_node(node),
            children,
            transforms,
            clip: ClipRect::extract(p, "clip"),
            on_click: bool::extract(p, "on_click"),
            on_hover: bool::extract(p, "on_hover"),
            draggable: bool::extract(p, "draggable"),
            drag_axis: DragAxis::extract(p, "drag_axis"),
            drag_bounds: DragBounds::extract(p, "drag_bounds"),
            cursor: String::extract(p, "cursor"),
            hit_rect: HitRect::extract(p, "hit_rect"),
            tooltip: String::extract(p, "tooltip"),
            hover_style: ShapeStyle::extract(p, "hover_style"),
            pressed_style: ShapeStyle::extract(p, "pressed_style"),
            focus_style: ShapeStyle::extract(p, "focus_style"),
            show_focus_ring: bool::extract(p, "show_focus_ring"),
            focus_ring_radius: f32::extract(p, "focus_ring_radius"),
            focusable: bool::extract(p, "focusable"),
            a11y: super::super::A11y::extract(p, "a11y"),
        }
    }
}

// ---------------------------------------------------------------------------
// Layer extraction
// ---------------------------------------------------------------------------

/// Extract canvas layers from a canvas widget's children.
///
/// Walks the node's children looking for `__layer__` type nodes.
/// Returns a map of layer name to shapes, ordered by layer name.
/// The layer name comes from the node's `name` prop (falling back
/// to the node ID).
pub fn extract_canvas_layers(node: &TreeNode) -> BTreeMap<String, Vec<CanvasShape>> {
    let mut layers = BTreeMap::new();
    for child in &node.children {
        if child.type_name == "__layer__" {
            let name = child
                .props
                .get_str("name")
                .map(String::from)
                .unwrap_or_else(|| child.id.clone());
            let shapes = child
                .children
                .iter()
                .filter_map(CanvasShape::from_node)
                .collect();
            layers.insert(name, shapes);
        }
    }
    layers
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the node ID, returning None for auto-generated IDs.
///
/// Auto-generated IDs (those starting with `__`) are treated as absent
/// since they have no user-facing meaning.
fn id_from_node(node: &TreeNode) -> Option<String> {
    if node.id.starts_with("__") {
        None
    } else {
        Some(node.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn tree_node(type_name: &str, props: Value) -> TreeNode {
        serde_json::from_value(json!({
            "id": "__auto__",
            "type": type_name,
            "props": props,
        }))
        .unwrap()
    }

    fn tree_node_with_id(id: &str, type_name: &str, props: Value) -> TreeNode {
        serde_json::from_value(json!({
            "id": id,
            "type": type_name,
            "props": props,
        }))
        .unwrap()
    }

    #[test]
    fn rect_shape() {
        let node = tree_node(
            "rect",
            json!({"x": 10.0, "y": 20.0, "w": 100.0, "h": 50.0, "fill": "#ff0000"}),
        );
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Rect(r) = shape {
            assert_eq!(r.x, 10.0);
            assert_eq!(r.w, 100.0);
            assert!(r.fill.is_some());
            assert!(r.id.is_none()); // auto-id filtered
        } else {
            panic!("expected Rect");
        }
    }

    #[test]
    fn rect_with_user_id() {
        let node = tree_node_with_id("bg", "rect", json!({"w": 50.0, "h": 50.0}));
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Rect(r) = shape {
            assert_eq!(r.id, Some("bg".into()));
        } else {
            panic!("expected Rect");
        }
    }

    #[test]
    fn circle_shape() {
        let node = tree_node("circle", json!({"x": 50.0, "y": 50.0, "r": 25.0}));
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Circle(c) = shape {
            assert_eq!(c.r, 25.0);
        } else {
            panic!("expected Circle");
        }
    }

    #[test]
    fn line_shape() {
        let node = tree_node(
            "line",
            json!({"x1": 0.0, "y1": 0.0, "x2": 100.0, "y2": 100.0}),
        );
        let shape = CanvasShape::from_node(&node).unwrap();
        assert!(matches!(shape, CanvasShape::Line(_)));
    }

    #[test]
    fn path_shape() {
        let node = tree_node(
            "path",
            json!({
                "commands": [["move_to", 0, 0], ["line_to", 10, 10]],
                "fill": "#000000"
            }),
        );
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Path(p) = shape {
            assert_eq!(p.commands.len(), 2);
            assert_eq!(p.commands[0], PathCommand::MoveTo { x: 0.0, y: 0.0 });
            assert_eq!(p.commands[1], PathCommand::LineTo { x: 10.0, y: 10.0 });
        } else {
            panic!("expected Path");
        }
    }

    #[test]
    fn text_shape() {
        let node = tree_node(
            "text",
            json!({"x": 10.0, "y": 20.0, "content": "hello", "size": 14.0}),
        );
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Text(t) = shape {
            assert_eq!(t.content, "hello");
            assert_eq!(t.size, Some(14.0));
        } else {
            panic!("expected Text");
        }
    }

    #[test]
    fn image_shape() {
        let node = tree_node(
            "image",
            json!({"source": "/img/cat.png", "x": 0.0, "y": 0.0, "w": 64.0, "h": 64.0}),
        );
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Image(i) = shape {
            assert_eq!(i.source, "/img/cat.png");
        } else {
            panic!("expected Image");
        }
    }

    #[test]
    fn svg_shape() {
        let node = tree_node(
            "svg",
            json!({"source": "/icons/star.svg", "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0}),
        );
        let shape = CanvasShape::from_node(&node).unwrap();
        assert!(matches!(shape, CanvasShape::Svg(_)));
    }

    #[test]
    fn group_with_children() {
        let node: TreeNode = serde_json::from_value(json!({
            "id": "grp",
            "type": "group",
            "props": {
                "transforms": [{"type": "translate", "x": 10.0, "y": 20.0}]
            },
            "children": [
                {"id": "__a1__", "type": "rect", "props": {"w": 50.0, "h": 50.0}},
                {"id": "__a2__", "type": "circle", "props": {"r": 10.0}}
            ]
        }))
        .unwrap();
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Group(g) = shape {
            assert_eq!(g.id, Some("grp".into()));
            assert_eq!(g.children.len(), 2);
            assert_eq!(g.transforms.len(), 1);
        } else {
            panic!("expected Group");
        }
    }

    #[test]
    fn group_interactive_fields() {
        let node: TreeNode = serde_json::from_value(json!({
            "id": "btn",
            "type": "group",
            "props": {
                "on_click": true,
                "cursor": "pointer",
                "focusable": true,
                "hover_style": {"fill": "#eee", "opacity": 0.8}
            },
            "children": []
        }))
        .unwrap();
        let shape = CanvasShape::from_node(&node).unwrap();
        if let CanvasShape::Group(g) = shape {
            assert_eq!(g.on_click, Some(true));
            assert_eq!(g.cursor, Some("pointer".into()));
            assert_eq!(g.focusable, Some(true));
            assert!(g.hover_style.is_some());
        } else {
            panic!("expected Group");
        }
    }

    #[test]
    fn extract_layers() {
        let canvas: TreeNode = serde_json::from_value(json!({
            "id": "canvas_1",
            "type": "canvas",
            "props": {},
            "children": [
                {
                    "id": "bg_layer",
                    "type": "__layer__",
                    "props": {"name": "background"},
                    "children": [
                        {"id": "__r1__", "type": "rect", "props": {"w": 800.0, "h": 600.0, "fill": "#eee"}}
                    ]
                },
                {
                    "id": "fg_layer",
                    "type": "__layer__",
                    "props": {"name": "foreground"},
                    "children": [
                        {"id": "__c1__", "type": "circle", "props": {"x": 50.0, "y": 50.0, "r": 10.0}}
                    ]
                }
            ]
        })).unwrap();
        let layers = extract_canvas_layers(&canvas);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers["background"].len(), 1);
        assert_eq!(layers["foreground"].len(), 1);
    }

    #[test]
    fn unknown_type_returns_none() {
        let node = tree_node("unknown_widget", json!({}));
        assert!(CanvasShape::from_node(&node).is_none());
    }
}
