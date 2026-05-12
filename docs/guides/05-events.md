# Events

Every interaction in a Plushie app produces an event. A button click, a
keystroke in a text input, a checkbox toggle, a window resize. Each one
arrives in your `update` method as an `Event` value. Understanding the
shape of those events and how to match on them is the core skill for
building anything beyond a static layout.

This chapter extends the counter from [chapter 3](03-your-first-app.md)
with more interactions, introduces widget-level and keyboard events, and
ends with a tour of the remaining event families. For the full taxonomy
and field lists, see the [events reference](../reference/events.md).

## How an event reaches your app

The Elm loop is `init` -> `view` -> `update`. After the initial render,
everything that happens in the renderer arrives at `update`:

```rust
fn update(model: &Self::Model, event: Event) -> (Self::Model, Command);
```

Events come from three sources:

- **Widgets** emit them directly (a button click, a text-input keystroke).
- **Subscriptions** emit them in response to global state changes (a key
  press, a timer tick, a window resize). Subscriptions are declared in
  `fn subscribe(..) -> Vec<Subscription>`; see
  [chapter 10](10-subscriptions.md).
- **Commands** emit them as async results (a file dialog response, a
  completed HTTP request).

Each source produces a different `Event` variant. Your `update` returns
the next model plus a `Command` (often `Command::none()`), and the
runtime re-runs `view` to diff the tree and ship patches to the
renderer.

## Widget events

Almost every interactive widget emits a `WidgetEvent`. Rather than match
on the raw `Event::Widget(..)` variant and destructure the inner struct
by hand, use the typed helper:

```rust
match event.widget_match() {
    Some(WidgetMatch::Click(id)) if id == "save" => model.save(),
    _ => {}
}
```

`Event::widget_match()` returns `Option<WidgetMatch<'_>>`. Each
`WidgetMatch` variant carries the widget ID and the typed primary value
for that interaction. A `Click` has just the ID. An `Input` has a `&str`.
A `Toggle` has a `bool`. A `Slide` has an `f64`. This is how you pattern
match on interactions; you rarely need to look at `Event::Widget(..)`
directly.

### Extending the counter

The counter from chapter 3 handled two clicks and nothing else. Here is
an expanded version that adds a reset button, a label input, and a step
slider. Every interaction is one arm of a single `match`:

```rust
use plushie::event::WidgetMatch::*;
use plushie::prelude::*;

#[derive(Clone)]
struct Counter {
    count: i32,
    step: i32,
    label: String,
}

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            Counter {
                count: 0,
                step: 1,
                label: String::new(),
            },
            Command::none(),
        )
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        match event.widget_match() {
            Some(Click(id)) if id == "inc" => next.count += next.step,
            Some(Click(id)) if id == "dec" => next.count -= next.step,
            Some(Click(id)) if id == "reset" => next.count = 0,
            Some(Input(id, value)) if id == "label" => {
                next.label = value.to_string();
            }
            Some(Slide(id, value)) if id == "step" => {
                next.step = value as i32;
            }
            _ => {}
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Counter")
            .child(
                column()
                    .padding(16)
                    .spacing(8.0)
                    .child(text_input("label", &model.label).placeholder("Label"))
                    .child(text(&format!("{}: {}", model.label, model.count)).id("count"))
                    .child(slider("step", (1.0, 10.0), model.step as f32))
                    .child(
                        row()
                            .spacing(8.0)
                            .children([
                                button("inc", "+"),
                                button("dec", "-"),
                                button("reset", "Reset"),
                            ]),
                    ),
            )
            .into()
    }
}
```

Every widget that carries an ID is reachable through this one match.
Notice that `Input` and `Slide` borrow their values (`&str` and `f64`),
so the arm converts to `String` or casts to `i32` inline. The `..` in
the `Click` destructure ignores fields the arm does not need; only
pointer-adjacent variants (`Press`, `DoubleClick`, `Move`) carry extra
fields.

### Common widget match shapes

| Variant | Typed value | Emitted by |
|---|---|---|
| `Click(id)` | | `button` |
| `Input(id, value)` | current text (`&str`) | `text_input`, `text_editor` |
| `Toggle(id, value)` | new state (`bool`) | `checkbox`, `toggler` |
| `Submit(id, value)` | submitted text (`&str`) | `text_input` with `on_submit(true)` |
| `Select(id, value)` | selected option (`&str`) | `pick_list`, `combo_box`, `radio` |
| `Slide(id, value)` | live value (`f64`) | `slider`, `vertical_slider` |
| `SlideRelease(id, value)` | committed value (`f64`) | `slider`, `vertical_slider` |

