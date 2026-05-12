# Animation

Motion is how the UI confirms that an action landed. A panel that
slides open, a checkbox that eases into its new tint, a badge that
pops when a value changes. This chapter shows how Plushie expresses
those animations and where each tool belongs.

Plushie's animation system is designed around one idea: the renderer
is closer to the screen than your Rust code. Declare the *intent* in
`view`, let the renderer interpolate frame by frame, and the wire
stays quiet between renders. No subscription, no per-frame message,
no model field that ticks. For the full API surface and the method
tables, see the [animation reference](../reference/animation.md).

## Why renderer-side animation

A fade, a resize, a slide: each is a value that changes smoothly
over time. The naive shape, driving the interpolation from the app,
costs a frame subscription, an `update` call, a `view` call, a tree
diff, and a patch. Every frame. For a 60hz animation that is sixty
round trips through the loop per second per animated prop.

Plushie instead lets you hand the renderer a descriptor. A
`Transition` says "go from the current value to 300 over 250
milliseconds, ease-out". The descriptor is written to the prop once,
the renderer interpolates locally, and nothing else crosses the wire
until the target changes or the animation finishes. In direct mode
there is no wire at all, but the argument is the same: the renderer
owns the frame loop, so the interpolation lives there instead of
bouncing through `update` every tick. See
[direct vs wire](../reference/direct-vs-wire.md) for the per-render
cost breakdown.

The payoff is that most animations in a Plushie app are single-line
prop changes. You return a next model from `update` the way you
always do, and the next view render carries the descriptor.

## Transition

A `Transition` interpolates a value from its current state to a
target over a fixed duration. Build one with `Transition::new`, then
attach it to an animatable prop.

```rust
use plushie::animation::{Easing, Transition};
use plushie::prelude::*;

container()
    .id("sidebar")
    .max_width(Transition::new(250.0_f32, 300).easing(Easing::EaseOut))
```

When the target changes on the next render, the renderer interpolates
from wherever the value currently is. Toggle a boolean in the model,
compute a new target in `view`, and the transition runs:

```rust
fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
    let width = if model.open { 250.0_f32 } else { 0.0_f32 };
    window("main")
        .child(
            container()
                .id("sidebar")
                .max_width(Transition::new(width, 300).easing(Easing::EaseInOut)),
        )
        .into()
}
```

Useful modifiers:

```rust
Transition::new(300.0_f32, 300)
    .easing(Easing::EaseOut)
    .delay(80)             // wait before starting
    .from(0.0_f32)          // explicit starting value on first appearance
    .on_complete("grew")   // emit TransitionComplete when it finishes
```

`from` only applies the first time the widget appears. A staggered
reveal that slides children into place:

```rust
column()
    .id("feed")
    .children(model.items.iter().enumerate().map(|(i, item)| {
        container()
            .id(&item.id)
            .max_width(
                Transition::new(300.0_f32, 250)
                    .from(0.0_f32)
                    .delay((i as u64) * 40)
                    .easing(Easing::EaseOut),
            )
            .child(text(&item.title))
            .into()
    }))
```

Each entry keeps the animation on its first mount; later renders see
the same `from` but the renderer ignores it after the first appearance.

## Easing

The easing curve shapes the motion inside the duration. `Easing` is
a `Copy` enum covering the standard CSS / easings.net set plus a
custom `CubicBezier(f32, f32, f32, f32)`.

Broad categories:

- Linear for continuous motion (progress sweeps, spinners).
- `EaseIn*` when something accelerates out (dismissals).
- `EaseOut*` when something decelerates in (entrances, the usual
  default for an arrival).
- `EaseInOut*` for transitions that start and end at rest.
- `EaseOutBack` for a gentle overshoot on playful entrances.
- `EaseOutElastic` and `EaseOutBounce` for attention grabbers. Use
  them sparingly.

`CubicBezier` matches the CSS `cubic-bezier()` function, so a curve
designed at <https://cubic-bezier.com/> transfers directly. The full
catalog of named variants is in the
[animation reference](../reference/animation.md).

## Spring

A `Spring` models a damped harmonic oscillator. There is no fixed
duration: the spring settles when velocity and displacement are both
near zero. Build one from a preset or from raw parameters.

```rust
use plushie::animation::Spring;
use plushie::prelude::*;

image("hero", "hero.png")
    .scale(Spring::bouncy(1.05_f32))
```

```rust
Spring::new(200.0_f32)
    .stiffness(250.0)
    .damping(18.0)
    .mass(1.0)
```

Presets cover the usual feel space: `gentle`, `snappy`, `bouncy`,
`stiff`, `molasses`. The reference page lists the stiffness and
damping values behind each one.

The quality that makes springs worth reaching for is interrupt
behaviour. When `to` changes mid-animation, the renderer preserves
the current velocity into the new target. A user who rapidly
toggles a widget never sees the animation jump: it curves smoothly
from wherever it happened to be. Springs are the natural fit for
hover, drag-release, toggle, and anything that can be redirected.

## Sequence

A `Sequence` chains animation steps on a single prop. Each step
begins at the previous step's end value.

