# Canvas

The canvas is a drawing surface for 2D shapes, paths, text, images,
and SVG content. Unlike layout widgets, which compose child widgets,
a canvas contains layers of shape builders that draw onto a bitmap
region of the view tree. Canvas builders live in `plushie::ui::canvas`
(re-exported from `plushie::ui`); path commands and the backing types
(`PathCommand`, `FillRule`, `LineCap`, `LineJoin`, `Angle`, `DragAxis`)
live in `plushie_core::types`.

For the canvas widget's top-level props, see
[built-in widgets](built-in-widgets.md#canvas-primitives).

## Canvas basics

A canvas starts with `canvas(id)`. The ID is required: canvas holds
renderer-side state (focus, drag) and is always interactive.
Shapes are added through `.child(..)` or `.children([..])`, exactly
like any other container builder:

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

A canvas participates in the layout system like any other widget:
it takes `width`, `height`, `background`, and standard `a11y` and
`event_rate` setters. Everything inside it is drawn, not laid out.

### Canvas setters

| Method | Signature | Description |
|---|---|---|
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height |
| `background` | `(c: impl Into<Animatable<Color>>) -> Self` | Canvas background |
| `on_press` | `(v: bool) -> Self` | Emit canvas-level press events |
| `on_release` | `(v: bool) -> Self` | Emit canvas-level release events |
| `on_move` | `(v: bool) -> Self` | Emit canvas-level pointer-move events |
| `on_scroll` | `(v: bool) -> Self` | Emit canvas-level scroll events |
| `interactive` | `(v: bool) -> Self` | Mark as hoverable / clickable at the canvas level |
| `arrow_mode` | `(mode: ArrowMode) -> Self` | Keyboard arrow navigation policy |
| `alt` | `(text: &str) -> Self` | Short accessible description |
| `description` | `(text: &str) -> Self` | Extended accessible description |
| `role` | `(role: &str) -> Self` | Override the accessible role |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec (0 = unbounded) |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` / `children` |  | Append / replace children |

Canvas-level pointer events carry `x`, `y`, `pointer` (mouse, touch,
pen), `finger` (touch id, `None` otherwise), `button`, and
`modifiers`. See [Events](events.md) for the payload shape.

## Layers and groups

A canvas child is typically a `layer`. Layers are drawn in
alphabetical order of their `name`; name them so that the intended
z-ordering is the lexical order:

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
    .child(layer("labels").children([
        canvas_text(50.0, 190.0, "A").size(12.0).fill(Color::hex("#333")),
        canvas_text(150.0, 190.0, "B").size(12.0).fill(Color::hex("#333")),
    ]))
```

Each layer maps to a separate cache on the renderer side: when the
contents of one layer change, only that layer is re-tessellated.
Splitting static decoration, dynamic data, and interactive controls
into separate layers keeps repaint costs proportional to what
actually changed.

Groups apply transforms, clip rectangles, drag behaviour, and hit
testing to a sub-tree of shapes. `group(id)` takes an explicit ID;
shapes inside a group inherit transforms in declaration order.

```rust
group("legend")
    .translate(200.0, 50.0)
    .rotate(15.0)
    .children([
        rect(0.0, 0.0, 80.0, 24.0).fill(Color::hex("#eee")),
        canvas_text(8.0, 16.0, "Values").size(12.0),
    ])
```

## Shape primitives

Shapes are leaf builders. Each has auto-generated node IDs but
accepts `.id(&str)` when an explicit scope prefix is needed. Every
shape supports `fill`, `stroke`, `stroke_width`, `stroke_cap`,
`stroke_join`, `stroke_dash`, `opacity`, `hover_style`,
`pressed_style`, and `focus_style`. Shape-specific constructors
and extras:

| Function | Constructor args | Shape-specific setters |
|---|---|---|
| `rect` | `(x, y, w, h: f32)` | `radius`, `radius_corners`, `fill_rule`, `fill_gradient` |
| `circle` | `(x, y, r: f32)` | `fill_rule`, `fill_gradient` |
| `line` | `(x1, y1, x2, y2: f32)` | stroke-only (no `fill`, `fill_rule`) |
| `path` | `(commands: IntoIter<Item = PathCommand>)` | `fill_rule`, `fill_gradient` |
| `canvas_text` | `(x, y: f32, content: &str)` | `size`, `font`, `align_x`, `align_y` |
| `canvas_image` | `(x, y: f32, source: &str)` | `width`, `height`, `rotation` |
| `canvas_svg` | `(x, y: f32, source: &str)` | `width`, `height` |

`rect` has two corner-radius modes. `.radius(px)` sets all four
corners; `.radius_corners(tl, tr, br, bl)` sets them individually.
Use `path_raw(commands: Vec<PropValue>)` only when path commands
have already been encoded elsewhere. New code should prefer the
typed [`PathCommand`] builders described below.

`canvas_image` takes a file path; `canvas_svg` takes SVG source
text, not a file path. Read the file at the call site (or load it
into the asset pipeline) and pass the bytes.

## Path commands

`path(..)` takes a sequence of typed `PathCommand` values. The
command builders live alongside the shape builders in
`plushie::ui::canvas`:

| Function | Arguments | Description |
|---|---|---|
| `move_to` | `(x, y: f32)` | Move the pen without drawing |
| `line_to` | `(x, y: f32)` | Straight line to point |
| `bezier_to` | `(cp1x, cp1y, cp2x, cp2y, x, y: f32)` | Cubic bezier curve |
| `quadratic_to` | `(cpx, cpy, x, y: f32)` | Quadratic bezier curve |
| `arc` | `(cx, cy, radius: f32, start: Angle, end: Angle)` | Circular arc by centre |
| `arc_to` | `(x1, y1, x2, y2, radius: f32)` | Tangent arc between two segments |
| `ellipse` | `(cx, cy, rx, ry: f32, rotation: Angle, start: Angle, end: Angle)` | Elliptical arc |
| `rounded_rect` | `(x, y, w, h: f32, radius: impl Into<Radius>)` | Rounded rectangle path |
| `close` | `()` | Close the current subpath |

`Angle` accepts any `f32` (interpreted as degrees, matching the
cross-SDK convention) or an explicit `Angle::deg(..)` /
`Angle::rad(..)` constructor. On the wire every angle is emitted
in degrees. `Radius` accepts an `f32` (uniform) or
`Radius::PerCorner { top_left, top_right, bottom_right, bottom_left }`.

```rust
path(vec![
    move_to(10.0, 0.0),
    line_to(20.0, 20.0),
    line_to(0.0, 20.0),
    close(),
])
.fill(Color::hex("#22c55e"))
```

## Fill and stroke

Fill and stroke are independent; either, both, or neither can be
set on a shape.

```rust
rect(0.0, 0.0, 100.0, 50.0)
    .fill(Color::hex("#3b82f6"))
    .stroke(Color::hex("#333"))
    .stroke_width(2.0)
    .stroke_cap(LineCap::Round)
    .stroke_join(LineJoin::Miter)
    .stroke_dash(&[5.0, 3.0], 0.0)
```

`.fill(impl Into<Background>)` accepts any `Color` or `Gradient`.
For inline linear gradients on canvas shapes, use
`.fill_gradient(x1, y1, x2, y2, stops)` where `stops` is a slice
of `(offset, hex_color)` pairs. For gradients shared across
shapes, build one with `linear_gradient(start, end, stops)` and
pass it to `.fill(..)`:

```rust
let sky = linear_gradient(
    (0.0, 0.0),
    (0.0, 200.0),
    [(0.0, Color::hex("#1a1a2e")), (1.0, Color::hex("#0f3460"))],
);

rect(0.0, 0.0, 400.0, 200.0).fill(sky.clone())
```

`FillRule::NonZero` is the default for closed paths;
`FillRule::EvenOdd` toggles regions by crossing count. Set via
`.fill_rule(FillRule::EvenOdd)` on `rect`, `circle`, and `path`.

`LineCap` values are `Butt` (default), `Round`, `Square`.
`LineJoin` values are `Miter` (default), `Round`, `Bevel`.
`.stroke_dash(segments, offset)` takes alternating dash/gap
lengths and an initial offset into the pattern.

## Transforms

Transforms apply to groups only, not to individual shapes. They
are applied in declaration order and recorded in the `transforms`
property on the group:

| Method | Signature | Description |
|---|---|---|
| `translate` | `(x, y: f32) -> Self` | Move the group |
| `rotate` | `(angle: impl Into<Angle>) -> Self` | Rotate around the group origin |
| `scale_xy` | `(x, y: f32) -> Self` | Non-uniform scale |
| `scale_uniform` | `(factor: f32) -> Self` | Uniform scale |

```rust
group("tile")
    .translate(100.0, 50.0)
    .rotate(45.0)                      // 45 degrees
    .rotate(Angle::rad(std::f32::consts::FRAC_PI_4))  // explicit radians
    .scale_uniform(1.25)
    .child(rect(0.0, 0.0, 40.0, 40.0).fill(Color::hex("#ef4444")))
```

The group's `x(v)` and `y(v)` setters are separate positional
offsets that apply before the transform list.

## Clipping

`clip(x, y, w, h)` on a group restricts drawing to a rectangular
region. One clip per group.

```rust
group("window")
    .clip(0.0, 0.0, 80.0, 80.0)
    .child(circle(40.0, 40.0, 60.0).fill(Color::hex("#3b82f6")))
```

## Text, images, and SVG

`canvas_text(x, y, content)` draws a text run at a fixed position.
Setters: `size(f32)`, `fill(impl Into<Background>)`, `font(Font)`,
`align_x(Align)`, `align_y(Align)`, `opacity(f32)`.

`canvas_image(x, y, source)` embeds a raster image loaded from a
file path. Setters: `width(f32)`, `height(f32)`, `rotation(impl
Into<Angle>)`, `opacity(f32)`.

`canvas_svg(x, y, source)` embeds an SVG source string (not a
path). Setters: `width(f32)`, `height(f32)`, `opacity(f32)`.

```rust
layer("icons").children([
    canvas_svg(10.0, 8.0, &include_str!("../assets/save.svg")).width(20.0).height(20.0),
    canvas_image(50.0, 8.0, "assets/logo.png").width(32.0).height(32.0).opacity(0.8),
])
```

## Interactive regions

`interactive(id)` returns a `GroupBuilder` with `on_click(true)`
pre-configured. Everything else is the same as `group(id)`. Use
it to wrap shapes that should respond to clicks, hover, focus, or
drag:

```rust
interactive("save")
    .cursor(CursorStyle::Pointer)
    .focusable(true)
    .a11y(&A11y::new().role(Role::Button).label("Save"))
    .child(canvas_svg(0.0, 0.0, &include_str!("../assets/save.svg"))
        .width(36.0)
        .height(36.0))
```

The interactive group's ID scopes its child events under the
canvas, so events arrive as `id: "save", scope: ["drawing", ..]`.
See [Scoped IDs](scoped-ids.md) for how this composition works.

### Interaction setters

Group-level setters for hit testing and events:

| Method | Signature | Description |
|---|---|---|
| `on_click` | `(enabled: bool) -> Self` | Emit click events |
| `on_hover` | `(enabled: bool) -> Self` | Emit enter / exit events |
| `draggable` | `(enabled: bool) -> Self` | Emit drag / drag_end events |
| `drag_axis` | `(axis: DragAxis) -> Self` | Constrain drag direction |
| `drag_bounds` | `(min_x, max_x, min_y, max_y: f32) -> Self` | Limit drag region |
| `focusable` | `(enabled: bool) -> Self` | Include in Tab order |
| `cursor` | `(c: CursorStyle) -> Self` | Cursor style on hover |
| `tooltip` | `(text: &str) -> Self` | Tooltip on hover |
| `hit_rect` | `(x, y, w, h: f32) -> Self` | Custom hit region |

### Visual feedback

`hover_style`, `pressed_style`, and `focus_style` take a
`PropValue` describing fill / stroke / opacity overrides. They
are available on both `GroupBuilder` (applies to the whole group)
and each shape builder (applies to the individual shape when its
parent interactive group is in that state).

Additional focus-ring controls on groups:

| Method | Signature | Description |
|---|---|---|
| `show_focus_ring` | `(enabled: bool) -> Self` | Toggle the default ring |
| `focus_ring_radius` | `(r: f32) -> Self` | Corner radius of the ring |

Canvas is a raw surface with no intrinsic semantic structure.
Interactive regions need explicit `a11y` annotations to be visible
to assistive technology; see [accessibility](accessibility.md).

## Drag axes

`DragAxis` constrains a draggable group's motion:

| Variant | Behaviour |
|---|---|
| `DragAxis::Both` | Unconstrained (default when not set) |
| `DragAxis::X` | Horizontal only |
| `DragAxis::Y` | Vertical only |

`drag_bounds(min_x, max_x, min_y, max_y)` clamps the drag to a
rectangle. Both setters can be combined with `draggable(true)`:

```rust
interactive("handle")
    .draggable(true)
    .drag_axis(DragAxis::X)
    .drag_bounds(0.0, 400.0, 0.0, 0.0)
    .child(circle(0.0, 16.0, 12.0).fill(Color::hex("#3b82f6")))
```

## Animation integration

Most numeric canvas props (shape sizes, colours, opacity, stroke
width, transform values) accept `impl Into<Animatable<T>>`, so a
`Transition`, `Spring`, or `Sequence` can be plugged directly into
a setter. The renderer interpolates on each frame; the SDK sees
only target values and descriptors. See the animation reference
(forthcoming) and [commands](commands.md) for how animations are
dispatched.

```rust
circle(200.0, 150.0, model.pulse_radius)
    .fill(Color::hex("#3b82f6"))
```

If `pulse_radius` is an `Animatable<f32>` driven by a
`Subscription::on_animation_frame`, the circle breathes without
any per-frame SDK work.

## See also

- [Built-in widgets](built-in-widgets.md) for the canvas widget's
  top-level props and the broader widget catalog.
- [Events](events.md) for the pointer and keyboard event shapes
  delivered to canvas and its interactive regions.
- [Themes and styling](themes-and-styling.md) for `Color`,
  `Gradient`, and `Background`.
- [Accessibility](accessibility.md) for annotating interactive
  canvas regions.
- [Scoped IDs](scoped-ids.md) for how canvas element IDs compose
  with the parent canvas ID.
