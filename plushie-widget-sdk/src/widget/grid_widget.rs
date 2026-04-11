use iced::widget::grid;
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

pub(crate) struct GridWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for GridWidget {
    fn type_names(&self) -> &[&str] {
        &["grid"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = &node.props;
        let cols = props
            .get_value("columns")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let spacing = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing");

        let column_width = prop_length(props, "column_width", Length::Shrink);
        let row_height = prop_length(props, "row_height", Length::Shrink);

        let children = ctx.render_children(node);

        let mut g = grid(children).columns(cols);

        if let Some(s) = spacing {
            g = g.spacing(s);
        }

        // Legacy pixel-only width/height props
        if let Some(w) = prop_f32(props, "width") {
            g = g.width(w);
        }
        if let Some(h) = prop_f32(props, "height") {
            g = g.height(h);
        }

        // Length-typed column_width: only Fixed maps to Pixels for iced's Grid::width
        if props.get_value("column_width").is_some()
            && let Length::Fixed(px) = column_width
        {
            g = g.width(px);
        }

        // Length-typed row_height: maps to Grid::height via Sizing::EvenlyDistribute
        if props.get_value("row_height").is_some() {
            g = g.height(row_height);
        }

        // Fluid mode: auto-wrap columns with a max cell width
        if let Some(max_w) = prop_f32(props, "fluid") {
            g = g.fluid(max_w);
        }

        g.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(GridWidget)
    }
}
