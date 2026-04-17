use iced::widget::{Space, container};
use iced::{Element, Fill, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Background, Border, Color, HorizontalAlignment, Length, Padding, PlushieType, Shadow,
    Style as CoreStyle, VerticalAlignment,
};

struct ContainerProps {
    padding: Option<Padding>,
    width: Option<Length>,
    height: Option<Length>,
    align_x: Option<HorizontalAlignment>,
    align_y: Option<VerticalAlignment>,
    background: Option<Background>,
    color: Option<Color>,
    border: Option<Border>,
    shadow: Option<Shadow>,
    style: Option<CoreStyle>,
}

impl ContainerProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            align_x: HorizontalAlignment::extract(p, "align_x"),
            align_y: VerticalAlignment::extract(p, "align_y"),
            background: Background::extract(p, "background"),
            color: Color::extract(p, "color"),
            border: Border::extract(p, "border"),
            shadow: Shadow::extract(p, "shadow"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

pub(crate) struct ContainerWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ContainerWidget {
    fn type_names(&self) -> &[&str] {
        &["container"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let cp = ContainerProps::from_node(node);
        let props = &node.props;

        let width = cp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = cp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let center = prop_bool_default(props, "center", false);
        let clip = prop_bool_default(props, "clip", false);

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let mut c = container(child).width(width).height(height).clip(clip);

        if let Some(ref p) = cp.padding {
            c = c.padding(iced_convert::padding(p));
        }

        if let Some(mw) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "max_width")
        {
            c = c.max_width(mw);
        }
        if let Some(mh) = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            props,
            "max_height",
        ) {
            c = c.max_height(mh);
        }

        if center {
            c = c.center(Fill);
        }

        if let Some(ax) = cp.align_x {
            c = c.align_x(iced_convert::horizontal_alignment(ax));
        }
        if let Some(ay) = cp.align_y {
            c = c.align_y(iced_convert::vertical_alignment(ay));
        }

        // Inline styling via typed props
        let bg = cp.background.as_ref().map(iced_convert::background);
        let text_color = cp.color.as_ref().map(iced_convert::color);
        let border_val = cp.border.as_ref().map(iced_convert::border);
        let shadow_val = cp.shadow.as_ref().map(iced_convert::shadow);
        let has_inline_style =
            bg.is_some() || text_color.is_some() || border_val.is_some() || shadow_val.is_some();

        if has_inline_style {
            c = c.style(move |_theme| {
                let mut style = container::Style {
                    background: bg,
                    text_color,
                    ..Default::default()
                };
                if let Some(b) = border_val {
                    style.border = b;
                }
                if let Some(s) = shadow_val {
                    style.shadow = s;
                }
                style
            });
        }

        // Named style or style map (overrides inline if both present)
        match &cp.style {
            Some(CoreStyle::Preset(name)) => {
                c = match name.as_str() {
                    "transparent" => c.style(container::transparent),
                    "rounded_box" => c.style(container::rounded_box),
                    "bordered_box" => c.style(container::bordered_box),
                    "dark" => c.style(container::dark),
                    "primary" => c.style(container::primary),
                    "secondary" => c.style(container::secondary),
                    "success" => c.style(container::success),
                    "danger" => c.style(container::danger),
                    "warning" => c.style(container::warning),
                    _ => {
                        log::warn!(
                            "unknown style {:?} for widget type {:?}, using default",
                            name,
                            "container"
                        );
                        c
                    }
                };
            }
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
                c = c.style(move |theme| {
                    let mut style = match ov.preset_base.as_deref() {
                        Some("transparent") => container::transparent(theme),
                        Some("rounded_box") => container::rounded_box(theme),
                        Some("bordered_box") => container::bordered_box(theme),
                        Some("dark") => container::dark(theme),
                        Some("primary") => container::primary(theme),
                        Some("secondary") => container::secondary(theme),
                        Some("success") => container::success(theme),
                        Some("danger") => container::danger(theme),
                        Some("warning") => container::warning(theme),
                        _ => container::Style::default(),
                    };
                    if let Some(bg) = ov.base.background {
                        style.background = Some(bg);
                    }
                    if let Some(tc) = ov.base.text_color {
                        style.text_color = Some(tc);
                    }
                    if let Some(brd) = ov.base.border {
                        style.border = brd;
                    }
                    if let Some(shd) = ov.base.shadow {
                        style.shadow = shd;
                    }
                    style
                });
            }
            None => {}
        }

        // Widget ID for operations targeting
        c = c.id(widget::Id::from(node.id.clone()));

        c.into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ContainerWidget)
    }
}
