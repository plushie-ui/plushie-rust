//! Canvas shape builders.
//!
//! Canvas shapes are children of a `canvas()` widget. Shapes are
//! leaf nodes (except `group`, which can contain other shapes).
//! Shape constructors use `#[track_caller]` auto-IDs since shapes
//! are not interactive by default.
//!
//! ```ignore
//! use plushie::prelude::*;
//!
//! let view = canvas("drawing")
//!     .width(400.0).height(300.0)
//!     .child(layer("bg").children([
//!         rect(0.0, 0.0, 400.0, 300.0).fill(Color::hex("#1a1a2e")),
//!         circle(200.0, 150.0, 40.0).fill(Color::red()),
//!         line(0.0, 0.0, 400.0, 300.0).stroke(Color::white()).stroke_width(2.0),
//!     ]));
//! ```

use serde_json::{json, Map, Value};

use crate::View;
use crate::types::*;

// ---------------------------------------------------------------------------
// CanvasBuilder
// ---------------------------------------------------------------------------

/// Builder for a canvas widget (interactive drawing surface).
pub struct CanvasBuilder {
    id: String,
    props: Map<String, Value>,
    children: Vec<View>,
}

/// Create a canvas widget. The ID is required (canvas is interactive).
pub fn canvas(id: &str) -> CanvasBuilder {
    CanvasBuilder {
        id: id.to_string(),
        props: Map::new(),
        children: vec![],
    }
}

impl CanvasBuilder {
    pub fn width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "width", w); self }
    pub fn height(mut self, h: f32) -> Self { super::set_prop(&mut self.props, "height", h); self }
    pub fn background(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "background", super::color_to_value(&c.into())); self }
    pub fn on_press(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "on_press", v); self }
    pub fn on_release(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "on_release", v); self }
    pub fn on_move(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "on_move", v); self }
    pub fn on_scroll(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "on_scroll", v); self }
    pub fn interactive(mut self, v: bool) -> Self { super::set_prop(&mut self.props, "interactive", v); self }
    pub fn arrow_mode(mut self, mode: &str) -> Self { super::set_prop(&mut self.props, "arrow_mode", mode); self }
    pub fn alt(mut self, text: &str) -> Self { super::set_prop(&mut self.props, "alt", text); self }
    pub fn description(mut self, text: &str) -> Self { super::set_prop(&mut self.props, "description", text); self }
    pub fn role(mut self, role: &str) -> Self { super::set_prop(&mut self.props, "role", role); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }

    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<I, V>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<View>,
    {
        self.children.extend(items.into_iter().map(Into::into));
        self
    }
}

