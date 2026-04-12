use iced::widget::{column, container};
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{HorizontalAlignment, Length, Padding, PlushieType};

struct ColumnProps {
    padding: Option<Padding>,
    width: Option<Length>,
    height: Option<Length>,
    max_width: Option<f32>,
    align_x: Option<HorizontalAlignment>,
    clip: Option<bool>,
    wrap: Option<bool>,
}

impl ColumnProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            max_width: f32::extract(p, "max_width"),
            align_x: HorizontalAlignment::extract(p, "align_x"),
            clip: bool::extract(p, "clip"),
            wrap: bool::extract(p, "wrap"),
        }
    }
}

pub(crate) struct ColumnWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ColumnWidget {
    fn type_names(&self) -> &[&str] {
        &["column"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let cp = ColumnProps::from_node(node);
        let spacing = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            &node.props,
            "spacing",
        );
        let max_width = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            &node.props,
            "max_width",
        )
        .or(cp.max_width);

        let children = ctx.render_children(node);

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
        let align_x = cp
            .align_x
            .map(iced_convert::horizontal_alignment)
            .unwrap_or(iced::alignment::Horizontal::Left);

        let mut col = column(children)
            .width(width)
            .height(height)
            .align_x(align_x)
            .clip(cp.clip.unwrap_or(false));

        if let Some(s) = spacing {
            col = col.spacing(s);
        }
        if let Some(ref p) = cp.padding {
            col = col.padding(iced_convert::padding(p));
        }
        if let Some(mw) = max_width {
            col = col.max_width(mw);
        }

        let elem: Element<'a, Message, Theme, R> = if cp.wrap.unwrap_or(false) {
            col.wrap().into()
        } else {
            col.into()
        };

        container(elem).id(widget::Id::from(node.id.clone())).into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ColumnWidget)
    }
}
