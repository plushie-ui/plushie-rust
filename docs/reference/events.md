# Events

Every user interaction, system notification, async result, and
platform-effect response reaches your app as an
[`Event`](../../crates/plushie/src/event.rs). The top-level enum
lives in `plushie::event::Event` and is re-exported from the
prelude. Typed per-variant data lives alongside it; widget-level
interactions are further classified by the `EventType` enum in
`plushie_core::event_type`.

For a gentler introduction, see the
[Events guide](../guides/05-events.md).

## How events flow

The Elm loop is `init` -> `view` -> `update`. After the initial
render, the runtime feeds every inbound event through
`App::update`:

```rust
fn update(model: &mut Self::Model, event: Event) -> Command;
```

`update` takes `&mut Self::Model` and returns a `Command`. Mutate
the model in place; return `Command::none()` when no side effect
is needed. The runtime then calls `view(model, widgets)`, diffs
the resulting `ViewList`, and forwards patches to the renderer
(direct-mode engine or wire subprocess). Events arrive in the
order they were produced, with coalescable high-frequency events
(pointer move, resize) collapsed to the latest value per source.

## Event variants

`Event` is non-exhaustive. Match the variants you care about and
pass the rest through with a wildcard arm.

| Variant | Payload struct | Source |
|---|---|---|
| `Event::Widget` | `WidgetEvent` | Widget callbacks (click, input, drag, etc.) |
| `Event::Key` | `KeyEvent` | `Subscription::on_key_press` / `on_key_release` |
| `Event::Window` | `WindowEvent` | Window lifecycle (open, close, resize, focus, file drop) |
| `Event::Timer` | `TimerEvent` | `Subscription::every` |
| `Event::Async` | `AsyncEvent` | `Command::task` result |
| `Event::Stream` | `StreamEvent` | `Command::stream` emission |
| `Event::Effect` | `EffectEvent` | `Command::effect` response (file dialog, clipboard, notification) |
| `Event::System` | `SystemEvent` | Renderer queries, theme change, animation frame, diagnostics |
| `Event::Modifiers` | `ModifiersEvent` | Modifier state change |
| `Event::Ime` | `ImeEvent` | Input method editor composition |
| `Event::CommandError` | `CommandError` | Renderer error for a command |

`Event` carries convenience accessors that return `Option<&T>`
for common cases: `as_widget`, `as_key_press`, `as_key_release`,
`as_window`, `as_timer`, `as_async`, `as_stream`, `as_effect`,
`as_system`. For widget events, prefer the typed
`widget_match` helper described next.

## Widget events

`Event::widget_match()` returns `Option<WidgetMatch<'_>>`, a
typed destructuring of the inner `WidgetEvent`. Each variant
carries the widget ID and the typed primary value for that
interaction kind. A timer tick surfaces here too
(`WidgetMatch::Timer(tag)`) so simple apps can handle interactions
and ticks in one match block.

```rust
use plushie::prelude::*;
use plushie::event::WidgetMatch::*;

fn update(model: &mut Model, event: Event) -> Command {
    match event.widget_match() {
        Some(Click("save")) => {
            model.save();
            Command::none()
        }
        Some(Input("name", text)) => {
            model.name = text.to_string();
            Command::none()
        }
        Some(Toggle("dark", on)) => {
            model.dark_mode = on;
            Command::none()
        }
        Some(Slide("volume", level)) => {
            model.volume = level;
            Command::none()
        }
        _ => Command::none(),
    }
}
```

The variant list is driven by `EventType` in
`plushie_core::event_type`. Each entry maps one wire family
string to one typed variant.

### Standard interactions

