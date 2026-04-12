use iced::widget::{Space, scrollable};
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::{Message, ScrollViewport};
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{Anchor, Color, Direction, Length, PlushieType};

struct ScrollableProps {
    width: Option<Length>,
    height: Option<Length>,
    direction: Option<Direction>,
    scrollbar_width: Option<f32>,
    scrollbar_margin: Option<f32>,
    scroller_width: Option<f32>,
    anchor: Option<Anchor>,
    on_scroll: Option<bool>,
    auto_scroll: Option<bool>,
    scrollbar_color: Option<Color>,
    scroller_color: Option<Color>,
}

impl ScrollableProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            direction: Direction::extract(p, "direction"),
            scrollbar_width: f32::extract(p, "scrollbar_width"),
            scrollbar_margin: f32::extract(p, "scrollbar_margin"),
            scroller_width: f32::extract(p, "scroller_width"),
            anchor: Anchor::extract(p, "anchor"),
            on_scroll: bool::extract(p, "on_scroll"),
            auto_scroll: bool::extract(p, "auto_scroll"),
            scrollbar_color: Color::extract(p, "scrollbar_color"),
            scroller_color: Color::extract(p, "scroller_color"),
        }
    }

    fn build_scrollbar(&self) -> scrollable::Scrollbar {
        let mut sb = scrollable::Scrollbar::default();
        if let Some(w) = self.scrollbar_width {
            sb = sb.width(w);
        }
        if let Some(m) = self.scrollbar_margin {
            sb = sb.margin(m);
        }
        if let Some(sw) = self.scroller_width {
            sb = sb.scroller_width(sw);
        }
        sb
    }
}

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
        let sp = ScrollableProps::from_node(node);
        let spacing = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            &node.props,
            "spacing",
        );

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let sb = sp.build_scrollbar();
        let direction = sp.direction.unwrap_or(Direction::Vertical);
        let mut s = scrollable(child).direction(iced_convert::scrollable_direction(direction, sb));

        let width = sp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = sp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        s = s.width(width).height(height);

        // Widget ID
        s = s.id(widget::Id::from(node.id.clone()));

        if let Some(spacing_val) = spacing {
            s = s.spacing(spacing_val);
        }

        // Anchor
        if let Some(a) = sp.anchor {
            s = s.anchor_y(iced_convert::anchor(a));
        }

        // on_scroll: emit viewport data when scroll position changes
        if sp.on_scroll.unwrap_or(false) {
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
        if sp.auto_scroll.unwrap_or(false) {
            s = s.auto_scroll(true);
        }

        // Scrollbar color styling (kept as iced::Color for style closure)
        let scrollbar_color = sp.scrollbar_color.as_ref().map(iced_convert::color);
        let scroller_color = sp.scroller_color.as_ref().map(iced_convert::color);
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
