use iced::widget::{column, container};
use iced::{Element, Length, Theme, widget};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
        let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
        let spacing = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing");
        let padding = parse_padding_value(props);
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let align_x = prop_horizontal_alignment(props, "align_x");
        let clip = prop_bool_default(props, "clip", false);

        let children = ctx.render_children(node);

        let mut col = column(children)
            .width(width)
            .height(height)
            .align_x(align_x)
            .clip(clip);

        if let Some(s) = spacing {
            col = col.spacing(s);
        }
        if let Some(p) = padding {
            col = col.padding(p);
        }

        if let Some(mw) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "max_width")
        {
            col = col.max_width(mw);
        }

        let elem: Element<'a, Message, Theme, R> = if prop_bool_default(props, "wrap", false) {
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