| `WidgetMatch` variant | Typed value | Emitted by |
|---|---|---|
| `Click(id)` | | `button` |
| `DoubleClick(id, PointerPress)` | `x`, `y`, `button`, `pointer`, `modifiers` | `pointer_area` with `on_double_click(true)` |
| `Input(id, &str)` | current text | `text_input`, `text_editor` |
| `Submit(id, &str)` | submitted text | `text_input` with `on_submit(true)` |
| `Paste(id, &str)` | pasted text | `text_input`, `text_editor` with `on_paste(true)` |
| `Toggle(id, bool)` | new state | `checkbox`, `toggler` |
| `Select(id, &str)` | selected option | `pick_list`, `combo_box`, `radio` |
| `Slide(id, f64)` | live value | `slider`, `vertical_slider` |
| `SlideRelease(id, f64)` | committed value | `slider`, `vertical_slider` |
| `Sort(id, &str)` | column key | `table` columns with `sortable(true)` |
| `Open(id)` / `Close(id)` | | `pick_list`, `combo_box`, disclosure widgets |
| `OptionHovered(id, &Value)` | option payload | `combo_box` with `on_option_hovered(true)` |
| `LinkClicked(id, &str)` | URL | `rich_text`, `markdown` |
| `TransitionComplete(id)` | | Animatable props with `on_complete` |

### Focus and status

| `WidgetMatch` variant | Typed value | Description |
|---|---|---|
| `Focused(id)` | | Widget gained keyboard focus |
| `Blurred(id)` | | Widget lost keyboard focus |
| `Status(id, &Value)` | raw status payload | Interaction status transition |
| `KeyBinding(id, &Value)` | binding payload | Declarative `key_bindings` rule matched |

Status names covered by `Status` include `"active"`, `"hovered"`,
`"focused"`, `"pressed"`, `"dragged"`, `"disabled"`, `"opened"`.
Not every widget emits every status: only sliders emit
`"dragged"`, for instance.

### Pointer and canvas

| `WidgetMatch` variant | Typed value | Description |
|---|---|---|
| `Press(id, PointerPress)` | `x`, `y`, `button`, `pointer`, `finger`, `modifiers`, `captured` | Pointer pressed |
| `Release(id, PointerRelease)` | press fields plus `lost: Option<bool>` | Pointer released |
| `Move(id, PointerMove)` | `x`, `y`, `pointer`, `finger`, `modifiers`, `captured` | Pointer moved (coalescable) |
| `Scroll(id, PointerScroll)` | `x`, `y`, `delta_x`, `delta_y`, `pointer`, `modifiers`, `captured` | Wheel or trackpad input (coalescable) |
| `Scrolled(id, ScrollPosition)` | absolute, relative, bounds, content extents | Scrollable viewport moved |
| `Enter(id, PointerBoundary)` / `Exit(id, PointerBoundary)` | optional `x`, `y`, `captured` | Pointer crossed hit region |
| `Drag(id, PointerDrag)` / `DragEnd(id, PointerDrag)` | `x`, `y`, `pointer`, `modifiers`, `captured` | Canvas-element drag gestures |
| `Resize(id, ResizeDimensions)` | `width`, `height` | `responsive`, `sensor` layout callback |

`MouseButton` is `Left`, `Right`, `Middle`, `Back`, or `Forward`.
`PointerKind` is `Mouse`, `Touch`, or `Pen`. The `finger` field
on `PointerPress`, `PointerRelease`, and `PointerMove` is
`Some(u64)` for touch events and `None` for mouse and pen input.

`PointerScroll` reports raw wheel input at pointer coordinates.
`ScrollPosition` reports where a scrollable widget's viewport
ended up after a scroll, which is a different concern. They
destructure through different `WidgetMatch` variants
(`Scroll` vs `Scrolled`) so a match can handle them
independently.

### Widget-level keys

Focused interactive widgets can also emit key events locally,
separate from the global `Event::Key` subscription path:

| `WidgetMatch` variant | Typed value |
|---|---|
| `KeyPress(id, KeyData)` | `key`, `modified_key`, `physical_key`, `modifiers`, `text`, `repeat` |
| `KeyRelease(id, KeyData)` | same shape (no `text`) |

### Pane grid

`pane_grid` carries four pane-specific variants. The typed
payload is currently the raw `&Value`; destructure via
`value.get("...")` for field access:

| `WidgetMatch` variant | Payload keys |
|---|---|
| `PaneResized(id, &Value)` | `split`, `ratio` |
| `PaneDragged(id, &Value)` | `pane`, `target`, `action`, `region`, `edge` |
| `PaneClicked(id, &Value)` | `pane` |
| `PaneFocusCycle(id)` | |

### Custom widget events

