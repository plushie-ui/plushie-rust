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
use serde_json::{Map, Value};

// ---------------------------------------------------------------------------
// ButtonBuilder
// ---------------------------------------------------------------------------

/// Builder for a clickable button.
pub struct ButtonBuilder {
    id: String,
    props: Map<String, Value>,
}

/// Create a button with the given ID and label text.
pub fn button(id: &str, label: &str) -> ButtonBuilder {
    let mut props = Map::new();
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

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
        self
    }

    /// Attach a transition animation to a property.
    pub fn transition(mut self, prop: &str, t: crate::animation::Transition) -> Self {
        let key = format!("__transition__{prop}");
        super::set_prop(&mut self.props, &key, serde_json::to_value(&t).unwrap_or_default());
        self
    }
}

impl From<ButtonBuilder> for View {
    fn from(b: ButtonBuilder) -> Self {
        View::leaf(b.id, "button", b.props)
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
    props: Map<String, Value>,
    child: Option<View>,
}

/// Create a pointer area with the given ID.
pub fn pointer_area(id: &str) -> PointerAreaBuilder {
    PointerAreaBuilder {
        id: id.to_string(),
        props: Map::new(),
        child: None,
    }
}

impl PointerAreaBuilder {
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
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

    /// Mouse cursor to show on hover (e.g. `"pointer"`, `"grab"`,
    /// `"crosshair"`, `"text"`).
    pub fn cursor(mut self, cursor: &str) -> Self {
        super::set_prop(&mut self.props, "cursor", cursor);
        self
    }

    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
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
        View::node(b.id, "pointer_area", b.props, children)
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
    props: Map<String, Value>,
    child: Option<View>,
}

/// Create a sensor with the given ID.
pub fn sensor(id: &str) -> SensorBuilder {
    SensorBuilder {
        id: id.to_string(),
        props: Map::new(),
        child: None,
    }
}

impl SensorBuilder {
    pub fn width(mut self, w: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "width", super::length_to_value(w.into()));
        self
    }

    pub fn height(mut self, h: impl Into<Length>) -> Self {
        super::set_prop(&mut self.props, "height", super::length_to_value(h.into()));
        self
    }

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

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
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
        View::node(b.id, "sensor", b.props, children)
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
    props: Map<String, Value>,
    child: Option<View>,
}

/// Create a tooltip with the given ID and tip text.
pub fn tooltip(id: &str, tip: &str) -> TooltipBuilder {
    let mut props = Map::new();
    super::set_prop(&mut props, "tip", tip);
    TooltipBuilder {
        id: id.to_string(),
        props,
        child: None,
    }
}

impl TooltipBuilder {
    pub fn position(mut self, pos: &str) -> Self {
        super::set_prop(&mut self.props, "position", pos);
        self
    }

    pub fn gap(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "gap", v);
        self
    }

    /// Tooltip padding in pixels.
    pub fn padding(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "padding", v);
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

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
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
        View::node(b.id, "tooltip", b.props, children)
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
    props: Map<String, Value>,
    child: Option<View>,
}

/// Create a themer with the given ID.
pub fn themer(id: &str) -> ThemerBuilder {
    ThemerBuilder {
        id: id.to_string(),
        props: Map::new(),
        child: None,
    }
}

impl ThemerBuilder {
    pub fn theme(mut self, theme: &str) -> Self {
        super::set_prop(&mut self.props, "theme", theme);
        self
    }

    pub fn event_rate(mut self, rate: u32) -> Self {
        super::set_prop(&mut self.props, "event_rate", rate);
        self
    }

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
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
        View::node(b.id, "themer", b.props, children)
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
    props: Map<String, Value>,
    children: Vec<View>,
}

/// Create an overlay with the given ID.
pub fn overlay(id: &str) -> OverlayBuilder {
    OverlayBuilder {
        id: id.to_string(),
        props: Map::new(),
        children: vec![],
    }
}

impl OverlayBuilder {
    pub fn position(mut self, pos: &str) -> Self {
        super::set_prop(&mut self.props, "position", pos);
        self
    }

    pub fn align(mut self, a: Align) -> Self {
        super::set_prop(&mut self.props, "align", super::align_to_value(a));
        self
    }

    pub fn flip(mut self, v: bool) -> Self {
        super::set_prop(&mut self.props, "flip", v);
        self
    }

    pub fn gap(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "gap", v);
        self
    }

    /// Horizontal offset in pixels after positioning.
    pub fn offset_x(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "offset_x", v);
        self
    }

    /// Vertical offset in pixels after positioning.
    pub fn offset_y(mut self, v: f32) -> Self {
        super::set_prop(&mut self.props, "offset_y", v);
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

    pub fn a11y(mut self, a11y: &serde_json::Value) -> Self {
        super::set_prop(&mut self.props, "a11y", a11y.clone());
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
        View::node(b.id, "overlay", b.props, b.children)
    }
}
