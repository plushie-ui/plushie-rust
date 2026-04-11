use iced::widget::{container, slider, vertical_slider};
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{Color, Length, PlushieType, Style as CoreStyle};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Apply rail color/width overrides to a slider or vertical_slider style.
/// Shared by both slider variants since they use the same `Rail` type.
fn apply_rail_overrides(
    style: &mut slider::Style,
    rail_color: Option<iced::Color>,
    rail_width: Option<f32>,
) {
    if let Some(rc) = rail_color {
        style.rail.backgrounds = (iced::Background::Color(rc), iced::Background::Color(rc));
    }
    if let Some(rw) = rail_width {
        style.rail.width = rw;
    }
}

/// Handle Slide/SlideRelease messages for sliders. Tracks the latest drag
/// value per (window_id, node_id) so SlideRelease can report the final
/// value (iced's release event doesn't carry the value itself).
fn handle_slider_message(
    last_values: &mut std::collections::HashMap<(String, String), f64>,
    msg: &Message,
) -> Option<Vec<crate::protocol::OutgoingEvent>> {
    match msg {
        Message::Slide(window_id, id, value) => {
            last_values.insert((window_id.clone(), id.clone()), *value);
            Some(vec![
                crate::protocol::OutgoingEvent::slide(id.clone(), *value)
                    .with_window_id(window_id.clone()),
            ])
        }
        Message::SlideRelease(window_id, id) => {
            let key = (window_id.clone(), id.clone());
            let value = last_values.remove(&key).unwrap_or(0.0);
            Some(vec![
                crate::protocol::OutgoingEvent::slide_release(id.clone(), value)
                    .with_window_id(window_id.clone()),
            ])
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// SliderWidget (stateful)
// ---------------------------------------------------------------------------

/// Stateful slider factory. Tracks last-emitted value to deduplicate
/// slide events (iced fires on every pixel move).
pub(crate) struct SliderWidget {
    last_values: std::collections::HashMap<(String, String), f64>,
}

impl SliderWidget {
    pub(crate) fn new() -> Self {
        Self {
            last_values: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for SliderWidget {
    fn type_names(&self) -> &[&str] {
        &["slider"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_slider(node, *ctx)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        handle_slider_message(&mut self.last_values, msg)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.last_values
            .remove(&(window_id.to_string(), node_id.to_string()));
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SliderWidget::new())
    }
}

// ---------------------------------------------------------------------------
// VerticalSliderWidget (stateful)
// ---------------------------------------------------------------------------

/// Vertical variant of [`SliderWidget`].
pub(crate) struct VerticalSliderWidget {
    last_values: std::collections::HashMap<(String, String), f64>,
}

impl VerticalSliderWidget {
    pub(crate) fn new() -> Self {
        Self {
            last_values: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for VerticalSliderWidget {
    fn type_names(&self) -> &[&str] {
        &["vertical_slider"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_vertical_slider(node, *ctx)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        handle_slider_message(&mut self.last_values, msg)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.last_values
            .remove(&(window_id.to_string(), node_id.to_string()));
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(VerticalSliderWidget::new())
    }
}

// ---------------------------------------------------------------------------
// Render logic
// ---------------------------------------------------------------------------

struct SliderProps {
    value: Option<f64>,
    step: Option<f64>,
    width: Option<Length>,
    default: Option<f64>,
    shift_step: Option<f64>,
    label: Option<String>,
    rail_color: Option<Color>,
    style: Option<CoreStyle>,
}

impl SliderProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            value: f64::extract(p, "value"),
            step: f64::extract(p, "step"),
            width: Length::extract(p, "width"),
            default: f64::extract(p, "default"),
            shift_step: f64::extract(p, "shift_step"),
            label: String::extract(p, "label"),
            rail_color: Color::extract(p, "rail_color"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

fn render_slider<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = &node.props;
    let sp = SliderProps::from_node(node);
    let range = prop_range_f64(props);
    let value = sp.value.unwrap_or(*range.start());
    let width = sp
        .width
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Fill);
    let id = node.id.clone();
    let release_id = node.id.clone();
    let window_id = ctx.window_id.to_string();
    let release_window_id = window_id.clone();

    let mut s = slider(range, value, move |v| {
        Message::Slide(window_id.clone(), id.clone(), v)
    })
    .on_release(Message::SlideRelease(release_window_id, release_id))
    .width(width);

    if let Some(st) = sp.step {
        // Clamp step to a small positive minimum to prevent division by
        // zero or infinite loops in iced's slider internals.
        s = s.step(st.max(f64::EPSILON));
    }
    if let Some(d) = sp.default {
        s = s.default(d);
    }
    if let Some(h) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "height") {
        s = s.height(h);
    }
    if let Some(ss) = sp.shift_step {
        s = s.shift_step(ss);
    }
    if let Some(label) = sp.label {
        s = s.label(label);
    }

    // Rail styling props (applied on top of any style preset)
    let rail_color = sp.rail_color.as_ref().map(iced_convert::color);
    let rail_width = prop_f32(props, "rail_width");
    let has_rail_overrides = rail_color.is_some() || rail_width.is_some();

    // Style with optional circular handle
    let circular = prop_bool_default(props, "circular_handle", false);
    if circular {
        let radius = prop_f32(props, "handle_radius").unwrap_or(8.0);
        s = s.style(move |theme, status| {
            let mut style = slider::default(theme, status).with_circular_handle(radius);
            apply_rail_overrides(&mut style, rail_color, rail_width);
            style
        });
    } else {
        match &sp.style {
            Some(CoreStyle::Preset(name)) => {
                s = match name.as_str() {
                    "default" => {
                        if has_rail_overrides {
                            s.style(move |theme: &iced::Theme, status| {
                                let mut style = slider::default(theme, status);
                                apply_rail_overrides(&mut style, rail_color, rail_width);
                                style
                            })
                        } else {
                            s.style(slider::default)
                        }
                    }
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            name,
                            "slider"
                        );
                        s
                    }
                };
            }
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
                s = s.style(move |theme: &iced::Theme, status| {
                    let mut style = slider::default(theme, status);
                    apply_slider_handle_fields(&mut style.handle, &ov.base);
                    apply_rail_overrides(&mut style, rail_color, rail_width);
                    if matches!(status, slider::Status::Hovered) {
                        if let Some(ref f) = ov.hovered {
                            apply_slider_handle_fields(&mut style.handle, f);
                        } else {
                            style.handle.background =
                                deviate_background(style.handle.background, 0.1);
                        }
                    }
                    style
                });
            }
            None => {}
        }
    }
    if !circular && sp.style.is_none() && has_rail_overrides {
        s = s.style(move |theme: &iced::Theme, status| {
            let mut style = slider::default(theme, status);
            apply_rail_overrides(&mut style, rail_color, rail_width);
            style
        });
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        s = s.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(s).id(widget::Id::from(node.id.clone())).into()
}

struct VerticalSliderProps {
    value: Option<f64>,
    step: Option<f64>,
    height: Option<Length>,
    default: Option<f64>,
    shift_step: Option<f64>,
    label: Option<String>,
    rail_color: Option<Color>,
    style: Option<CoreStyle>,
}

impl VerticalSliderProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            value: f64::extract(p, "value"),
            step: f64::extract(p, "step"),
            height: Length::extract(p, "height"),
            default: f64::extract(p, "default"),
            shift_step: f64::extract(p, "shift_step"),
            label: String::extract(p, "label"),
            rail_color: Color::extract(p, "rail_color"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

fn render_vertical_slider<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = &node.props;
    let vp = VerticalSliderProps::from_node(node);
    let range = prop_range_f64(props);
    let value = vp.value.unwrap_or(*range.start());
    let width = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "width");
    let height = vp
        .height
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Fill);
    let id = node.id.clone();
    let release_id = node.id.clone();
    let window_id = ctx.window_id.to_string();
    let release_window_id = window_id.clone();

    let mut s = vertical_slider(range, value, move |v| {
        Message::Slide(window_id.clone(), id.clone(), v)
    })
    .on_release(Message::SlideRelease(release_window_id, release_id))
    .height(height);

    if let Some(w) = width {
        s = s.width(w);
    }

    if let Some(st) = vp.step {
        s = s.step(st.max(f64::EPSILON));
    }
    if let Some(d) = vp.default {
        s = s.default(d);
    }
    if let Some(ss) = vp.shift_step {
        s = s.shift_step(ss);
    }
    if let Some(label) = vp.label {
        s = s.label(label);
    }

    // Rail styling props (applied on top of any style preset)
    let rail_color = vp.rail_color.as_ref().map(iced_convert::color);
    let rail_width = prop_f32(props, "rail_width");
    let has_rail_overrides = rail_color.is_some() || rail_width.is_some();

    // Style: preset name or custom style map
    match &vp.style {
        Some(CoreStyle::Preset(name)) => {
            s = match name.as_str() {
                "default" => {
                    if has_rail_overrides {
                        s.style(move |theme: &iced::Theme, status| {
                            let mut style = vertical_slider::default(theme, status);
                            apply_rail_overrides(&mut style, rail_color, rail_width);
                            style
                        })
                    } else {
                        s.style(vertical_slider::default)
                    }
                }
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
                        "vertical_slider"
                    );
                    s
                }
            };
        }
        Some(CoreStyle::Custom(style_map)) => {
            let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
            s = s.style(move |theme: &iced::Theme, status| {
                let mut style = vertical_slider::default(theme, status);
                apply_slider_handle_fields(&mut style.handle, &ov.base);
                apply_rail_overrides(&mut style, rail_color, rail_width);
                if matches!(status, vertical_slider::Status::Hovered) {
                    if let Some(ref f) = ov.hovered {
                        apply_slider_handle_fields(&mut style.handle, f);
                    } else {
                        style.handle.background = deviate_background(style.handle.background, 0.1);
                    }
                }
                style
            });
        }
        None => {}
    }
    if vp.style.is_none() && has_rail_overrides {
        s = s.style(move |theme: &iced::Theme, status| {
            let mut style = vertical_slider::default(theme, status);
            apply_rail_overrides(&mut style, rail_color, rail_width);
            style
        });
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        s = s.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(s).id(widget::Id::from(node.id.clone())).into()
}