Custom widgets (defined via `plushie-widget-sdk`) dispatch
through a single `Custom` arm. The `family` field is the full
wire string (e.g. `"star_rating:select"`), so a widget can have
multiple event kinds distinguished by name:

```rust
match event.widget_match() {
    Some(WidgetMatch::Custom { family: "star_rating:select", id, value }) => {
        let rating = value.get("rating").and_then(|v| v.as_u64()).unwrap_or(0);
        model.rating.insert(id.to_string(), rating);
    }
    _ => {}
}
```

When a finer destructure is needed, drop to the raw `WidgetEvent`:
`Event::as_widget()` exposes `event_type: EventType`,
`scoped_id: ScopedId`, and `value: Value`. `scoped_id.id` is the
local widget ID; `scoped_id.scope` is the reversed ancestor
chain (nearest first, window last); `scoped_id.full` is the
canonical joined path suitable for logging and tests.

## Keyboard events

Global keyboard events arrive as `Event::Key(KeyEvent)` when a
matching `Subscription::on_key_press` / `on_key_release` is
active. The `Key` enum in `plushie_core::key` has named variants
for navigation, editing, modifier, and function keys,
`Char(char)` for single printable characters, and
`Named(String)` as a forward-compatible fallback using the
iced/winit PascalCase name.

```rust
pub struct KeyEvent {
    pub event_type: KeyEventType,          // Press | Release
    pub key: Key,
    pub modified_key: Option<Key>,
    pub physical_key: Option<Key>,
    pub location: KeyLocation,             // Standard | Left | Right | Numpad
    pub modifiers: KeyModifiers,
    pub text: Option<String>,
    pub repeat: bool,
    pub captured: bool,
    pub window_id: Option<String>,
}
```

`KeyEvent::is_press()` and `KeyEvent::is_release()` are
convenience boolean checks; `Event::as_key_press(&self)` and
`Event::as_key_release(&self)` return `Option<&KeyEvent>` gated
by the event phase.

### Modifiers

`KeyModifiers` lives in `plushie_core::protocol` and is
re-exported as `plushie::types::KeyModifiers`. All fields are
plain `bool`; `Default` yields all-false.

| Field | Meaning |
|---|---|
| `shift` | Shift held |
| `ctrl` | Control held |
| `alt` | Alt held (Option on macOS) |
| `logo` | Super / Windows / Command key |
| `command` | Platform-aware: Ctrl on Linux and Windows, Command on macOS |

Match on `command: true` for cross-platform shortcuts; it
resolves to the right key on each platform.

An `Event::Modifiers(ModifiersEvent)` arrives separately when
only the modifier state changes (no key pressed or released).
Subscribe with `Subscription::on_modifiers_changed`.

## Pointer events

Widget-level pointer events come through `WidgetMatch::Press`,
`Release`, `Move`, `Scroll`, `Enter`, `Exit`, plus the canvas
`Drag` / `DragEnd` pair. Raw pointer data types live in
`plushie::event` (re-exported from `plushie_core::pointer`):
`PointerPress`, `PointerRelease`, `PointerMove`, `PointerScroll`,
`PointerBoundary`, `PointerDrag`, `ScrollPosition`.

`pointer_area` owns most pointer emission. Opt in per axis on
the builder:

```rust
pointer_area("canvas-overlay")
    .on_press("primary")
    .on_move(true)
    .on_scroll(true)
    .on_double_click(true)
    .child(canvas("surface").into())
```

### Coalescing and rate limiting

`Move` and `Resize` are coalescable. When multiple events of the
same kind arrive for the same source before the runtime drains
its queue, only the most recent payload is delivered. A
zero-delay timer flushes the pending value before the next
non-coalescable event, preserving ordering.

Every widget builder accepts `event_rate(u32)` to cap events per
second for that widget's source. `0` means unbounded (the
default). Global pointer subscriptions expose the same ceiling
via `.max_rate(u32)` on the subscription.

## Window events

`Event::Window(WindowEvent)` covers every window lifecycle
transition. `window_id` is always present; other fields are
populated only for the relevant event kinds:

```rust
pub struct WindowEvent {
    pub event_type: WindowEventType,
    pub window_id: String,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub path: Option<String>,
    pub scale_factor: Option<f32>,
}
```

