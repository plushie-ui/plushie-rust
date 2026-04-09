//! Display widgets -- read-only visual content.
//!
//! - `text` -- single-style text with size, color, font, alignment, wrapping
//! - `rich_text` -- multi-span text with per-span styling, links, highlights
//! - `image` -- raster images from file paths or in-memory registry handles
//! - `svg` -- vector images with optional tint color
//! - `markdown` -- rendered markdown with syntax-highlighted code blocks
//! - `progress_bar` -- horizontal or vertical fill indicator
//! - `rule` -- horizontal or vertical divider line
//! - `space` -- invisible spacer for layout padding
//! - `qr_code` -- QR code rendered via canvas with configurable colors

use iced::widget::text::LineHeight;
use iced::widget::{Space, canvas, progress_bar, rich_text, rule, span, text};
use iced::{
    Color, Element, Font, Length, Padding, Pixels, Point, Radians, Rotation, Size, Theme, mouse,
};
use serde_json::Value;

use super::helpers::*;
use crate::PlushieRenderer;
use crate::extensions::RenderCtx;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::theming::parse_hex_color;

// ---------------------------------------------------------------------------
// Text
// ---------------------------------------------------------------------------

pub(crate) fn render_text<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let content = prop_str(props, "content").unwrap_or_default();
    let size = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "size")
        .or(ctx.default_text_size);

    let mut t = text(content);
    if let Some(s) = size {
        t = t.size(s);
    }
    let font = props
        .and_then(|p| p.get("font"))
        .map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        t = t.font(f);
    }
    if let Some(c) = props.and_then(|p| p.get("color")).and_then(parse_color) {
        t = t.color(c);
    }
    if let Some(w) = value_to_length_opt(props.and_then(|p| p.get("width"))) {
        t = t.width(w);
    }
    if let Some(h) = value_to_length_opt(props.and_then(|p| p.get("height"))) {
        t = t.height(h);
    }
    if let Some(lh) = parse_line_height(props) {
        t = t.line_height(lh);
    }
    if let Some(ax) = props
        .and_then(|p| p.get("align_x"))
        .and_then(|v| v.as_str())
        .and_then(value_to_horizontal_alignment)
    {
        t = t.align_x(ax);
    }
    if let Some(ay) = props
        .and_then(|p| p.get("align_y"))
        .and_then(|v| v.as_str())
        .and_then(value_to_vertical_alignment)
    {
        t = t.align_y(ay);
    }
    if let Some(w) = parse_wrapping(props) {
        t = t.wrapping(w);
    }
    if let Some(shaping) = parse_shaping(props) {
        t = t.shaping(shaping);
    }
    if let Some(e) = parse_ellipsis(props) {
        t = t.ellipsis(e);
    }

    // Named style
    if let Some(style_name) = prop_str(props, "style") {
        t = match style_name.as_str() {
            "primary" => t.style(text::primary),
            "secondary" => t.style(text::secondary),
            "success" => t.style(text::success),
            "danger" => t.style(text::danger),
            "warning" => t.style(text::warning),
            _ => {
                log::warn!(
                    "unknown style {:?} for widget type {:?}, using default",
                    style_name,
                    "text"
                );
                t.style(text::default)
            }
        };
    }

    t.into()
}

// ---------------------------------------------------------------------------
// Rich Text
// ---------------------------------------------------------------------------

