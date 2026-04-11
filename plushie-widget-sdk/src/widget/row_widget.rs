use iced::widget::{container, row};
use iced::{Element, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
        let props = &node.props;
        let spacing = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing");
        let padding = parse_padding_value(props);
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let align_y = prop_vertical_alignment(props, "align_y");
        let clip = prop_bool_default(props, "clip", false);

        let children = ctx.render_children(node);

        let mut r = row(children)
            .width(width)
            .height(height)
            .align_y(align_y)
            .clip(clip);

        if let Some(s) = spacing {
            r = r.spacing(s);
        }
        if let Some(p) = padding {
            r = r.padding(p);
        }

        let max_width =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "max_width");

        let elem: Element<'a, Message, Theme, R> = if prop_bool_default(props, "wrap", false) {
            r.wrap().into()
        } else {
            r.into()
        };

        // Row doesn't have max_width natively; wrap in a container to constrain it.
        let row_elem = if let Some(mw) = max_width {
            container(elem).max_width(mw).into()
        } else {
            elem
        };

        container(row_elem)
            .id(widget::Id::from(node.id.clone()))
            .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(RowWidget)
    }
}
