# Canvas

Canvas is a different shape from the widget tree. Instead of
composing containers, inputs, and buttons, you draw on a 2D surface:
rectangles, circles, lines, paths, text, and images. Those shapes
can be grouped, transformed, and made interactive. Clicks, hovers,
and drags flow back through `update` as ordinary `Event` values.

This chapter builds a small free-form drawing app to cover the
shape catalog, the path builder, transforms, layers, and pointer
interaction. For the full surface, see the
[canvas reference](../reference/canvas.md).

## The canvas widget

A canvas starts with `canvas(id)` and holds shape builders as
children. The ID is required: canvas carries renderer-side state
(focus, drag) and scopes its child events. It takes the usual
`width` and `height` like any other widget.

```rust
use plushie::prelude::*;

canvas("drawing")
    .width(400.0)
    .height(300.0)
    .child(layer("bg").children([
        rect(0.0, 0.0, 400.0, 300.0).fill(Color::hex("#1a1a2e")),
        circle(200.0, 150.0, 40.0).fill(Color::red()),
    ]))
```

The canvas itself is a widget. It sits inside rows, columns, and
containers like anything else, and everything nested inside is
drawn rather than laid out. Coordinates are in pixels relative to
the canvas origin at the top-left.

## Shape primitives

Shapes are leaf builders. Each one accepts `fill`, `stroke`,
`stroke_width`, `opacity`, and the interactive style overrides
(`hover_style`, `pressed_style`, `focus_style`). Fill and stroke
are independent: set one, both, or neither.

```rust
rect(0.0, 0.0, 100.0, 50.0)
    .fill(Color::hex("#3b82f6"))
    .stroke(Color::hex("#333"))
    .stroke_width(2.0)
    .stroke_cap(LineCap::Round)
    .stroke_dash(&[5.0, 3.0], 0.0)
```

`rect(x, y, w, h)` draws a rectangle, with optional corner
`radius(px)` or per-corner `radius_corners(tl, tr, br, bl)`.
`circle(x, y, r)` draws a circle. `line(x1, y1, x2, y2)` draws a
stroked segment (lines have no fill). `canvas_text(x, y, content)`
draws a text run with `size`, `font`, and `align_x` / `align_y`.

Colors come from `Color::hex`, `Color::rgb`, the named helpers
(`Color::red()`, `Color::black()`, ...), or via an `impl Into<Color>`
shortcut where a hex `&str` also works. Gradients are created with
`linear_gradient(start, end, stops)` for reuse, or inline with
`.fill_gradient(x1, y1, x2, y2, &[(offset, hex), ...])` on rect,
circle, and path.

## Paths

`path(commands)` takes any iterable of `PathCommand` values. The
command builders live next to the shape builders in
`plushie::ui::canvas`: `move_to`, `line_to`, `bezier_to`,
`quadratic_to`, `arc`, `arc_to`, `ellipse`, `rounded_rect`, and
`close`.

```rust
path(vec![
    move_to(10.0, 0.0),
    line_to(20.0, 20.0),
    line_to(0.0, 20.0),
    close(),
])
.fill(Color::hex("#22c55e"))
```

Paths can be filled, stroked, or both. `fill_rule(FillRule::EvenOdd)`
switches from the default non-zero fill to the crossing-count
rule, which matters for self-intersecting shapes like stars. A
path with no `close()` at the end and only a `stroke` set becomes
an open polyline.

Angles on `arc` and `ellipse` accept any `f32` (interpreted as
degrees) or an explicit `Angle::deg(..)` / `Angle::rad(..)`.

## Transforms

Transforms apply to groups, not individual shapes. `group(id)`
collects shapes and applies transforms in declaration order:

```rust
group("tile")
    .translate(100.0, 50.0)
    .rotate(45.0)
    .scale_uniform(1.25)
    .child(rect(0.0, 0.0, 40.0, 40.0).fill(Color::hex("#ef4444")))
```

