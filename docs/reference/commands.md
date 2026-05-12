# Commands

Commands are values returned from `App::update` (and `App::init`).
The runner executes them after the update cycle completes. They
are how an app triggers work that does not fit inside the pure
model transition: background tasks, focus changes, window
operations, platform effects, and more. The type lives at
`plushie::command::Command` and is re-exported by the prelude.

Every command is either a piece of data (a widget-targeted op, a
renderer op, an effect request) or a boxed async closure
(`Command::task`, `Command::stream`). There is no implicit
execution: a command only runs if an update returns it.

## Returning commands

`App::update` returns the next model and a `Command` by value:

```rust
use plushie::prelude::*;

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Click("save")) => {
            next.dirty = false;
            (next, Command::focus("editor"))
        }
        Some(Click("export")) => {
            (next, Command::batch([
                Command::file_save("export"),
                Command::notification("notify", "Exporting", "Saving to file..."),
            ]))
        }
        _ => (next, Command::none()),
    }
}
```

`Command::none()` is the do-nothing return. `Command::batch(...)`
takes anything that iterates into `Command`, so an array literal,
a `Vec<Command>`, or a chained iterator all work.

## Control flow

| Method | Signature | Description |
|---|---|---|
| `none` | `() -> Command` | No-op command. |
| `batch` | `(impl IntoIterator<Item = Command>) -> Command` | Execute multiple commands. |
| `exit` | `() -> Command` | Shut down the app. |
| `send_after` | `(Duration, Event) -> Command` | Dispatch an event after a delay. |
| `dispatch` | `(Event) -> Command` | Dispatch an event on the next turn of the loop (zero-delay `send_after`). |

`Command::send_after` is a one-shot timer. For recurring timers,
use [`Subscription::every`](subscriptions.md). `Command::dispatch`
lifts a value back into the MVU loop without spawning any task,
which is useful when an update has finished computing a value and
wants the next update invocation to react to it.

## Async tasks

Async work uses a tag to correlate the command with the result
delivered later through `Event::Async`.

| Method | Signature | Description |
|---|---|---|
| `task` | `(&str, FnOnce() -> Future<Output = Result<Value, Value>>) -> Command` | Run an async future. |
| `stream` | `(&str, FnOnce(StreamEmitter) -> Future<Output = Result<Value, Value>>) -> Command` | Run a streaming future that emits intermediate values. |
| `cancel` | `(&str) -> Command` | Cancel an in-flight task or stream by tag. |

A task's future must resolve to `Result<serde_json::Value,
serde_json::Value>`. The first argument is the correlation tag:

```rust
use plushie::prelude::*;
use serde_json::json;

