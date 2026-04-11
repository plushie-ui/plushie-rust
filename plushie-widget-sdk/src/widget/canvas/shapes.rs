//! Canvas shape drawing, fill/stroke parsing, and path building.

use iced::widget::canvas;
use iced::{Color, Pixels, Point, Radians, Size, Vector, alignment};
use serde_json::Value;

use plushie_core::types::canvas::{
    self as canvas_types, CanvasShape, ClipRect, Transform,
};

use super::types::MAX_SHAPES_PER_LAYER;
use crate::PlushieRenderer;
use crate::iced_convert;
use crate::widget::helpers::*;

/// Parse a `fill_rule` string into a `canvas::fill::Rule`. Defaults to `NonZero`.
pub(super) fn parse_fill_rule(value: Option<&Value>) -> canvas::fill::Rule {
    match value.and_then(|v| v.as_str()) {
        Some("even_odd") => canvas::fill::Rule::EvenOdd,
        _ => canvas::fill::Rule::NonZero,
    }
}

/// Parse a canvas fill value. If string, hex color. If gradient object,
/// build a gradient::Linear. Falls back to white. The `shape` parameter
/// provides the parent shape object for reading the `fill_rule` key.
#[allow(dead_code)] // used by tests
pub(crate) fn parse_canvas_fill(value: &Value, shape: &Value) -> canvas::Fill {
    parse_canvas_fill_themed(value, shape, None)
}

/// Parse a canvas fill with optional theme-aware color resolution.
///
/// When `theme` is `Some`, color strings that match palette names
/// (`"primary"`, `"text"`, `"background"`, `"success"`, `"danger"`,
/// `"warning"`) are resolved against the theme instead of being
/// treated as hex strings.
pub(super) fn parse_canvas_fill_themed(
    value: &Value,
    shape: &Value,
    theme: Option<&iced::Theme>,
) -> canvas::Fill {
    let rule = parse_fill_rule(shape.get("fill_rule"));
    match value {
        Value::String(_) => {
            let color = theme
                .and_then(|t| resolve_color(value, t))
                .or_else(|| parse_color(value))
                .unwrap_or(Color::WHITE);
            canvas::Fill {
                style: canvas::Style::Solid(color),
                rule,
            }
        }
        Value::Object(obj) => match obj.get("type").and_then(|v| v.as_str()) {
            Some("linear") => {
                // Warn on unrecognized canvas gradient keys
                let valid_keys: &[&str] = &["type", "start", "end", "stops"];
                for key in obj.keys() {
                    if !valid_keys.contains(&key.as_str()) {
                        log::warn!(
                            "unrecognized canvas gradient key '{}' (valid: {:?})",
                            key,
                            valid_keys
                        );
                    }
                }

                let start = obj
                    .get("start")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        Point::new(
                            a.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                            a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        )
                    })
                    .unwrap_or(Point::ORIGIN);
                let end = obj
                    .get("end")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        Point::new(
                            a.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                            a.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                        )
                    })
                    .unwrap_or(Point::ORIGIN);
                let mut linear = canvas::gradient::Linear::new(start, end);
                if let Some(stops) = obj.get("stops").and_then(|v| v.as_array()) {
                    for stop in stops {
                        if let Some(arr) = stop.as_array() {
                            let offset = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                            let color = arr
                                .get(1)
                                .and_then(|v| {
                                    theme
                                        .and_then(|t| resolve_color(v, t))
                                        .or_else(|| parse_color(v))
                                })
                                .unwrap_or(Color::TRANSPARENT);
                            linear = linear.add_stop(offset, color);
                        }
                    }
                }
                canvas::Fill {
                    style: canvas::Style::Gradient(canvas::Gradient::Linear(linear)),
                    rule,
                }
            }
            Some(other) => {
                log::warn!(
                    "unrecognized canvas gradient type '{}' (supported: \"linear\")",
                    other
                );
                let color = parse_color(value).unwrap_or(Color::WHITE);
                canvas::Fill {
                    style: canvas::Style::Solid(color),
                    rule,
                }
            }
            _ => {
                let color = parse_color(value).unwrap_or(Color::WHITE);
                canvas::Fill {
                    style: canvas::Style::Solid(color),
                    rule,
                }
            }
        },
        _ => canvas::Fill {
            style: canvas::Style::Solid(Color::WHITE),
            rule,
        },
    }
}

