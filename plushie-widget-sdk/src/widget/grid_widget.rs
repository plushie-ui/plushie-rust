use iced::widget::grid;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{Length, PlushieType};

struct GridProps {
    columns: Option<u32>,
    width: Option<f32>,
    height: Option<f32>,
    column_width: Option<Length>,
    row_height: Option<Length>,
    fluid: Option<f32>,
}

impl GridProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            columns: u32::extract(p, "columns"),
            width: f32::extract(p, "width"),
            height: f32::extract(p, "height"),
            column_width: Length::extract(p, "column_width"),
            row_height: Length::extract(p, "row_height"),
            fluid: f32::extract(p, "fluid"),
        }
    }
}

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
        let gp = GridProps::from_node(node);
        let spacing =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, &node.props, "spacing");

        let cols = gp.columns.unwrap_or(1) as usize;

        let children = ctx.render_children(node);

        let mut g = grid(children).columns(cols);

        if let Some(s) = spacing {
            g = g.spacing(s);
        }

        // Legacy pixel-only width/height props
        if let Some(w) = gp.width {
            g = g.width(w);
        }
        if let Some(h) = gp.height {
            g = g.height(h);
        }

        // Length-typed column_width: only Fixed maps to Pixels for iced's Grid::width
        if let Some(ref cw) = gp.column_width {
            if let Length::Fixed(px) = *cw {
                g = g.width(px);
            }
        }

        // Length-typed row_height: maps to Grid::height via Sizing::EvenlyDistribute
        if let Some(ref rh) = gp.row_height {
            g = g.height(iced_convert::length(rh));
        }

        // Fluid mode: auto-wrap columns with a max cell width
        if let Some(max_w) = gp.fluid {
            g = g.fluid(max_w);
        }

        g.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(GridWidget)
    }
}