| `WindowEventType` | When |
|---|---|
| `Opened` | Window was just created |
| `Closed` | Window has closed |
| `CloseRequested` | User clicked the close control (not yet closed) |
| `Moved` | Window position changed; `x`, `y`, `position` set |
| `Resized` | Window resized; `width`, `height` set |
| `Focused` / `Unfocused` | Keyboard focus gained or lost |
| `Rescaled` | DPI scale changed; `scale_factor` set |
| `FileHovered` | A file is being dragged over the window; `path` set |
| `FileDropped` | A file was dropped; `path` set |
| `FilesHoveredLeft` | A hovered drag left without dropping |

Handle `CloseRequested` to intercept window close (save prompts,
confirmation dialogs). The default window-config flag
`exit_on_close_request` converts the main window's request into
app exit automatically.

## System, timer, async, stream, effect

### System events

`Event::System(SystemEvent)` carries renderer-side queries and
platform signals. Fields: `event_type: SystemEventType`, plus
`tag`, `value`, `id`, `window_id` populated per variant.

| `SystemEventType` | Description |
|---|---|
| `SystemInfo` | Response to a system-info query |
| `SystemTheme` | OS theme reported by the platform |
| `AnimationFrame` | Per-frame tick for animations (requires `Subscription::on_animation_frame`) |
| `ThemeChanged` | Active plushie theme changed |
| `AllWindowsClosed` | Renderer closed its last window |
| `ImageList` | Response to an image-list query |
| `TreeHash` | Response to a tree-hash query |
| `FindFocused` | Response to a focused-widget query |
| `Announce` | Screen-reader announcement delivered |
| `Diagnostic` | Validation or warning from the renderer |
| `RecoveryFailed` | Renderer could not recover from an error |
| `SessionError` | Renderer reported a session-level failure |
| `SessionClosed` | Renderer closed a session |
| `Error` | Generic renderer-side error |

### Timer events

`Event::Timer(TimerEvent)` is delivered by
`Subscription::every(duration, tag)`. Fields are `tag: String`
and `timestamp: u64` (milliseconds since the Unix epoch).
`WidgetMatch::Timer(tag)` surfaces the same event in the widget
match enum.

### Async and stream

`Command::task` returns `Event::Async(AsyncEvent)` with
`tag: String` and `result: Result<Value, Value>`.
`Command::stream` emits `Event::Stream(StreamEvent)` for each
intermediate value with `tag: String` and `value: Value`; the
stream's final outcome arrives as an `AsyncEvent` with the same
tag.

### Effects

`Command::effect` returns `Event::Effect(EffectEvent)` with the
issuing `tag` and a typed `EffectResult`:

| `EffectResult` variant | Meaning |
|---|---|
| `FileOpened { path }` | User picked a single file |
| `FilesOpened { paths }` | User picked multiple files |
| `FileSaved { path }` | User chose a save path |
| `DirectorySelected { path }` / `DirectoriesSelected { paths }` | Directory pick |
| `ClipboardText { text }` / `ClipboardHtml { html }` | Clipboard read |
| `ClipboardWritten` / `ClipboardCleared` | Clipboard write / clear acknowledged |
| `NotificationShown` | System notification posted |
| `Cancelled` | User dismissed the dialog (not an error) |
| `Timeout` | Effect exceeded its timeout |
| `Error(String)` | Platform error with a message |
| `RendererRestarted` | Renderer restarted while the effect was pending |
| `Unsupported` | Backend does not support this effect kind |
| `Shutdown` | Runner was tearing down |
| `Other(Value)` | Untyped fallback for forward compatibility |
| `Orphaned(Value)` | Response arrived for an effect the tracker forgot |

`Cancelled` is a normal outcome; treat it as "user said no", not
as a failure. Explicit `Timeout` and `Unsupported` variants give
you clear branches for graceful fallback.

### Command errors

Failing commands produce `Event::CommandError(CommandError)`:
`reason`, optional `id`, optional `family`, optional
`widget_type`, optional `message`. Use `reason` as the
machine-readable branch key and `message` for display.

### IME

`Event::Ime(ImeEvent)` covers input method composition for CJK
and complex scripts. Fields: `event_type: ImeEventType` (`Opened`,
`Preedit`, `Commit`, `Closed`), `id`, `scope`, `text`, `cursor`
as `(start, end)` byte offsets in the preedit string, `captured`,
`window_id`. Subscribe with `Subscription::on_ime`.

