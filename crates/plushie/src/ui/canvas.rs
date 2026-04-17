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

use super::{PropMap, PropValue};

use crate::View;
use crate::types::*;
pub use plushie_core::types::canvas::PathCommand;

/// Push a transform entry to the "transforms" array in a props map.
fn push_transform(props: &mut PropMap, kind: &str, fields: &[(&str, f32)]) {
    use super::PropValue;
    let mut entry = PropMap::new();
    entry.insert("type", PropValue::Str(kind.into()));
    for (k, v) in fields {
        entry.insert(*k, PropValue::F64(*v as f64));
    }
    if let Some(PropValue::Array(arr)) = props.get_mut("transforms") {
        arr.push(PropValue::Object(entry));
    } else {
        props.insert(
            "transforms",
            PropValue::Array(vec![PropValue::Object(entry)]),
        );
    }
}

/// Merge a key/value into the bundled `"stroke"` object within a props map.
/// Creates the stroke object if it doesn't exist yet.
fn stroke_set(props: &mut PropMap, key: &str, val: impl Into<super::PropValue>) {
    use super::PropValue;
    let val = val.into();
    if let Some(PropValue::Object(map)) = props.get_mut("stroke") {
        map.insert(key, val);
    } else {
        let mut map = PropMap::new();
        map.insert(key, val);
        props.insert("stroke", PropValue::Object(map));
    }
}

/// Build a reusable linear [`Gradient`] value.
///
/// The returned gradient can be stored in a variable and applied to
/// multiple shapes via `From<Gradient> for Background`, or passed to
/// `.fill(..)` directly.
///
/// ```ignore
/// let sky = linear_gradient((0.0, 0.0), (0.0, 200.0), [
///     (0.0, Color::hex("#1a1a2e")),
///     (1.0, Color::hex("#0f3460")),
/// ]);
///
/// rect(0.0, 0.0, 400.0, 200.0).fill(sky.clone());
/// rect(0.0, 200.0, 400.0, 400.0).fill(sky);
/// ```
pub fn linear_gradient<I, C>(start: (f32, f32), end: (f32, f32), stops: I) -> Gradient
where
    I: IntoIterator<Item = (f32, C)>,
    C: Into<Color>,
{
    Gradient::linear(
        start,
        end,
        stops
            .into_iter()
            .map(|(offset, c)| (offset, c.into()))
            .collect(),
    )
}

/// Build a gradient fill value in the wire format the renderer expects.
fn gradient_fill(x1: f32, y1: f32, x2: f32, y2: f32, stops: &[(f32, &str)]) -> super::PropValue {
    use super::PropValue;
    let stops_pv: Vec<PropValue> = stops
        .iter()
        .map(|(offset, color)| {
            PropValue::Array(vec![
                PropValue::F64(*offset as f64),
                PropValue::Str(color.to_string()),
            ])
        })
        .collect();
    let mut m = PropMap::new();
    m.insert("type", PropValue::Str("linear".into()));
    m.insert(
        "start",
        PropValue::Array(vec![PropValue::F64(x1 as f64), PropValue::F64(y1 as f64)]),
    );
    m.insert(
        "end",
        PropValue::Array(vec![PropValue::F64(x2 as f64), PropValue::F64(y2 as f64)]),
    );
    m.insert("stops", PropValue::Array(stops_pv));
    PropValue::Object(m)
}

// ---------------------------------------------------------------------------
// CanvasBuilder
// ---------------------------------------------------------------------------

/// Builder for a canvas widget (interactive drawing surface).
pub struct CanvasBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a canvas widget. The ID is required (canvas is interactive).
pub fn canvas(id: &str) -> CanvasBuilder {
    CanvasBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        children: vec![],
    }
}

impl CanvasBuilder {
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }
    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }
    pub fn background(mut self, c: impl Into<Animatable<Color>>) -> Self {
        super::set_prop(&mut self.props, "background", c.into().wire_encode());
        self
    }
    pub fn on_press(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_press", v);
        self
    }
    pub fn on_release(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_release", v);
        self
    }
    pub fn on_move(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_move", v);
        self
    }
    pub fn on_scroll(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_scroll", v);
        self
    }
    pub fn interactive(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "interactive", v);
        self
    }
    pub fn arrow_mode(mut self, mode: ArrowMode) -> Self {
        super::set_prop(&mut self.props, "arrow_mode", mode.wire_encode());
        self
    }
    pub fn alt(mut self, text: &str) -> Self {
        super::set_prop(&mut self.props, "alt", text);
        self
    }
    pub fn description(mut self, text: &str) -> Self {
        super::set_prop(&mut self.props, "description", text);
        self
    }
    pub fn role(mut self, role: &str) -> Self {
        super::set_prop(&mut self.props, "role", role);
        self
    }
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
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

