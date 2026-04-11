//! Layout widget builders.
//!
//! Container widgets that control spatial arrangement of children.
//! Each builder produces a [`View`] via the `From` impl.
//!
//! ```ignore
//! use plushie::prelude::*;
//!
//! let view = window("main").title("Counter").child(
//!     column().spacing(8).padding(16).children([
//!         text("Hello!"),
//!         row().spacing(4).children([
//!             button("a", "A"),
//!             button("b", "B"),
//!         ]),
//!     ])
//! );
//! ```

use crate::View;
use crate::types::*;
use super::{PropMap, PropValue};

// ---------------------------------------------------------------------------
// WindowBuilder
// ---------------------------------------------------------------------------

/// Builder for a top-level window node.
///
/// Windows always require an explicit ID since they are the root
/// scope for all contained widgets.
pub struct WindowBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a window. The ID is required (windows are always scoped).
pub fn window(id: &str) -> WindowBuilder {
    WindowBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        children: vec![],
    }
}

impl WindowBuilder {
    pub fn title(mut self, title: &str) -> Self {
        super::set_prop(&mut self.props, "title", title);
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn position(mut self, x: f32, y: f32) -> Self {
        super::set_prop(&mut self.props, "position", PropValue::Array(vec![PropValue::F64(x as f64), PropValue::F64(y as f64)]));
        self
    }

    pub fn min_size(mut self, w: f32, h: f32) -> Self {
        super::set_prop(&mut self.props, "min_size", PropValue::Array(vec![PropValue::F64(w as f64), PropValue::F64(h as f64)]));
        self
    }

    pub fn max_size(mut self, w: f32, h: f32) -> Self {
        super::set_prop(&mut self.props, "max_size", PropValue::Array(vec![PropValue::F64(w as f64), PropValue::F64(h as f64)]));
        self
    }

    pub fn theme(mut self, theme: &str) -> Self {
        super::set_prop(&mut self.props, "theme", theme);
        self
    }

    pub fn scale_factor(mut self, factor: f64) -> Self {
        super::set_prop(&mut self.props, "scale_factor", factor);
        self
    }

    pub fn maximized(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "maximized", v);
        self
    }