/// Render a rich_text widget with per-span styling.
///
/// # Accessibility limitations
///
/// Rich text renders as multiple spans within a single iced `rich_text`
/// widget. The accessibility tree sees individual text fragments but has
/// no summary label for the composite. Hosts that need an accessible
/// name for the rich_text as a whole should set `a11y.label` on the
/// node. Link spans are rendered with iced's native link support, which
/// provides basic AT announcements but does not expose a separate
/// focusable link role per span.
pub(crate) fn render_rich_text<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let width = prop_length(props, "width", Length::Shrink);
    let height = prop_length(props, "height", Length::Shrink);

    // spans is an array of objects: {text, size, color, font, link}
    let spans_value = props
        .and_then(|p| p.get("spans"))
        .and_then(|v| v.as_array());

    let span_list: Vec<iced::widget::text::Span<'a, String, Font>> = spans_value
        .map(|arr| {
            arr.iter()
                .map(|sv| {
                    let content = sv
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_owned();
                    let mut s = span(content);
                    if let Some(sz) = sv.get("size").and_then(|v| v.as_f64()) {
                        s = s.size(Pixels(sz as f32));
                    }
                    if let Some(c) = sv.get("color").and_then(parse_color) {
                        s = s.color(c);
                    }
                    if let Some(f) = sv.get("font") {
                        s = s.font(parse_font(f));
                    }
                    if let Some(link) = sv.get("link").and_then(|v| v.as_str()) {
                        s = s.link(link.to_owned());
                    }
                    if let Some(true) = sv.get("underline").and_then(|v| v.as_bool()) {
                        s = s.underline(true);
                    }
                    if let Some(true) = sv.get("strikethrough").and_then(|v| v.as_bool()) {
                        s = s.strikethrough(true);
                    }
                    if let Some(lh) = sv.get("line_height").and_then(|v| v.as_f64()) {
                        s = s.line_height(LineHeight::Relative(lh as f32));
                    }
                    if let Some(p) = sv.get("padding") {
                        if let Some(n) = p.as_f64() {
                            s = s.padding(n as f32);
                        } else if let Some(obj) = p.as_object() {
                            let top = obj.get("top").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let right =
                                obj.get("right").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let bottom =
                                obj.get("bottom").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let left =
                                obj.get("left").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            s = s.padding(Padding {
                                top,
                                right,
                                bottom,
                                left,
                            });
                        }
                    }
                    if let Some(hl) = sv.get("highlight").and_then(|v| v.as_object()) {
                        if let Some(bg) = hl.get("background").and_then(parse_color) {
                            s = s.background(bg);
                        }
                        if let Some(b) = hl.get("border") {
                            s = s.border(parse_border(b));
                        }
                    }
                    s
                })
                .collect()
        })
        .unwrap_or_default();

    let id = node.id.clone();
    let mut rt = rich_text(span_list).width(width).height(height);

    if let Some(sz) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "size")
        .or(ctx.default_text_size)
    {
        rt = rt.size(sz);
    }
    let font = props
        .and_then(|p| p.get("font"))
        .map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        rt = rt.font(f);
    }
    if let Some(c) = props.and_then(|p| p.get("color")).and_then(parse_color) {
        rt = rt.color(c);
    }
    if let Some(lh) = parse_line_height(props) {
        rt = rt.line_height(lh);
    }
    if let Some(w) = parse_wrapping(props) {
        rt = rt.wrapping(w);
    }
    if let Some(e) = parse_ellipsis(props) {
        rt = rt.ellipsis(e);
    }

    let window_id = ctx.window_id.to_string();
    rt =
        rt.on_link_click(move |link| Message::Click(window_id.clone(), format!("{}:{}", id, link)));

    rt.into()
}

// ---------------------------------------------------------------------------
// Image
// ---------------------------------------------------------------------------

pub(crate) fn render_image<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    use iced::widget::Image;
    use iced::widget::image::FilterMethod;

    let props = node.props.as_object();
    let width = prop_length(props, "width", Length::Shrink);
    let height = prop_length(props, "height", Length::Shrink);
    let content_fit = prop_content_fit(props);

    // source can be a string (file path) or an object with a "handle" field
    // (in-memory image from the registry).
    let source_val = props.and_then(|p| p.get("source"));
    if source_val.is_none() {
        log::warn!("[id={}] image: no 'source' prop specified", node.id);
    }
    let handle: iced::widget::image::Handle = match source_val {
        Some(Value::Object(obj)) => {
            if let Some(name) = obj.get("handle").and_then(|v| v.as_str()) {
                match ctx.images.get(name) {
                    Some(h) => h.clone(),
                    None => {
                        log::warn!("[id={}] image: unknown registry handle: {name}", node.id);
                        iced::widget::image::Handle::from_bytes(vec![])
                    }
                }
            } else {
                iced::widget::image::Handle::from_bytes(vec![])
            }
        }
        _ => {
            let path = prop_str(props, "source").unwrap_or_default();
            iced::widget::image::Handle::from_path(path)
        }
    };

    let mut img = Image::new(handle).width(width).height(height);
    if let Some(cf) = content_fit {
        img = img.content_fit(cf);
    }
    if let Some(r) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "rotation")
    {
        img = img.rotation(Rotation::from(Radians(r.to_radians())));
    }
    if let Some(o) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "opacity") {
        img = img.opacity(o);
    }
    if let Some(br) = prop_animated_f32(
        &ctx.caches.interpolated_props,
        &node.id,
        props,
        "border_radius",
    ) {
        img = img.border_radius(br);
    }
    if let Some(fm_str) = prop_str(props, "filter_method") {
        let fm = match fm_str.to_ascii_lowercase().as_str() {
            "nearest" => FilterMethod::Nearest,
            _ => FilterMethod::Linear,
        };
        img = img.filter_method(fm);
    }
    if let Some(expand) = prop_bool(props, "expand") {
        img = img.expand(expand);
    }
    if let Some(scale) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "scale")
    {
        img = img.scale(scale);
    }
    if let Some(alt) = prop_str(props, "alt") {
        img = img.alt(alt);
    }
    if let Some(desc) = prop_str(props, "description") {
        img = img.description(desc);
    }
    if prop_bool_default(props, "decorative", false) {
        img = img.decorative();
    }
    // crop: {"x": u32, "y": u32, "width": u32, "height": u32}
    if let Some(crop_obj) = props
        .and_then(|p| p.get("crop"))
        .and_then(|v| v.as_object())
    {
        let cx = crop_obj.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let cy = crop_obj.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let cw = crop_obj.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let ch = crop_obj.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        img = img.crop(iced::Rectangle {
            x: cx,
            y: cy,
            width: cw,
            height: ch,
        });
    }

    img.into()
}

