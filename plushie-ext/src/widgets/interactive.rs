//! Interactive wrapper widgets -- add behavior to child content.
//!
//! - `button` -- clickable wrapper with press/release events, style variants
//! - `mouse_area` -- invisible overlay that captures mouse events (enter,
//!   exit, move, scroll, right/middle click)
//! - `sensor` -- debounced resize observer that reports container dimensions
//! - `tooltip` -- popup hint shown on hover or keyboard focus
//! - `themer` -- overrides the iced theme for its subtree
//! - `window` -- top-level window node (rendered as a container)
//! - `overlay` -- positioned popup anchored to a sibling element

use std::time::Duration;

use iced::widget::{Space, button, container, mouse_area, sensor, text, tooltip};
use iced::{Element, Fill, Length, Theme, mouse, widget};

use super::helpers::*;
use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::render_ctx::RenderCtx;

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

pub(crate) fn render_button<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let id = node.id.clone();
    let window_id = ctx.window_id.to_string();

    // Button can have either a text label or child content
    let child: Element<'a, Message, Theme, R> = if !node.children.is_empty() {
        node.children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into())
    } else {
        let label = prop_str(props, "label")
            .or_else(|| prop_str(props, "content"))
            .unwrap_or_default();
        text(label).into()
    };

    let padding = parse_padding_value(props);
    let width = prop_length(props, "width", Length::Shrink);
    let height = prop_length(props, "height", Length::Shrink);
    let clip = prop_bool_default(props, "clip", false);
    let disabled =
        prop_bool_default(props, "disabled", false) || !prop_bool_default(props, "enabled", true);

    let mut b = button(child).width(width).height(height).clip(clip);

    if let Some(p) = padding {
        b = b.padding(p);
    }

    if !disabled {
        b = b.on_press(Message::Click(window_id, id));
    }

    // Style: string name or style map object
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            b = match style_name {
                "primary" => b.style(button::primary),
                "secondary" => b.style(button::secondary),
                "success" => b.style(button::success),
                "warning" => b.style(button::warning),
                "danger" => b.style(button::danger),
                "text" => b.style(button::text),
                "background" => b.style(button::background),
                "subtle" => b.style(button::subtle),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "button"
                    );
                    b.style(button::primary)
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            b = b.style(move |theme: &iced::Theme, status| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("primary") => button::primary(theme, status),
                    Some("secondary") => button::secondary(theme, status),
                    Some("success") => button::success(theme, status),
                    Some("danger") => button::danger(theme, status),
                    Some("warning") => button::warning(theme, status),
                    Some("text") => button::text(theme, status),
                    Some("background") => button::background(theme, status),
                    Some("subtle") => button::subtle(theme, status),
                    _ => button::primary(theme, status),
                };
                apply_button_fields(&mut style, &ov.base);
                match status {
                    button::Status::Hovered => {
                        if let Some(ref f) = ov.hovered {
                            apply_button_fields(&mut style, f);
                        } else {
                            style.background = auto_derive_hover_bg(style.background);
                        }
                    }
                    button::Status::Pressed => {
                        if let Some(ref f) = ov.pressed {
                            apply_button_fields(&mut style, f);
                        }
                    }
                    button::Status::Disabled => {
                        if let Some(ref f) = ov.disabled {
                            apply_button_fields(&mut style, f);
                        } else {
                            style.background = auto_derive_disabled_bg(style.background);
                            style.text_color = auto_derive_disabled_text(style.text_color);
                            style.border = auto_derive_disabled_border(style.border);
                            style.shadow = auto_derive_disabled_shadow(style.shadow);
                        }
                    }
                    _ => {}
                }
                style
            });
        }
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        b = b.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(b).id(widget::Id::from(node.id.clone())).into()
}

// ---------------------------------------------------------------------------
// MouseArea
// ---------------------------------------------------------------------------