The full catalog lives in the [events reference](../reference/events.md).

### Importing the variants

`WidgetMatch` variants live in `plushie::event::WidgetMatch`. Glob-import
inside a function to keep the arms short:

```rust
fn update(model: &Model, event: Event) -> (Model, Command) {
    let mut next = model.clone();
    use plushie::event::WidgetMatch::*;
    match event.widget_match() {
        Some(Click(id)) if id == "save" => { /* ... */ }
        _ => {}
    }
    (next, Command::none())
}
```

The reference pages use this convention throughout.

## Scoped IDs for routing list events

The counter uses a single `"inc"` button. In a real app you will often
have many instances of the same widget: a delete button per row, a
checkbox per item. Plushie handles this with **scoped IDs**.

When a named container wraps a group of widgets, the container's ID
becomes a scope. The child's ID stays local:

```rust
column()
    .id("list")
    .children(model.todos.iter().map(|todo| {
        container()
            .id(&todo.id)
            .child(
                row()
                    .spacing(8.0)
                    .child(checkbox("done", todo.done))
                    .child(text(&todo.text))
                    .child(button("delete", "x")),
            )
            .into()
    }))
```

The delete button for todo `t1` is wired as `list/t1/delete` on the
wire. In the update handler, the local ID stays `"delete"` and the item
ID surfaces in the scope chain:

```rust
match event.widget_match() {
    Some(Click(id)) if id == "delete" => {
        if let Some(item_id) = event.scope().and_then(|s| s.first()) {
            model.todos.retain(|t| t.id != *item_id);
        }
    }
    _ => {}
}
```

`event.scope()` returns the reversed ancestor chain (nearest container
first, window last), so `scope[0]` is always the immediate enclosing
named container. The scope chain is what lets one `Click("delete")`
arm handle every row.

Scoped IDs are covered in depth in the
[scoped IDs reference](../reference/scoped-ids.md), and chapter 6
builds a todo list that leans on them heavily.

## Keyboard events

Keyboard events come through two different paths depending on whether
you want global shortcuts or widget-local key handling.

### Global shortcuts via subscription

Declare a keyboard subscription from `subscribe`:

```rust
fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![Subscription::on_key_press()]
}
```

Key presses then arrive as `Event::Key(KeyEvent)`. Use the
`as_key_press` accessor to destructure safely:

```rust
use plushie::prelude::Key;

fn update(model: &Editor, event: Event) -> (Editor, Command) {
    let mut next = model.clone();
    if let Some(key) = event.as_key_press() {
        match (&key.key, key.modifiers.command, key.modifiers.shift) {
            (Key::Char('s'), true, false) => return (next, model.save()),
            (Key::Char('s'), true, true) => return (next, model.save_as()),
            (Key::Escape, _, _) => next.close_dialog(),
            _ => {}
        }
    }
    (next, Command::none())
}
```

The `command` modifier field resolves to Command on macOS and Ctrl on
Linux and Windows, so a single match arm covers both platforms. `Key`
has named variants for navigation, editing, and function keys, plus
`Key::Char(char)` for printable characters and `Key::Named(String)` as
a forward-compatible fallback.

`KeyEvent` also carries `modified_key` (the key after keyboard layout
applies, useful for layout-dependent shortcuts), `physical_key` (layout
independent, useful for game controls), `text` (the printable result,
if any), and `repeat` (true for held-key auto-repeat).

### Widget-level keys

Focused interactive widgets can emit key events through
`WidgetMatch::KeyPress` and `KeyRelease`, scoped to the focused widget's
ID. This is useful when a specific widget needs to react to keys without
subscribing globally:

```rust
match event.widget_match() {
    Some(WidgetMatch::KeyPress(id, key)) if id == "editor" => {
        if key.key == Key::Tab && !key.modifiers.shift {
            model.indent();
        }
    }
    _ => {}
}
```

The widget-level path does not need an `on_key_press` subscription.

## Pointer events

Pointer events (mouse, touch, pen) come from two places:

- **Global** subscriptions: `Subscription::on_pointer_move`,
  `on_pointer_button`, `on_pointer_scroll`, `on_pointer_touch`. These
  deliver `WidgetEvent` whose ID is the window ID and whose scope is
  empty. Appropriate for global drag tracking or overlay cursors.
- **Widget-level** via the `pointer_area` widget, which emits events
  scoped to its own ID.

`pointer_area` opts in per axis:

```rust
pointer_area("canvas-overlay")
    .on_press("primary")
    .on_move(true)
    .on_scroll(true)
    .on_double_click(true)
    .child(canvas("surface").into())
```

