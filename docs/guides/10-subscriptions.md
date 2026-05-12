# Subscriptions

So far every event has come from direct widget interaction: a
button click, a checkbox toggle, a slider drag. Other events
originate outside the widget tree: a keyboard shortcut, a timer
tick, the renderer reporting a new vsync tick, a window close
request. Those reach your app through **subscriptions**.

This chapter shows how to declare subscriptions from the `App`
trait, walks through timers, keyboard, pointer, window, and
animation-frame sources, and ends with the clock example wired
end to end. The [subscriptions reference](../reference/subscriptions.md)
lists every constructor and modifier; this chapter covers the
patterns you reach for first.

## Declaring subscriptions

The `App` trait includes an optional `subscribe` method:

```rust
fn subscribe(model: &Self::Model) -> Vec<Subscription>;
```

It takes the model by shared reference and returns a
`Vec<Subscription>`. The default implementation returns
`vec![]`. Override it when the app needs event sources outside
the widget tree.

```rust
use plushie::prelude::*;

impl App for Editor {
    type Model = Self;

    fn subscribe(_model: &Self) -> Vec<Subscription> {
        vec![Subscription::on_key_press()]
    }

    // init, update, view as usual
}
```

The runtime calls `subscribe` after every update, diffs the
returned list against the active set, and starts or stops the
underlying sources so the next cycle matches exactly. You never
call start or stop by hand. Describe the desired set; the
runtime reconciles.

Because the list is a function of the model, subscriptions are
conditional by default. Drop a subscription from the list and
its source stops. Add it back and the source starts again on
the next cycle.

## Timers

`Subscription::every` fires on a recurring interval:

```rust
use std::time::Duration;
use plushie::prelude::*;

fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![Subscription::every(Duration::from_millis(16), "tick")]
}
```

The second argument is a **tag** that rides along in the
delivered `TimerEvent`. Two timers with different tags run
independently:

```rust
vec![
    Subscription::every(Duration::from_secs(1), "clock"),
    Subscription::every(Duration::from_millis(16), "frame"),
]
```

Match on the tag in `update`:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(WidgetMatch::Timer("clock")) => next.time = now_string(),
        Some(WidgetMatch::Timer("frame")) => next.frame += 1,
        _ => {}
    }
    (next, Command::none())
}
```

A sixteen-millisecond timer is the classic frame-level cadence
for SDK-side interpolation or manual animation loops. For GPU
vsync, prefer `on_animation_frame` (below).

Timer backlog is bounded: at most one tick is queued at a time,
so a slow `update` does not cause a burst of ticks to fire on
catch-up.

## Keyboard

`Subscription::on_key_press` subscribes to global key presses.
Events arrive as `Event::Key(KeyEvent)`; destructure with the
`as_key_press` accessor:

```rust
use plushie::prelude::*;

fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![Subscription::on_key_press()]
}

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    if let Some(key) = event.as_key_press() {
        match (&key.key, key.modifiers.command) {
            (Key::Char('s'), true) => return (next, model.save()),
            (Key::Char('n'), true) => return (next, model.new_file()),
            (Key::Escape, _) => next.dismiss_error(),
            _ => {}
        }
    }
    (next, Command::none())
}
```

The `command` modifier field resolves to Command on macOS and
Ctrl on Linux and Windows, so a single match arm covers both.
Use `on_key_release` for key-up events, or
`on_modifiers_changed` to track Shift, Ctrl, Alt, and Logo
state changes without a regular key press.

### Scoping to a window

In a multi-window app, `.for_window(id)` limits a subscription
to a specific window. Keyboard events from other windows are
not delivered:

```rust
fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![
        Subscription::on_key_press().for_window("editor"),
        Subscription::on_key_press().for_window("console"),
    ]
}
```

The scope surfaces in `KeyEvent::window_id`, so a single match
arm can still cover multiple windows by reading that field.
When a whole group of subscriptions targets the same window,
`Subscription::window_group` applies `for_window` to each:

```rust
let mut subs = vec![Subscription::on_window_event()];
subs.extend(Subscription::window_group("editor", [
    Subscription::on_key_press(),
    Subscription::on_ime(),
]));
subs
```

The guide [chapter on scoped IDs and multi-window layout](../reference/windows-and-layout.md)
goes deeper on window IDs.

## Pointer

Pointer subscriptions cover global mouse, touch, and pen input.
`on_pointer_move` tracks movement across the whole window:

```rust
Subscription::on_pointer_move().max_rate(60)
```

`.max_rate(rate)` caps delivery to `rate` events per second.
The renderer still tracks the underlying state every frame and
coalesces intermediate events, delivering only the latest value
at each interval. A rate of `0` keeps the subscription active
on the renderer side but delivers no events to the app: useful
for presence tracking without per-frame `update` calls.

`on_pointer_button`, `on_pointer_scroll`, and `on_pointer_touch`
round out the pointer set. All four deliver `WidgetEvent`
structs whose widget ID is the window ID and whose scope is
empty. For pointer handling inside a specific region of the
tree, use the `pointer_area` widget instead; it scopes events
to its own ID and opts in per axis.

```rust
if let Some(WidgetMatch::Move(_, mv)) = event.widget_match() {
    model.cursor = (mv.x, mv.y);
}
```

## Window lifecycle

`Subscription::on_window_event` delivers every lifecycle phase
for every window the app has open: open, close request, resize,
focus, unfocus, move, file drop. Match on
`WindowEventType` in `update`:

```rust
use plushie::event::WindowEventType;

fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![Subscription::on_window_event()]
}

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    if let Some(w) = event.as_window() {
        match w.event_type {
            WindowEventType::CloseRequested if model.dirty => {
                next.prompt_save();
            }
            WindowEventType::Resized => {
                next.width = w.width.unwrap_or(model.width);
                next.height = w.height.unwrap_or(model.height);
            }
            WindowEventType::Focused => {
                next.focused_window = Some(w.window_id.clone());
            }
            _ => {}
        }
    }
    (next, Command::none())
}
```

Narrower constructors exist when the app only cares about one
phase: `on_window_close`, `on_window_resize`, `on_window_focus`,
`on_window_unfocus`, `on_window_open`, `on_window_move`,
`on_file_drop`. Pair the catch-all or a narrower constructor,
but not both: a close event delivered by both
`on_window_event` and `on_window_close` would arrive twice.

## Animation frame

`Subscription::on_animation_frame` fires once per renderer
vsync tick. Use it when the SDK needs to advance per-frame
state in lockstep with the display:

```rust
fn subscribe(model: &Self) -> Vec<Subscription> {
    if model.animating {
        vec![Subscription::on_animation_frame()]
    } else {
        vec![]
    }
}

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    if let Some(sys) = event.as_system() {
        if matches!(sys.event_type, SystemEventType::AnimationFrame) {
            next.advance_tween();
        }
    }
    (next, Command::none())
}
```

Renderer-side `Transition` and `Spring` animations run inside
the renderer and do not need this subscription. Reach for
`on_animation_frame` when the frame-by-frame state lives in the
Rust model (SDK-side tweens, physics simulations, custom
easing).

## Conditional subscriptions

Because `subscribe` reads the model, the active set can change
with state. Drop the subscription from the returned list and
the runtime stops its source on the next cycle; add it back and
the source starts again.

```rust
fn subscribe(model: &Self) -> Vec<Subscription> {
    let mut subs = vec![Subscription::on_key_press()];

    if model.auto_save && model.dirty {
        subs.push(Subscription::every(Duration::from_secs(1), "auto_save"));
    }

    if model.tracking_cursor {
        subs.push(Subscription::on_pointer_move().max_rate(30));
    }

    subs
}
```

No manual start or stop logic. The runtime diffs the list each
cycle and reconciles the live set. A timer that belongs only in
one view stays out of the list in every other state.

## Putting it together: the clock

The `clock` example in `crates/plushie/examples/clock.rs` is
the minimal subscription app. One timer, one `WidgetMatch::Timer`
arm, one label:

```rust
use std::time::Duration;
use plushie::prelude::*;

#[derive(Clone)]
struct Clock {
    time: String,
}

impl App for Clock {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Clock { time: current_time() }, Command::none())
    }

    fn subscribe(_model: &Self) -> Vec<Subscription> {
        vec![Subscription::every(Duration::from_secs(1), "tick")]
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        if let Some(WidgetMatch::Timer("tick")) = event.widget_match() {
            next.time = current_time();
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Clock")
            .child(
                column()
                    .padding(24)
                    .spacing(16.0)
                    .width(Fill)
                    .align_x(Align::Center)
                    .child(text(&model.time).id("clock_display").size(48.0)),
            )
            .into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Clock>()
}
```

Run it with `cargo run -p plushie --example clock`. The display
refreshes every second with no explicit timer management.

For a keyboard-focused counterpart, the `shortcuts` example
subscribes to `on_key_press` and logs every key event to a
scrollable list. Run it with
`cargo run -p plushie --example shortcuts`.

## What's next

Subscriptions cover event sources that push data into the app.
The other direction, side effects the app initiates (file
dialogs, clipboard, HTTP requests, background tasks), runs
through commands. [Chapter 11](11-async-and-effects.md) picks
up there.
