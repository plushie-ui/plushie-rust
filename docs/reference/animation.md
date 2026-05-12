# Animation

Animation descriptors describe motion as prop values. A widget
setter that accepts `impl Into<Animatable<T>>` takes either a
static `T` (no animation) or a descriptor (`Transition`, `Spring`,
`Sequence`). The renderer detects the descriptor during prop
diffing and interpolates locally at the frame loop, so nothing
crosses the wire between renders. See
[direct vs wire](direct-vs-wire.md) for the per-render cost model.

Descriptor types live in `plushie_core::animation` and re-export
through `plushie::animation`. The SDK-only `Tween` for client-side
interpolation lives in `plushie::animation::tween`.

## Declarative vs stateful

Two mechanisms, both in `plushie::animation`:

| Mechanism | Drives | Use when |
|---|---|---|
| `Transition`, `Spring`, `Sequence` | Renderer-side interpolation | You want a prop to move between values without touching the model |
| `Tween` | SDK-side interpolation, driven by `on_animation_frame` | The animated value must live in the model (canvas drawing, physics, logic gating) |

Declarative descriptors cost a single prop write at render time.
Stateful tweens cost a subscription tick, an `update` cycle, a
view call, and a patch per frame.

## Transition

A timed interpolation from the current (or `from`) value to `to`
over `duration` milliseconds with an easing curve.

```rust
use plushie::animation::{Transition, Easing};

let size: Transition<f32> = Transition::new(24.0_f32, 300)
    .easing(Easing::EaseOut)
    .delay(100);
```

| Method | Signature | Description |
|---|---|---|
| `new` | `(to: impl Into<T>, duration_ms: u64) -> Self` | Constructor. Defaults: `EaseInOut`, `delay = 0`, no repeat. |
| `to` | `(v: impl Into<T>) -> Self` | Overwrite the target value. |
| `easing` | `(e: Easing) -> Self` | Set the easing curve. |
| `delay` | `(ms: u64) -> Self` | Delay before interpolation starts. |
| `from` | `(v: impl Into<T>) -> Self` | Explicit start value (ignored after first appearance). |
| `repeat` | `(n: u32) -> Self` | Repeat a finite number of times. |
| `repeat_forever` | `() -> Self` | Loop indefinitely. |
| `auto_reverse` | `(v: bool) -> Self` | Reverse direction on each repeat. |
| `on_complete` | `(tag: &str) -> Self` | Emit a `TransitionComplete` widget event when the animation ends. |
| `looping` | `(to: impl Into<T>, duration_ms: u64) -> Self` | Shortcut for `repeat_forever().auto_reverse(true)`. |

Use `Transition` when you need fixed, predictable timing:
entrance and exit animations, staggered reveals, coordinated
motion across several widgets, progress-style linear sweeps.

### Repeat semantics

`repeat` stores `Repeat::Times(n)`. `repeat_forever` stores
`Repeat::Forever`, which encodes on the wire as `-1`. `looping`
composes `repeat_forever` with `auto_reverse(true)` for ping-pong
cycles.

## Spring

A damped-harmonic-oscillator animation with no fixed duration.
The spring settles when velocity and displacement are both near
zero.

```rust
use plushie::animation::Spring;

let scale: Spring<f32> = Spring::bouncy(1.05_f32);
let custom: Spring<f32> = Spring::new(200.0_f32)
    .stiffness(250.0)
    .damping(18.0);
```

| Method | Signature | Description |
|---|---|---|
| `new` | `(to: impl Into<T>) -> Self` | Constructor. Defaults: `stiffness = 100`, `damping = 10`, `mass = 1`, `velocity = 0`. |
| `to` | `(v: impl Into<T>) -> Self` | Overwrite the target. |
| `stiffness` | `(s: f64) -> Self` | Higher values pull harder toward the target. |
| `damping` | `(d: f64) -> Self` | Higher values reduce overshoot. |
| `mass` | `(m: f64) -> Self` | Higher values add inertia. |
| `velocity` | `(v: f64) -> Self` | Initial velocity. |
| `from` | `(v: impl Into<T>) -> Self` | Explicit start value. |
| `on_complete` | `(tag: &str) -> Self` | Emit `TransitionComplete` when the spring settles. |

Springs interrupt gracefully: when `to` changes mid-animation the
renderer preserves the current velocity into the new target. That
makes them a natural fit for interactive feedback (hover, drag
release, toggle) where the animation may be redirected at any
point.

### Presets

