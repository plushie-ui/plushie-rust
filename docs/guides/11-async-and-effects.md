# Async and Effects

Most real apps need work that does not fit inside a synchronous
`update`: an HTTP request, a file save, a clipboard read, a system
notification. Plushie handles these through commands returned from
`update`. The runner executes the command, and the result arrives
later as another `Event` variant that `update` matches on just like
a click.

This chapter walks through the three shapes that cover nearly every
background-work need: `Command::task` for one-off async work,
`Command::stream` for work that produces intermediate values, and
the effect constructors (`Command::file_open`, `Command::clipboard_read`,
`Command::notification`, ...) for platform operations owned by the
renderer. For the full command catalog see the
[commands reference](../reference/commands.md).

## Returning commands from update

`App::update` returns the next model and a `Command` by value. The
runner runs the command after the update returns, then calls `view`
and diffs:

```rust
use plushie::prelude::*;

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Click(id)) if id == "save" => {
            next.dirty = false;
            (next, Command::focus("editor"))
        }
        _ => (next, Command::none()),
    }
}
```

`Command::none()` is the do-nothing command. Every branch of a match
must return a model and a `Command`; use `Command::none()` wherever
the update only changes state.

## Command::task for async work

`Command::task` runs a future on a background executor. The future
must resolve to `Result<serde_json::Value, serde_json::Value>`. Both
arms carry a JSON payload so either branch can transport a typed
message back into the loop:

```rust
use plushie::prelude::*;
use serde_json::json;

Command::task("fetch_result", || async {
    let body = reqwest::get("https://api.example.com/data")
        .await
        .map_err(|e| json!(e.to_string()))?
        .text()
        .await
        .map_err(|e| json!(e.to_string()))?;
    Ok(json!(body))
})
```

The first argument is a **tag**. Every async command carries a tag,
and the tag comes back on the resulting event so the match arm can
tell one in-flight task from another. Tags are plain `&str` values:
keep them short and descriptive.

The result arrives as `Event::Async(AsyncEvent)`. Use the
`as_async` accessor to match on the tag and destructure the result:

```rust
fn update(model: &FetchApp, event: Event) -> (FetchApp, Command) {
    let mut next = model.clone();
    if let Some(Click("fetch")) = event.widget_match() {
        next.status = Status::Loading;
        return (next, Command::task("fetch_result", || async {
            Ok(json!(fetch_data().await?))
        }));
    }

    if let Some(a) = event.as_async()
        && a.tag == "fetch_result"
    {
        match &a.result {
            Ok(value) => {
                next.status = Status::Done;
                next.result = value.as_str().map(String::from);
            }
            Err(reason) => {
                next.status = Status::Error;
                next.error = reason.as_str().map(String::from);
            }
        }
    }

    (next, Command::none())
}
```

`AsyncEvent` is a plain struct with `tag: String` and
`result: Result<Value, Value>`. Exactly one of three things happens
to a task: it resolves to `Ok(value)`, it resolves to `Err(value)`,
or it is cancelled and no event is delivered. A panic in the future
is captured as a synthesised error value.

The executor is chosen by the run mode. Direct mode uses iced's
task runtime; wire mode uses tokio. `TestSession` runs async tasks
inline on a current-thread runtime, which is why the test suite in
the async example can assert the post-click state without polling.

## Command::stream for incremental work

`Command::stream` is like `task` but the future also receives a
`StreamEmitter` that it can call repeatedly. Each emit delivers an
`Event::Stream(StreamEvent)` with the tag; the future's final
`Result` resolves as an `AsyncEvent` on the same tag:

```rust
Command::stream("csv_import", |emitter| async move {
    for (n, line) in fetch_lines().await?.into_iter().enumerate() {
        emitter.emit(json!({"line": n, "data": parse(&line)}));
    }
    Ok(json!({"done": true}))
})
```

Handle the intermediate values with `as_stream`:

```rust
if let Some(s) = event.as_stream() && s.tag == "csv_import" {
    if let Some(n) = s.value["line"].as_u64() {
        model.progress = n;
    }
}
if let Some(a) = event.as_async() && a.tag == "csv_import" {
    model.progress_bar = None;
    if let Err(reason) = &a.result {
        model.error = reason.as_str().map(String::from);
    }
}
```

`StreamEvent` carries `tag: String` and `value: Value`. Typed
payloads can go through `StreamEmitter::emit_event` when the value
type implements `WidgetEventEncode`.

## Command::batch for composing commands

`Command::batch` takes anything that iterates into `Command`:

```rust
Command::batch([
    Command::clipboard_write("copy", &model.source),
    Command::notification("copied", "Copied", "Source copied to clipboard"),
    Command::focus("editor"),
])
```

Batched commands run in the order they were given. `Command::none()`
is a safe member of a batch, which is useful in conditional arms:

```rust
Command::batch([
    if model.dirty { Command::file_save("save") } else { Command::none() },
    Command::focus("editor"),
])
```

There is no direct "chain on success" combinator. To fire a second
async after the first returns, match on the first `AsyncEvent` and
return the second `Command::task` from that arm. Every hop stays
visible in `update`, which is easy to reason about and easy to test.

## Effects

Effects are asynchronous requests to the renderer for platform
operations that the SDK process cannot perform itself: file
dialogs, clipboard access, system notifications. Like async tasks,
every effect carries a tag, and the response arrives with the same
tag on `Event::Effect(EffectEvent)`.

### File dialogs

