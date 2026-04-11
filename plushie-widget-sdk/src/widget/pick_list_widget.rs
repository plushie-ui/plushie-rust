use iced::widget::{container, pick_list};
use iced::{Element, Length, Theme, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct PickListWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for PickListWidget {
    fn type_names(&self) -> &[&str] {
        &["pick_list"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_pick_list(node, *ctx)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PickListWidget)
    }
}

fn render_pick_list<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
    let options: Vec<String> = props
        .and_then(|p| p.get("options"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let selected = prop_str(props, "selected");
    let placeholder = prop_str(props, "placeholder");
    let width = prop_length(props, "width", Length::Shrink);
    let padding = parse_padding_value(props);
    let id = node.id.clone();
    let window_id = ctx.window_id.to_string();

    let mut pl = pick_list(selected, options, |v: &String| v.clone())
        .on_select(move |v: String| Message::Select(window_id.clone(), id.clone(), v))
        .width(width);

    if let Some(p) = padding {
        pl = pl.padding(p);
    }

    if let Some(p) = placeholder {
        pl = pl.placeholder(p);
    }
    if let Some(ts) = prop_f32(props, "text_size").or(ctx.default_text_size) {
        pl = pl.text_size(ts);
    }
    let font = props
        .and_then(|p| p.get("font"))
        .map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        pl = pl.font(f);
    }
    if let Some(mh) = prop_f32(props, "menu_height") {
        pl = pl.menu_height(mh);
    }
    if let Some(lh) = parse_line_height(props) {
        pl = pl.line_height(lh);
    }
    if let Some(shaping) = parse_shaping(props) {
        pl = pl.shaping(shaping);
    }

    if let Some(handle) = parse_pick_list_handle(props) {
        pl = pl.handle(handle);
    }
    if let Some(e) = parse_ellipsis(props) {
        pl = pl.ellipsis(e);
    }

    // Menu style: inline style object for the dropdown menu
    if let Some(ms) = parse_menu_style(props) {
        pl = pl.menu_style(move |theme: &iced::Theme| {
            let mut style = iced::overlay::menu::default(theme);
            apply_menu_style_overrides(&mut style, &ms);
            style
        });
    }

    // Style: string name or style map object
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            pl = match style_name {
                "default" => pl.style(pick_list::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "pick_list"
                    );
                    pl
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            pl = pl.style(move |theme: &iced::Theme, status| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("default") => pick_list::default(theme, status),
                    _ => pick_list::default(theme, status),
                };
                apply_pick_list_fields(&mut style, &ov.base);
                if matches!(status, pick_list::Status::Hovered) {
                    if let Some(ref f) = ov.hovered {
                        apply_pick_list_fields(&mut style, f);
                    } else {
                        style.background = deviate_background(style.background, 0.1);
                    }
                }
                style
            });
        }
    }

    if prop_bool_default(props, "on_open", false) {
        let open_id = node.id.clone();
        pl = pl.on_open(Message::Event {
            window_id: ctx.window_id.to_string(),
            id: open_id,
            data: Value::Null,
            family: "open".into(),
        });
    }
    if prop_bool_default(props, "on_close", false) {
        let close_id = node.id.clone();
        pl = pl.on_close(Message::Event {
            window_id: ctx.window_id.to_string(),
            id: close_id,
            data: Value::Null,
            family: "close".into(),
        });
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        pl = pl.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    container(pl).id(widget::Id::from(node.id.clone())).into()
}
