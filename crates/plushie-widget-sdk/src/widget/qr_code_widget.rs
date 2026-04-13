use iced::widget::canvas;
use iced::{Element, Length, Point, Size, Theme, mouse};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;

use plushie_core::types::{ErrorCorrection, PlushieType};

// ---------------------------------------------------------------------------
// QrCodeProgram (canvas program for drawing QR modules)
// ---------------------------------------------------------------------------

struct QrCodeProgram<'a, R: PlushieRenderer = iced::Renderer> {
    modules: Vec<Vec<bool>>,
    cell_size: f32,
    cell_color: iced::Color,
    background: iced::Color,
    cache: Option<&'a (u64, canvas::Cache<R>)>,
}

impl<R: PlushieRenderer> canvas::Program<Message, iced::Theme, R> for QrCodeProgram<'_, R> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &R,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<R>> {
        let draw_fn = |frame: &mut canvas::Frame<R>| {
            // Fill background
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.background);
            // Draw each dark module as a filled square
            for (row_idx, row) in self.modules.iter().enumerate() {
                for (col_idx, &dark) in row.iter().enumerate() {
                    if dark {
                        let x = col_idx as f32 * self.cell_size;
                        let y = row_idx as f32 * self.cell_size;
                        frame.fill_rectangle(
                            Point::new(x, y),
                            Size::new(self.cell_size, self.cell_size),
                            self.cell_color,
                        );
                    }
                }
            }
        };

        if let Some((_hash, cache)) = self.cache {
            vec![cache.draw(renderer, bounds.size(), draw_fn)]
        } else {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            draw_fn(&mut frame);
            vec![frame.into_geometry()]
        }
    }
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

struct QrCodeProps {
    data: Option<String>,
    cell_size: Option<f32>,
    total_size: Option<f32>,
    cell_color: Option<plushie_core::types::Color>,
    background: Option<plushie_core::types::Color>,
    error_correction: Option<ErrorCorrection>,
    alt: Option<String>,
    description: Option<String>,
}

impl QrCodeProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            data: String::extract(p, "data"),
            cell_size: f32::extract(p, "cell_size"),
            total_size: f32::extract(p, "total_size"),
            cell_color: plushie_core::types::Color::extract(p, "cell_color"),
            background: plushie_core::types::Color::extract(p, "background"),
            error_correction: ErrorCorrection::extract(p, "error_correction"),
            alt: String::extract(p, "alt"),
            description: String::extract(p, "description"),
        }
    }
}

// ---------------------------------------------------------------------------
// QrCodeWidget (stateful, owns R-generic canvas::Cache)
// ---------------------------------------------------------------------------

/// Stateful QR code factory (owns R-generic `canvas::Cache`).
pub(crate) struct QrCodeWidget<R: PlushieRenderer> {
    /// Per-qr_code cache with content hash for invalidation.
    /// Keyed by (window_id, node_id).
    caches: std::collections::HashMap<(String, String), (u64, canvas::Cache<R>)>,
}

impl<R: PlushieRenderer> QrCodeWidget<R> {
    pub(crate) fn new() -> Self {
        Self {
            caches: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for QrCodeWidget<R> {
    fn type_names(&self) -> &[&str] {
        &["qr_code"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let key = (window_id.to_string(), node.id.clone());
        let qp = QrCodeProps::from_node(node);
        let data = qp.data.unwrap_or_default();
        let cell_size = qp.cell_size.unwrap_or(4.0);
        let total_size = qp.total_size;
        let ec = qp.error_correction;

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        cell_size.to_bits().hash(&mut hasher);
        total_size.map(|ts| ts.to_bits()).hash(&mut hasher);
        ec.hash(&mut hasher);
        let hash = hasher.finish();

        match self.caches.get_mut(&key) {
            Some((existing_hash, cache)) => {
                if *existing_hash != hash {
                    cache.clear();
                    *existing_hash = hash;
                }
            }
            None => {
                self.caches.insert(key, (hash, canvas::Cache::new()));
            }
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let qp = QrCodeProps::from_node(node);
        let data = qp.data.unwrap_or_default();
        let ec = qp.error_correction;
        let cell_color = qp
            .cell_color
            .as_ref()
            .map(iced_convert::color)
            .unwrap_or(iced::Color::BLACK);
        let background = qp
            .background
            .as_ref()
            .map(iced_convert::color)
            .unwrap_or(iced::Color::WHITE);

        let ec_level = match ec {
            Some(ErrorCorrection::Low) => qrcode::EcLevel::L,
            Some(ErrorCorrection::Quartile) => qrcode::EcLevel::Q,
            Some(ErrorCorrection::High) => qrcode::EcLevel::H,
            Some(ErrorCorrection::Medium) | None => qrcode::EcLevel::M,
        };

        let qr = match qrcode::QrCode::with_error_correction_level(data.as_bytes(), ec_level) {
            Ok(qr) => qr,
            Err(e) => {
                log::warn!("[id={}] qr_code: failed to encode data: {e}", node.id);
                return iced::widget::text(format!("QR code error: {e}")).into();
            }
        };

        let width = qr.width();

        // Derive cell_size: explicit cell_size wins, then total_size, then default.
        let cell_size = if let Some(cs) = qp.cell_size {
            cs.clamp(1.0, 50.0)
        } else if let Some(ts) = qp.total_size {
            (ts / width as f32).clamp(1.0, 50.0)
        } else {
            4.0
        };

        let modules: Vec<Vec<bool>> = (0..width)
            .map(|y| {
                (0..width)
                    .map(|x| qr[(x, y)] == qrcode::types::Color::Dark)
                    .collect()
            })
            .collect();

        let pixel_size = width as f32 * cell_size;

        let key = (ctx.window_id.to_string(), node.id.clone());
        let cache_entry = self.caches.get(&key);

        let mut qr_canvas =
            iced::widget::Canvas::<_, Message, iced::Theme, R>::new(QrCodeProgram {
                modules,
                cell_size,
                cell_color,
                background,
                cache: cache_entry,
            })
            .width(Length::Fixed(pixel_size))
            .height(Length::Fixed(pixel_size));

        if let Some(alt) = qp.alt {
            qr_canvas = qr_canvas.alt(alt);
        }
        if let Some(desc) = qp.description {
            qr_canvas = qr_canvas.description(desc);
        }

        qr_canvas.into()
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.caches
            .remove(&(window_id.to_string(), node_id.to_string()));
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(QrCodeWidget::new())
    }
}