impl From<CanvasBuilder> for View {
    fn from(b: CanvasBuilder) -> Self {
        super::view_node(b.id, "canvas", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// LayerBuilder
// ---------------------------------------------------------------------------

/// Builder for a named layer inside a canvas.
pub struct LayerBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a named layer inside a canvas.
pub fn layer(name: &str) -> LayerBuilder {
    let mut props = PropMap::new();
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
        super::view_node(b.id, "__layer__", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// GroupBuilder
// ---------------------------------------------------------------------------

/// Builder for a shape group inside a canvas.
pub struct GroupBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a shape group with the given ID.
pub fn group(id: &str) -> GroupBuilder {
    GroupBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        children: vec![],
    }
}

impl GroupBuilder {
    pub fn x(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "x", v);
        self
    }
    pub fn y(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "y", v);
        self
    }
    pub fn on_click(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_click", enabled);
        self
    }
    /// Emit enter/leave events when the cursor hovers over this group.
    pub fn on_hover(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "on_hover", enabled);
        self
    }
    pub fn draggable(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "draggable", enabled);
        self
    }
    /// Constrain drag direction.
    pub fn drag_axis(mut self, axis: DragAxis) -> Self {
        super::set_prop(&mut self.props, "drag_axis", axis.wire_encode());
        self
    }
    /// Make this group keyboard-focusable.
    pub fn focusable(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "focusable", enabled);
        self
    }
    /// Show the default focus ring when focused (default: true).
    pub fn show_focus_ring(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "show_focus_ring", enabled);
        self
    }
    /// Corner radius for the focus ring.
    pub fn focus_ring_radius(mut self, r: f32) -> Self {
        super::set_prop(&mut self.props, "focus_ring_radius", r);
        self
    }
    /// Style overrides applied when the cursor hovers over this group.
    pub fn hover_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "hover_style", style);
        self
    }
    /// Style overrides applied when the mouse is pressed on this group.
    pub fn pressed_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "pressed_style", style);
        self
    }
    /// Style overrides applied when this group has keyboard focus.
    pub fn focus_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "focus_style", style);
        self
    }
    pub fn cursor(mut self, c: CursorStyle) -> Self {
        super::set_prop(&mut self.props, "cursor", c.wire_encode());
        self
    }
    pub fn tooltip(mut self, text: &str) -> Self {
        super::set_prop(&mut self.props, "tooltip", text);
        self
    }
    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }
    pub fn a11y(mut self, a11y: &A11y) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.wire_encode());
        self
    }

    pub fn translate(mut self, x: f32, y: f32) -> Self {
        push_transform(&mut self.props, "translate", &[("x", x), ("y", y)]);
        self
    }

    /// Rotate the group by the given angle.
    ///
    /// Accepts degrees (bare number) or explicit `Angle::rad()`:
    /// ```ignore
    /// group.rotate(45.0)                    // 45 degrees
    /// group.rotate(Angle::rad(PI / 4.0))    // same, explicit radians
    /// ```
    pub fn rotate(mut self, angle: impl Into<crate::types::Angle>) -> Self {
        let a = angle.into();
        push_transform(&mut self.props, "rotate", &[("angle", a.degrees())]);
        self
    }

    pub fn scale_xy(mut self, x: f32, y: f32) -> Self {
        push_transform(&mut self.props, "scale", &[("x", x), ("y", y)]);
        self
    }

    pub fn scale_uniform(self, factor: f32) -> Self {
        self.scale_xy(factor, factor)
    }

    pub fn clip(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        let mut c = PropMap::new();
        c.insert("x", PropValue::F64(x as f64));
        c.insert("y", PropValue::F64(y as f64));
        c.insert("w", PropValue::F64(w as f64));
        c.insert("h", PropValue::F64(h as f64));
        super::set_prop(&mut self.props, "clip", PropValue::Object(c));
        self
    }

    pub fn hit_rect(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        let mut hr = PropMap::new();
        hr.insert("x", PropValue::F64(x as f64));
        hr.insert("y", PropValue::F64(y as f64));
        hr.insert("w", PropValue::F64(w as f64));
        hr.insert("h", PropValue::F64(h as f64));
        super::set_prop(&mut self.props, "hit_rect", PropValue::Object(hr));
        self
    }

    pub fn drag_bounds(mut self, min_x: f32, max_x: f32, min_y: f32, max_y: f32) -> Self {
        let mut db = PropMap::new();
        db.insert("min_x", PropValue::F64(min_x as f64));
        db.insert("max_x", PropValue::F64(max_x as f64));
        db.insert("min_y", PropValue::F64(min_y as f64));
        db.insert("max_y", PropValue::F64(max_y as f64));
        super::set_prop(&mut self.props, "drag_bounds", PropValue::Object(db));
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
        super::view_node(b.id, "group", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// RectBuilder
// ---------------------------------------------------------------------------

/// Builder for a rectangle shape.
pub struct RectBuilder {
    id: String,
    props: PropMap,
}

/// Create a rectangle shape at `(x, y)` with size `w` x `h`.
#[track_caller]
pub fn rect(x: f32, y: f32, w: f32, h: f32) -> RectBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "w", w);
    super::set_prop(&mut props, "h", h);
    RectBuilder {
        id: super::auto_id("rect"),
        props,
    }
}

