use iced::widget::{container, row};
use iced::{Element, Theme, widget};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{HorizontalAlignment, Length, Padding, PlushieType, VerticalAlignment};

struct RowProps {
    padding: Option<Padding>,
    width: Option<Length>,
    height: Option<Length>,
    align_x: Option<HorizontalAlignment>,
    align_y: Option<VerticalAlignment>,
    clip: Option<bool>,
    wrap: Option<bool>,
}

impl RowProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            align_x: HorizontalAlignment::extract(p, "align_x"),
            align_y: VerticalAlignment::extract(p, "align_y"),
            clip: bool::extract(p, "clip"),
            wrap: bool::extract(p, "wrap"),
        }
    }
}

pub(crate) struct RowWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for RowWidget {
    fn type_names(&self) -> &[&str] {
        &["row"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let rp = RowProps::from_node(node);
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
        );

        let children = ctx.render_children(node);

        let width = rp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = rp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let align_y = rp
            .align_y
            .map(iced_convert::vertical_alignment)
            .unwrap_or(iced::alignment::Vertical::Top);

        let mut r = row(children)
            .width(width)
            .height(height)
            .align_y(align_y)
            .clip(rp.clip.unwrap_or(false));

        if let Some(s) = spacing {
            r = r.spacing(s);
        }
        if let Some(ref p) = rp.padding {
            r = r.padding(iced_convert::padding(p));
        }

        let elem: Element<'a, Message, Theme, R> = if rp.wrap.unwrap_or(false) {
            r.wrap().into()
        } else {
            r.into()
        };

        // Row doesn't have max_width natively; wrap in a container to constrain it.
        let mut row_container = container(elem).id(widget::Id::from(node.id.clone()));
        if let Some(mw) = max_width {
            row_container = row_container.max_width(mw);
        }
        if let Some(ax) = rp.align_x {
            row_container = row_container.align_x(iced_convert::horizontal_alignment(ax));
        }

        row_container.into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RowWidget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_align_x() {
        let node = crate::testing::node_with_props("row", "row", json!({"align_x": "center"}));
        let props = RowProps::from_node(&node);

        assert_eq!(props.align_x, Some(HorizontalAlignment::Center));
    }
}