    pub fn fullscreen(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "fullscreen", v);
        self
    }

    pub fn visible(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "visible", v);
        self
    }

    pub fn resizable(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "resizable", v);
        self
    }

    pub fn decorations(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "decorations", v);
        self
    }

    pub fn transparent(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "transparent", v);
        self
    }

    /// Whether the window close button is shown.
    pub fn closeable(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "closeable", v);
        self
    }

    /// Whether the window can be minimized.
    pub fn minimizable(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "minimizable", v);
        self
    }

    /// Blur the window background (platform-dependent).
    pub fn blur(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "blur", v);
        self
    }

    /// Window stacking level: `"normal"`, `"always_on_top"`, or
    /// `"always_on_bottom"`.
    pub fn level(mut self, level: &str) -> Self {
        super::set_prop(&mut self.props, "level", level);
        self
    }

    /// Whether closing the window exits the application.
    pub fn exit_on_close_request(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "exit_on_close_request", v);
        self
    }

    /// Initial window size as `(width, height)` in pixels.
    pub fn size(mut self, w: f32, h: f32) -> Self {
        super::set_prop(&mut self.props, "size", PropValue::Array(vec![PropValue::F64(w as f64), PropValue::F64(h as f64)]));
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

impl From<WindowBuilder> for View {
    fn from(b: WindowBuilder) -> Self {
        super::view_node(b.id, "window", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// ColumnBuilder
// ---------------------------------------------------------------------------

/// Builder for a vertical layout container.
pub struct ColumnBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a column with an auto-generated ID.
#[track_caller]
pub fn column() -> ColumnBuilder {
    ColumnBuilder {
        id: super::auto_id("column"),
        props: PropMap::new(),
        children: vec![],
    }
}

impl ColumnBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn spacing(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", v);
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn max_width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "max_width", super::length_to_value(w.into()));
        self
    }

    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }

    pub fn clip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "clip", v);
        self
    }

    pub fn wrap(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "wrap", enabled);
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

    /// Animate a property with a timed transition.
    ///
    /// The transition descriptor replaces the prop value. The renderer
    /// interpolates from the current value to the transition's `to`.
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        super::set_prop(&mut self.props, prop, t.wire_encode());
        self
    }

    /// Animate a property with spring physics.
    pub fn spring(mut self, prop: &str, s: crate::animation::Spring) -> Self {
        super::set_prop(&mut self.props, prop, s.wire_encode());
        self
    }

    /// Animate a property with a sequence of steps.
    pub fn sequence(mut self, prop: &str, seq: crate::animation::Sequence) -> Self {
        super::set_prop(&mut self.props, prop, seq.wire_encode());
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

impl From<ColumnBuilder> for View {
    fn from(b: ColumnBuilder) -> Self {
        super::view_node(b.id, "column", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// RowBuilder
// ---------------------------------------------------------------------------

/// Builder for a horizontal layout container.
pub struct RowBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a row with an auto-generated ID.
#[track_caller]
pub fn row() -> RowBuilder {
    RowBuilder {
        id: super::auto_id("row"),
        props: PropMap::new(),
        children: vec![],
    }
}

impl RowBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn spacing(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", v);
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn max_width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "max_width", super::length_to_value(w.into()));
        self
    }

    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }

    pub fn align_y(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_y", super::valign_to_value(a));
        self
    }

    pub fn clip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "clip", v);
        self
    }

    pub fn wrap(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "wrap", enabled);
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

    /// Animate a property with a timed transition.
    ///
    /// The transition descriptor replaces the prop value. The renderer
    /// interpolates from the current value to the transition's `to`.
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        super::set_prop(&mut self.props, prop, t.wire_encode());
        self
    }

    /// Animate a property with spring physics.
    pub fn spring(mut self, prop: &str, s: crate::animation::Spring) -> Self {
        super::set_prop(&mut self.props, prop, s.wire_encode());
        self
    }

    /// Animate a property with a sequence of steps.
    pub fn sequence(mut self, prop: &str, seq: crate::animation::Sequence) -> Self {
        super::set_prop(&mut self.props, prop, seq.wire_encode());
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

impl From<RowBuilder> for View {
    fn from(b: RowBuilder) -> Self {
        super::view_node(b.id, "row", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// ContainerBuilder
// ---------------------------------------------------------------------------

/// Builder for a single-child container with alignment and sizing.
pub struct ContainerBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a container with an auto-generated ID.
#[track_caller]
pub fn container() -> ContainerBuilder {
    ContainerBuilder {
        id: super::auto_id("container"),
        props: PropMap::new(),
        child: None,
    }
}

impl ContainerBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn max_width(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "max_width", v);
        self
    }

    pub fn max_height(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "max_height", v);
        self
    }

    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }

    pub fn align_y(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_y", super::valign_to_value(a));
        self
    }

    pub fn center_x(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "center_x", v);
        self
    }

    pub fn center_y(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "center_y", v);
        self
    }

    pub fn clip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "clip", v);
        self
    }

    pub fn background(mut self, c: impl Into<Color>) -> Self {
        super::set_prop(&mut self.props, "background", super::color_to_value(&c.into()));
        self
    }

    pub fn color(mut self, c: impl Into<Color>) -> Self {
        super::set_prop(&mut self.props, "color", super::color_to_value(&c.into()));
        self
    }

    pub fn border(mut self, b: Border) -> Self {
        super::set_prop(&mut self.props, "border", b.wire_encode());
        self
    }

    pub fn shadow(mut self, s: Shadow) -> Self {
        super::set_prop(&mut self.props, "shadow", s.wire_encode());
        self
    }

    pub fn center(mut self, enabled: bool) -> Self {
        super::set_prop(&mut self.props, "center", enabled);
        self
    }

    pub fn style(mut self, s: impl Into<Style>) -> Self {
        let s = s.into();
        super::set_prop(&mut self.props, "style", super::style_to_value(&s));
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

    /// Animate a property with a timed transition.
    ///
    /// The transition descriptor replaces the prop value. The renderer
    /// interpolates from the current value to the transition's `to`.
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        super::set_prop(&mut self.props, prop, t.wire_encode());
        self
    }

    /// Animate a property with spring physics.
    pub fn spring(mut self, prop: &str, s: crate::animation::Spring) -> Self {
        super::set_prop(&mut self.props, prop, s.wire_encode());
        self
    }

    /// Animate a property with a sequence of steps.
    pub fn sequence(mut self, prop: &str, seq: crate::animation::Sequence) -> Self {
        super::set_prop(&mut self.props, prop, seq.wire_encode());
        self
    }

    /// Set the single child of this container.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ContainerBuilder> for View {
    fn from(b: ContainerBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "container", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// StackBuilder
// ---------------------------------------------------------------------------

/// Builder for a z-axis stacking container.
///
/// Children are layered on top of each other, with later children
/// rendered above earlier ones.
pub struct StackBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a stack with an auto-generated ID.
#[track_caller]
pub fn stack() -> StackBuilder {
    StackBuilder {
        id: super::auto_id("stack"),
        props: PropMap::new(),
        children: vec![],
    }
}

impl StackBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn clip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "clip", v);
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