## Pattern matching cookbook

### Widget match on a small app

```rust
use plushie::prelude::*;
use plushie::event::WidgetMatch::*;

fn update(model: &mut Counter, event: Event) -> Command {
    match event.widget_match() {
        Some(Click("inc")) => model.count += 1,
        Some(Click("dec")) => model.count -= 1,
        Some(Input("label", text)) => model.label = text.to_string(),
        _ => {}
    }
    Command::none()
}
```

### Scope-aware match for dynamic lists

When items live inside a named scope, the container's ID
appears in `scope`. The short ID used in the match remains the
local widget ID; use `Event::scope()` to narrow by container:

```rust
fn update(model: &mut Todos, event: Event) -> Command {
    if let Some(WidgetMatch::Click("delete")) = event.widget_match() {
        if let Some(scope) = event.scope()
            && let Some(item_id) = scope.first()
        {
            model.items.remove(item_id);
        }
    }
    Command::none()
}
```

### Save shortcut with modifiers

```rust
use plushie::event::{Event, KeyEventType};
use plushie::prelude::Key;

fn update(model: &mut Editor, event: Event) -> Command {
    if let Some(key) = event.as_key_press() {
        match (&key.key, key.modifiers.command, key.modifiers.shift) {
            (Key::Char('s'), true, false) => return model.save(),
            (Key::Char('s'), true, true) => return model.save_as(),
            (Key::Escape, _, _) => model.close_dialog(),
            _ => {}
        }
    }
    Command::none()
}
```

### Window close with unsaved changes

```rust
use plushie::event::{Event, WindowEventType};

fn update(model: &mut Editor, event: Event) -> Command {
    if let Some(w) = event.as_window() {
        match w.event_type {
            WindowEventType::CloseRequested if model.dirty => {
                model.prompt_save();
                return Command::none();
            }
            WindowEventType::CloseRequested => {
                return Command::renderer(RendererOp::window_close(&w.window_id));
            }
            WindowEventType::Resized => {
                model.width = w.width.unwrap_or(model.width);
                model.height = w.height.unwrap_or(model.height);
            }
            _ => {}
        }
    }
    Command::none()
}
```

### Mouse press vs touch press

```rust
use plushie::event::WidgetMatch::*;
use plushie::prelude::{MouseButton, PointerKind};

match event.widget_match() {
    Some(Press("canvas", p)) if p.pointer == PointerKind::Mouse
        && p.button == MouseButton::Left =>
    {
        model.select_at(p.x, p.y);
    }
    Some(Press("canvas", p)) if p.pointer == PointerKind::Touch => {
        if let Some(finger) = p.finger {
            model.touch_start(finger, p.x, p.y);
        }
    }
    _ => {}
}
```

### Async task result

```rust
fn update(model: &mut News, event: Event) -> Command {
    if let Some(a) = event.as_async() {
        if a.tag == "fetch_headlines" {
            match &a.result {
                Ok(value) => model.apply_headlines(value),
                Err(err) => model.error = err.to_string(),
            }
            model.loading = false;
        }
    }
    Command::none()
}
```

### Effect result

```rust
use plushie::event::EffectResult;

fn update(model: &mut App, event: Event) -> Command {
    if let Some(e) = event.as_effect() {
        if e.tag == "open_file" {
            match &e.result {
                EffectResult::FileOpened { path } => model.load(path),
                EffectResult::Cancelled => { /* user dismissed, nothing to do */ }
                EffectResult::Error(msg) => model.error = msg.clone(),
                _ => {}
            }
        }
    }
    Command::none()
}
```

## See also

- [Subscriptions](subscriptions.md) for keyboard, timer, and
  other event sources.
- [Commands](commands.md) for the commands that produce
  `Async`, `Stream`, and `Effect` events.
- [Built-in widgets](built-in-widgets.md) for the setter that
  enables each event on a given widget.
- [Scoped IDs](scoped-ids.md) for how container scoping shapes
  `scope` chains.
- [App lifecycle](app-lifecycle.md) for the full `App` trait and
  how `update` fits into the runtime.
