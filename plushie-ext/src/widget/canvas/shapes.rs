//! Canvas shape drawing, fill/stroke parsing, and path building.

use iced::widget::canvas;
use iced::{Color, Pixels, Point, Radians, Size, Vector, alignment};
use serde_json::Value;

use super::json_f32;
use super::types::MAX_SHAPES_PER_LAYER;
use crate::PlushieRenderer;
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
/// carries its own `"transforms"` and `"clip"` fields, applied in
/// [`draw_canvas_shape`] when rendering the `"group"` type.
pub(super) fn draw_canvas_shapes<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    shapes: &[&Value],
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
) {
    for &shape in shapes {
        let shape_type = shape.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match shape_type {
            // Standalone clip commands are no longer supported.
            "push_clip" | "pop_clip" => {
                log::warn!(
                    "canvas: standalone '{shape_type}' commands are no longer supported. \
                     Use group clip instead."
                );
            }
            _ => draw_canvas_shape(frame, shape, images, theme),
        }
    }
}

/// Apply per-shape opacity to a `canvas::Fill`. Multiplies the opacity
/// into solid color alpha. Gradient stops are left unchanged (the host
/// should bake opacity into gradient stop colors if needed).
pub(super) fn apply_opacity_to_fill(shape: &Value, mut fill: canvas::Fill) -> canvas::Fill {
    if let Some(opacity) = shape.get("opacity").and_then(|v| v.as_f64()) {
        let a = opacity as f32;
        if let canvas::Style::Solid(ref mut c) = fill.style {
            c.a *= a;
        }
    }
    fill
}

/// Apply per-shape opacity to a `canvas::Stroke`.
pub(super) fn apply_opacity_to_stroke(
    shape: &Value,
    mut stroke: canvas::Stroke<'static>,
) -> canvas::Stroke<'static> {
    if let Some(opacity) = shape.get("opacity").and_then(|v| v.as_f64()) {
        let a = opacity as f32;
        if let canvas::Style::Solid(ref mut c) = stroke.style {
            c.a *= a;
        }
    }
    stroke
}

/// Apply per-shape opacity to a plain color (used by text fill and
/// legacy line stroke).
pub(super) fn apply_opacity_to_color(shape: &Value, mut color: Color) -> Color {
    if let Some(opacity) = shape.get("opacity").and_then(|v| v.as_f64()) {
        color.a *= opacity as f32;
    }
    color
}

/// Parse horizontal text alignment from a JSON string value.
pub(super) fn parse_canvas_text_align_x(value: Option<&Value>) -> iced::widget::text::Alignment {
    match value.and_then(|v| v.as_str()) {
        Some("left") => iced::widget::text::Alignment::Left,
        Some("center") => iced::widget::text::Alignment::Center,
        Some("right") => iced::widget::text::Alignment::Right,
        _ => iced::widget::text::Alignment::Default,
    }
}

/// Parse vertical text alignment from a JSON string value.
pub(super) fn parse_canvas_text_align_y(value: Option<&Value>) -> alignment::Vertical {
    match value.and_then(|v| v.as_str()) {
        Some("center") => alignment::Vertical::Center,
        Some("bottom") => alignment::Vertical::Bottom,
        _ => alignment::Vertical::Top,
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

/// Apply a group's transforms to the drawing frame.
///
/// Reads the `"transforms"` array and applies each entry in order.
/// Supported transform types: `translate`, `rotate`, `scale`.
/// The caller is responsible for calling `frame.push_transform()` before
/// and `frame.pop_transform()` after this function.
pub(super) fn apply_group_transforms<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    group: &Value,
) {
    let transforms = match group.get("transforms").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return,
    };
    for t in transforms {
        let t_type = t.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match t_type {
            "translate" => {
                let x = t.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let y = t.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                frame.translate(Vector::new(x, y));
            }
            "rotate" => {
                let angle = t.get("angle").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                frame.rotate(Radians(angle));
            }
            "scale" => {
                if let Some(factor) = t.get("factor").and_then(|v| v.as_f64()) {
                    frame.scale(factor as f32);
                } else {
                    let x = t.get("x").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    let y = t.get("y").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                    frame.scale_nonuniform(Vector::new(x, y));
                }
            }
            _ => {
                log::warn!("canvas group: unknown transform type '{t_type}'");
            }
        }
    }
}