Each preset is a constructor that returns a `Spring` with a named
parameter set.

| Preset | Stiffness | Damping | Feel |
|---|---|---|---|
| `Spring::gentle(to)` | 120 | 14 | Slow, smooth, no overshoot |
| `Spring::snappy(to)` | 200 | 20 | Quick, minimal overshoot |
| `Spring::bouncy(to)` | 300 | 10 | Quick with visible overshoot |
| `Spring::stiff(to)` | 400 | 30 | Very quick, crisp stop |
| `Spring::molasses(to)` | 60 | 12 | Slow, heavy, deliberate |

Tune by feel: high stiffness plus low damping bounces; low
stiffness plus high damping crawls; larger mass delays both start
and stop.

## Easing

`Easing` is a `Copy` enum with named curves plus a custom cubic
bezier. All variants match the standard CSS/easings.net
definitions.

| Family | Variants |
|---|---|
| Linear | `Linear` |
| Sine (default for `Transition`) | `EaseIn`, `EaseOut`, `EaseInOut` |
| Power | `EaseInQuad` / `EaseOutQuad` / `EaseInOutQuad`, `EaseInCubic` / ..., `EaseInQuart` / ..., `EaseInQuint` / ... |
| Exponential | `EaseInExpo`, `EaseOutExpo`, `EaseInOutExpo` |
| Circular | `EaseInCirc`, `EaseOutCirc`, `EaseInOutCirc` |
| Overshoot | `EaseInBack`, `EaseOutBack`, `EaseInOutBack` |
| Oscillating | `EaseInElastic`, `EaseOutElastic`, `EaseInOutElastic` |
| Bounce | `EaseInBounce`, `EaseOutBounce`, `EaseInOutBounce` |
| Custom | `CubicBezier(f32, f32, f32, f32)` |

Rules of thumb:

- Things appearing: `EaseOut*` decelerates into the resting
  position.
- Things disappearing: `EaseIn*` accelerates away.
- Things moving within the UI: `EaseInOut*` (the default).
- Continuous motion (progress bars, spinners): `Linear`.
- Playful entrances: `EaseOutBack` for gentle overshoot.
- Attention grabbers: `EaseOutElastic` sparingly.

`CubicBezier(x1, y1, x2, y2)` matches the CSS `cubic-bezier()`
function. Use <https://cubic-bezier.com/> to design curves
visually.

## Sequence

Chain animation steps on a single prop. Steps run in order; each
step begins at the previous step's end value.

```rust
use plushie::animation::{Sequence, Transition, Spring, Easing};

let max_width: Sequence<f32> = Sequence::new(vec![
    Transition::new(300.0_f32, 200).from(0.0).into(),
    Transition::new(300.0_f32, 500).easing(Easing::Linear).into(),
    Spring::new(0.0_f32).stiffness(200.0).damping(18.0).into(),
]);
```

| Method | Signature | Description |
|---|---|---|
| `new` | `(steps: Vec<AnimationStep<T>>) -> Self` | Construct from a step vector. |
| `on_complete` | `(tag: &str) -> Self` | Emit `TransitionComplete` when the final step finishes. |

`AnimationStep<T>` is `Transition(Transition<T>)` or
`Spring(Spring<T>)`. Both animation types implement
`From<_> for AnimationStep<T>`, so `.into()` inside a `vec![...]`
literal produces the right variant.

## Animatable\<T\>

Widget builder setters that accept animated values take
`impl Into<Animatable<T>>`:

```rust
pub enum Animatable<T: PlushieType> {
    Value(T),
    Transition(Transition<T>),
    Spring(Spring<T>),
    Sequence(Sequence<T>),
}
```

`Animatable<T>` has `From` impls for the underlying `T`, for each
descriptor type, and for a few ergonomic shortcuts:
`&str` / `String` on `Animatable<Color>` accept hex strings,
`Animatable<Background>` accepts `Color`, `&str`, or `Gradient`,
`Animatable<LineHeight>` accepts bare `f32` / `f64`, and
`Animatable<Angle>` accepts bare `f32` / `i32` (degrees).

```rust
use plushie::prelude::*;
use plushie::animation::{Transition, Spring, Easing};

text("title", "Hello")
    .size(24.0_f32)                                       // static
    .color(Transition::new(Color::red(), 300))            // transition
    .line_height(Spring::snappy(1.4_f32));                // spring
```

## Tween