// ---------------------------------------------------------------------------
// SVG
// ---------------------------------------------------------------------------

/// Render an SVG widget.
///
/// Both windowed (wgpu) and headless (tiny-skia) backends use resvg for
/// SVG rasterization, so output should be pixel-identical at the same
/// resolution. However, text within SVGs may render differently across
/// platforms due to font availability (fontconfig on Linux, CoreText on
/// macOS, DirectWrite on Windows). For deterministic headless screenshots,
/// ensure SVG text uses fonts available on the CI platform or embed fonts
/// in the SVG.
pub(crate) fn render_svg<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    use iced::widget::Svg;

    let props = node.props.as_object();
    let source = prop_str(props, "source").unwrap_or_default();
    if source.is_empty() {
        log::warn!("[id={}] svg: no 'source' prop specified", node.id);
    }
    let width = prop_length(props, "width", Length::Shrink);
    let height = prop_length(props, "height", Length::Shrink);
    let content_fit = prop_content_fit(props);

    let mut s = Svg::from_path(source).width(width).height(height);
    if let Some(cf) = content_fit {
        s = s.content_fit(cf);
    }
    if let Some(r) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "rotation")
    {
        s = s.rotation(Rotation::from(Radians(r.to_radians())));
    }
    if let Some(o) = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "opacity") {
        s = s.opacity(o);
    }
    if let Some(alt) = prop_str(props, "alt") {
        s = s.alt(alt);
    }
    if let Some(desc) = prop_str(props, "description") {
        s = s.description(desc);
    }
    if prop_bool_default(props, "decorative", false) {
        s = s.decorative();
    }
    if let Some(color_str) = prop_str(props, "color")
        && let Some(c) = crate::theming::parse_hex_color(&color_str)
    {
        s = s.style(move |_theme, _status| iced::widget::svg::Style { color: Some(c) });
    }

    s.into()
}

// ---------------------------------------------------------------------------
// Progress Bar
// ---------------------------------------------------------------------------

pub(crate) fn render_progress_bar<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let range = prop_range_f32(props);
    let value = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "value")
        .unwrap_or(0.0)
        .clamp(*range.start(), *range.end());
    let width = prop_length(props, "width", Length::Fill);
    let height = prop_length(props, "height", Length::Shrink);

    let mut pb = progress_bar(range, value).length(width).girth(height);

    if prop_bool_default(props, "vertical", false) {
        pb = pb.vertical();
    }
    if let Some(label) = prop_str(props, "label") {
        pb = pb.label(label);
    }

    // Style: string name or style map object
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            pb = match style_name {
                "primary" => pb.style(progress_bar::primary),
                "secondary" => pb.style(progress_bar::secondary),
                "success" => pb.style(progress_bar::success),
                "danger" => pb.style(progress_bar::danger),
                "warning" => pb.style(progress_bar::warning),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "progress_bar"
                    );
                    pb.style(progress_bar::primary)
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            pb = pb.style(move |theme: &iced::Theme| {
                let mut style = match ov.preset_base.as_deref() {
                    Some("primary") => progress_bar::primary(theme),
                    Some("secondary") => progress_bar::secondary(theme),
                    Some("success") => progress_bar::success(theme),
                    Some("danger") => progress_bar::danger(theme),
                    Some("warning") => progress_bar::warning(theme),
                    _ => progress_bar::primary(theme),
                };
                apply_progress_bar_fields(&mut style, &ov.base);
                style
            });
        }
    }

    pb.into()
}

