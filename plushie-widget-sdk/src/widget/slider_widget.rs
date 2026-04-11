use iced::widget::{container, slider, vertical_slider};
use iced::{Element, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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

fn render_slider<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
    let range = prop_range_f64(props);
    let value = prop_f64(props, "value").unwrap_or(*range.start());
    let step = prop_f64(props, "step");
    let width = prop_length(props, "width", Length::Fill);
    let id = node.id.clone();
    let release_id = node.id.clone();
    let window_id = ctx.window_id.to_string();
    let release_window_id = window_id.clone();

    let mut s = slider(range, value, move |v| {
        Message::Slide(window_id.clone(), id.clone(), v)
    })
    .on_release(Message::SlideRelease(release_window_id, release_id))
    .width(width);

    if let Some(st) = step {
        // Clamp step to a small positive minimum to prevent division by
        // zero or infinite loops in iced's slider internals.
        s = s.step(st.max(f64::EPSILON));
    }
    if let Some(d) = prop_f64(props, "default") {
        s = s.default(d);
    }
    if let Some(h) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "height") {
        s = s.height(h);
    }
    if let Some(ss) = prop_f64(props, "shift_step") {
        s = s.shift_step(ss);
    }
    if let Some(label) = prop_str(props, "label") {
        s = s.label(label);
    }

    // Rail styling props (applied on top of any style preset)
    let rail_color = prop_color(props, "rail_color");
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
    } else if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            s = match style_name {
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
                        style_name,
                        "slider"
                    );
                    s
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            s = s.style(move |theme: &iced::Theme, status| {
                let mut style = slider::default(theme, status);
                apply_slider_handle_fields(&mut style.handle, &ov.base);
                apply_rail_overrides(&mut style, rail_color, rail_width);
                if matches!(status, slider::Status::Hovered) {
                    if let Some(ref f) = ov.hovered {
                        apply_slider_handle_fields(&mut style.handle, f);
                    } else {
                        style.handle.background = deviate_background(style.handle.background, 0.1);
                    }
                }
                style
            });
        }
    } else if has_rail_overrides {
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

fn render_vertical_slider<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
    let range = prop_range_f64(props);
    let value = prop_f64(props, "value").unwrap_or(*range.start());
    let step = prop_f64(props, "step");
    let width = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "width");
    let height = prop_length(props, "height", Length::Fill);
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

    if let Some(st) = step {
        s = s.step(st.max(f64::EPSILON));
    }
    if let Some(d) = prop_f64(props, "default") {
        s = s.default(d);
    }
    if let Some(ss) = prop_f64(props, "shift_step") {
        s = s.shift_step(ss);
    }
    if let Some(label) = prop_str(props, "label") {
        s = s.label(label);
    }

    // Rail styling props (applied on top of any style preset)
    let rail_color = prop_color(props, "rail_color");
    let rail_width = prop_f32(props, "rail_width");
    let has_rail_overrides = rail_color.is_some() || rail_width.is_some();

    // Style: string name or style map object
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            s = match style_name {
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
                        style_name,
                        "vertical_slider"
                    );
                    s
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
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
    } else if has_rail_overrides {
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