`translate(x, y)` moves the group. `rotate(angle)` rotates around
the group origin. `scale_xy(x, y)` scales per-axis,
`scale_uniform(f)` scales both. `clip(x, y, w, h)` restricts
drawing to a rectangle (one clip per group).

Transforms compose: nested groups multiply their parents'
transforms, so a rotated parent containing a translated child
behaves the way you would expect from any 2D graphics system.

## Layers and groups

Layers organise z-order. `layer(name)` is drawn in alphabetical
order of `name`, so names double as the ordering key:

```rust
canvas("chart")
    .width(400.0).height(200.0)
    .child(layer("background").child(
        rect(0.0, 0.0, 400.0, 200.0).fill(Color::hex("#f5f5f5")),
    ))
    .child(layer("data").children([
        rect(10.0, 50.0, 80.0, 150.0).fill(Color::hex("#3b82f6")),
        rect(110.0, 100.0, 80.0, 100.0).fill(Color::hex("#22c55e")),
    ]))
    .child(layer("labels").child(
        canvas_text(50.0, 190.0, "A").size(12.0).fill(Color::hex("#333")),
    ))
```

Each layer is a separate cache on the renderer side. When the
contents of one layer change, only that layer re-tessellates. Put
static decoration, dynamic data, and interactive controls in
separate layers so repaint cost tracks what actually changed.

Groups inside a layer share transforms and clipping. They also
scope element IDs: a shape inside `group("legend")` inside
`canvas("chart")` arrives as a widget event with scope
`["legend", "chart", ...]`.

## Text and images

`canvas_text`, `canvas_image`, and `canvas_svg` are the canvas
counterparts to the stand-alone widgets. They draw at a fixed
position inside a layer or group:

```rust
layer("icons").children([
    canvas_svg(10.0, 8.0, &include_str!("../assets/save.svg"))
        .width(20.0)
        .height(20.0),
    canvas_image(50.0, 8.0, "assets/logo.png")
        .width(32.0)
        .height(32.0)
        .opacity(0.8),
])
```

`canvas_svg` takes SVG source text, not a path: read the file at
the call site with `include_str!` or `std::fs::read_to_string`.
`canvas_image` takes a file path that the renderer resolves. SVG
is the natural choice for interface elements because it scales
without pixelation; raster images are best for photos and exported
artwork.

## Interactive regions

`interactive(id)` returns a `GroupBuilder` with `on_click(true)`
pre-configured. Everything else works like a regular group: add
transforms, wrap shapes, annotate for accessibility. The group's
ID is what arrives in the event.

```rust
interactive("save")
    .cursor(CursorStyle::Pointer)
    .focusable(true)
    .a11y(&A11y::new().role(Role::Button).label("Save"))
    .hover_style(PropValue::object([("fill", "#2563eb".into())]))
    .pressed_style(PropValue::object([("fill", "#1d4ed8".into())]))
    .child(rect(0.0, 0.0, 100.0, 36.0)
        .fill(Color::hex("#3b82f6"))
        .radius(6.0))
    .child(canvas_text(50.0, 11.0, "Save")
        .fill(Color::hex("#ffffff"))
        .size(14.0))
```

Hover and press styles are applied by the renderer; no event
handling is needed for the visual feedback. Canvas has no
intrinsic semantic structure, so interactive regions need
explicit `a11y` annotations to be visible to assistive
technology. The [accessibility reference](../reference/accessibility.md)
covers the full annotation surface.

Canvas-level pointer events (press, move, release, scroll on the
canvas itself, outside any interactive group) are gated by
setters on the canvas: `.on_press(true)`, `.on_move(true)`,
`.on_release(true)`, `.on_scroll(true)`. They carry `x`, `y`,
`pointer`, `finger`, `button`, and `modifiers`. Match them with
the tuple-form variants of `WidgetMatch`:

```rust
use plushie::event::WidgetMatch::*;
use plushie::prelude::PointerKind;

match event.widget_match() {
    Some(Press("drawing", p)) if p.pointer == PointerKind::Mouse => {
        model.begin_stroke(p.x, p.y);
    }
    Some(Move("drawing", m)) => {
        model.extend_stroke(m.x, m.y);
    }
    Some(Release("drawing", _)) => {
        model.end_stroke();
    }
    _ => {}
}
```

`Press`, `Move`, and `Release` each carry a typed payload
(`PointerPress`, `PointerMove`, `PointerRelease`). See the
[events reference](../reference/events.md#pointer-events) for
the full field list.

## A small drawing app

Here is a minimal free-form sketcher. The model stores completed
strokes and the in-progress one. Mouse press starts a stroke,
moves extend it, release commits it:

```rust
use plushie::event::WidgetMatch::*;
use plushie::prelude::*;

#[derive(Default)]
struct Sketch {
    strokes: Vec<Vec<(f32, f32)>>,
    current: Option<Vec<(f32, f32)>>,
}

impl App for Sketch {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Sketch::default(), Command::none())
    }

    fn update(model: &mut Self, event: Event) -> Command {
        match event.widget_match() {
            Some(Press("pad", p)) => {
                model.current = Some(vec![(p.x, p.y)]);
            }
            Some(Move("pad", m)) => {
                if let Some(stroke) = model.current.as_mut() {
                    stroke.push((m.x, m.y));
                }
            }
            Some(Release("pad", _)) => {
                if let Some(stroke) = model.current.take() {
                    if stroke.len() > 1 {
                        model.strokes.push(stroke);
                    }
                }
            }
            Some(Click("clear")) => {
                model.strokes.clear();
                model.current = None;
            }
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        let stroke_path = |points: &[(f32, f32)]| -> View {
            let mut commands = Vec::with_capacity(points.len());
            if let Some((x, y)) = points.first() {
                commands.push(move_to(*x, *y));
                for (x, y) in &points[1..] {
                    commands.push(line_to(*x, *y));
                }
            }
            path(commands)
                .stroke(Color::hex("#1a1a2e"))
                .stroke_width(2.0)
                .stroke_cap(LineCap::Round)
                .into()
        };

        let mut strokes: Vec<View> =
            model.strokes.iter().map(|s| stroke_path(s)).collect();
        if let Some(current) = &model.current {
            strokes.push(stroke_path(current));
        }

        window("main").child(
            column().children([
                canvas("pad")
                    .width(480.0)
                    .height(320.0)
                    .on_press(true)
                    .on_move(true)
                    .on_release(true)
                    .child(layer("ink").children(strokes))
                    .into(),
                button("clear", "Clear").into(),
            ]),
        ).into()
    }
}
```

Each stroke is a `path` built from `move_to` plus a sequence of
`line_to` commands. The `Move` events are coalesced by the
runtime, so dragging across a few hundred pixels produces only as
many updates as a frame can handle. The `Clear` button is a
regular widget sitting outside the canvas: mix canvas and normal
widgets freely.

## Animating canvas values

Numeric canvas props (shape coordinates, sizes, colors, stroke
widths, transform values) accept `impl Into<Animatable<T>>`, so a
`Transition`, `Spring`, or `Sequence` drops straight into a
setter. The renderer interpolates frame by frame; `update` only
sees the target value.

```rust
circle(200.0, 150.0, Transition::new(model.pulse_radius, 400))
    .fill(Color::hex("#3b82f6"))
```

For canvas drawings that react to physics or multi-phase logic,
an SDK-side `Tween` driven by
`Subscription::on_animation_frame()` is usually the right fit.
See the [animation chapter](09-animation.md) for the full shape
of renderer-side vs SDK-side animation.

## What's next

Canvas is the drawing primitive; custom widgets are the
composition primitive. When a particular piece of canvas art,
interactive regions and all, starts repeating across an app, it
is time to wrap it up as a typed component. The next chapter
covers [custom widgets](13-custom-widgets.md): naming them,
parameterising them, and exposing a builder API that fits the
rest of the SDK.