`Tween` is SDK-only: a stateful `f64` interpolator that the app
advances on each `on_animation_frame` tick. Use it when the
animated value drives logic outside the widget tree (canvas
drawing, physics, conditional behaviour).

```rust
use plushie::animation::{Tween, Easing, SpringConfig};

let mut timed = Tween::new(0.0, 100.0, 500)
    .easing(Easing::EaseOutCubic);
timed.start(now_ms);
timed.advance(now_ms + 250);
let v = timed.value();         // Option<f64>

let mut phys = Tween::spring(0.0, 1.0, SpringConfig::bouncy());
```

| Constructor | Signature | Description |
|---|---|---|
| `Tween::new` | `(from: f64, to: f64, duration_ms: u64) -> Tween` | Timed tween with `EaseInOut` default. |
| `Tween::looping` | `(from: f64, to: f64, duration_ms: u64) -> Tween` | Ping-pong forever. |
| `Tween::spring` | `(from: f64, to: f64, config: SpringConfig) -> Tween` | Spring physics tween. Panics on non-positive mass. |

| Method | Signature | Description |
|---|---|---|
| `easing` | `(e: Easing) -> Self` | Easing curve (timed only). |
| `delay` | `(ms: u64) -> Self` | Delay before interpolation (timed only). |
| `duration` | `(ms: u64) -> Self` | Override the duration (timed only). |
| `repeat` | `(n: u32) -> Self` | Finite repeat (timed only). |
| `repeat_forever` | `() -> Self` | Infinite repeat (timed only). |
| `auto_reverse` | `(v: bool) -> Self` | Reverse direction on each repeat. |
| `start` | `(&mut self, timestamp: u64)` | Start at the given monotonic timestamp. |
| `start_once` | `(&mut self, timestamp: u64)` | Start only if not already started. |
| `advance` | `(&mut self, timestamp: u64)` | Advance to the given timestamp. |
| `redirect` | `(&mut self, to: f64, timestamp: u64)` | Redirect to a new target from the current value; springs preserve velocity. |
| `redirect_with` | `(&mut self, to: f64, timestamp: u64, opts: RedirectOpts)` | Redirect with optional easing / duration override (timed only). |
| `value` | `(&self) -> Option<f64>` | Current interpolated value, `None` if not started. |
| `finished` | `(&self) -> bool` | `true` once the animation has reached its end. |
| `running` | `(&self) -> bool` | Started and not finished. |
| `is_spring` | `(&self) -> bool` | Whether this is a spring tween. |

`SpringConfig` carries spring parameters for `Tween::spring`. It
exposes the same presets as `Spring::*`
(`gentle`, `bouncy`, `snappy`, `stiff`, `molasses`) plus builder
setters `stiffness`, `damping`, `mass`, `initial_velocity`.

`RedirectOpts` carries optional `easing` and `duration` overrides
for `redirect_with` on timed tweens.

## Animation frame subscription

`Subscription::on_animation_frame()` returns a renderer-driven
tick, typically one per frame. Each tick delivers a
`SystemEvent` with `event_type = SystemEventType::AnimationFrame`
carrying a monotonic timestamp in the `value` field.

```rust
use std::time::Duration;
use plushie::prelude::*;

fn subscribe(model: &Self::Model) -> Vec<Subscription> {
    if model.animating {
        vec![Subscription::on_animation_frame()]
    } else {
        vec![]
    }
}
```

Gate the subscription on state that actually needs frames. A
permanently-on frame subscription keeps the runtime busy for
every paint regardless of whether anything is animating. Return
an empty list when no tweens are active and the renderer stops
delivering ticks until the next `subscribe` call brings the
subscription back.

Apply `.max_rate(n)` to throttle to `n` events per second when
the full frame rate is not needed. `.for_window(id)` scopes
delivery to a single window.

## `TransitionComplete` widget event

When a `Transition`, `Spring`, or `Sequence` carries
`on_complete(tag)`, the renderer emits a `WidgetEvent` with
`event_type = TransitionComplete` once the animation finishes.
Match it like any other widget event:

```rust
use plushie::prelude::*;
use WidgetMatch::*;

match event.widget_match() {
    Some(TransitionComplete(id)) if id == "sidebar" => {
        model.sidebar_collapsed = true;
    }
    _ => {}
}
```

Use the completion event to chain phases from `update` that the
renderer cannot express alone: removing a widget from the tree
after a collapse, swapping content after a cross-fade, advancing
a multi-step workflow.

## Animatable props

Builder setters that accept `impl Into<Animatable<T>>` are
animatable. Unlisted props snap immediately.