impl RectBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn fill(mut self, bg: impl Into<Background>) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            super::background_to_value(&bg.into()),
        );
        self
    }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self {
        stroke_set(&mut self.props, "color", super::color_to_value(&c.into()));
        self
    }
    pub fn stroke_width(mut self, w: f32) -> Self {
        stroke_set(&mut self.props, "width", w);
        self
    }
    pub fn stroke_cap(mut self, cap: LineCap) -> Self {
        stroke_set(&mut self.props, "cap", cap.wire_encode());
        self
    }
    pub fn stroke_join(mut self, join: LineJoin) -> Self {
        stroke_set(&mut self.props, "join", join.wire_encode());
        self
    }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        let mut dash = PropMap::new();
        let segs: Vec<PropValue> = segments.iter().map(|s| PropValue::F64(*s as f64)).collect();
        dash.insert("segments", PropValue::Array(segs));
        dash.insert("offset", PropValue::F64(offset as f64));
        stroke_set(&mut self.props, "dash", PropValue::Object(dash));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
    /// Uniform corner radius.
    pub fn radius(mut self, r: f32) -> Self {
        super::set_prop(&mut self.props, "radius", r);
        self
    }
    /// Per-corner radius (top-left, top-right, bottom-right, bottom-left).
    pub fn radius_corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        let mut r = PropMap::new();
        r.insert("top_left", PropValue::F64(tl as f64));
        r.insert("top_right", PropValue::F64(tr as f64));
        r.insert("bottom_right", PropValue::F64(br as f64));
        r.insert("bottom_left", PropValue::F64(bl as f64));
        super::set_prop(&mut self.props, "radius", PropValue::Object(r));
        self
    }
    /// Fill rule: `"nonzero"` (default) or `"evenodd"`.
    pub fn fill_rule(mut self, rule: FillRule) -> Self {
        super::set_prop(&mut self.props, "fill_rule", rule.wire_encode());
        self
    }
    pub fn fill_gradient(
        mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: &[(f32, &str)],
    ) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            gradient_fill(x1, y1, x2, y2, stops),
        );
        self
    }
    /// Style overrides when parent group is hovered.
    pub fn hover_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "hover_style", style);
        self
    }
    /// Style overrides when parent group is pressed.
    pub fn pressed_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "pressed_style", style);
        self
    }
    /// Style overrides when parent group has keyboard focus.
    pub fn focus_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "focus_style", style);
        self
    }
}

impl From<RectBuilder> for View {
    fn from(b: RectBuilder) -> View {
        super::view_leaf(b.id, "rect", b.props)
    }
}

// ---------------------------------------------------------------------------
// CircleBuilder
// ---------------------------------------------------------------------------

/// Builder for a circle shape.
pub struct CircleBuilder {
    id: String,
    props: PropMap,
}

/// Create a circle shape centered at `(x, y)` with radius `r`.
#[track_caller]
pub fn circle(x: f32, y: f32, r: f32) -> CircleBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "r", r);
    CircleBuilder {
        id: super::auto_id("circle"),
        props,
    }
}

