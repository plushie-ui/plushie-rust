use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use iced::widget::keyed;
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widgets::helpers::*;

pub(crate) struct KeyedColumnWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for KeyedColumnWidget {
    fn type_names(&self) -> &[&str] {
        &["keyed_column"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let spacing =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing");
        let padding = parse_padding_value(props);
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);

        let keyed_children: Vec<(u64, Element<'a, Message, Theme, R>)> = node
            .children
            .iter()
            .map(|c| {
                let mut hasher = DefaultHasher::new();
                c.id.hash(&mut hasher);
                let key = hasher.finish();
                let elem = ctx.render_child(c);
                (key, elem)
            })
            .collect();

        let mut kc = keyed::Column::with_children(keyed_children);
        kc = kc.width(width).height(height);

        if let Some(s) = spacing {
            kc = kc.spacing(s);
        }
        if let Some(p) = padding {
            kc = kc.padding(p);
        }

        if let Some(mw) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "max_width")
        {
            kc = kc.max_width(mw);
        }

        kc.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(KeyedColumnWidget)
    }
}
