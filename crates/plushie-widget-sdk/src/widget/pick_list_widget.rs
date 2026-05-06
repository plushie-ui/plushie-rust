use iced::widget::{container, pick_list};
use iced::{Element, Theme, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    A11y, Ellipsis, Font, HasPopup, Length, LineHeight, Padding, PlushieType, Shaping,
    Style as CoreStyle,
};

struct PickListProps {
    options: Vec<String>,
    selected: Option<String>,
    placeholder: Option<String>,
    width: Option<Length>,
    padding: Option<Padding>,
    text_size: Option<f32>,
    font: Option<Font>,
    line_height: Option<LineHeight>,
    menu_height: Option<f32>,
    shaping: Option<Shaping>,
    ellipsis: Option<Ellipsis>,
    style: Option<CoreStyle>,
}

impl PickListProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        let options = prop_str_array(p, "options").unwrap_or_default();
        Self {
            options,
            selected: String::extract(p, "selected"),
            placeholder: String::extract(p, "placeholder"),
            width: Length::extract(p, "width"),
            padding: Padding::extract(p, "padding"),
            text_size: f32::extract(p, "text_size"),
            font: Font::extract(p, "font"),
            line_height: LineHeight::extract(p, "line_height"),
            menu_height: f32::extract(p, "menu_height"),
            shaping: Shaping::extract(p, "shaping"),
            ellipsis: Ellipsis::extract(p, "ellipsis"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

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

    /// Flow the pick_list's `placeholder` prop into
    /// `a11y.description` as a fallback when the author hasn't set
    /// one, and declare `has_popup: Listbox` so screen readers
    /// announce the popup affordance. Mirrors combo_box_widget so the
    /// two visually-similar widgets behave consistently. Host SDK
    /// builders may author this on the tree directly; the fallback
    /// keeps custom widget crates that skip the builder honest.
    fn infer_a11y(&self, node: &TreeNode) -> Option<crate::a11y::A11yOverrides> {
        let props = &node.props;
        let mut a11y = A11y::new().has_popup(HasPopup::Listbox);
        if let Some(desc) = crate::prop_helpers::prop_str(props, "placeholder") {
            a11y = a11y.description(desc);
        }
        Some(crate::a11y::A11yOverrides::from_core(&a11y))
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PickListWidget)
    }
}

fn render_pick_list<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let pp = PickListProps::from_node(node);
    let id = node.id.clone();
    let window_id = ctx.window_id.to_string();

    let width = pp
        .width
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Shrink);

    let mut pl = pick_list(pp.selected, pp.options, |v: &String| v.clone())
        .on_select(move |v: String| Message::Event {
            window_id: window_id.clone(),
            id: id.clone(),
            value: Value::String(v),
            family: "select".into(),
        })
        .width(width);

    if let Some(ref p) = pp.padding {
        pl = pl.padding(iced_convert::padding(p));
    }
    if let Some(p) = pp.placeholder {
        pl = pl.placeholder(p);
    }
    if let Some(ts) = pp.text_size.or(ctx.default_text_size) {
        pl = pl.text_size(ts);
    }
    let font = pp.font.map(|f| iced_convert::font(&f)).or(ctx.default_font);
    if let Some(f) = font {
        pl = pl.font(f);
    }
    if let Some(mh) = pp.menu_height {
        pl = pl.menu_height(mh);
    }
    if let Some(lh) = pp.line_height {
        pl = pl.line_height(iced_convert::line_height(lh));
    }
    if let Some(s) = pp.shaping {
        pl = pl.shaping(iced_convert::shaping(s));
    }

    // Handle: keep as raw prop access (complex iced type)
    if let Some(handle) = parse_pick_list_handle(&node.props) {
        pl = pl.handle(handle);
    }
    if let Some(e) = pp.ellipsis {
        pl = pl.ellipsis(iced_convert::ellipsis(e));
    }

    // Menu style: keep as raw prop access (complex inline style object)
    if let Some(ms) = parse_menu_style(&node.props) {
        pl = pl.menu_style(move |theme: &iced::Theme| {
            let mut style = iced::overlay::menu::default(theme);
            apply_menu_style_overrides(&mut style, &ms);
            style
        });
    }

    // Style: preset name or custom style map
    match &pp.style {
        Some(CoreStyle::Preset(name)) => {
            pl = match name.as_str() {
                "default" => pl.style(pick_list::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
                        "pick_list"
                    );
                    pl
                }
            };
        }
        Some(CoreStyle::Custom(style_map)) => {
            let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
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
        None => {}
    }

    if prop_bool_default(&node.props, "on_open", false) {
        let open_id = node.id.clone();
        pl = pl.on_open(Message::Event {
            window_id: ctx.window_id.to_string(),
            id: open_id,
            value: Value::Null,
            family: "open".into(),
        });
    }
    if prop_bool_default(&node.props, "on_close", false) {
        let close_id = node.id.clone();
        pl = pl.on_close(Message::Event {
            window_id: ctx.window_id.to_string(),
            id: close_id,
            value: Value::Null,
            family: "close".into(),
        });
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        pl = pl.on_status_change(move |status| Message::Event {
            window_id: status_wid.clone(),
            id: status_id.clone(),
            value: Value::String(status.to_string()),
            family: "status".into(),
        });
    }

    container(pl).id(widget::Id::from(node.id.clone())).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn infer(props: serde_json::Value) -> Option<crate::a11y::A11yOverrides> {
        let node = crate::testing::node_with_props("pl", "pick_list", props);
        <PickListWidget as PlushieWidget<iced::Renderer>>::infer_a11y(&PickListWidget, &node)
    }

    #[test]
    fn has_popup_listbox_always_present() {
        let o = infer(json!({})).expect("pick_list should always infer has_popup");
        assert_eq!(o.core().has_popup, Some(HasPopup::Listbox));
    }

    #[test]
    fn placeholder_flows_to_description() {
        let o = infer(json!({"placeholder": "Choose"})).expect("placeholder should infer");
        assert_eq!(o.core().description.as_deref(), Some("Choose"));
        assert_eq!(o.core().has_popup, Some(HasPopup::Listbox));
    }
}