Command::task("fetch", || async move {
    let body = reqwest::get("https://api.example.com/data")
        .await
        .map_err(|e| json!(e.to_string()))?
        .text()
        .await
        .map_err(|e| json!(e.to_string()))?;
    Ok(json!(body))
})
```

The result arrives in `update` as an `Event::Async(AsyncEvent)`
with the original tag:

```rust
if let Some(a) = event.as_async() && a.tag == "fetch" {
    match &a.result {
        Ok(value) => model.text = value.as_str().map(String::from),
        Err(err) => model.error = err.as_str().map(String::from),
    }
}
```

`AsyncEvent` is a plain struct with `tag: String` and
`result: Result<Value, Value>`. The delivery contract is exactly
one of: `Ok(value)`, `Err(value)` (including a synthesised
`{"error": "panic", "message": ...}` when the future panics), or
nothing at all if the task is cancelled before it finishes.

### Streaming

`Command::stream` supplies the future with a `StreamEmitter` it
can clone and pass around. Each `emitter.emit(value)` delivers an
`Event::Stream(StreamEvent)` with the emitter's tag; the future's
final `Result` resolves to a closing `Event::Async(AsyncEvent)`
on the same tag.

```rust
Command::stream("import", |emitter| async move {
    for line in fetch_lines().await? {
        emitter.emit(line);
    }
    Ok(serde_json::json!({"done": true}))
})
```

`StreamEvent` carries `tag: String` and `value: Value`. Typed
payloads can be emitted through `StreamEmitter::emit_event` when
the value type implements `WidgetEventEncode`.

### Cancellation and tag reuse

`Command::cancel("fetch")` aborts an in-flight task or stream.
Cancellation is best-effort: a task that has not yet started is
dropped; a task already running is aborted at the next await
point; a finished task is a no-op. No event is delivered for a
cancelled task.

Reusing a tag replaces the in-flight task. Starting a new
`Command::task("fetch", ..)` while another `fetch` is running
cancels the first on the author's behalf. This is the usual
pattern for "latest wins" fetches keyed by a search box.

Direct mode routes cancellation through
`iced::task::Handle::abort`, which detaches the future rather
than force-dropping it; a non-cancellable inner future may run
to completion with its result discarded. Wire mode uses
`tokio::task::JoinHandle::abort`. `TestSession` runs async tasks
inline on a current-thread runtime, so cancellation between
turns is a no-op unless the task has already yielded.

## Effects

Platform effects are built through `Command` constructors and
carry an `EffectRequest` payload from
`plushie_core::ops::EffectRequest`. Results arrive as
`Event::Effect(EffectEvent)` with `tag: String` and a typed
`result: EffectResult`.

### File dialogs

| Method | Signature | Description |
|---|---|---|
| `file_open` | `(&str) -> Command` | Single file picker. |
| `file_open_with` | `(&str, FileDialogOpts) -> Command` | Single file picker with options. |
| `file_open_multiple` | `(&str) -> Command` | Multi-file picker. |
| `file_open_multiple_with` | `(&str, FileDialogOpts) -> Command` | Multi-file picker with options. |
| `file_save` | `(&str) -> Command` | Save dialog. |
| `file_save_with` | `(&str, FileDialogOpts) -> Command` | Save dialog with options. |
| `directory_select` | `(&str) -> Command` | Single directory picker. |
| `directory_select_with` | `(&str, FileDialogOpts) -> Command` | Single directory picker with options. |
| `directory_select_multiple` | `(&str) -> Command` | Multi-directory picker. |
| `directory_select_multiple_with` | `(&str, FileDialogOpts) -> Command` | Multi-directory picker with options. |

`FileDialogOpts` is a builder: `FileDialogOpts::new().title("Open")
.directory("/home").filter("Rust", &["rs", "toml"])`.

```rust
use plushie::prelude::*;