/// Parse a canvas stroke from a JSON object.
#[allow(dead_code)] // used by tests
pub(crate) fn parse_canvas_stroke(value: &Value) -> canvas::Stroke<'static> {
    parse_canvas_stroke_themed(value, None)
}

/// Parse a canvas stroke with optional theme-aware color resolution.
pub(super) fn parse_canvas_stroke_themed(
    value: &Value,
    theme: Option<&iced::Theme>,
) -> canvas::Stroke<'static> {
    let obj = match value.as_object() {
        Some(o) => o,
        None => return canvas::Stroke::default(),
    };
    let color = theme
        .and_then(|t| obj.get("color").and_then(|v| resolve_color(v, t)))
        .or_else(|| obj.get("color").and_then(parse_color))
        .unwrap_or(Color::WHITE);
    let width = obj
        .get("width")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(1.0);
    let cap = match obj.get("cap").and_then(|v| v.as_str()).unwrap_or("butt") {
        "round" => canvas::LineCap::Round,
        "square" => canvas::LineCap::Square,
        _ => canvas::LineCap::Butt,
    };
    let join = match obj.get("join").and_then(|v| v.as_str()).unwrap_or("miter") {
        "round" => canvas::LineJoin::Round,
        "bevel" => canvas::LineJoin::Bevel,
        _ => canvas::LineJoin::Miter,
    };
    let mut stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(width)
        .with_line_cap(cap)
        .with_line_join(join);
    if let Some(dash_obj) = obj.get("dash").and_then(|v| v.as_object()) {
        let segments_val = dash_obj
            .get("segments")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let segments: Vec<f32> = segments_val
            .iter()
            .filter_map(|v| v.as_f64().map(|n| n as f32))
            .collect();
        let offset = dash_obj
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0);
        // LineDash borrows segments, but we need 'static. Intern via a
        // global cache so identical patterns reuse the same allocation and
        // we only leak once per unique dash pattern (not per render).
        let segments: &'static [f32] = intern_dash_segments(segments);
        stroke.line_dash = canvas::LineDash { segments, offset };
    }
    stroke
}

/// Maximum number of unique dash patterns cached. Beyond this limit,
/// new patterns are still leaked (LineDash requires `'static` segments)
/// but not inserted into the cache, bounding the HashMap's memory.
const MAX_DASH_CACHE: usize = 1024;

/// Intern a dash segment array so that identical patterns share one
/// leaked allocation. Without this, every re-render of a dashed stroke
/// leaked a fresh `Box<[f32]>` via `Box::leak`.
///
/// When the cache reaches [`MAX_DASH_CACHE`] entries, new unique
/// patterns still get a leaked slice (LineDash requires `'static`
/// segments) but are not inserted into the cache. A one-time warning
/// is logged when this limit is hit.
fn intern_dash_segments(segments: Vec<f32>) -> &'static [f32] {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{LazyLock, Mutex};

    static CACHE: LazyLock<Mutex<HashMap<Vec<u32>, &'static [f32]>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));
    static WARNED: AtomicBool = AtomicBool::new(false);

    let key: Vec<u32> = segments.iter().map(|s| s.to_bits()).collect();
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());

    if let Some(existing) = cache.get(&key) {
        return existing;
    }

    let leaked: &'static [f32] = Box::leak(segments.into_boxed_slice());

    if cache.len() >= MAX_DASH_CACHE {
        if !WARNED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "dash segment cache full ({MAX_DASH_CACHE} entries); \
                 new patterns will leak without caching"
            );
        }
        return leaked;
    }

    cache.insert(key, leaked);
    leaked
}

