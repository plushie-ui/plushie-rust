//! Interactive widget builders.
//!
//! Widgets that respond to user input. All interactive widgets
//! require an explicit ID since they generate events that must
//! be matchable in `update/2`.
//!
//! ```ignore
//! use plushie::prelude::*;
//!
//! let view = button("save", "Save")
//!     .style(Style::primary())
//!     .width(Length::Fixed(120.0));
//!
//! let area = pointer_area("canvas")
//!     .child(my_canvas_view);
//! ```

use crate::View;
use crate::types::*;
use super::PropMap;

// ---------------------------------------------------------------------------
// ButtonBuilder
// ---------------------------------------------------------------------------

/// Builder for a clickable button.
pub struct ButtonBuilder {
    id: String,
    props: PropMap,
}

/// Create a button with the given ID and label text.
pub fn button(id: &str, label: &str) -> ButtonBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "label", label);
    ButtonBuilder {
        id: id.to_string(),
        props,
    }
}

impl ButtonBuilder {
    pub fn style(mut self, s: impl Into<Style>) -> Self {
        let s = s.into();
        super::set_prop(&mut self.props, "style", super::style_to_value(&s));
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "disabled", v);
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

    pub fn padding(mut self, p: impl Into<Padding>) -> Self {
        super::set_prop(&mut self.props, "padding", super::padding_to_value(p.into()));
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
}

impl From<ButtonBuilder> for View {
    fn from(b: ButtonBuilder) -> Self {
        super::view_leaf(b.id, "button", b.props)
    }
}

// ---------------------------------------------------------------------------
// PointerAreaBuilder
// ---------------------------------------------------------------------------

/// Builder for a pointer event capture region.
///
/// Wraps a single child and reports pointer events (press, release,
/// enter, exit, move, scroll) on the child's bounds.
pub struct PointerAreaBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a pointer area with the given ID.
pub fn pointer_area(id: &str) -> PointerAreaBuilder {
    PointerAreaBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        child: None,
    }
}

impl PointerAreaBuilder {
    pub fn on_press(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_press", v);
        self
    }

    pub fn on_release(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_release", v);
        self
    }

    pub fn on_enter(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_enter", v);
        self
    }

    pub fn on_exit(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_exit", v);
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

    pub fn on_middle_press(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_middle_press", v);
        self
    }

    pub fn on_right_press(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_right_press", v);
        self
    }

    /// Enable right mouse button release events.
    pub fn on_right_release(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_right_release", v);
        self
    }

    /// Enable middle mouse button release events.
    pub fn on_middle_release(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_middle_release", v);
        self
    }

    /// Enable double-click events.
    pub fn on_double_click(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_double_click", v);
        self
    }

    /// Mouse cursor to show on hover.
    pub fn cursor(mut self, cursor: CursorStyle) -> Self {
        super::set_prop(&mut self.props, "cursor", cursor.wire_encode());
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

    /// Set the single child of this pointer area.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<PointerAreaBuilder> for View {
    fn from(b: PointerAreaBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "pointer_area", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// SensorBuilder
// ---------------------------------------------------------------------------

/// Builder for a layout sensor.
///
/// Wraps a single child and reports layout events (size changes,
/// position) without capturing pointer input.
pub struct SensorBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a sensor with the given ID.
pub fn sensor(id: &str) -> SensorBuilder {
    SensorBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        child: None,
    }
}

impl SensorBuilder {
    /// Delay in milliseconds before emitting events.
    pub fn delay(mut self, ms: u32) -> Self {
        super::set_prop(&mut self.props, "delay", ms);
        self
    }

    /// Distance in pixels to anticipate visibility.
    pub fn anticipate(mut self, pixels: f32) -> Self {
        super::set_prop(&mut self.props, "anticipate", pixels);
        self
    }

    /// Enable resize events.
    pub fn on_resize(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "on_resize", v);
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

    /// Set the single child of this sensor.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<SensorBuilder> for View {
    fn from(b: SensorBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "sensor", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// TooltipBuilder
// ---------------------------------------------------------------------------

/// Builder for a tooltip wrapper.
///
/// Wraps a single child and shows a tooltip on hover.
pub struct TooltipBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a tooltip with the given ID and tip text.
pub fn tooltip(id: &str, tip: &str) -> TooltipBuilder {
    let mut props = PropMap::new();
    super::set_prop(&mut props, "tip", tip);
    TooltipBuilder {
        id: id.to_string(),
        props,
        child: None,
    }
}

impl TooltipBuilder {
    pub fn position(mut self, pos: Position) -> Self {
        super::set_prop(&mut self.props, "position", pos.wire_encode());
        self
    }

    pub fn gap(mut self, v: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "gap", v.into().wire_encode());
        self
    }

    /// Tooltip padding in pixels.
    pub fn padding(mut self, v: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "padding", v.into().wire_encode());
        self
    }

    /// Keep tooltip within the viewport bounds.
    pub fn snap_within_viewport(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "snap_within_viewport", v);
        self
    }

    /// Delay in milliseconds before showing the tooltip.
    pub fn delay(mut self, ms: u32) -> Self {
        super::set_prop(&mut self.props, "delay", ms);
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

    /// Set the single child of this tooltip.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<TooltipBuilder> for View {
    fn from(b: TooltipBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "tooltip", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// ThemerBuilder
// ---------------------------------------------------------------------------

/// Builder for a theme override container.
///
/// Applies a different theme to its child subtree.
pub struct ThemerBuilder {
    id: String,
    props: PropMap,
    child: Option<View>,
}

/// Create a themer with the given ID.
pub fn themer(id: &str) -> ThemerBuilder {
    ThemerBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        child: None,
    }
}

impl ThemerBuilder {
    pub fn theme(mut self, theme: impl Into<Theme>) -> Self {
        let theme: Theme = theme.into();
        super::set_prop(&mut self.props, "theme", theme.wire_encode());
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

    /// Set the single child of this themer.
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.child = Some(child.into());
        self
    }
}

impl From<ThemerBuilder> for View {
    fn from(b: ThemerBuilder) -> Self {
        let children = b.child.into_iter().collect();
        super::view_node(b.id, "themer", b.props, children)
    }
}

// ---------------------------------------------------------------------------
// OverlayBuilder
// ---------------------------------------------------------------------------

/// Builder for a popup overlay container.
///
/// Renders its children as an overlay positioned relative to
/// the parent widget.
pub struct OverlayBuilder {
    id: String,
    props: PropMap,
    children: Vec<View>,
}

/// Create an overlay with the given ID.
pub fn overlay(id: &str) -> OverlayBuilder {
    OverlayBuilder {
        id: id.to_string(),
        props: PropMap::new(),
        children: vec![],
    }
}

impl OverlayBuilder {
    pub fn position(mut self, pos: Position) -> Self {
        super::set_prop(&mut self.props, "position", pos.wire_encode());
        self
    }

    pub fn align(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align", super::cross_align_to_value(a));
        self
    }

    pub fn flip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "flip", v);
        self
    }

    pub fn gap(mut self, v: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "gap", v.into().wire_encode());
        self
    }

    /// Horizontal offset in pixels after positioning.
    pub fn offset_x(mut self, v: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "offset_x", v.into().wire_encode());
        self
    }

    /// Vertical offset in pixels after positioning.
    pub fn offset_y(mut self, v: impl Into<Animatable<f32>>) -> Self {
        super::set_prop(&mut self.props, "offset_y", v.into().wire_encode());
        self
    }

    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
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

impl From<OverlayBuilder> for View {
    fn from(b: OverlayBuilder) -> Self {
        super::view_node(b.id, "overlay", b.props, b.children)
    }
}