impl CircleBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn fill(mut self, bg: impl Into<Background>) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            super::background_to_value(&bg.into()),
        );
        self
    }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self {
        stroke_set(&mut self.props, "color", super::color_to_value(&c.into()));
        self
    }
    pub fn stroke_width(mut self, w: f32) -> Self {
        stroke_set(&mut self.props, "width", w);
        self
    }
    pub fn stroke_cap(mut self, cap: LineCap) -> Self {
        stroke_set(&mut self.props, "cap", cap.wire_encode());
        self
    }
    pub fn stroke_join(mut self, join: LineJoin) -> Self {
        stroke_set(&mut self.props, "join", join.wire_encode());
        self
    }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        let mut dash = PropMap::new();
        let segs: Vec<PropValue> = segments.iter().map(|s| PropValue::F64(*s as f64)).collect();
        dash.insert("segments", PropValue::Array(segs));
        dash.insert("offset", PropValue::F64(offset as f64));
        stroke_set(&mut self.props, "dash", PropValue::Object(dash));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
    /// Fill rule: `"nonzero"` (default) or `"evenodd"`.
    pub fn fill_rule(mut self, rule: FillRule) -> Self {
        super::set_prop(&mut self.props, "fill_rule", rule.wire_encode());
        self
    }
    pub fn fill_gradient(
        mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: &[(f32, &str)],
    ) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            gradient_fill(x1, y1, x2, y2, stops),
        );
        self
    }
    pub fn hover_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "hover_style", style);
        self
    }
    pub fn pressed_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "pressed_style", style);
        self
    }
    pub fn focus_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "focus_style", style);
        self
    }
}

impl From<CircleBuilder> for View {
    fn from(b: CircleBuilder) -> View {
        super::view_leaf(b.id, "circle", b.props)
    }
}

// ---------------------------------------------------------------------------
// LineBuilder
// ---------------------------------------------------------------------------

/// Builder for a line shape.
pub struct LineBuilder {
    id: String,
    props: PropMap,
}

/// Create a line from `(x1, y1)` to `(x2, y2)`.
#[track_caller]
pub fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> LineBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "x1", x1);
    super::set_prop(&mut props, "y1", y1);
    super::set_prop(&mut props, "x2", x2);
    super::set_prop(&mut props, "y2", y2);
    LineBuilder {
        id: super::auto_id("line"),
        props,
    }
}

impl LineBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self {
        stroke_set(&mut self.props, "color", super::color_to_value(&c.into()));
        self
    }
    pub fn stroke_width(mut self, w: f32) -> Self {
        stroke_set(&mut self.props, "width", w);
        self
    }
    pub fn stroke_cap(mut self, cap: LineCap) -> Self {
        stroke_set(&mut self.props, "cap", cap.wire_encode());
        self
    }
    pub fn stroke_join(mut self, join: LineJoin) -> Self {
        stroke_set(&mut self.props, "join", join.wire_encode());
        self
    }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        let mut dash = PropMap::new();
        let segs: Vec<PropValue> = segments.iter().map(|s| PropValue::F64(*s as f64)).collect();
        dash.insert("segments", PropValue::Array(segs));
        dash.insert("offset", PropValue::F64(offset as f64));
        stroke_set(&mut self.props, "dash", PropValue::Object(dash));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
    pub fn hover_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "hover_style", style);
        self
    }
    pub fn pressed_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "pressed_style", style);
        self
    }
    pub fn focus_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "focus_style", style);
        self
    }
}

impl From<LineBuilder> for View {
    fn from(b: LineBuilder) -> View {
        super::view_leaf(b.id, "line", b.props)
    }
}

// ---------------------------------------------------------------------------
// PathBuilder
// ---------------------------------------------------------------------------

/// Builder for an SVG path shape.
pub struct PathBuilder {
    id: String,
    props: PropMap,
}

/// Create a canvas path shape from a sequence of typed
/// [`PathCommand`]s.
///
/// ```ignore
/// use plushie::prelude::*;
///
/// path(vec![
///     move_to(0.0, 0.0),
///     line_to(100.0, 0.0),
///     line_to(50.0, 80.0),
///     close(),
/// ]).fill(Color::blue())
/// ```
///
/// Use [`path_raw`] if you have an existing SVG path data string you
/// want to pass verbatim.
#[track_caller]
pub fn path(commands: impl IntoIterator<Item = PathCommand>) -> PathBuilder {
    let mut props = PropMap::new();
    let encoded: Vec<PropValue> = commands.into_iter().map(|cmd| cmd.wire_encode()).collect();
    super::set_prop(&mut props, "commands", PropValue::Array(encoded));
    PathBuilder {
        id: super::auto_id("path"),
        props,
    }
}