// ---------------------------------------------------------------------------
// Rule (horizontal/vertical divider)
// ---------------------------------------------------------------------------

pub(crate) fn render_rule<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let direction = prop_str(props, "direction").unwrap_or_default();

    // Thickness is the cross-axis dimension:
    // horizontal rule -> height, vertical rule -> width.
    // "thickness" is a universal alias for either.
    let thickness = if direction == "vertical" {
        prop_f32(props, "width")
    } else {
        prop_f32(props, "height")
    }
    .or_else(|| prop_f32(props, "thickness"))
    .unwrap_or(1.0);

    let mut r = if direction == "vertical" {
        rule::vertical(thickness)
    } else {
        rule::horizontal(thickness)
    };
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
        if let Some(style_name) = style_val.as_str() {
            r = match style_name {
                "default" => r.style(rule::default),
                "weak" => r.style(rule::weak),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "rule"
                    );
                    r
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            r = r.style(move |theme: &iced::Theme| {
                let base_fn: fn(&iced::Theme) -> rule::Style = match ov.preset_base.as_deref() {
                    Some("default") => rule::default,
                    Some("weak") => rule::weak,
                    _ => rule::default,
                };
                apply_rule_style(base_fn(theme), &ov.base)
            });
        }
    }
    r.into()
}

// ---------------------------------------------------------------------------
// Space
// ---------------------------------------------------------------------------

pub(crate) fn render_space<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    _ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let width = prop_length(props, "width", Length::Shrink);
    let height = prop_length(props, "height", Length::Shrink);
    Space::new().width(width).height(height).into()
}

// ---------------------------------------------------------------------------
// QR Code
// ---------------------------------------------------------------------------

struct QrCodeProgram<'a, R: PlushieRenderer = iced::Renderer> {
    modules: Vec<Vec<bool>>,
    cell_size: f32,
    cell_color: Color,
    background: Color,
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

/// Render a qr_code with the provided cache entry.
/// Called by QrCodeWidget::render() with factory-owned cache.
pub(crate) fn render_qr_code_with_cache<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    cache_entry: Option<&'a (u64, canvas::Cache<R>)>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let data = prop_str(props, "data").unwrap_or_default();
    let cell_size = prop_f32(props, "cell_size").unwrap_or(4.0).clamp(1.0, 50.0);
    let ec_str = prop_str(props, "error_correction").unwrap_or_default();
    let cell_color = prop_str(props, "cell_color")
        .and_then(|s| parse_hex_color(&s))
        .unwrap_or(Color::BLACK);
    let background = prop_str(props, "background")
        .and_then(|s| parse_hex_color(&s))
        .unwrap_or(Color::WHITE);

    let ec_level = match ec_str.as_str() {
        "low" => qrcode::EcLevel::L,
        "quartile" => qrcode::EcLevel::Q,
        "high" => qrcode::EcLevel::H,
        _ => qrcode::EcLevel::M,
    };

    let qr = match qrcode::QrCode::with_error_correction_level(data.as_bytes(), ec_level) {
        Ok(qr) => qr,
        Err(e) => {
            log::warn!("[id={}] qr_code: failed to encode data: {e}", node.id);
            return text(format!("QR code error: {e}")).into();
        }
    };

    let width = qr.width();
    let modules: Vec<Vec<bool>> = (0..width)
        .map(|y| {
            (0..width)
                .map(|x| qr[(x, y)] == qrcode::types::Color::Dark)
                .collect()
        })
        .collect();

    let pixel_size = width as f32 * cell_size;

    let mut qr_canvas = iced::widget::Canvas::<_, Message, iced::Theme, R>::new(QrCodeProgram {
        modules,
        cell_size,
        cell_color,
        background,
        cache: cache_entry,
    })
    .width(Length::Fixed(pixel_size))
    .height(Length::Fixed(pixel_size));

    if let Some(alt) = prop_str(props, "alt") {
        qr_canvas = qr_canvas.alt(alt);
    }
    if let Some(desc) = prop_str(props, "description") {
        qr_canvas = qr_canvas.description(desc);
    }

    qr_canvas.into()
}

// ---------------------------------------------------------------------------
// Cache ensure functions
// ---------------------------------------------------------------------------
