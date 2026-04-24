# Subscriptions

Subscriptions are declarative event sources. The runtime calls
`App::subscribe` after every update, diffs the returned list
against the active set, and starts or stops the individual
sources so the next cycle matches the returned list exactly.
Subscription types and constructors live in
`plushie::subscription` and are re-exported from
`plushie::prelude`.

## Declaring subscriptions

The `App` trait method takes the model by shared reference and
returns a `Vec<Subscription>`:

```rust
fn subscribe(model: &Self::Model) -> Vec<Subscription>
```

The default implementation returns `vec![]`. Returning a
different list is how an app signals that the runtime should
start or stop timers, keyboard listeners, pointer streams, or
any other subscription-driven event source.

Because `subscribe` is a function of the model, subscriptions
are conditional by default. If a timer belongs only in a certain
state, include it in the list when that state is active and omit
it otherwise. The runtime handles start-up and tear-down.

```rust
fn subscribe(model: &Self::Model) -> Vec<Subscription> {
    let mut subs = vec![Subscription::on_key_press()];
    if model.auto_save && model.dirty {
        subs.push(Subscription::every(Duration::from_secs(1), "auto_save"));
    }
    subs
}
```

Returning the same list every cycle costs a key-set comparison
and nothing else. The runtime short-circuits when the diff is
empty.

## Constructors

All constructors live on the `Subscription` type in
`plushie::subscription`.

| Constructor | Signature | Description |
|---|---|---|
| `every` | `(interval: Duration, tag: &str) -> Subscription` | Recurring timer. Tag is embedded in the delivered `TimerEvent`. |
| `on_key_press` | `() -> Subscription` | `KeyEvent` with `event_type = Press`. |
| `on_key_release` | `() -> Subscription` | `KeyEvent` with `event_type = Release`. |
| `on_modifiers_changed` | `() -> Subscription` | `ModifiersEvent` whenever Shift, Ctrl, Alt, Logo, or Command state changes. |
| `on_window_event` | `() -> Subscription` | `WindowEvent` for every window lifecycle phase. |
| `on_window_open` | `() -> Subscription` | `WindowEvent` with `event_type = Opened`. |
| `on_window_close` | `() -> Subscription` | `WindowEvent` with `event_type = CloseRequested`. |
| `on_window_resize` | `() -> Subscription` | `WindowEvent` with `event_type = Resized`. |
| `on_window_focus` | `() -> Subscription` | `WindowEvent` with `event_type = Focused`. |
| `on_window_unfocus` | `() -> Subscription` | `WindowEvent` with `event_type = Unfocused`. |
| `on_window_move` | `() -> Subscription` | `WindowEvent` with `event_type = Moved`. |
| `on_pointer_move` | `() -> Subscription` | Pointer movement (coalescable). |
| `on_pointer_button` | `() -> Subscription` | Pointer button press and release. |
| `on_pointer_scroll` | `() -> Subscription` | Pointer scroll wheel (coalescable). |
| `on_pointer_touch` | `() -> Subscription` | Touch input (press, move, release). |
| `on_ime` | `() -> Subscription` | `ImeEvent` for input-method composition. |
| `on_theme_change` | `() -> Subscription` | `SystemEvent` with `event_type = SystemTheme` on OS theme change. |
| `on_animation_frame` | `() -> Subscription` | `SystemEvent` with `event_type = AnimationFrame` per vsync tick. |
| `on_file_drop` | `() -> Subscription` | `WindowEvent` when files are dropped on a window. |
| `on_event` | `() -> Subscription` | Catch-all. Delivers every renderer event. |

`on_window_event` is a superset of the specific variants. Do
not pair it with `on_window_resize`, `on_window_focus`, and so
on, or matching events will be delivered twice.

