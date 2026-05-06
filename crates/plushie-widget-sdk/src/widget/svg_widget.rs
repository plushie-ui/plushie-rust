use iced::widget::Svg;
use iced::{Element, Radians, Rotation, Theme};

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::svg_guard::{self, DecodeOutcome};
use crate::widget::helpers::*;

use parking_lot::Mutex;
use plushie_core::types::{Color, ContentFit, Length, PlushieType};
use std::collections::HashMap;

/// Paths that failed the SVG guard. Re-validation is skipped for
/// known-bad paths so the guard doesn't burn CPU every frame on a
/// stuck or malicious source. parking_lot::Mutex avoids the
/// poisoning surface of std::sync::Mutex; the cache is throwaway
/// hot-path memoisation, not state we need to fence on a panic.
/// Capped at [`MAX_FAILED_PATHS`] so a churning unique-source stream
/// cannot grow the map without bound; past the cap, new failures are
/// not recorded and the worst case is wasted CPU on retry, not memory.
static FAILED_PATHS: Mutex<Option<HashMap<String, DecodeFailure>>> = Mutex::new(None);

/// Cap on the failed-path memoisation map.
const MAX_FAILED_PATHS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodeFailure {
    ParseError,
    Timeout,
}

struct SvgProps {
    width: Option<Length>,
    height: Option<Length>,
    content_fit: Option<ContentFit>,
    color: Option<Color>,
    alt: Option<String>,
    description: Option<String>,
}

impl SvgProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            content_fit: ContentFit::extract(p, "content_fit"),
            color: Color::extract(p, "color"),
            alt: String::extract(p, "alt"),
            description: String::extract(p, "description"),
        }
    }
}

pub(crate) struct SvgWidget;

impl SvgWidget {
    /// Pre-validate the SVG source under the decode guard. Caches
    /// known-bad paths so subsequent frames skip the work.
    fn guard_source(node_id: &str, source: &str) {
        if source.is_empty() {
            return;
        }
        // Fast path: already known-bad; skip re-parse.
        {
            let guard = FAILED_PATHS.lock();
            if let Some(map) = guard.as_ref()
                && map.contains_key(source)
            {
                return;
            }
        }
        let bytes = match std::fs::read_to_string(source) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[id={node_id}] svg: failed to read '{source}': {e}");
                return;
            }
        };
        // Interactive deadline: the rendering loop wants to tick
        // well within a frame budget. Headless callers bypass this
        // guard today because iced's headless path doesn't go
        // through SvgWidget::render.
        let deadline = svg_guard::INTERACTIVE_TIMEOUT;
        match svg_guard::parse_with_timeout(bytes, deadline) {
            DecodeOutcome::Ok => {}
            DecodeOutcome::ParseError(msg) => {
                crate::diagnostics::warn(plushie_core::Diagnostic::SvgParseError {
                    id: node_id.to_string(),
                    source: source.to_string(),
                    detail: msg,
                });
                Self::record_failure(source, DecodeFailure::ParseError);
            }
            DecodeOutcome::Timeout => {
                crate::diagnostics::warn(plushie_core::Diagnostic::SvgDecodeTimeout {
                    id: node_id.to_string(),
                    source: source.to_string(),
                    deadline_debug: format!("{deadline:?}"),
                });
                Self::record_failure(source, DecodeFailure::Timeout);
            }
        }
    }

    fn record_failure(source: &str, kind: DecodeFailure) {
        let mut guard = FAILED_PATHS.lock();
        let map = guard.get_or_insert_with(HashMap::new);
        if map.contains_key(source) || map.len() < MAX_FAILED_PATHS {
            map.insert(source.to_string(), kind);
        }
    }

    fn is_failed(source: &str) -> bool {
        let guard = FAILED_PATHS.lock();
        guard.as_ref().is_some_and(|m| m.contains_key(source))
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for SvgWidget {
    fn type_names(&self) -> &[&str] {
        &["svg"]
    }

    fn prepare(&mut self, node: &TreeNode, _window_id: &str, _theme: &Theme) {
        let source = prop_str(&node.props, "source").unwrap_or_default();
        Self::guard_source(&node.id, &source);
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let sp = SvgProps::from_node(node);
        let props = &node.props;

        // source: kept as raw prop access (file path string)
        let source = prop_str(props, "source").unwrap_or_default();
        if source.is_empty() {
            log::warn!("[id={}] svg: no 'source' prop specified", node.id);
        }

        // If the guard marked this source as bad on a previous
        // frame, render an empty placeholder instead of asking
        // iced to re-attempt the decode.
        if !source.is_empty() && Self::is_failed(&source) {
            return iced::widget::Space::new().into();
        }

        let width = sp
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = sp
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);

        let mut s = Svg::from_path(source).width(width).height(height);
        if let Some(cf) = sp.content_fit {
            s = s.content_fit(iced_convert::content_fit(cf));
        }
        if let Some(r) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "rotation")
        {
            s = s.rotation(Rotation::from(Radians(r.to_radians())));
        }
        if let Some(o) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "opacity")
        {
            s = s.opacity(o);
        }
        if let Some(alt) = sp.alt {
            s = s.alt(alt);
        }
        if let Some(desc) = sp.description {
            s = s.description(desc);
        }
        if prop_bool_default(props, "decorative", false) {
            s = s.decorative();
        }
        if let Some(ref c) = sp.color {
            let ic = iced_convert::color(c);
            s = s.style(move |_theme, _status| iced::widget::svg::Style { color: Some(ic) });
        }

        s.into()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SvgWidget)
    }
}