let cmd = Command::file_open_with(
    "import",
    FileDialogOpts::new()
        .title("Import")
        .filter("Rust", &["rs"]),
);
```

Results are typed:

```rust
if let Some(e) = event.as_effect() && e.tag == "import" {
    match &e.result {
        EffectResult::FileOpened { path } => model.open(path),
        EffectResult::Cancelled => {}
        EffectResult::Error(msg) => model.error = Some(msg.clone()),
        _ => {}
    }
}
```

Dialog-returned paths are captured at the moment the dialog
closes. Treat them as input: re-verify existence, type, symlink
target, and permissions before opening, executing, or writing
through them.

### Clipboard

| Method | Signature | Description |
|---|---|---|
| `clipboard_read` | `(&str) -> Command` | Read plain text. |
| `clipboard_write` | `(&str, &str) -> Command` | Write plain text. |
| `clipboard_read_html` | `(&str) -> Command` | Read HTML content. |
| `clipboard_write_html` | `(&str, &str, Option<&str>) -> Command` | Write HTML with optional plain-text fallback. |
| `clipboard_clear` | `(&str) -> Command` | Clear the system clipboard. |
| `clipboard_read_primary` | `(&str) -> Command` | Read primary selection (X11/Wayland). |
| `clipboard_write_primary` | `(&str, &str) -> Command` | Write primary selection (X11/Wayland). |

HTML written via `clipboard_write_html` is forwarded verbatim. If
the source is user-supplied, sanitise before writing; receiving
applications may execute embedded scripts or load external
resources when rendering the payload.

### Notifications

| Method | Signature | Description |
|---|---|---|
| `notification` | `(&str, &str, &str) -> Command` | Show a notification with tag, title, body. |
| `notification_with` | `(&str, &str, &str, NotificationOpts) -> Command` | Notification with options. |

`NotificationOpts` builds through `icon`, `timeout`, `urgency`,
and `sound` setters. Urgency is `NotificationUrgency::Low`,
`Normal`, or `Critical`. On macOS, delivery may require the app
to be bundled or have notification entitlements.

### Effect lifecycle

- Tag-based matching. Every effect takes a tag; the same tag
  returns in `EffectEvent::tag`.
- One effect per tag. Starting a new effect with a tag that has
  a pending request discards the previous one.
- Default timeouts: file dialogs 120 s, clipboard and
  notifications 5 s, unknown kinds 30 s. Override by constructing
  the underlying `RendererOp::Effect` with an explicit `timeout`.
- Timeouts deliver as `EffectResult::Timeout`.
- User cancellation delivers `EffectResult::Cancelled` (not an
  error).
- Renderer restarts deliver `EffectResult::RendererRestarted` for
  any pending effect.
- The `EffectResult::Shutdown`, `Unsupported`, and `Orphaned`
  variants cover runner teardown, unimplemented effects, and
  responses that outlived their tracker entries respectively.

For tests, `TestSession::register_effect_stub(kind, response)`
intercepts effects before they reach the runner. See
[Testing](testing.md).

## Focus

| Method | Signature | Description |
|---|---|---|
| `focus` | `(&str) -> Command` | Move keyboard focus to a widget by ID. |
| `focus_next` | `() -> Command` | Move focus to the next focusable widget. |
| `focus_previous` | `() -> Command` | Move focus to the previous focusable widget. |
| `focus_next_within` | `(&str) -> Command` | Move focus to the next focusable widget inside a scope. |
| `focus_previous_within` | `(&str) -> Command` | Move focus to the previous focusable widget inside a scope. |

Scoped focus is for menus, pane grids, and other contained Tab
cycles. For modal focus traps, set `a11y.modal = true` on the
container; iced auto-traps focus at modal boundaries without
needing the scoped variants.

## Text cursor

All text commands target a text input or editor by scoped ID.

| Method | Signature | Description |
|---|---|---|
| `select_all` | `(&str) -> Command` | Select all content. |
| `move_cursor_to_front` | `(&str) -> Command` | Move cursor to start. |
| `move_cursor_to_end` | `(&str) -> Command` | Move cursor to end. |
| `move_cursor_to` | `(&str, usize) -> Command` | Move cursor to a character position. |
| `select_range` | `(&str, usize, usize) -> Command` | Select the given range. |

## Scroll

| Method | Signature | Description |
|---|---|---|
| `scroll_to` | `(&str, f32, f32) -> Command` | Scroll to an absolute position. |
| `scroll_by` | `(&str, f32, f32) -> Command` | Scroll by a relative offset. |
| `snap_to` | `(&str, f32, f32) -> Command` | Snap scroll to a position (no animation). |
| `snap_to_end` | `(&str) -> Command` | Snap scroll to the end of content. |

## Window operations

Window commands target a window by its ID (the string passed to
`window("main")` in the view).

| Method | Signature | Description |
|---|---|---|
| `close_window` | `(&str) -> Command` | Close a window. |
| `resize_window` | `(&str, f32, f32) -> Command` | Set window size in logical pixels. |
| `move_window` | `(&str, f32, f32) -> Command` | Set window position. |
| `maximize_window` | `(&str) -> Command` | Maximize a window. |
| `unmaximize_window` | `(&str) -> Command` | Restore from maximized. |
| `minimize_window` | `(&str) -> Command` | Minimize a window. |
| `unminimize_window` | `(&str) -> Command` | Restore from minimized. |
| `set_window_mode` | `(&str, WindowMode) -> Command` | Switch between Windowed and Fullscreen. |
| `toggle_maximize` | `(&str) -> Command` | Toggle maximized state. |
| `toggle_decorations` | `(&str) -> Command` | Toggle title bar and borders. |
| `focus_window` | `(&str) -> Command` | Bring a window to the front. |
| `set_window_level` | `(&str, WindowLevel) -> Command` | Stacking level (Normal, AlwaysOnTop, AlwaysOnBottom). |
| `drag_window` | `(&str) -> Command` | Begin an interactive drag. |
| `drag_resize_window` | `(&str, &str) -> Command` | Begin an interactive resize from an edge. |
| `request_attention` | `(&str, Option<NotificationUrgency>) -> Command` | Taskbar flash or bounce. |
| `screenshot` | `(&str, &str) -> Command` | Capture a window; result arrives as `SystemEvent`. |
| `set_resizable` | `(&str, bool) -> Command` | Allow or block user resizing. |
| `set_min_size` | `(&str, f32, f32) -> Command` | Set the minimum window size. |
| `set_max_size` | `(&str, f32, f32) -> Command` | Set the maximum window size. |
| `enable_mouse_passthrough` | `(&str) -> Command` | Let mouse events pass through the window. |
| `disable_mouse_passthrough` | `(&str) -> Command` | Restore normal mouse handling. |
| `show_system_menu` | `(&str) -> Command` | Show the native title-bar menu. |
| `set_icon` | `(&str, Vec<u8>, u32, u32) -> Command` | Set window icon from raw RGBA pixels. |
| `set_resize_increments` | `(&str, f32, f32) -> Command` | Constrain resize steps. |

Each of these builds a `Command::Renderer(RendererOp::Window(WindowOp::...))`
internally. Direct callers that need the raw op for testing or
wire inspection can reach it through
`plushie_core::ops::WindowOp`.

## Window queries

Window queries return `Event::System(SystemEvent)` values keyed
by the tag argument.

| Method | Signature | Description |
|---|---|---|
| `window_size` | `(&str, &str) -> Command` | Query the window's size. |
| `window_position` | `(&str, &str) -> Command` | Query the window's position. |
| `is_maximized` | `(&str, &str) -> Command` | Query maximized state. |
| `is_minimized` | `(&str, &str) -> Command` | Query minimized state. |
| `window_mode` | `(&str, &str) -> Command` | Query windowed/fullscreen mode. |
| `scale_factor` | `(&str, &str) -> Command` | Query DPI scale factor. |
| `monitor_size` | `(&str, &str) -> Command` | Query the containing monitor's size. |
| `raw_id` | `(&str, &str) -> Command` | Query the platform-native window handle. |

`SystemEvent` carries `event_type`, `tag`, `value`, `id`, and
`window_id`. Matching is by `tag`.

## System

| Method | Signature | Description |
|---|---|---|
| `allow_automatic_tabbing` | `(bool) -> Command` | Toggle macOS automatic window tabbing. |
| `system_theme` | `(&str) -> Command` | Query current OS light/dark preference. |
| `system_info` | `(&str) -> Command` | Query OS and renderer metadata. |
| `announce` | `(&str, Live) -> Command` | Screen-reader announcement with explicit politeness. |
| `announce_text` | `(&str) -> Command` | Polite announcement (shorthand). |

`Live::Polite` queues after ongoing speech; `Live::Assertive`
interrupts it. Reserve assertive for status the user must hear
immediately.

## Images

Images are keyed by a string handle. The same handle is used in
widget props (`image("logo").handle("logo")`) and in commands.

| Method | Signature | Description |
|---|---|---|
| `create_image` | `(&str, Vec<u8>) -> Command` | Create from encoded bytes (PNG, JPEG, ...). |
| `create_image_rgba` | `(&str, u32, u32, Vec<u8>) -> Command` | Create from raw RGBA pixels. |
| `update_image` | `(&str, Vec<u8>) -> Command` | Replace with new encoded bytes. |
| `update_image_rgba` | `(&str, u32, u32, Vec<u8>) -> Command` | Replace with new raw RGBA pixels. |
| `delete_image` | `(&str) -> Command` | Delete by handle. |
| `list_images` | `(&str) -> Command` | List loaded handles; result arrives as `SystemEvent`. |
| `clear_images` | `() -> Command` | Delete all loaded images. |

`create_image_rgba` and `update_image_rgba` panic if
`pixels.len() != width * height * 4`. The renderer would
otherwise interpret out-of-range bytes as image data, producing
garbled output with no clear error origin; a panic at the call
site points straight at the bug.

## Widget commands

Widgets can receive typed commands from an app. The typed form
uses a `#[derive(WidgetCommand)]` enum; the raw form takes a
family string and a JSON value.