impl From<CanvasBuilder> for View {
    fn from(b: CanvasBuilder) -> Self {
        View::node(b.id, "canvas", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// LayerBuilder
// ---------------------------------------------------------------------------

/// Builder for a named layer inside a canvas.
pub struct LayerBuilder {
    id: String,
    props: Map<String, Value>,
    children: Vec<View>,
}

/// Create a named layer inside a canvas.
pub fn layer(name: &str) -> LayerBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "name", name);
    LayerBuilder {
        id: name.to_string(),
        props,
        children: vec![],
    }
}

impl LayerBuilder {
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<I, V>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<View>,
    {
        self.children.extend(items.into_iter().map(Into::into));
        self
    }
}

impl From<LayerBuilder> for View {
    fn from(b: LayerBuilder) -> Self {
        View::node(b.id, "__layer__", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// GroupBuilder
// ---------------------------------------------------------------------------

/// Builder for a shape group inside a canvas.
pub struct GroupBuilder {
    id: String,
    props: Map<String, Value>,
    children: Vec<View>,
}

/// Create a shape group with the given ID.
pub fn group(id: &str) -> GroupBuilder {
    GroupBuilder {
        id: id.to_string(),
        props: Map::new(),
        children: vec![],
    }
}

impl GroupBuilder {
    pub fn x(mut self, v: f32) -> Self { super::set_prop(&mut self.props, "x", v); self }
    pub fn y(mut self, v: f32) -> Self { super::set_prop(&mut self.props, "y", v); self }
    pub fn on_click(mut self, enabled: bool) -> Self { super::set_prop(&mut self.props, "on_click", enabled); self }
    pub fn draggable(mut self, enabled: bool) -> Self { super::set_prop(&mut self.props, "draggable", enabled); self }
    pub fn cursor(mut self, c: &str) -> Self { super::set_prop(&mut self.props, "cursor", c); self }
    pub fn tooltip(mut self, text: &str) -> Self { super::set_prop(&mut self.props, "tooltip", text); self }
    pub fn event_rate(mut self, rate: u32) -> Self { super::set_prop(&mut self.props, "event_rate", rate); self }
    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self { super::set_prop(&mut self.props, "a11y", a11y.clone()); self }

    pub fn translate(mut self, x: f32, y: f32) -> Self {
        let transforms = self.props.entry("transforms").or_insert(json!([]));
        if let Some(arr) = transforms.as_array_mut() {
            arr.push(json!({"type": "translate", "x": x, "y": y}));
        }
        self
    }

    pub fn rotate(mut self, angle: f32) -> Self {
        let transforms = self.props.entry("transforms").or_insert(json!([]));
        if let Some(arr) = transforms.as_array_mut() {
            arr.push(json!({"type": "rotate", "angle": angle}));
        }
        self
    }

    pub fn scale_xy(mut self, x: f32, y: f32) -> Self {
        let transforms = self.props.entry("transforms").or_insert(json!([]));
        if let Some(arr) = transforms.as_array_mut() {
            arr.push(json!({"type": "scale", "x": x, "y": y}));
        }
        self
    }

    pub fn scale_uniform(self, factor: f32) -> Self {
        self.scale_xy(factor, factor)
    }

    pub fn clip(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        super::set_prop(&mut self.props, "clip", json!({"x": x, "y": y, "w": w, "h": h}));
        self
    }

    pub fn hit_rect(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        super::set_prop(&mut self.props, "hit_rect", json!({"x": x, "y": y, "w": w, "h": h}));
        self
    }

    pub fn drag_bounds(mut self, min_x: f32, max_x: f32, min_y: f32, max_y: f32) -> Self {
        super::set_prop(&mut self.props, "drag_bounds", json!({
            "min_x": min_x, "max_x": max_x, "min_y": min_y, "max_y": max_y
        }));
        self
    }

    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<I, V>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<View>,
    {
        self.children.extend(items.into_iter().map(Into::into));
        self
    }
}

impl From<GroupBuilder> for View {
    fn from(b: GroupBuilder) -> Self {
        View::node(b.id, "group", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// RectBuilder
// ---------------------------------------------------------------------------

/// Builder for a rectangle shape.
pub struct RectBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a rectangle shape at `(x, y)` with size `w` x `h`.
#[track_caller]
pub fn rect(x: f32, y: f32, w: f32, h: f32) -> RectBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "w", w);
    super::set_prop(&mut props, "h", h);
    RectBuilder { id: super::auto_id("rect"), props }
}

impl RectBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn fill(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "fill", super::color_to_value(&c.into())); self }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "stroke", super::color_to_value(&c.into())); self }
    pub fn stroke_width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "stroke_width", w); self }
    pub fn stroke_cap(mut self, cap: &str) -> Self { super::set_prop(&mut self.props, "stroke_cap", cap); self }
    pub fn stroke_join(mut self, join: &str) -> Self { super::set_prop(&mut self.props, "stroke_join", join); self }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        super::set_prop(&mut self.props, "stroke_dash", json!({"segments": segments, "offset": offset}));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
    pub fn radius(mut self, r: f32) -> Self { super::set_prop(&mut self.props, "radius", r); self }
    pub fn fill_gradient(mut self, x1: f32, y1: f32, x2: f32, y2: f32, stops: &[(f32, &str)]) -> Self {
        let stops_json: Vec<Value> = stops.iter().map(|(offset, color)| {
            json!({"offset": offset, "color": color})
        }).collect();
        super::set_prop(&mut self.props, "fill", json!({
            "type": "linear", "x1": x1, "y1": y1, "x2": x2, "y2": y2,
            "stops": stops_json
        }));
        self
    }
}

impl From<RectBuilder> for View {
    fn from(b: RectBuilder) -> View {
        View::leaf(b.id, "rect", b.props)
    }
}