impl From<StackBuilder> for View {
    fn from(b: StackBuilder) -> Self {
        super::view_node(b.id, "stack", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// GridBuilder
// ---------------------------------------------------------------------------

/// Builder for a grid layout container.
pub struct GridBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a grid with an auto-generated ID.
#[track_caller]
pub fn grid() -> GridBuilder {
    GridBuilder {
        id: super::auto_id("grid"),
        props: PropMap::new(),
        children: vec![],
    }
}

impl GridBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// Number of columns in the grid.
    pub fn columns(mut self, n: u32) -> Self {
        super::set_prop(&mut self.props, "columns", n);
        self
    }

    pub fn spacing(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", v);
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    /// Enable fluid grid mode. Columns auto-wrap; `max_cell_width`
    /// sets the maximum width of each cell in pixels.
    pub fn fluid(mut self, max_cell_width: f32) -> Self {
        super::set_prop(&mut self.props, "fluid", max_cell_width);
        self
    }

    /// Width of each column.
    pub fn column_width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "column_width", super::length_to_value(w.into()));
        self
    }

    /// Height of each row.
    pub fn row_height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "row_height", super::length_to_value(h.into()));
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

impl From<GridBuilder> for View {
    fn from(b: GridBuilder) -> Self {
        super::view_node(b.id, "grid", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// PinBuilder
// ---------------------------------------------------------------------------

/// Builder for an absolutely positioned single-child container.
pub struct PinBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a pin with an auto-generated ID.
#[track_caller]
pub fn pin() -> PinBuilder {
    PinBuilder {
        id: super::auto_id("pin"),
        props: PropMap::new(),
        child: None,
    }
}

impl PinBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn x(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "x", v);
        self
    }

    pub fn y(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "y", v);
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
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

    /// Set the single child of this pin.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<PinBuilder> for View {
    fn from(b: PinBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "pin", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// KeyedColumnBuilder
// ---------------------------------------------------------------------------

/// Builder for a keyed vertical layout container.
///
/// Same as column, but children are diffed by key rather than
/// position, producing minimal move operations on reorder.
pub struct KeyedColumnBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a keyed column with an auto-generated ID.
#[track_caller]
pub fn keyed_column() -> KeyedColumnBuilder {
    KeyedColumnBuilder {
        id: super::auto_id("keyed_column"),
        props: PropMap::new(),
        children: vec![],
    }
}

impl KeyedColumnBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn spacing(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", v);
        self
    }

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn align_x(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align_x", super::halign_to_value(a));
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        super::set_prop(&mut self.props, "max_width", w);
        self
    }

    pub fn clip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "clip", v);
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

impl From<KeyedColumnBuilder> for View {
    fn from(b: KeyedColumnBuilder) -> Self {
        super::view_node(b.id, "keyed_column", b.props, b.children)
    }
}

// ---------------------------------------------------------------------------
// FloatingBuilder
// ---------------------------------------------------------------------------

/// Builder for a floating overlay container.
///
/// The constructor is named `floating()` to avoid the Rust `float` keyword.
pub struct FloatingBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a floating container with an auto-generated ID.
///
/// Named `floating` to avoid the Rust `float` keyword.
#[track_caller]
pub fn floating() -> FloatingBuilder {
    FloatingBuilder {
        id: super::auto_id("float"),
        props: PropMap::new(),
        child: None,
    }
}

impl FloatingBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// Horizontal translation in pixels.
    pub fn translate_x(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "translate_x", v);
        self
    }

    /// Vertical translation in pixels.
    pub fn translate_y(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "translate_y", v);
        self
    }

