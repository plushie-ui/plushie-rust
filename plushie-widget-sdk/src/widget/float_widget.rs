use iced::widget::Space;
use iced::{Element, Theme, Vector};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct FloatWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for FloatWidget {
    fn type_names(&self) -> &[&str] {
        &["float"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();

        let child: Element<'a, Message, Theme, R> = node
            .children
            .first()
            .map(|c| ctx.render_child(c))
            .unwrap_or_else(|| Space::new().into());

        let tx = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            props,
            "translate_x",
        )
        .unwrap_or(0.0);
        let ty = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            props,
            "translate_y",
        )
        .unwrap_or(0.0);

        let mut f =
            iced::widget::float(child).translate(move |_content, _viewport| Vector::new(tx, ty));

        if let Some(s) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "scale")
        {
            f = f.scale(s);
        }

        f.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(FloatWidget)
    }
}