// ---------------------------------------------------------------------------
// CircleBuilder
// ---------------------------------------------------------------------------

/// Builder for a circle shape.
pub struct CircleBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a circle shape centered at `(x, y)` with radius `r`.
#[track_caller]
pub fn circle(x: f32, y: f32, r: f32) -> CircleBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "r", r);
    CircleBuilder { id: super::auto_id("circle"), props }
}

impl CircleBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn fill(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "fill", super::color_to_value(&c.into())); self }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "stroke", super::color_to_value(&c.into())); self }
    pub fn stroke_width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "stroke_width", w); self }
    pub fn stroke_cap(mut self, cap: &str) -> Self { super::set_prop(&mut self.props, "stroke_cap", cap); self }
    pub fn stroke_join(mut self, join: &str) -> Self { super::set_prop(&mut self.props, "stroke_join", join); self }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        super::set_prop(&mut self.props, "stroke_dash", json!({"segments": segments, "offset": offset}));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
    pub fn fill_gradient(mut self, x1: f32, y1: f32, x2: f32, y2: f32, stops: &[(f32, &str)]) -> Self {
        let stops_json: Vec<Value> = stops.iter().map(|(offset, color)| {
            json!({"offset": offset, "color": color})
        }).collect();
        super::set_prop(&mut self.props, "fill", json!({
            "type": "linear", "x1": x1, "y1": y1, "x2": x2, "y2": y2,
            "stops": stops_json
        }));
        self
    }
}

impl From<CircleBuilder> for View {
    fn from(b: CircleBuilder) -> View {
        View::leaf(b.id, "circle", b.props)
    }
}

// ---------------------------------------------------------------------------
// LineBuilder
// ---------------------------------------------------------------------------

/// Builder for a line shape.
pub struct LineBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a line from `(x1, y1)` to `(x2, y2)`.
#[track_caller]
pub fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> LineBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "x1", x1);
    super::set_prop(&mut props, "y1", y1);
    super::set_prop(&mut props, "x2", x2);
    super::set_prop(&mut props, "y2", y2);
    LineBuilder { id: super::auto_id("line"), props }
}

impl LineBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "stroke", super::color_to_value(&c.into())); self }
    pub fn stroke_width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "stroke_width", w); self }
    pub fn stroke_cap(mut self, cap: &str) -> Self { super::set_prop(&mut self.props, "stroke_cap", cap); self }
    pub fn stroke_join(mut self, join: &str) -> Self { super::set_prop(&mut self.props, "stroke_join", join); self }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        super::set_prop(&mut self.props, "stroke_dash", json!({"segments": segments, "offset": offset}));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
}

impl From<LineBuilder> for View {
    fn from(b: LineBuilder) -> View {
        View::leaf(b.id, "line", b.props)
    }
}

// ---------------------------------------------------------------------------
// PathBuilder
// ---------------------------------------------------------------------------

/// Builder for an SVG path shape.
pub struct PathBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create an SVG path shape from a path data string.
///
/// ```ignore
/// path("M 0 0 L 100 100 L 0 100 Z").fill(Color::blue())
/// ```
#[track_caller]
pub fn path(data: &str) -> PathBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "data", data);
    PathBuilder { id: super::auto_id("path"), props }
}

impl PathBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn fill(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "fill", super::color_to_value(&c.into())); self }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "stroke", super::color_to_value(&c.into())); self }
    pub fn stroke_width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "stroke_width", w); self }
    pub fn stroke_cap(mut self, cap: &str) -> Self { super::set_prop(&mut self.props, "stroke_cap", cap); self }
    pub fn stroke_join(mut self, join: &str) -> Self { super::set_prop(&mut self.props, "stroke_join", join); self }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        super::set_prop(&mut self.props, "stroke_dash", json!({"segments": segments, "offset": offset}));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
    pub fn fill_rule(mut self, rule: &str) -> Self { super::set_prop(&mut self.props, "fill_rule", rule); self }
    pub fn fill_gradient(mut self, x1: f32, y1: f32, x2: f32, y2: f32, stops: &[(f32, &str)]) -> Self {
        let stops_json: Vec<Value> = stops.iter().map(|(offset, color)| {
            json!({"offset": offset, "color": color})
        }).collect();
        super::set_prop(&mut self.props, "fill", json!({
            "type": "linear", "x1": x1, "y1": y1, "x2": x2, "y2": y2,
            "stops": stops_json
        }));
        self
    }
}