/// Create a canvas path shape from pre-encoded path commands.
///
/// Escape hatch for cases where the caller has already encoded a
/// `Vec<PropValue>` of commands (e.g. generated programmatically).
/// Prefer [`path`] with typed [`PathCommand`]s for new code.
#[track_caller]
pub fn path_raw(encoded_commands: Vec<PropValue>) -> PathBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "commands", PropValue::Array(encoded_commands));
    PathBuilder {
        id: super::auto_id("path"),
        props,
    }
}

// -- Path command builders --

/// Move to `(x, y)` without drawing.
pub fn move_to(x: f32, y: f32) -> PathCommand {
    PathCommand::MoveTo { x, y }
}

/// Draw a straight line to `(x, y)`.
pub fn line_to(x: f32, y: f32) -> PathCommand {
    PathCommand::LineTo { x, y }
}

/// Draw a cubic bezier curve to `(x, y)` with control points
/// `(cp1x, cp1y)` and `(cp2x, cp2y)`.
pub fn bezier_to(cp1x: f32, cp1y: f32, cp2x: f32, cp2y: f32, x: f32, y: f32) -> PathCommand {
    PathCommand::BezierTo {
        cp1x,
        cp1y,
        cp2x,
        cp2y,
        x,
        y,
    }
}

/// Draw a quadratic bezier curve to `(x, y)` with control point
/// `(cpx, cpy)`.
pub fn quadratic_to(cpx: f32, cpy: f32, x: f32, y: f32) -> PathCommand {
    PathCommand::QuadraticTo { cpx, cpy, x, y }
}

/// Draw a circular arc centered at `(cx, cy)` with the given radius,
/// sweeping from `start` to `end`.
pub fn arc(cx: f32, cy: f32, radius: f32, start: Angle, end: Angle) -> PathCommand {
    PathCommand::Arc {
        cx,
        cy,
        radius,
        start_angle: start,
        end_angle: end,
    }
}

/// Draw an arc with the given tangent segment and radius.
pub fn arc_to(x1: f32, y1: f32, x2: f32, y2: f32, radius: f32) -> PathCommand {
    PathCommand::ArcTo {
        x1,
        y1,
        x2,
        y2,
        radius,
    }
}

/// Draw an elliptical arc centered at `(cx, cy)` with radii `(rx, ry)`.
pub fn ellipse(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    rotation: Angle,
    start: Angle,
    end: Angle,
) -> PathCommand {
    PathCommand::Ellipse {
        cx,
        cy,
        rx,
        ry,
        rotation,
        start_angle: start,
        end_angle: end,
    }
}

/// Draw a rounded rectangle. The `radius` accepts an `f32` for
/// uniform corners or a [`Radius::PerCorner`] for individual values.
pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, radius: impl Into<Radius>) -> PathCommand {
    PathCommand::RoundedRect {
        x,
        y,
        w,
        h,
        radius: radius.into(),
    }
}

/// Close the current sub-path.
pub fn close() -> PathCommand {
    PathCommand::Close
}

impl PathBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn fill(mut self, bg: impl Into<Background>) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            super::background_to_value(&bg.into()),
        );
        self
    }
    pub fn stroke(mut self, c: impl Into<Color>) -> Self {
        stroke_set(&mut self.props, "color", super::color_to_value(&c.into()));
        self
    }
    pub fn stroke_width(mut self, w: f32) -> Self {
        stroke_set(&mut self.props, "width", w);
        self
    }
    pub fn stroke_cap(mut self, cap: LineCap) -> Self {
        stroke_set(&mut self.props, "cap", cap.wire_encode());
        self
    }
    pub fn stroke_join(mut self, join: LineJoin) -> Self {
        stroke_set(&mut self.props, "join", join.wire_encode());
        self
    }
    pub fn stroke_dash(mut self, segments: &[f32], offset: f32) -> Self {
        let mut dash = PropMap::new();
        let segs: Vec<PropValue> = segments.iter().map(|s| PropValue::F64(*s as f64)).collect();
        dash.insert("segments", PropValue::Array(segs));
        dash.insert("offset", PropValue::F64(offset as f64));
        stroke_set(&mut self.props, "dash", PropValue::Object(dash));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
    pub fn fill_rule(mut self, rule: FillRule) -> Self {
        super::set_prop(&mut self.props, "fill_rule", rule.wire_encode());
        self
    }
    pub fn fill_gradient(
        mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: &[(f32, &str)],
    ) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            gradient_fill(x1, y1, x2, y2, stops),
        );
        self
    }
    pub fn hover_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "hover_style", style);
        self
    }
    pub fn pressed_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "pressed_style", style);
        self
    }
    pub fn focus_style(mut self, style: PropValue) -> Self {
        super::set_prop(&mut self.props, "focus_style", style);
        self
    }
}