/// Build a Path from an array of path commands.
pub(super) fn build_path_from_commands(commands: &[Value]) -> canvas::Path {
    canvas::Path::new(|builder| {
        for cmd in commands {
            if let Some(s) = cmd.as_str() {
                if s == "close" {
                    builder.close();
                }
                continue;
            }
            let arr = match cmd.as_array() {
                Some(a) if !a.is_empty() => a,
                _ => continue,
            };
            let cmd_name = arr[0].as_str().unwrap_or("");
            let f = |i: usize| -> f32 {
                arr.get(i)
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32)
                    .unwrap_or(0.0)
            };
            match cmd_name {
                "move_to" => builder.move_to(Point::new(f(1), f(2))),
                "line_to" => builder.line_to(Point::new(f(1), f(2))),
                "bezier_to" => builder.bezier_curve_to(
                    Point::new(f(1), f(2)),
                    Point::new(f(3), f(4)),
                    Point::new(f(5), f(6)),
                ),
                "quadratic_to" => {
                    builder.quadratic_curve_to(Point::new(f(1), f(2)), Point::new(f(3), f(4)))
                }
                "arc" => {
                    builder.arc(canvas::path::Arc {
                        center: Point::new(f(1), f(2)),
                        radius: f(3),
                        start_angle: Radians(f(4)),
                        end_angle: Radians(f(5)),
                    });
                }
                "arc_to" => {
                    builder.arc_to(Point::new(f(1), f(2)), Point::new(f(3), f(4)), f(5));
                }
                "ellipse" => {
                    builder.ellipse(canvas::path::arc::Elliptical {
                        center: Point::new(f(1), f(2)),
                        radii: Vector::new(f(3), f(4)),
                        rotation: Radians(f(5)),
                        start_angle: Radians(f(6)),
                        end_angle: Radians(f(7)),
                    });
                }
                "rounded_rect" => {
                    builder.rounded_rectangle(
                        Point::new(f(1), f(2)),
                        Size::new(f(3), f(4)),
                        iced::border::Radius::new(f(5)),
                    );
                }
                _ => {}
            }
        }
    })
}

/// Draw a sequence of shapes.
///
/// Clips and transforms are handled at the group level -- each group
/// carries its own transforms and clip fields, applied in
/// [`draw_canvas_shape`] when rendering groups.
pub(super) fn draw_canvas_shapes<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    shapes: &[&CanvasShape],
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
) {
    for shape in shapes {
        draw_canvas_shape(frame, shape, images, theme);
    }
}

/// Apply per-shape opacity to a `canvas::Fill`. Multiplies the opacity
/// into solid color alpha. Gradient stops are left unchanged (the host
/// should bake opacity into gradient stop colors if needed).
pub(super) fn apply_opacity_to_fill(opacity: Option<f32>, mut fill: canvas::Fill) -> canvas::Fill {
    if let Some(a) = opacity {
        if let canvas::Style::Solid(ref mut c) = fill.style {
            c.a *= a;
        }
    }
    fill
}

/// Apply per-shape opacity to a `canvas::Stroke`.
pub(super) fn apply_opacity_to_stroke(
    opacity: Option<f32>,
    mut stroke: canvas::Stroke<'static>,
) -> canvas::Stroke<'static> {
    if let Some(a) = opacity {
        if let canvas::Style::Solid(ref mut c) = stroke.style {
            c.a *= a;
        }
    }
    stroke
}

/// Apply per-shape opacity to a plain color (used by text fill and
/// legacy line stroke).
pub(super) fn apply_opacity_to_color(opacity: Option<f32>, mut color: Color) -> Color {
    if let Some(a) = opacity {
        color.a *= a;
    }
    color
}

/// Parse horizontal text alignment from a JSON string value.
#[cfg(test)]
pub(super) fn parse_canvas_text_align_x(value: Option<&Value>) -> iced::widget::text::Alignment {
    match value.and_then(|v| v.as_str()) {
        Some("left") => iced::widget::text::Alignment::Left,
        Some("center") => iced::widget::text::Alignment::Center,
        Some("right") => iced::widget::text::Alignment::Right,
        _ => iced::widget::text::Alignment::Default,
    }
}

/// Parse vertical text alignment from a JSON string value.
#[cfg(test)]
pub(super) fn parse_canvas_text_align_y(value: Option<&Value>) -> alignment::Vertical {
    match value.and_then(|v| v.as_str()) {
        Some("center") => alignment::Vertical::Center,
        Some("bottom") => alignment::Vertical::Bottom,
        _ => alignment::Vertical::Top,
    }
}

