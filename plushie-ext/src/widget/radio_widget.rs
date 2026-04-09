use iced::widget::container;
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct RadioWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for RadioWidget {
    fn type_names(&self) -> &[&str] {
        &["radio"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_radio(node, *ctx)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RadioWidget)
    }
}

fn render_radio<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let value = prop_str(props, "value").unwrap_or_default();
    let selected_str = prop_str(props, "selected").unwrap_or_default();
    let label = prop_str(props, "label").unwrap_or_else(|| value.clone());
    // Use "group" prop as the event ID so all radios in a group emit the same ID.
    let event_id = prop_str(props, "group").unwrap_or_else(|| node.id.clone());

    let is_selected = if value == selected_str {
        Some(0u8)
    } else {
        None
    };
    let select_value = value;

    let mut r = iced::widget::Radio::new(label, 0u8, is_selected, move |_| {
        Message::Select(
            ctx.window_id.to_string(),
            event_id.clone(),
            select_value.clone(),
        )
    });

    if let Some(s) = prop_f32(props, "spacing") {
        r = r.spacing(s);
    }
    if let Some(w) = value_to_length_opt(props.and_then(|p| p.get("width"))) {
        r = r.width(w);
    }
    if let Some(sz) = prop_f32(props, "size") {
        r = r.size(sz);
    }
    if let Some(ts) = prop_f32(props, "text_size").or(ctx.default_text_size) {
        r = r.text_size(ts);
    }
    let font = props
        .and_then(|p| p.get("font"))
        .map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        r = r.font(f);
    }
    if let Some(lh) = parse_line_height(props) {
        r = r.line_height(lh);
    }
    if let Some(shaping) = parse_shaping(props) {
        r = r.shaping(shaping);
    }
    if let Some(w) = parse_wrapping(props) {
        r = r.wrapping(w);
    }

    // Style: string name or style map object
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            r = match style_name {
                "default" => r.style(iced::widget::radio::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "radio"
                    );
                    r
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            r = r.style(move |theme: &iced::Theme, status| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("default") => iced::widget::radio::default(theme, status),
                    _ => iced::widget::radio::default(theme, status),
                };
                apply_radio_fields(&mut style, &ov.base);
                if matches!(status, iced::widget::radio::Status::Hovered { .. }) {
                    if let Some(ref f) = ov.hovered {
                        apply_radio_fields(&mut style, f);
                    } else {
                        style.background = deviate_background(style.background, 0.1);
                    }
                }
                style
            });
        }
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        r = r.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(r).id(widget::Id::from(node.id.clone())).into()
}
