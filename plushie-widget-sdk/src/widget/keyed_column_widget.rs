use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use iced::widget::keyed;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{HorizontalAlignment, Length, Padding, PlushieType};

struct KeyedColumnProps {
    padding: Option<Padding>,
    width: Option<Length>,
    height: Option<Length>,
    max_width: Option<f32>,
    align_x: Option<HorizontalAlignment>,
}

impl KeyedColumnProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            max_width: f32::extract(p, "max_width"),
            align_x: HorizontalAlignment::extract(p, "align_x"),
        }
    }
}

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
        let kp = KeyedColumnProps::from_node(node);
        let spacing =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, &node.props, "spacing");
        let max_width = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            &node.props,
            "max_width",
        )
        .or(kp.max_width);

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

        let width = kp.width.as_ref().map(iced_convert::length).unwrap_or(iced::Length::Shrink);
        let height = kp.height.as_ref().map(iced_convert::length).unwrap_or(iced::Length::Shrink);

        let align_x = kp
            .align_x
            .map(iced_convert::horizontal_alignment)
            .map(iced::Alignment::from)
            .unwrap_or(iced::Alignment::Start);

        let mut kc = keyed::Column::with_children(keyed_children);
        kc = kc.width(width).height(height).align_items(align_x);

        if let Some(s) = spacing {
            kc = kc.spacing(s);
        }
        if let Some(ref p) = kp.padding {
            kc = kc.padding(iced_convert::padding(p));
        }
        if let Some(mw) = max_width {
            kc = kc.max_width(mw);
        }

        kc.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(KeyedColumnWidget)
    }
}