    pub fn scale(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "scale", v);
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
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

    /// Set the single child of this floating container.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<FloatingBuilder> for View {
    fn from(b: FloatingBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "float", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// ResponsiveBuilder
// ---------------------------------------------------------------------------

/// Builder for a responsive container that adapts to available width.
pub struct ResponsiveBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a responsive container with an auto-generated ID.
#[track_caller]
pub fn responsive() -> ResponsiveBuilder {
    ResponsiveBuilder {
        id: super::auto_id("responsive"),
        props: PropMap::new(),
        child: None,
    }
}

impl ResponsiveBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
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

    /// Set the single child of this responsive container.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ResponsiveBuilder> for View {
    fn from(b: ResponsiveBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "responsive", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// ScrollableBuilder
// ---------------------------------------------------------------------------

/// Builder for a scrollable container.
pub struct ScrollableBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a scrollable container with an auto-generated ID.
#[track_caller]
pub fn scrollable() -> ScrollableBuilder {
    ScrollableBuilder {
        id: super::auto_id("scrollable"),
        props: PropMap::new(),
        child: None,
    }
}

impl ScrollableBuilder {
    /// Set an explicit ID (creates a scope for child widget IDs).
    pub fn id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn spacing(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", v);
        self
    }

    pub fn direction(mut self, dir: &str) -> Self {
        super::set_prop(&mut self.props, "direction", dir);
        self
    }

    pub fn scrollbar_width(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "scrollbar_width", v);
        self
    }

    pub fn scrollbar_margin(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "scrollbar_margin", v);
        self
    }

    pub fn scroller_width(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "scroller_width", v);
        self
    }

    pub fn anchor(mut self, a: &str) -> Self {
        super::set_prop(&mut self.props, "anchor", a);
        self
    }

    /// Emit scroll viewport events.
    pub fn on_scroll(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_scroll", v);
        self
    }

    /// Auto-scroll to show new content at the anchor end.
    pub fn auto_scroll(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "auto_scroll", v);
        self
    }

    /// Scrollbar track color.
    pub fn scrollbar_color(mut self, c: impl Into<Color>) -> Self {
        super::set_prop(&mut self.props, "scrollbar_color", super::color_to_value(&c.into()));
        self
    }

    /// Scroller handle color.
    pub fn scroller_color(mut self, c: impl Into<Color>) -> Self {
        super::set_prop(&mut self.props, "scroller_color", super::color_to_value(&c.into()));
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

    /// Set the single child of this scrollable container.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ScrollableBuilder> for View {
    fn from(b: ScrollableBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "scrollable", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// PaneGridBuilder
// ---------------------------------------------------------------------------

/// Builder for a pane grid layout.
///
/// Always requires an explicit ID since pane grids are interactive
/// (panes can be resized and rearranged).
pub struct PaneGridBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create a pane grid. The ID is required (pane grids are interactive).
pub fn pane_grid(id: &str) -> PaneGridBuilder {
    PaneGridBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        children: vec![],
    }
}

impl PaneGridBuilder {
    pub fn spacing(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "spacing", v);
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

    pub fn split_axis(mut self, axis: &str) -> Self {
        super::set_prop(&mut self.props, "split_axis", axis);
        self
    }

    /// List of pane identifiers in this grid.
    pub fn panes(mut self, pane_ids: &[&str]) -> Self {
        let ids: Vec<PropValue> = pane_ids.iter().map(|s| PropValue::Str(s.to_string())).collect();
        super::set_prop(&mut self.props, "panes", PropValue::Array(ids));
        self
    }

    /// Minimum pane size in pixels.
    pub fn min_size(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "min_size", v);
        self
    }

    /// Color for the split divider between panes.
    pub fn divider_color(mut self, c: impl Into<Color>) -> Self {
        super::set_prop(&mut self.props, "divider_color", super::color_to_value(&c.into()));
        self
    }

    /// Divider thickness in pixels.
    pub fn divider_width(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "divider_width", v);
        self
    }

    /// Grabbable area around dividers in pixels.
    pub fn leeway(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "leeway", v);
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

impl From<PaneGridBuilder> for View {
    fn from(b: PaneGridBuilder) -> Self {
        super::view_node(b.id, "pane_grid", b.props, b.children)
    }
}