/// Parse horizontal text alignment from a string.
fn parse_text_align_x(value: Option<&str>) -> iced::widget::text::Alignment {
    match value {
        Some("left") => iced::widget::text::Alignment::Left,
        Some("center") => iced::widget::text::Alignment::Center,
        Some("right") => iced::widget::text::Alignment::Right,
        _ => iced::widget::text::Alignment::Default,
    }
}

/// Parse vertical text alignment from a string.
fn parse_text_align_y(value: Option<&str>) -> alignment::Vertical {
    match value {
        Some("center") => alignment::Vertical::Center,
        Some("bottom") => alignment::Vertical::Bottom,
        _ => alignment::Vertical::Top,
    }
}

/// Parse a font from a string name.
fn parse_font_str(s: &str) -> iced::Font {
    match s {
        "monospace" => iced::Font::MONOSPACE,
        _ => iced::Font::new(Box::leak(s.to_string().into_boxed_str())),
    }
}

/// Pick the most important `Action` when multiple events fire in one
/// `update()` call. iced's `Action` can only carry one message, so
/// when shape events (enter/leave/click) and raw canvas events
/// (move/press/release) fire simultaneously, we keep the shape event.
/// Raw canvas events use Replace coalescing, so the next frame
/// delivers the latest position anyway.
pub(super) fn pick_action(
    existing: Option<iced::widget::Action<crate::message::Message>>,
    new: iced::widget::Action<crate::message::Message>,
) -> iced::widget::Action<crate::message::Message> {
    existing.unwrap_or(new)
}

/// Apply typed transforms to the drawing frame.
///
/// Applies each entry in order.
/// The caller is responsible for calling `frame.push_transform()` before
/// and `frame.pop_transform()` after this function.
pub(super) fn apply_group_transforms<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    transforms: &[Transform],
) {
    for t in transforms {
        match t {
            Transform::Translate { x, y } => {
                frame.translate(Vector::new(*x, *y));
            }
            Transform::Rotate { angle } => {
                frame.rotate(Radians(*angle));
            }
            Transform::Scale { x, y } => {
                frame.scale_nonuniform(Vector::new(*x, *y));
            }
            Transform::ScaleUniform { factor } => {
                frame.scale(*factor);
            }
        }
    }
}

/// Apply a group's clip region to the frame, drawing children inside.
///
/// If a `ClipRect` is provided, children are drawn clipped to that
/// rectangle. Otherwise children are drawn directly.
pub(super) fn draw_with_group_clip<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    clip: Option<&ClipRect>,
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
    children: &[&CanvasShape],
    draw_fn: impl FnOnce(
        &mut canvas::Frame<R>,
        &[&CanvasShape],
        &crate::image_registry::ImageRegistry,
        &iced::Theme,
    ),
) {
    if let Some(c) = clip {
        let clip_rect = iced::Rectangle {
            x: c.x,
            y: c.y,
            width: c.w,
            height: c.h,
        };
        frame.with_clip(clip_rect, |f| {
            draw_fn(f, children, images, theme);
        });
    } else {
        draw_fn(frame, children, images, theme);
    }
}

/// Truncate a shape list if it exceeds the per-layer limit. Prevents
/// excessive tessellation work from an oversized payload.
pub(super) fn truncate_shapes(name: &str, mut shapes: Vec<CanvasShape>) -> Vec<CanvasShape> {
    if shapes.len() > MAX_SHAPES_PER_LAYER {
        log::warn!(
            "canvas layer `{name}` has {} shapes, truncating to {MAX_SHAPES_PER_LAYER}",
            shapes.len(),
        );
        shapes.truncate(MAX_SHAPES_PER_LAYER);
    }
    shapes
}

/// Resolve a color value that may be a hex string OR a theme palette name.
///
/// Theme palette names: `"primary"`, `"text"`, `"background"`, `"success"`,
/// `"danger"`, `"warning"`. When a canvas shape uses one of these as a fill
/// or stroke color, the renderer resolves it against the current iced theme
/// at draw time instead of treating it as a literal hex string.
///
/// Returns `None` if the value is neither a valid hex color nor a recognized
/// theme palette name.
pub(super) fn resolve_color(value: &Value, theme: &iced::Theme) -> Option<Color> {
    let s = value.as_str()?;

    // Try hex first (most common case).
    if s.starts_with('#') {
        return parse_color(value);
    }

    // Theme palette name resolution.
    let palette = theme.palette();
    match s {
        "primary" => Some(palette.primary.base.color),
        "text" => Some(palette.background.base.text),
        "background" => Some(palette.background.base.color),
        "success" => Some(palette.success.base.color),
        "danger" => Some(palette.danger.base.color),
        "warning" => Some(palette.warning.base.color),
        _ => {
            // Fall back to hex parsing (handles non-# prefixed hex, etc.)
            parse_color(value)
        }
    }
}