| Method | Signature | Description |
|---|---|---|
| `widget` | `(&str, impl WidgetCommandEncode) -> Command` | Send a typed command to a widget. |
| `send` | `(&str, &str, Value) -> Command` | Send a raw (family, value) command. |
| `widget_batch` | `(impl IntoIterator<Item = WidgetCommand>) -> Command` | Apply a batch atomically. |

```rust
use plushie::prelude::*;
use plushie::command::WidgetCommand;

#[derive(WidgetCommand)]
enum GaugeCommand {
    SetValue(f32),
    Reset,
    SetRange { min: f32, max: f32 },
}

Command::widget("temp-gauge", GaugeCommand::SetValue(72.0))
```

`Command::widget_batch` differs from `Command::batch` in that
intermediate events are buffered: observers see one consistent
state after all commands commit. Build items with
`WidgetCommand::new(id, cmd)` for typed commands or
`WidgetCommand::raw(id, family, value)` for ad-hoc ones.

The pane-grid builders (`pane_split`, `pane_close`, `pane_swap`,
`pane_maximize`, `pane_restore`) are thin wrappers around
`send` that target a pane-grid widget by ID.

## Fonts, hashing, frames

| Method | Signature | Description |
|---|---|---|
| `load_font` | `(impl Into<String>, Vec<u8>) -> Command` | Load a font by family name from raw bytes. |
| `tree_hash` | `(&str) -> Command` | Request a structural hash of the current tree. |
| `find_focused` | `(&str) -> Command` | Query which widget has keyboard focus. |
| `advance_frame` | `(u64) -> Command` | Advance the animation clock (test/headless mode). |