| Prop | Builder(s) | Underlying T |
|---|---|---|
| `size` | `text`, `rich_text`, `text_input`, `text_editor`, `checkbox`, `toggler`, `radio` | `f32` |
| `color` | `text`, `rich_text`, `container`, `svg` (tint) | `Color` |
| `opacity` | `image`, `svg` | `f32` |
| `scale` | `image` | `f32` |
| `rotation` | `image`, `svg` | `Angle` |
| `border_radius` | `image` | `f32` |
| `background` | `container`, `canvas`, `qr_code` | `Background` / `Color` |
| `max_width` | `column`, `row`, `container`, `scrollable` | `f32` |
| `max_height` | `container`, `text_editor` | `f32` |
| `min_height` | `text_input` | `f32` |
| `spacing` | `column`, `row`, `scrollable`, `checkbox`, `radio`, `toggler`, `markdown`, `pane_grid` | `f32` |
| `translate_x`, `translate_y` | `floating` | `f32` |
| `x`, `y` | `pin` | `f32` |
| `scale` | `floating` | `f32` |
| `offset_x`, `offset_y`, `gap` | `tooltip`, `pointer_area` | `f32` |
| `width`, `height` | `rule` | `f32` |
| `line_height` | text-bearing widgets | `LineHeight` |
| `placeholder_color`, `selection_color` | `text_input`, `text_editor` | `Color` |
| `handle_radius`, `rail_width`, `rail_color` | `slider`, `vertical_slider` | `f32` / `Color` |
| `scrollbar_width`, `scrollbar_margin`, `scroller_width`, `scrollbar_color`, `scroller_color` | `scrollable` | `f32` / `Color` |
| `divider_width`, `divider_color`, `min_size`, `leeway` | `pane_grid` | `f32` / `Color` |
| `cell_size`, `cell_color`, `background` | `qr_code` | `f32` / `Color` |
| `text_size`, `h1_size`, `h2_size`, `h3_size`, `code_size`, `link_color` | `markdown` | `f32` / `Color` |
| `header_text_size`, `row_text_size` | `table` | `f32` |
| `menu_height` | `pick_list` | `f32` |

Props typed as `Length` (`Length::Fill`, `Length::Fixed(n)`,
`Length::FillPortion(n)`) are not animatable because they are
layout directives, not numbers. Use `max_width` or `max_height`
for animated size.

## Triggering animations from commands

Animations are prop values, so they start whenever the next view
renders a new descriptor. To trigger one from `update`, return a
next model that carries the descriptor. A `Command` is not required.

```rust
use plushie::prelude::*;
use plushie::animation::{Transition, Easing};
use WidgetMatch::*;

fn update(model: &Self::Model, event: Event) -> (Self::Model, Command) {
    let mut next = model.clone();
    if let Some(Click(id)) = event.widget_match() {
        if id == "toggle" {
            next.sidebar_open = !model.sidebar_open;
        }
    }
    (next, Command::none())
}

fn view(model: &Self::Model, _widgets: &mut WidgetRegistrar) -> ViewList {
    let width = if model.sidebar_open { 250.0_f32 } else { 0.0_f32 };
    window("main")
        .child(
            container()
                .id("sidebar")
                .max_width(Transition::new(width, 250).easing(Easing::EaseInOut)),
        )
        .into()
}
```

Commands enter the picture when the animation should start as a
side effect of something else: a delayed trigger (`Command::task`
returns a tag, `update` flips a flag, next render carries the
descriptor), a chained phase (handle a
`TransitionComplete` match and return a follow-up command), or an
external signal (async result sets a model field whose descriptor
depends on it).

## Testing

The test harness exposes two helpers for animated state:

- `session.advance_frame(timestamp_ms)` advances all renderer-side
  animations to the given timestamp. The test loop calls `view`,
  diffs the tree, and updates the snapshot just as a real frame
  would.
- `session.skip_transitions()` fast-forwards every in-flight
  transition to completion (implemented as
  `advance_frame(10_000)`).

```rust
let mut session = TestSession::<MySidebar>::start();
session.click("toggle");
session.advance_frame(150);
// sidebar is mid-animation here
session.skip_transitions();
session.assert_text("sidebar", "Sidebar contents");
```

## See also

- [Built-in widgets](built-in-widgets.md)
- [Events](events.md)
- [Subscriptions](subscriptions.md)
- [Direct vs wire](direct-vs-wire.md)
- [Commands](commands.md)