`on_pointer_*` subscriptions are global. They deliver a
`WidgetEvent` whose ID is the window ID and whose scope is
empty. For widget-local pointer handling, use the
[`pointer_area`](built-in-widgets.md#pointer_area) widget
instead.

`on_animation_frame` drives SDK-side tweens
(`plushie::animation::Tween`). Renderer-side transitions and
springs (`Transition`, `Spring`) do not need this subscription;
they run inside the renderer.

## Chainable modifiers

Both modifier methods take the subscription by value and return
the modified subscription.

```rust
Subscription::on_pointer_move().max_rate(60)
Subscription::on_key_press().for_window("settings")
Subscription::on_key_press().for_window("editor").max_rate(30)
```

### `.max_rate(rate: u32) -> Subscription`

Caps how often the subscription delivers events, in events per
second. The renderer coalesces intermediate events and delivers
the most recent state at each interval.

A rate of `0` keeps the subscription active (the renderer still
tracks the underlying state) but stops delivering events to the
app. Useful when an app wants presence tracking without the
per-frame cost.

`max_rate` applies to renderer-side subscriptions. Timer
subscriptions control their frequency through the `interval`
argument to `every`.

### `.for_window(window_id: &str) -> Subscription`

Scopes a subscription to a single window. Keyboard and pointer
events from other windows are not delivered.

For a whole set of subscriptions that all target the same
window, the associated function `Subscription::window_group`
maps `for_window` over an iterator:

```rust
Subscription::window_group("settings", [
    Subscription::on_key_press(),
    Subscription::on_pointer_move(),
])
```

`window_group` returns a `Vec<Subscription>` that can be
extended or concatenated into the list returned from `subscribe`.

## Examples

### Timer tick

```rust
use std::time::Duration;
use plushie::prelude::*;

impl App for Clock {
    type Model = Self;

    fn subscribe(_model: &Self) -> Vec<Subscription> {
        vec![Subscription::every(Duration::from_secs(1), "tick")]
    }

    fn update(model: &mut Self, event: Event) -> Command {
        if let Some(timer) = event.as_timer() {
            if timer.tag == "tick" {
                model.now = chrono::Local::now();
            }
        }
        Command::none()
    }
}
```

### Keyboard chord

```rust
use plushie::prelude::*;
use plushie_core::Key;

fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![Subscription::on_key_press()]
}

fn update(model: &mut Self, event: Event) -> Command {
    if let Some(key) = event.as_key_press() {
        if key.modifiers.command && key.key == Key::Char("s".into()) {
            return save(model);
        }
    }
    Command::none()
}
```

The `command` modifier resolves to Cmd on macOS and Ctrl on
Linux and Windows, so the same match arm covers both.

### Pointer tracking with rate limit

```rust
use plushie::prelude::*;

fn subscribe(model: &Self::Model) -> Vec<Subscription> {
    if model.tracking {
        vec![Subscription::on_pointer_move().max_rate(60)]
    } else {
        vec![]
    }
}
```

When `tracking` flips to `false`, the subscription disappears
from the list and the runtime tells the renderer to stop
emitting pointer-move events.

### Multi-window scope

```rust
fn subscribe(model: &Self::Model) -> Vec<Subscription> {
    let mut subs = vec![Subscription::on_window_event()];
    if model.editor_open {
        subs.extend(Subscription::window_group("editor", [
            Subscription::on_key_press(),
            Subscription::on_ime(),
        ]));
    }
    subs
}
```

## Event flow

A subscription adds an event source. When the source fires, the
runtime wraps the payload in the appropriate `Event` variant
and calls `update(&mut model, event)`. The loop is the same
regardless of whether the event originated in a widget, a
subscription, or a command result:

```
subscribe -> runtime arms source -> event fires ->
    runtime delivers Event -> update(&mut model, event) -> view
```

High-frequency sources (pointer moves, pointer scroll,
animation frames, window resizes while dragging) are
**coalescable**: the renderer keeps only the latest value per
source within each rate-limit window and drops the rest. This
is true even without `max_rate`; the rate limit just makes the
window explicit. A 16 ms `every` subscription does not deliver
a burst of backlogged ticks after a slow `update`; at most one
tick is queued at a time.

## Diffing

The runtime keys each subscription by `(kind, tag)`:

- Timer subscriptions key by `("every", tag)`. The tag comes
  from the second argument to `every`.
- Renderer subscriptions key by `(kind, tag)` where the
  default tag is the wire identifier (`"on_key_press"`,
  `"on_pointer_move"`). Adding `.for_window("main")` changes
  the tag to `"main#on_key_press"` so a per-window subscription
  does not collide with a global one.

Each cycle the runtime sorts the returned list by key and
compares it against the previous cycle:

- **Unchanged key set**: no work beyond checking whether any
  subscription's `max_rate` changed.
- **New key**: start the timer or send a subscribe message to
  the renderer.
- **Removed key**: cancel the timer or send an unsubscribe
  message.
- **Changed `max_rate` on an existing key**: re-send the
  subscribe message with the new rate.

Duplicate keys in the returned list are equivalent to a single
entry. Only one subscription per key is active at a time, per
window if scoped.

## See also

- [Events](events.md) for the event types subscriptions
  deliver.
- [Commands](commands.md) for the other direction: effects
  and renderer operations issued from `update`.
- [App lifecycle](app-lifecycle.md) for how `subscribe` fits
  with `init`, `update`, and `view`.
- [Windows and layout](windows-and-layout.md) for multi-window
  setups and window IDs referenced by `.for_window`.
- [Built-in widgets](built-in-widgets.md) for per-widget
  alternatives like `pointer_area`.