pub(crate) fn render_mouse_area<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let child: Element<'a, Message, Theme, R> = node
        .children
        .first()
        .map(|c| ctx.render_child(c))
        .unwrap_or_else(|| Space::new().into());

    let id = node.id.clone();
    let release_id = format!("{}:release", node.id);
    let window_id = ctx.window_id.to_string();

    let mut ma = mouse_area(child)
        .on_press({
            let wid = window_id.clone();
            let nid = id.clone();
            move |_p| Message::Click(wid.clone(), nid.clone())
        })
        .on_release({
            let wid = window_id.clone();
            let rid = release_id.clone();
            move |_p| Message::Click(wid.clone(), rid.clone())
        });

    // Conditional event handlers (opt-in via boolean props)
    if prop_bool_default(props, "on_middle_press", false) {
        let ev_id = node.id.clone();
        let wid = window_id.clone();
        ma = ma.on_middle_press(move |p| {
            Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "middle_press".into(), p.x, p.y)
        });
    }
    if prop_bool_default(props, "on_right_press", false) {
        let ev_id = node.id.clone();
        let wid = window_id.clone();
        ma = ma.on_right_press(move |p| {
            Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "right_press".into(), p.x, p.y)
        });
    }
    if prop_bool_default(props, "on_right_release", false) {
        let ev_id = node.id.clone();
        let wid = window_id.clone();
        ma = ma.on_right_release(move |p| {
            Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "right_release".into(), p.x, p.y)
        });
    }
    if prop_bool_default(props, "on_middle_release", false) {
        let ev_id = node.id.clone();
        let wid = window_id.clone();
        ma = ma.on_middle_release(move |p| {
            Message::MouseAreaEvent(
                wid.clone(),
                ev_id.clone(),
                "middle_release".into(),
                p.x,
                p.y,
            )
        });
    }
    if prop_bool_default(props, "on_double_click", false) {
        let ev_id = node.id.clone();
        let wid = window_id.clone();
        ma = ma.on_double_click(move |p| {
            Message::MouseAreaEvent(wid.clone(), ev_id.clone(), "double_click".into(), p.x, p.y)
        });
    }
    if prop_bool_default(props, "on_enter", false) {
        let ev_id = node.id.clone();
        ma = ma.on_enter(Message::MouseAreaEvent(
            window_id.clone(),
            ev_id,
            "enter".into(),
            0.0,
            0.0,
        ));
    }
    if prop_bool_default(props, "on_exit", false) {
        let ev_id = node.id.clone();
        ma = ma.on_exit(Message::MouseAreaEvent(
            window_id.clone(),
            ev_id,
            "exit".into(),
            0.0,
            0.0,
        ));
    }
    if prop_bool_default(props, "on_move", false) {
        let ev_id = node.id.clone();
        let move_window_id = window_id.clone();
        ma = ma.on_move(move |p| {
            Message::MouseAreaMove(move_window_id.clone(), ev_id.clone(), p.x, p.y)
        });
    }
    if prop_bool_default(props, "on_scroll", false) {
        let ev_id = node.id.clone();
        let scroll_window_id = window_id.clone();
        ma = ma.on_scroll(move |delta, position| {
            let (dx, dy) = match delta {
                mouse::ScrollDelta::Lines { x, y } => (x, y),
                mouse::ScrollDelta::Pixels { x, y } => (x, y),
            };
            Message::MouseAreaScroll(
                scroll_window_id.clone(),
                ev_id.clone(),
                dx,
                dy,
                position.x,
                position.y,
            )
        });
    }

    if let Some(cursor) = prop_str(props, "cursor")
        && let Some(interaction) = parse_interaction(&cursor)
    {
        ma = ma.interaction(interaction);
    }

    ma.into()
}

// ---------------------------------------------------------------------------
// Sensor
// ---------------------------------------------------------------------------

pub(crate) fn render_sensor<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let child: Element<'a, Message, Theme, R> = node
        .children
        .first()
        .map(|c| ctx.render_child(c))
        .unwrap_or_else(|| Space::new().into());

    // Sensor needs a key. Use the node id.
    let id = node.id.clone();
    let show_id = node.id.clone();
    let resize_id = node.id.clone();
    let hide_id = format!("{}:hide", node.id);

    let props = node.props.as_object();

    let mut s = sensor(child)
        .key(id)
        .on_show({
            let window_id = ctx.window_id.to_string();
            move |size| {
                Message::SensorResize(
                    window_id.clone(),
                    format!("{}:show", show_id),
                    size.width,
                    size.height,
                )
            }
        })
        .on_resize({
            let window_id = ctx.window_id.to_string();
            move |size| {
                Message::SensorResize(
                    window_id.clone(),
                    resize_id.clone(),
                    size.width,
                    size.height,
                )
            }
        })
        .on_hide(Message::Click(ctx.window_id.to_string(), hide_id));

    if let Some(d) = prop_f64(props, "delay") {
        s = s.delay(Duration::from_millis(d as u64));
    }
    if let Some(a) = prop_f32(props, "anticipate") {
        s = s.anticipate(a);
    }

    s.into()
}

// ---------------------------------------------------------------------------
// Tooltip
// ---------------------------------------------------------------------------