impl From<PathBuilder> for View {
    fn from(b: PathBuilder) -> View {
        View::leaf(b.id, "path", b.props)
    }
}

// ---------------------------------------------------------------------------
// CanvasTextBuilder
// ---------------------------------------------------------------------------

/// Builder for text rendered inside a canvas.
pub struct CanvasTextBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a text element inside a canvas at `(x, y)`.
#[track_caller]
pub fn canvas_text(x: f32, y: f32, content: &str) -> CanvasTextBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "content", content);
    CanvasTextBuilder { id: super::auto_id("text"), props }
}

impl CanvasTextBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn size(mut self, s: f32) -> Self { super::set_prop(&mut self.props, "size", s); self }
    /// Fill color for the text.
    pub fn fill(mut self, c: impl Into<Color>) -> Self { super::set_prop(&mut self.props, "fill", super::color_to_value(&c.into())); self }
    pub fn font(mut self, f: Font) -> Self { super::set_prop(&mut self.props, "font", serde_json::to_value(&f).unwrap()); self }
    /// Horizontal text alignment: `"left"`, `"center"`, or `"right"`.
    pub fn align_x(mut self, a: &str) -> Self { super::set_prop(&mut self.props, "align_x", a); self }
    /// Vertical text alignment: `"top"`, `"center"`, or `"bottom"`.
    pub fn align_y(mut self, a: &str) -> Self { super::set_prop(&mut self.props, "align_y", a); self }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
}

impl From<CanvasTextBuilder> for View {
    fn from(b: CanvasTextBuilder) -> View {
        View::leaf(b.id, "text", b.props)
    }
}

// ---------------------------------------------------------------------------
// CanvasImageBuilder
// ---------------------------------------------------------------------------

/// Builder for an image rendered inside a canvas.
pub struct CanvasImageBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create an image element inside a canvas at `(x, y)`.
#[track_caller]
pub fn canvas_image(x: f32, y: f32, source: &str) -> CanvasImageBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "source", source);
    CanvasImageBuilder { id: super::auto_id("image"), props }
}

impl CanvasImageBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "w", w); self }
    pub fn height(mut self, h: f32) -> Self { super::set_prop(&mut self.props, "h", h); self }
    /// Rotation angle in radians.
    pub fn rotation(mut self, angle: f32) -> Self { super::set_prop(&mut self.props, "rotation", angle); self }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
}

impl From<CanvasImageBuilder> for View {
    fn from(b: CanvasImageBuilder) -> View {
        View::leaf(b.id, "image", b.props)
    }
}

// ---------------------------------------------------------------------------
// CanvasSvgBuilder
// ---------------------------------------------------------------------------

/// Builder for an SVG element rendered inside a canvas.
pub struct CanvasSvgBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create an SVG element inside a canvas at `(x, y)`.
#[track_caller]
pub fn canvas_svg(x: f32, y: f32, source: &str) -> CanvasSvgBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "source", source);
    CanvasSvgBuilder { id: super::auto_id("svg"), props }
}

impl CanvasSvgBuilder {
    pub fn id(mut self, id: &str) -> Self { self.id = id.to_string(); self }
    pub fn width(mut self, w: f32) -> Self { super::set_prop(&mut self.props, "w", w); self }
    pub fn height(mut self, h: f32) -> Self { super::set_prop(&mut self.props, "h", h); self }
    pub fn opacity(mut self, o: f32) -> Self { super::set_prop(&mut self.props, "opacity", o); self }
}

impl From<CanvasSvgBuilder> for View {
    fn from(b: CanvasSvgBuilder) -> View {
        View::leaf(b.id, "svg", b.props)
    }
}

// ---------------------------------------------------------------------------
// Interactive helper
// ---------------------------------------------------------------------------

/// Create an interactive canvas element. Alias for `group()` with
/// interactive props pre-configured.
pub fn interactive(id: &str) -> GroupBuilder {
    group(id).on_click(true)
}