/// Theme-aware version of json_color. Resolves palette names against
/// the theme before falling back to hex parsing.
#[cfg(test)]
#[allow(dead_code)]
pub(super) fn json_color_themed(val: &Value, key: &str, theme: &iced::Theme) -> Color {
    val.get(key)
        .and_then(|v| resolve_color(v, theme))
        .unwrap_or(Color::WHITE)
}

/// Resolve a core Color to an iced Color with theme palette support.
///
/// Palette names ("primary", "text", "background", etc.) are resolved
/// against the theme. Hex strings are parsed directly.
fn resolve_core_color(c: &plushie_core::types::Color, theme: &iced::Theme) -> Color {
    let s = c.as_hex();
    if s.starts_with('#') {
        return parse_hex_color(s).unwrap_or(Color::WHITE);
    }
    let palette = theme.palette();
    match s {
        "primary" => palette.primary.base.color,
        "text" => palette.background.base.text,
        "background" => palette.background.base.color,
        "success" => palette.success.base.color,
        "danger" => palette.danger.base.color,
        "warning" => palette.warning.base.color,
        _ => parse_hex_color(s).unwrap_or(Color::WHITE),
    }
}

/// Convert a typed CanvasFill to an iced canvas::Fill with theme support.
fn typed_canvas_fill(
    fill: &canvas_types::CanvasFill,
    fill_rule: Option<&canvas_types::FillRule>,
    theme: &iced::Theme,
) -> canvas::Fill {
    let rule = fill_rule
        .map(|r| iced_convert::fill_rule(*r))
        .unwrap_or(canvas::fill::Rule::NonZero);
    match fill {
        canvas_types::CanvasFill::Color(c) => canvas::Fill {
            style: canvas::Style::Solid(resolve_core_color(c, theme)),
            rule,
        },
        canvas_types::CanvasFill::Gradient(g) => canvas::Fill {
            style: canvas::Style::Gradient(iced_convert::canvas_gradient(g)),
            rule,
        },
    }
}

/// Convert a typed Stroke to an iced canvas::Stroke with theme support.
fn typed_canvas_stroke(
    s: &canvas_types::Stroke,
    theme: &iced::Theme,
) -> canvas::Stroke<'static> {
    let color = resolve_core_color(&s.color, theme);
    let mut out = canvas::Stroke::default()
        .with_color(color)
        .with_width(s.width)
        .with_line_cap(
            s.cap.map(iced_convert::line_cap).unwrap_or(canvas::LineCap::Butt),
        )
        .with_line_join(
            s.join.map(iced_convert::line_join).unwrap_or(canvas::LineJoin::Miter),
        );
    if let Some(ref dash) = s.dash {
        let segments = intern_dash_segments(dash.segments.clone());
        out.line_dash = canvas::LineDash {
            segments,
            offset: dash.offset as usize,
        };
    }
    out
}