`load_font` registers the family name in the renderer's loaded-
font registry; subsequent `default_font.family` settings and
widget `font.family` props resolve without parsing font metadata.

## Composition

### Batching

`Command::batch` takes anything `IntoIterator<Item = Command>`.
The batched commands run in order. `Command::none()` in a batch
is a safe no-op, useful in conditional arms:

```rust
Command::batch([
    if model.dirty { Command::file_save("save") } else { Command::none() },
    Command::focus("editor"),
])
```

### Chaining async results

There is no direct "chain" combinator. The pattern is to return
the first `Command::task`, then return the next command from the
`Event::Async` arm:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    if let Some(a) = event.as_async() {
        if a.tag == "fetch_index" {
            if let Ok(index) = a.result.as_ref() {
                let next_url = index["next"].as_str().unwrap_or("").to_string();
                return (model.clone(), Command::task("fetch_body", move || async move {
                    fetch_body(&next_url).await
                }));
            }
        }
    }
    (model.clone(), Command::none())
}
```

This keeps every asynchronous hop visible in `update` and
testable with `TestSession::await_async`.

### Lifting values into the loop

`Command::dispatch(Event::...)` queues an event for the next
update cycle. It is the idiomatic way to finish an update with
"now react to this value" when a single `update` call cannot
both return a new model and see that new state.

## Lifecycle and guarantees

- Commands run after `update` returns. The runner never executes
  a command mid-update, and `view` is called once per update
  round, after every command has been dispatched to its
  executor.
- `Command::Renderer(...)` variants ship to the renderer in
  the order they are returned; wire mode serialises them at
  the process boundary, direct mode passes them in-process
  with no serialisation overhead.
- Async commands run on a background executor. Delivery back
  into the MVU loop is ordered by completion time, not by
  command issue time. Correlate with tags.
- Dropping a `Command` without returning it from `update` is a
  no-op: the command never runs.
- Renderer restarts (wire mode) are handled by
  `App::handle_renderer_exit`. Pending effects are resolved
  with `EffectResult::RendererRestarted`; in-flight async tasks
  continue to run in the SDK process and may deliver stale
  results if they depended on renderer state.

## See also

- [Events](events.md)
- [Subscriptions](subscriptions.md)
- [App lifecycle](app-lifecycle.md)
- [Composition patterns](composition-patterns.md)
- [Testing](testing.md)