/// Render a tooltip widget that shows a popup hint on hover/focus.
///
/// # Accessibility
///
/// The tooltip `tip` text is rendered visually on hover but is not
/// automatically exposed as the child widget's accessible description.
/// For AT users to hear the tooltip content, the host should wire the
/// tooltip text into the child's `a11y.description` prop, or use
/// `a11y.described_by` to point to a separate text node containing the
/// same content. iced's tooltip widget itself does not currently emit
/// an accessible `Tooltip` role -- the visual popup appears/disappears
/// without AT notification.
pub(crate) fn render_tooltip<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let tip = prop_str(props, "tip").unwrap_or_default();
    let gap = prop_f32(props, "gap");
    let position = prop_str(props, "position")
        .map(|s| match s.to_ascii_lowercase().as_str() {
            "bottom" => tooltip::Position::Bottom,
            "left" => tooltip::Position::Left,
            "right" => tooltip::Position::Right,
            "follow_cursor" | "follow" => tooltip::Position::FollowCursor,
            _ => tooltip::Position::Top,
        })
        .unwrap_or(tooltip::Position::Top);

    let child: Element<'a, Message, Theme, R> = node
        .children
        .first()
        .map(|c| ctx.render_child(c))
        .unwrap_or_else(|| Space::new().into());

    let mut tt = tooltip(child, text(tip), position);
    if let Some(g) = gap {
        tt = tt.gap(g);
    }

    // Tooltip padding is a single f32 value (not per-side)
    if let Some(p) = prop_f32(props, "padding") {
        tt = tt.padding(p);
    }

    let snap = prop_bool_default(props, "snap_within_viewport", true);
    tt = tt.snap_within_viewport(snap);

    if let Some(d) = prop_f64(props, "delay") {
        tt = tt.delay(Duration::from_millis(d as u64));
    }

    // Style: string name or style map (tooltip uses container::Style)
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            tt = match style_name {
                "transparent" => tt.style(container::transparent),
                "rounded_box" => tt.style(container::rounded_box),
                "bordered_box" => tt.style(container::bordered_box),
                "dark" => tt.style(container::dark),
                "primary" => tt.style(container::primary),
                "secondary" => tt.style(container::secondary),
                "success" => tt.style(container::success),
                "danger" => tt.style(container::danger),
                "warning" => tt.style(container::warning),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "tooltip"
                    );
                    tt
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            tt = tt.style(move |_theme| container_style_from_base(&ov.base));
        }
    }

    tt.into()
}

// ---------------------------------------------------------------------------
// Themer (applies a sub-theme to child content)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Window (top-level container)
// ---------------------------------------------------------------------------

pub(crate) fn render_window<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let padding = parse_padding_value(props);
    let width = prop_length(props, "width", Fill);
    let height = prop_length(props, "height", Fill);

    let child_ctx = ctx.with_window_id(&node.id);

    let child: Element<'a, Message, Theme, R> = node
        .children
        .first()
        .map(|c| child_ctx.render_child(c))
        .unwrap_or_else(|| Space::new().into());

    let mut c = container(child).width(width).height(height);

    if let Some(p) = padding {
        c = c.padding(p);
    }

    c.into()
}

// ---------------------------------------------------------------------------
// Overlay
// ---------------------------------------------------------------------------

pub(crate) fn render_overlay<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    use super::overlay;

    let props = node.props.as_object();
    let position = prop_str(props, "position").unwrap_or_else(|| "below".to_string());
    let gap = prop_f32(props, "gap").unwrap_or(0.0);
    let offset_x = prop_f32(props, "offset_x").unwrap_or(0.0);
    let offset_y = prop_f32(props, "offset_y").unwrap_or(0.0);
    let flip = prop_bool_default(props, "flip", false);
    let align = match prop_str(props, "align").as_deref() {
        Some("start") => overlay::Align::Start,
        Some("end") => overlay::Align::End,
        _ => overlay::Align::Center,
    };

    let children = &node.children;
    if children.len() < 2 {
        return text(format!("overlay requires 2 children (id={})", node.id)).into();
    }

    let anchor = ctx.render_child(&children[0]);
    let content = ctx.render_child(&children[1]);

    let pos = match position.as_str() {
        "above" => overlay::Position::Above,
        "left" => overlay::Position::Left,
        "right" => overlay::Position::Right,
        _ => overlay::Position::Below,
    };

    overlay::OverlayWrapper::new(anchor, content, pos, gap, offset_x, offset_y, flip, align).into()
}