/// Draw a single typed canvas shape into the frame.
pub(super) fn draw_canvas_shape<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    shape: &CanvasShape,
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
) {
    match shape {
        CanvasShape::Rect(r) => {
            let rect_path = match &r.radius {
                Some(radius) => {
                    let iced_radius = iced_convert::radius(radius.clone());
                    canvas::Path::rounded_rectangle(
                        Point::new(r.x, r.y),
                        Size::new(r.w, r.h),
                        iced_radius,
                    )
                }
                None => canvas::Path::rectangle(Point::new(r.x, r.y), Size::new(r.w, r.h)),
            };
            if let Some(ref fill) = r.fill {
                let iced_fill = apply_opacity_to_fill(
                    r.opacity,
                    typed_canvas_fill(fill, r.fill_rule.as_ref(), theme),
                );
                frame.fill(&rect_path, iced_fill);
            } else if r.stroke.is_none() {
                // Legacy fallback: no fill or stroke means solid white fill
                let color = apply_opacity_to_color(r.opacity, Color::WHITE);
                frame.fill_rectangle(Point::new(r.x, r.y), Size::new(r.w, r.h), color);
            }
            if let Some(ref stroke) = r.stroke {
                let iced_stroke = apply_opacity_to_stroke(
                    r.opacity,
                    typed_canvas_stroke(stroke, theme),
                );
                frame.stroke(&rect_path, iced_stroke);
            }
        }
        CanvasShape::Circle(c) => {
            let circle_path = canvas::Path::circle(Point::new(c.x, c.y), c.r);
            if let Some(ref fill) = c.fill {
                let iced_fill = apply_opacity_to_fill(
                    c.opacity,
                    typed_canvas_fill(fill, c.fill_rule.as_ref(), theme),
                );
                frame.fill(&circle_path, iced_fill);
            } else if c.stroke.is_none() {
                let color = apply_opacity_to_color(c.opacity, Color::WHITE);
                frame.fill(&circle_path, color);
            }
            if let Some(ref stroke) = c.stroke {
                let iced_stroke = apply_opacity_to_stroke(
                    c.opacity,
                    typed_canvas_stroke(stroke, theme),
                );
                frame.stroke(&circle_path, iced_stroke);
            }
        }
        CanvasShape::Line(l) => {
            let line_path = canvas::Path::line(
                Point::new(l.x1, l.y1),
                Point::new(l.x2, l.y2),
            );
            if let Some(ref stroke) = l.stroke {
                let iced_stroke = apply_opacity_to_stroke(
                    l.opacity,
                    typed_canvas_stroke(stroke, theme),
                );
                frame.stroke(&line_path, iced_stroke);
            } else {
                // Legacy: line without explicit stroke defaults to a 1px white line
                let color = apply_opacity_to_color(l.opacity, Color::WHITE);
                frame.stroke(
                    &line_path,
                    canvas::Stroke::default().with_color(color).with_width(1.0),
                );
            }
        }
        CanvasShape::Text(t) => {
            let fill_color = t.fill.as_ref()
                .and_then(|f| match f {
                    canvas_types::CanvasFill::Color(c) => Some(resolve_core_color(c, theme)),
                    _ => None,
                })
                .unwrap_or(Color::WHITE);
            let fill_color = apply_opacity_to_color(t.opacity, fill_color);
            let align_x = parse_text_align_x(t.align_x.as_deref());
            let align_y = parse_text_align_y(t.align_y.as_deref());
            let mut canvas_text = canvas::Text {
                content: t.content.clone(),
                position: Point::new(t.x, t.y),
                color: fill_color,
                align_x,
                align_y,
                ..canvas::Text::default()
            };
            if let Some(s) = t.size {
                canvas_text.size = Pixels(s);
            }
            if let Some(ref font_name) = t.font {
                canvas_text.font = parse_font_str(font_name);
            }
            frame.fill_text(canvas_text);
        }
        CanvasShape::Path(p) => {
            let path = build_path_from_commands(&p.commands);
            if let Some(ref fill) = p.fill {
                let iced_fill = apply_opacity_to_fill(
                    p.opacity,
                    typed_canvas_fill(fill, p.fill_rule.as_ref(), theme),
                );
                frame.fill(&path, iced_fill);
            }
            if let Some(ref stroke) = p.stroke {
                let iced_stroke = apply_opacity_to_stroke(
                    p.opacity,
                    typed_canvas_stroke(stroke, theme),
                );
                frame.stroke(&path, iced_stroke);
            }
        }
        CanvasShape::Image(img_shape) => {
            let bounds = iced::Rectangle {
                x: img_shape.x,
                y: img_shape.y,
                width: img_shape.w,
                height: img_shape.h,
            };
            // Source can be a file path or a "{handle:name}" registry reference.
            // The ImageShape.source is a plain string; check if it matches a
            // registry handle pattern.
            let handle = if let Some(h) = images.get(&img_shape.source) {
                h.clone()
            } else {
                iced::widget::image::Handle::from_path(&img_shape.source)
            };
            let rotation = img_shape.rotation
                .map(|r| Radians(r))
                .unwrap_or(Radians(0.0));
            let opacity = img_shape.opacity.unwrap_or(1.0);
            let img = iced::advanced::image::Image {
                handle,
                filter_method: iced::advanced::image::FilterMethod::default(),
                rotation,
                border_radius: Default::default(),
                opacity,
            };
            frame.draw_image(bounds, img);
        }
        CanvasShape::Svg(s) => {
            let bounds = iced::Rectangle {
                x: s.x,
                y: s.y,
                width: s.w,
                height: s.h,
            };
            let handle = iced::widget::svg::Handle::from_path(&s.source);
            frame.draw_svg(bounds, &handle);
        }
        CanvasShape::Group(g) => {
            let child_refs: Vec<&CanvasShape> = g.children.iter().collect();
            let has_transforms = !g.transforms.is_empty();

            if has_transforms {
                frame.push_transform();
                apply_group_transforms(frame, &g.transforms);
            }

            draw_with_group_clip(
                frame,
                g.clip.as_ref(),
                images,
                theme,
                &child_refs,
                |f, c, img, theme| {
                    draw_canvas_shapes(f, c, img, theme);
                },
            );

            if has_transforms {
                frame.pop_transform();
            }
        }
    }
}