The `on_press` setter takes a tag string; only enabled axes emit
events, which keeps high-frequency streams silent unless the app
actually wants them.

Once enabled, match on the widget variants:

```rust
use plushie::prelude::{MouseButton, PointerKind};

match event.widget_match() {
    Some(Press(id, press)) if id == "canvas-overlay"
        && press.pointer == PointerKind::Mouse
        && press.button == MouseButton::Left =>
    {
        model.select_at(press.x, press.y);
    }
    Some(Press(id, press)) if id == "canvas-overlay"
        && press.pointer == PointerKind::Touch =>
    {
        if let Some(finger) = press.finger {
            model.touch_start(finger, press.x, press.y);
        }
    }
    Some(Move(id, mv)) if id == "canvas-overlay" => {
        model.hover_at(mv.x, mv.y);
    }
    _ => {}
}
```

`PointerKind` is `Mouse`, `Touch`, or `Pen`. `MouseButton` covers the
usual `Left`, `Right`, `Middle`, `Back`, `Forward`. The `finger` field
is `Some(u64)` for touch events and `None` for mouse and pen, which
lets multi-touch code key off finger ID.

`Move` and `Scroll` are coalescable: if several arrive between
`update` calls, only the latest is delivered. This keeps the loop
responsive under fast drags without any app-side throttling.

## Window events

Window lifecycle events arrive as `Event::Window(WindowEvent)` when a
window subscription is active. The general catch-all is
`Subscription::on_window_event`; narrower variants
(`on_window_resize`, `on_window_close`, `on_window_focus`) exist for
apps that only care about one phase.

```rust
use plushie::event::WindowEventType;

fn subscribe(_model: &Self) -> Vec<Subscription> {
    vec![Subscription::on_window_event()]
}

fn update(model: &Editor, event: Event) -> (Editor, Command) {
    let mut next = model.clone();
    if let Some(w) = event.as_window() {
        match w.event_type {
            WindowEventType::CloseRequested if model.dirty => {
                next.prompt_save();
                return (next, Command::none());
            }
            WindowEventType::CloseRequested => {
                return (next, Command::renderer(RendererOp::window_close(&w.window_id)));
            }
            WindowEventType::Resized => {
                next.width = w.width.unwrap_or(model.width);
                next.height = w.height.unwrap_or(model.height);
            }
            WindowEventType::Focused => next.focused_window = Some(w.window_id.clone()),
            _ => {}
        }
    }
    (next, Command::none())
}
```

The per-field options (`x`, `y`, `width`, `height`, `path`,
`scale_factor`) are populated only for the relevant event types.
`CloseRequested` is the place to intercept a close: save prompts,
unsaved-change dialogs, or anything else that should run before the
window actually closes. See the
[events reference](../reference/events.md) for the full variant list.

## Pattern-matching idioms

A few shapes recur often enough to name.

### Match the variant, then the ID

When the variant itself is enough (a click only needs `id`), put the ID
in a guard so the structural match stays shallow:

```rust
Some(Click(id)) if id == "save" => model.save(),
```

### Destructure the payload inline

When the arm needs the value, destructure in the pattern:

```rust
Some(Input(id, value)) if id == "search" => model.query = value.to_string(),
Some(Toggle(id, value)) if id == "dark" => model.dark_mode = value,
```

`value` is borrowed for variants that carry strings, so convert to
`String` with `to_string()` when the model needs ownership.

### Combine scope and ID

Match on ID in the pattern, read the scope chain inside the arm:

```rust
Some(Click(id)) if id == "delete" => {
    let Some(item_id) = event.scope().and_then(|s| s.first()) else {
        return Command::none();
    };
    model.todos.retain(|t| t.id != *item_id);
}
```

The `let ... else` short-circuit is the ergonomic way to narrow a
`None` scope to a real return; guards cannot bind the scope head for
reuse, so they pair well with inline destructuring in the arm body.

### Always include a wildcard

`Event` is non-exhaustive; new variants may appear in future releases.
End every match with `_ => {}` or `_ => Command::none()` so code keeps
compiling without change:

```rust
match event.widget_match() {
    Some(Click(id)) if id == "save" => { /* ... */ }
    _ => {}
}
```

## What's next

This chapter covered the widget, keyboard, and window paths, which are
the ones you reach for daily. Async, stream, effect, IME, and system
events arrive through the same `update` function; each gets its own
chapter when its producing command or subscription is introduced.

The counter handled exactly one widget per interaction. In
[chapter 6](06-lists-and-inputs.md) we build a todo list that renders
many widgets from a collection, routes events back to the right item
through the scope chain, and handles text submission with a clean
event shape.
