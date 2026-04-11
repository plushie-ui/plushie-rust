use iced::widget::scrollable::Anchor;
use iced::widget::{Space, scrollable};
use iced::{Element, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::{Message, ScrollViewport};
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct ScrollableWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ScrollableWidget {
    fn type_names(&self) -> &[&str] {
        &["scrollable"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let spacing = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing");

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let direction = prop_str(props, "direction").unwrap_or_default();

        // Build scrollbar configuration from props
        let build_scrollbar = |props: JsonProps<'_>| -> scrollable::Scrollbar {
            let mut sb = scrollable::Scrollbar::default();
            if let Some(w) = prop_f32(props, "scrollbar_width") {
                sb = sb.width(w);
            }
            if let Some(m) = prop_f32(props, "scrollbar_margin") {
                sb = sb.margin(m);
            }
            if let Some(sw) = prop_f32(props, "scroller_width") {
                sb = sb.scroller_width(sw);
            }
            sb
        };

        let sb = build_scrollbar(props);
        let mut s = match direction.as_str() {
            "horizontal" => scrollable(child).direction(scrollable::Direction::Horizontal(sb)),
            "both" => scrollable(child).direction(scrollable::Direction::Both {
                vertical: sb,
                horizontal: build_scrollbar(props),
            }),
            _ => scrollable(child).direction(scrollable::Direction::Vertical(sb)),
        };

        s = s.width(width).height(height);

        // Widget ID
        s = s.id(widget::Id::from(node.id.clone()));

        if let Some(sp) = spacing {
            s = s.spacing(sp);
        }

        // Anchor
        if let Some(anchor_str) = prop_str(props, "anchor") {
            match anchor_str.to_ascii_lowercase().as_str() {
                "end" | "bottom" | "right" => {
                    s = s.anchor_y(Anchor::End);
                }
                _ => {}
            }
        }

        // on_scroll: emit viewport data when scroll position changes
        if prop_bool_default(props, "on_scroll", false) {
            let window_id = ctx.window_id.to_string();
            let scroll_id = node.id.clone();
            s = s.on_scroll(move |viewport| {
                let abs = viewport.absolute_offset();
                let rel = viewport.relative_offset();
                let bounds = viewport.bounds();
                let content_bounds = viewport.content_bounds();
                Message::ScrollEvent(
                    window_id.clone(),
                    scroll_id.clone(),
                    ScrollViewport {
                        absolute_x: abs.x,
                        absolute_y: abs.y,
                        relative_x: rel.x,
                        relative_y: rel.y,
                        viewport_width: bounds.width,
                        viewport_height: bounds.height,
                        content_width: content_bounds.width,
                        content_height: content_bounds.height,
                    },
                )
            });
        }

        // auto_scroll: automatically scroll to show new content
        if prop_bool_default(props, "auto_scroll", false) {
            s = s.auto_scroll(true);
        }

        // Scrollbar color styling
        let scrollbar_color = prop_color(props, "scrollbar_color");
        let scroller_color = prop_color(props, "scroller_color");
        if scrollbar_color.is_some() || scroller_color.is_some() {
            s = s.style(move |theme: &iced::Theme, status| {
                let mut style = scrollable::default(theme, status);
                if let Some(sc) = scrollbar_color {
                    style.vertical_rail.background = Some(iced::Background::Color(sc));
                    style.horizontal_rail.background = Some(iced::Background::Color(sc));
                }
                if let Some(sc) = scroller_color {
                    style.vertical_rail.scroller.background = iced::Background::Color(sc);
                    style.horizontal_rail.scroller.background = iced::Background::Color(sc);
                }
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

        s.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ScrollableWidget)
    }
}