/// Draw a shape with style overrides applied from an interactive state
/// (hover, pressed, or focus). The override can replace fill, stroke,
/// and/or opacity on the shape being drawn.
pub(super) fn draw_canvas_shape_with_overrides<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    shape: &CanvasShape,
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
    overrides: &plushie_core::types::canvas::ShapeStyle,
) {
    // Build a modified clone of the shape with overrides applied, then draw it.
    // This approach keeps the drawing logic in draw_canvas_shape.
    let modified = apply_style_overrides(shape, overrides);
    draw_canvas_shape(frame, &modified, images, theme);
}

/// Apply a ShapeStyle override to a shape, returning a modified clone.
fn apply_style_overrides(
    shape: &CanvasShape,
    overrides: &plushie_core::types::canvas::ShapeStyle,
) -> CanvasShape {
    use plushie_core::types::canvas::CanvasFill;
    use plushie_core::types::Color as CoreColor;

    let override_fill = overrides.fill.as_ref().map(|f| CanvasFill::Color(CoreColor::hex(f)));
    let override_stroke = overrides.stroke.as_ref().and_then(|v| {
        <canvas_types::Stroke as plushie_core::types::PlushieType>::wire_decode(v)
    });
    let override_opacity = overrides.opacity;

    match shape {
        CanvasShape::Rect(r) => {
            let mut r = r.clone();
            if let Some(f) = override_fill { r.fill = Some(f); }
            if let Some(s) = override_stroke { r.stroke = Some(s); }
            if let Some(o) = override_opacity { r.opacity = Some(o); }
            CanvasShape::Rect(r)
        }
        CanvasShape::Circle(c) => {
            let mut c = c.clone();
            if let Some(f) = override_fill { c.fill = Some(f); }
            if let Some(s) = override_stroke { c.stroke = Some(s); }
            if let Some(o) = override_opacity { c.opacity = Some(o); }
            CanvasShape::Circle(c)
        }
        CanvasShape::Line(l) => {
            let mut l = l.clone();
            if let Some(s) = override_stroke { l.stroke = Some(s); }
            if let Some(o) = override_opacity { l.opacity = Some(o); }
            CanvasShape::Line(l)
        }
        CanvasShape::Text(t) => {
            let mut t = t.clone();
            if let Some(f) = override_fill { t.fill = Some(f); }
            if let Some(o) = override_opacity { t.opacity = Some(o); }
            CanvasShape::Text(t)
        }
        CanvasShape::Path(p) => {
            let mut p = p.clone();
            if let Some(f) = override_fill { p.fill = Some(f); }
            if let Some(s) = override_stroke { p.stroke = Some(s); }
            if let Some(o) = override_opacity { p.opacity = Some(o); }
            CanvasShape::Path(p)
        }
        CanvasShape::Image(i) => {
            let mut i = i.clone();
            if let Some(o) = override_opacity { i.opacity = Some(o); }
            CanvasShape::Image(i)
        }
        // Svg and Group don't have fill/stroke/opacity overrides
        _ => shape.clone(),
    }
}