```rust
use plushie::prelude::*;

fn update(model: &Pad, event: Event) -> (Pad, Command) {
    if let Some(Click("import")) = event.widget_match() {
        return (model.clone(), Command::file_open_with(
            "import",
            FileDialogOpts::new()
                .title("Import Experiment")
                .filter("Rust", &["rs"]),
        ));
    }
    (model.clone(), Command::none())
}
```

`FileDialogOpts` chains `title`, `directory`, and `filter` setters.
For a plain picker with no options, call `Command::file_open("tag")`
without the `_with` suffix.

The response is typed. Match on the tag, then on the
`EffectResult` variant:

```rust
use plushie::event::EffectResult;

if let Some(e) = event.as_effect() && e.tag == "import" {
    match &e.result {
        EffectResult::FileOpened { path } => model.load(path),
        EffectResult::Cancelled => { /* user dismissed, nothing to do */ }
        EffectResult::Error(msg) => model.error = Some(msg.clone()),
        _ => {}
    }
}
```

The related constructors cover multi-file picks
(`file_open_multiple`, `file_open_multiple_with`), save dialogs
(`file_save`, `file_save_with`), and directory pickers
(`directory_select`, `directory_select_multiple`, each with a
`_with` variant). See the
[commands reference](../reference/commands.md) for the full table.

A path returned from a dialog is captured at the moment the dialog
closes. Treat it as untrusted input: re-verify existence, type,
symlink target, and permissions before opening or writing through
it.

### Clipboard

```rust
if let Some(Click("copy")) = event.widget_match() {
    return Command::clipboard_write("copy", &model.source);
}

if let Some(Click("paste")) = event.widget_match() {
    return Command::clipboard_read("paste");
}

if let Some(e) = event.as_effect() && e.tag == "paste" {
    if let EffectResult::ClipboardText { text } = &e.result {
        model.buffer.push_str(text);
    }
}
```

Also available: `clipboard_read_html`, `clipboard_write_html`,
`clipboard_clear`. On X11 and Wayland, `clipboard_read_primary`
and `clipboard_write_primary` reach the middle-click selection
buffer. HTML written through `clipboard_write_html` is forwarded
verbatim; sanitise user-supplied content before writing.

### Notifications

```rust
Command::notification("saved", "Saved", &format!("Wrote {path}"))
```

For options (icon, timeout, urgency, sound), use
`Command::notification_with` with a `NotificationOpts` builder. On
macOS, delivery may require the app to be bundled or have
notification entitlements; the renderer surfaces the failure as
`EffectResult::Error`.

## Error handling

Every async and effect branch should account for three outcomes:
success, failure, and user cancellation. `EffectResult::Cancelled`
is not an error. A user dismissing a file dialog is expected
behaviour; treat it as a no-op:

```rust
match &e.result {
    EffectResult::FileOpened { path } => model.load(path),
    EffectResult::Cancelled => {}
    EffectResult::Error(msg) => model.error = Some(msg.clone()),
    EffectResult::Timeout => model.error = Some("Dialog timed out".into()),
    _ => {}
}
```

Async tasks use `Result<Value, Value>`. The error branch is
whatever JSON value the future chose to return:

```rust
match &a.result {
    Ok(value) => model.apply(value),
    Err(reason) => {
        model.error = reason
            .get("message")
            .and_then(|v| v.as_str())
            .map(String::from);
    }
}
```

Effect defaults: file dialogs time out at 120 seconds, clipboard
and notifications at 5 seconds, unknown kinds at 30 seconds. A
timeout delivers `EffectResult::Timeout`. A renderer restart during
a pending effect delivers `EffectResult::RendererRestarted`. The
`Unsupported`, `Shutdown`, and `Orphaned` variants cover backend
gaps, teardown, and orphaned responses respectively.

## Cancellation and tag reuse

`Command::cancel("fetch")` aborts an in-flight task or stream. A
task that has not yet started is dropped; one that has started is
aborted at its next await point; one that has already finished is
a no-op. No event is delivered for a cancelled task.

Reusing a tag is the usual idiom for "latest wins". Issuing
`Command::task("search", ...)` while another `search` is in flight
cancels the first automatically:

```rust
Some(Input(id, query)) if id == "search" => {
    model.query = query.to_string();
    return Command::task("search", move || async move {
        Ok(json!(run_search(&query).await?))
    });
}
```

Each keystroke supersedes the previous in-flight search; only the
last one delivers a result. No debounce timer and no manual
cancellation call are needed.

The same rule applies to effects: one effect per tag. Starting a
new effect with a tag that has a pending request discards the
previous one.

## Testing async work

`TestSession` runs async tasks inline on a current-thread runtime,
so tests can assert post-dispatch state without polling. For
effects, the session has a stub table keyed by `EffectKind`: install
a response before triggering the effect, and the stub replies
instead of the real platform:

```rust
use plushie::event::{EffectKind, EffectResult};
use plushie::test::TestSession;

let mut session = TestSession::<Pad>::start();
session.register_effect_stub(
    EffectKind::FileOpen,
    EffectResult::FileOpened { path: "/tmp/test.rs".into() },
);
session.click("import");
assert_eq!(session.model().source, "/* test file */");
```

The full testing API, including stream-tick helpers and effect op
assertions, is covered in the
[testing reference](../reference/testing.md).

## What's next

Widgets, events, subscriptions, and commands together cover almost
every UI an app needs to build. The next chapter shifts into
pixel-level territory: the [canvas](12-canvas.md) widget for
drawing shapes, paths, and custom graphics that sit outside the
widget tree.