```rust
use plushie::animation::{Easing, Sequence, Spring, Transition};

let max_width: Sequence<f32> = Sequence::new(vec![
    Transition::new(300.0_f32, 200).from(0.0_f32).into(),
    Transition::new(300.0_f32, 500).easing(Easing::Linear).into(),
    Spring::new(0.0_f32).stiffness(200.0).damping(18.0).into(),
]);

container()
    .id("banner")
    .max_width(max_width)
```

`Transition` and `Spring` both implement `Into<AnimationStep<T>>`,
so a `vec![ ... .into(), ... .into() ]` literal produces the right
variants. Attach `on_complete("banner-done")` to emit a single
`TransitionComplete` event when the final step finishes.

## `Animatable<T>`

Setters that take animated values accept `impl Into<Animatable<T>>`.
The `Animatable` enum wraps a static `T`, a `Transition<T>`, a
`Spring<T>`, or a `Sequence<T>`, and every one of those has a
`From` impl for `Animatable<T>`. That means the same setter takes
both shapes:

```rust
use plushie::animation::{Easing, Spring, Transition};
use plushie::prelude::*;

text("title", "Hello")
    .size(24.0_f32)                                          // static
    .color(Transition::new(Color::red(), 300))               // transition
    .line_height(Spring::snappy(1.4_f32));                   // spring
```

A handful of ergonomic shortcuts save a line or two:
`Animatable<Color>` accepts a hex `&str`, `Animatable<Background>`
accepts `Color`, `&str`, or `Gradient`, and
`Animatable<LineHeight>` accepts bare `f32`. The [animatable props
table](../reference/animation.md#animatable-props) lists every
widget setter that takes an `Animatable` and the underlying type.

`Length` props (`Length::Fill`, `Length::Fixed(n)`,
`Length::FillPortion(n)`) are layout directives, not numbers, and
are not animatable. Use `max_width` or `max_height` when the size
needs to animate.

## `Tween` for SDK-side animation

Some values cannot live on the widget tree. A physics simulation that
drives a canvas drawing, a multi-phase animation with branching
logic, a value that gates whether other code runs. For these cases
the animated state belongs in the model, and Plushie ships `Tween`
for exactly that.

```rust
use plushie::animation::{Easing, Tween};

struct Model {
    slide: Tween,
}

let mut slide = Tween::new(0.0, 1.0, 400).easing(Easing::EaseOutCubic);
slide.start(now_ms);
```

`Tween` is a stateful `f64` interpolator. It exposes the same easing
curves and repeat modes as `Transition`, plus a spring variant
(`Tween::spring(from, to, SpringConfig::bouncy())`) that uses the
same physics as the declarative `Spring`. Read the current value
with `.value() -> Option<f64>`, advance it with
`.advance(timestamp_ms)`, redirect it with `.redirect(new_to,
timestamp_ms)`. The reference page documents the full method set.

Use `Tween` when the value drives logic outside the tree. For a prop
on a widget, reach for `Transition` or `Spring` first: they are
cheaper and shorter.

## `on_animation_frame` subscription

`Tween` needs something to tick it. That something is
`Subscription::on_animation_frame()`, which delivers one
`SystemEvent` per renderer frame with a monotonic timestamp in the
`value` field.

```rust
use plushie::event::SystemEventType;
use plushie::prelude::*;

impl App for Physics {
    type Model = Self;

    fn subscribe(model: &Self) -> Vec<Subscription> {
        if model.running {
            vec![Subscription::on_animation_frame()]
        } else {
            vec![]
        }
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        if let Some(sys) = event.as_system() {
            if sys.event_type == SystemEventType::AnimationFrame {
                let t = sys.value.as_u64().unwrap_or(0);
                next.slide.advance(t);
            }
        }
        (next, Command::none())
    }
}
```

Gate the subscription on state that actually needs frames.
Returning an empty list when nothing is animating stops the ticks
until the next `subscribe` call brings them back. Apply
`.max_rate(30)` to throttle to 30 events per second where the full
frame rate is overkill, and `.for_window(id)` to scope delivery to
a single window. [Chapter 10](10-subscriptions.md) covers the wider
subscription surface.

## `TransitionComplete`

When a `Transition`, `Spring`, or `Sequence` carries
`on_complete(tag)`, the renderer emits a `TransitionComplete`
widget event once the animation finishes. It matches like any other
widget event:

```rust
use plushie::prelude::*;
use WidgetMatch::*;

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(TransitionComplete(id)) if id == "sidebar" => {
            next.sidebar_collapsed = true;
        }
        _ => {}
    }
    (next, Command::none())
}
```

The event carries the widget's scoped ID, so a list of items
animating in parallel can route completions back to the right entry
through `event.scope()` in the same way widget clicks do (see
[chapter 5](05-events.md)).

Completion events are how you chain phases that the renderer cannot
express alone: removing a widget after it has collapsed, swapping
content after a cross-fade, advancing a multi-step workflow. When
the next phase needs to fire a command, the completion match arm
returns it directly.

## What's next

Animations are prop values. That design is why most of this chapter
fits on one page: you already know how to write `view`, and the
animation story is "write `view` with a `Transition` or a `Spring`
where you previously wrote a number". The renderer does the rest.

The `on_animation_frame` subscription that drives `Tween` is one
corner of a larger surface: window events, keyboard, pointer,
timers, and watchers all arrive through the same `subscribe` hook.
[Chapter 10](10-subscriptions.md) covers the full set.