/// Apply a group's clip region to the frame, drawing children inside.
///
/// If the group has a `"clip"` field with `{x, y, w, h}`, children are
/// drawn clipped to that rectangle. Otherwise children are drawn directly.
pub(super) fn draw_with_group_clip<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    group: &Value,
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
    children: &[&Value],
    draw_fn: impl FnOnce(
        &mut canvas::Frame<R>,
        &[&Value],
        &crate::image_registry::ImageRegistry,
        &iced::Theme,
    ),
) {
    if let Some(clip) = group.get("clip").and_then(|v| v.as_object()) {
        let x = clip.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let y = clip.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let w = clip.get("w").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let h = clip.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let clip_rect = iced::Rectangle {
            x,
            y,
            width: w,
            height: h,
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
pub(super) fn truncate_shapes(name: &str, mut shapes: Vec<Value>) -> Vec<Value> {
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

/// Theme-aware version of [`json_color`](super::json_color). Resolves palette
/// names against the theme before falling back to hex parsing.
pub(super) fn json_color_themed(val: &Value, key: &str, theme: &iced::Theme) -> Color {
    val.get(key)
        .and_then(|v| resolve_color(v, theme))
        .unwrap_or(Color::WHITE)
}

/// Draw a single shape (or transform command) into the frame.
pub(super) fn draw_canvas_shape<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    shape: &Value,
    images: &crate::image_registry::ImageRegistry,
    theme: &iced::Theme,
) {
    let shape_type = shape.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match shape_type {
        // Standalone transform/clip commands are no longer supported.
        // Transforms and clips now live on groups via "transforms" and "clip" fields.
        "push_transform" | "pop_transform" | "translate" | "rotate" | "scale" => {
            log::warn!(
                "canvas: standalone '{shape_type}' commands are no longer supported. \
                 Use group transforms instead."
            );
        }
        // -- Primitive shapes --
        "rect" => {
            let x = json_f32(shape, "x");
            let y = json_f32(shape, "y");
            let w = json_f32(shape, "w");
            let h = json_f32(shape, "h");
            let rect_path = if let Some(radius_val) = shape.get("radius") {
                let radius = if let Some(r) = radius_val.as_f64() {
                    // Uniform radius
                    iced::border::Radius::from(r as f32)
                } else if let Some(obj) = radius_val.as_object() {
                    // Per-corner radius
                    iced::border::Radius {
                        top_left: obj.get("top_left").and_then(|v| v.as_f64()).unwrap_or(0.0)
                            as f32,
                        top_right: obj.get("top_right").and_then(|v| v.as_f64()).unwrap_or(0.0)
                            as f32,
                        bottom_right: obj
                            .get("bottom_right")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0) as f32,
                        bottom_left: obj
                            .get("bottom_left")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0) as f32,
                    }
                } else {
                    iced::border::Radius::from(0.0)
                };
                canvas::Path::rounded_rectangle(Point::new(x, y), Size::new(w, h), radius)
            } else {
                canvas::Path::rectangle(Point::new(x, y), Size::new(w, h))
            };
            if let Some(fill_val) = shape.get("fill") {
                let fill = apply_opacity_to_fill(
                    shape,
                    parse_canvas_fill_themed(fill_val, shape, Some(theme)),
                );
                frame.fill(&rect_path, fill);
            } else if shape.get("stroke").is_none() {
                // Legacy fallback: no fill or stroke key means solid white fill
                let color = apply_opacity_to_color(shape, Color::WHITE);
                frame.fill_rectangle(Point::new(x, y), Size::new(w, h), color);
            }
            if let Some(stroke_val) = shape.get("stroke") {
                let stroke = apply_opacity_to_stroke(
                    shape,
                    parse_canvas_stroke_themed(stroke_val, Some(theme)),
                );
                frame.stroke(&rect_path, stroke);
            }
        }
        "circle" => {
            let x = json_f32(shape, "x");
            let y = json_f32(shape, "y");
            let r = json_f32(shape, "r");
            let circle_path = canvas::Path::circle(Point::new(x, y), r);
            if let Some(fill_val) = shape.get("fill") {
                let fill = apply_opacity_to_fill(
                    shape,
                    parse_canvas_fill_themed(fill_val, shape, Some(theme)),
                );
                frame.fill(&circle_path, fill);
            } else if shape.get("stroke").is_none() {
                let color = apply_opacity_to_color(shape, Color::WHITE);
                frame.fill(&circle_path, color);
            }
            if let Some(stroke_val) = shape.get("stroke") {
                let stroke = apply_opacity_to_stroke(
                    shape,
                    parse_canvas_stroke_themed(stroke_val, Some(theme)),
                );
                frame.stroke(&circle_path, stroke);
            }
        }
        "line" => {
            let x1 = json_f32(shape, "x1");
            let y1 = json_f32(shape, "y1");
            let x2 = json_f32(shape, "x2");
            let y2 = json_f32(shape, "y2");
            let line_path = canvas::Path::line(Point::new(x1, y1), Point::new(x2, y2));
            if let Some(stroke_val) = shape.get("stroke") {
                let stroke = apply_opacity_to_stroke(
                    shape,
                    parse_canvas_stroke_themed(stroke_val, Some(theme)),
                );
                frame.stroke(&line_path, stroke);
            } else {
                // Legacy: use fill color as stroke color
                let color = apply_opacity_to_color(shape, json_color_themed(shape, "fill", theme));
                let width = shape
                    .get("width")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32)
                    .unwrap_or(1.0);
                frame.stroke(
                    &line_path,
                    canvas::Stroke::default()
                        .with_color(color)
                        .with_width(width),
                );
            }
        }
        "text" => {
            let x = json_f32(shape, "x");
            let y = json_f32(shape, "y");
            let content = shape.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let fill_color = apply_opacity_to_color(shape, json_color_themed(shape, "fill", theme));
            let size = shape.get("size").and_then(|v| v.as_f64()).map(|v| v as f32);
            let align_x = parse_canvas_text_align_x(
                shape
                    .get("align_x")
                    .or_else(|| shape.get("horizontal_alignment")),
            );
            let align_y = parse_canvas_text_align_y(
                shape
                    .get("align_y")
                    .or_else(|| shape.get("vertical_alignment")),
            );
            let mut canvas_text = canvas::Text {
                content: content.to_owned(),
                position: Point::new(x, y),
                color: fill_color,
                align_x,
                align_y,
                ..canvas::Text::default()
            };
            if let Some(s) = size {
                canvas_text.size = Pixels(s);
            }
            if let Some(f) = shape.get("font") {
                canvas_text.font = parse_font(f);
            }
            frame.fill_text(canvas_text);
        }
        "path" => {
            let commands = shape
                .get("commands")
                .and_then(|v| v.as_array())
                .map(|a| a.as_slice())
                .unwrap_or(&[]);
            let path = build_path_from_commands(commands);
            if let Some(fill_val) = shape.get("fill") {
                let fill = apply_opacity_to_fill(
                    shape,
                    parse_canvas_fill_themed(fill_val, shape, Some(theme)),
                );
                frame.fill(&path, fill);
            }
            if let Some(stroke_val) = shape.get("stroke") {
                let stroke = apply_opacity_to_stroke(
                    shape,
                    parse_canvas_stroke_themed(stroke_val, Some(theme)),
                );
                frame.stroke(&path, stroke);
            }
        }
        "image" => {
            let x = json_f32(shape, "x");
            let y = json_f32(shape, "y");
            let w = json_f32(shape, "w");
            let h = json_f32(shape, "h");
            let bounds = iced::Rectangle {
                x,
                y,
                width: w,
                height: h,
            };
            // Source can be a string (file path) or an object with "handle" key
            // (in-memory image from the registry), same as the Image widget.
            let source_val = shape.get("source");
            let handle = match source_val {
                Some(Value::Object(obj)) => {
                    if let Some(name) = obj.get("handle").and_then(|v| v.as_str()) {
                        match images.get(name) {
                            Some(h) => h.clone(),
                            None => {
                                log::warn!("canvas image: unknown registry handle: {name}");
                                return;
                            }
                        }
                    } else {
                        return;
                    }
                }
                _ => {
                    let path = source_val.and_then(|v| v.as_str()).unwrap_or("");
                    iced::widget::image::Handle::from_path(path)
                }
            };
            let rotation = shape
                .get("rotation")
                .and_then(|v| v.as_f64())
                .map(|r| Radians(r as f32))
                .unwrap_or(Radians(0.0));
            let opacity = shape
                .get("opacity")
                .and_then(|v| v.as_f64())
                .map(|o| o as f32)
                .unwrap_or(1.0);
            let img = iced::advanced::image::Image {
                handle,
                filter_method: iced::advanced::image::FilterMethod::default(),
                rotation,
                border_radius: Default::default(),
                opacity,
            };
            frame.draw_image(bounds, img);
        }
        "svg" => {
            let source = shape.get("source").and_then(|v| v.as_str()).unwrap_or("");
            let x = json_f32(shape, "x");
            let y = json_f32(shape, "y");
            let w = json_f32(shape, "w");
            let h = json_f32(shape, "h");
            let bounds = iced::Rectangle {
                x,
                y,
                width: w,
                height: h,
            };
            let handle = iced::widget::svg::Handle::from_path(source);
            frame.draw_svg(bounds, &handle);
        }
        "group" => {
            if let Some(children) = shape.get("children").and_then(|v| v.as_array()) {
                let child_refs: Vec<&Value> = children.iter().collect();
                let has_transforms = shape
                    .get("transforms")
                    .and_then(|v| v.as_array())
                    .is_some_and(|a| !a.is_empty());

                if has_transforms {
                    frame.push_transform();
                    apply_group_transforms(frame, shape);
                }

                // draw_with_group_clip handles the clip field (if present)
                // using frame.with_clip, which manages its own scope.
                draw_with_group_clip(
                    frame,
                    shape,
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
        _ => {}
    }
}