impl From<PathBuilder> for View {
    fn from(b: PathBuilder) -> View {
        super::view_leaf(b.id, "path", b.props)
    }
}

// ---------------------------------------------------------------------------
// CanvasTextBuilder
// ---------------------------------------------------------------------------

/// Builder for text rendered inside a canvas.
pub struct CanvasTextBuilder {
    id: String,
    props: PropMap,
}

/// Create a text element inside a canvas at `(x, y)`.
#[track_caller]
pub fn canvas_text(x: f32, y: f32, content: &str) -> CanvasTextBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "content", content);
    CanvasTextBuilder {
        id: super::auto_id("text"),
        props,
    }
}

impl CanvasTextBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn size(mut self, s: f32) -> Self {
        super::set_prop(&mut self.props, "size", s);
        self
    }
    /// Fill color for the text.
    pub fn fill(mut self, bg: impl Into<Background>) -> Self {
        super::set_prop(
            &mut self.props,
            "fill",
            super::background_to_value(&bg.into()),
        );
        self
    }
    pub fn font(mut self, f: Font) -> Self {
        super::set_prop(&mut self.props, "font", f.wire_encode());
        self
    }
    /// Horizontal text alignment.
    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }
    /// Vertical text alignment.
    pub fn align_y(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_y", super::valign_to_value(a));
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
}

impl From<CanvasTextBuilder> for View {
    fn from(b: CanvasTextBuilder) -> View {
        super::view_leaf(b.id, "text", b.props)
    }
}

// ---------------------------------------------------------------------------
// CanvasImageBuilder
// ---------------------------------------------------------------------------

/// Builder for an image rendered inside a canvas.
pub struct CanvasImageBuilder {
    id: String,
    props: PropMap,
}

/// Create an image element inside a canvas at `(x, y)`.
#[track_caller]
pub fn canvas_image(x: f32, y: f32, source: &str) -> CanvasImageBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "source", source);
    CanvasImageBuilder {
        id: super::auto_id("image"),
        props,
    }
}

impl CanvasImageBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        super::set_prop(&mut self.props, "w", w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        super::set_prop(&mut self.props, "h", h);
        self
    }
    /// Rotation angle. Accepts degrees (bare number) or explicit radians.
    pub fn rotation(mut self, angle: impl Into<crate::types::Angle>) -> Self {
        let a = angle.into();
        super::set_prop(&mut self.props, "rotation", a.degrees());
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
}

impl From<CanvasImageBuilder> for View {
    fn from(b: CanvasImageBuilder) -> View {
        super::view_leaf(b.id, "image", b.props)
    }
}

// ---------------------------------------------------------------------------
// CanvasSvgBuilder
// ---------------------------------------------------------------------------

/// Builder for an SVG element rendered inside a canvas.
pub struct CanvasSvgBuilder {
    id: String,
    props: PropMap,
}

/// Create an SVG element inside a canvas at `(x, y)`.
#[track_caller]
pub fn canvas_svg(x: f32, y: f32, source: &str) -> CanvasSvgBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "x", x);
    super::set_prop(&mut props, "y", y);
    super::set_prop(&mut props, "source", source);
    CanvasSvgBuilder {
        id: super::auto_id("svg"),
        props,
    }
}

impl CanvasSvgBuilder {
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        super::set_prop(&mut self.props, "w", w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        super::set_prop(&mut self.props, "h", h);
        self
    }
    pub fn opacity(mut self, o: f32) -> Self {
        super::set_prop(&mut self.props, "opacity", o);
        self
    }
}

impl From<CanvasSvgBuilder> for View {
    fn from(b: CanvasSvgBuilder) -> View {
        super::view_leaf(b.id, "svg", b.props)
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
