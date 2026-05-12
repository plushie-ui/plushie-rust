# App lifecycle

A plushie app is an implementation of the `plushie::App` trait. The
trait encodes the [Elm architecture](https://guide.elm-lang.org/architecture/):
`init` produces the initial model, `update` folds events into the
model and returns commands, `view` renders the model as a window
tree, and `subscribe` declares active event sources. The runner
drives those callbacks in a loop. This page covers the trait, the
loop, the startup and shutdown edges, and the wire-mode hooks for
renderer exit.

## The `App` trait

`plushie::App` lives in `crates/plushie/src/lib.rs`. Every app
supplies an associated `Model` type plus three required methods;
four more have defaults and can be overridden when needed.

| Method | Signature | Called | Default |
|---|---|---|---|
| `init` | `fn init() -> (Self::Model, Command)` | once at startup | required |
| `update` | `fn update(model: &Self::Model, event: Event) -> (Self::Model, Command)` | once per event | required |
| `view` | `fn view(model: &Self::Model, widgets: &mut WidgetRegistrar) -> ViewList` | after every update | required |
| `subscribe` | `fn subscribe(model: &Self::Model) -> Vec<Subscription>` | after every update | `vec![]` |
| `settings` | `fn settings() -> Settings` | once at startup | `Settings::default()` |
| `window_config` | `fn window_config(model: &Self::Model) -> WindowConfig` | when new windows open | `WindowConfig::default()` |
| `handle_renderer_exit` | `fn handle_renderer_exit(model: &mut Self::Model, reason: ExitReason)` | wire mode, on renderer exit | no-op |
| `restart_policy` | `fn restart_policy() -> RestartPolicy` | once at startup (wire) | `RestartPolicy::default()` |

`Self::Model` must be `Send + 'static`. It is owned by the runner,
passed as `&` to `update`, `view`, and `subscribe`.

```rust
use plushie::prelude::*;

#[derive(Clone)]
struct Counter { count: i32 }

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Counter { count: 0 }, Command::none())
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        match event.widget_match() {
            Some(Click("inc")) => next.count += 1,
            _ => {}
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main").child(
            column().children([
                text(&format!("{}", model.count)),
                button("inc", "+"),
            ]),
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Counter>()
}
```

## Lifecycle overview

The runner (direct or wire) drives the app through three phases:
startup, a long-running event loop, and shutdown. The same
trait methods are called in both modes. Wire mode adds a
renderer restart branch that feeds back into the event loop
without restarting the app process.

```
startup:   init -> settings -> view -> subscribe -> windows open
loop:      wait for event -> update -> view -> diff -> subscribe
shutdown:  Command::Exit or last-window-closed -> runner returns
```

Direct mode embeds the renderer in the app process; the entire
lifecycle runs inside the iced daemon. Wire mode spawns the
`plushie-renderer` subprocess and communicates over stdin/stdout;
the app process owns the MVU loop, the renderer owns iced. See
[direct-vs-wire.md](direct-vs-wire.md) for the full split.

## Init

`init()` returns `(Self::Model, Command)`. The command runs after
the first frame is on screen, not before. This keeps the initial
UI visible immediately while async work (data fetches, focus
requests, notifications) queues up for the next update cycle.

```rust
fn init() -> (Self, Command) {
    let model = Counter { count: 0, loading: true };
    let cmd = Command::task("load", || async move {
        Ok(serde_json::json!(load_counter().await?))
    });
    (model, cmd)
}
```

Return `Command::none()` when no startup work is needed. Batch
several commands with `Command::batch([..])`.

`init` takes no arguments. Configuration that would otherwise ride
in via parameters comes from environment variables, from
`App::settings`, or from dependency-injected globals the app
sets up before calling `plushie::run`.

## View

`view` is pure over the model and a `WidgetRegistrar`. It returns
a `ViewList`, the list of top-level windows to render.

```rust
fn view(model: &Self, widgets: &mut WidgetRegistrar) -> ViewList {
    window("main").child(main_content(model)).into()
}
```

Common return shapes:

- Single window: a `window(..)` builder, or any type convertible to
  `View`, via `.into()`.
- Multiple peer windows: `Vec<View>` or `[View; N]`.
- Nothing to display (loading splash suppressed, error screen
  cleared): `ViewList::new()` or `()`.

The runner calls `view` after every successful update, including
updates that did not change the model. The diff pipeline short
circuits when the new tree matches the previous one, so an
unchanged tree costs a structural compare and no wire traffic.

If `view` panics, the runner keeps the last good tree and logs
the error. A consecutive-panic counter escalates to a frozen-UI
overlay once the threshold is reached so the user sees something
other than a stuck UI. See the view-errors module in
`crates/plushie/src/runtime/view_errors.rs` for the exact
behaviour.

`widgets` is the registrar for composite widgets. Call
`WidgetView::<W>::new(id).register(widgets)` during `view` to
attach widget state that outlives a single frame.

## Update

```rust
fn update(model: &Self::Model, event: Event) -> (Self::Model, Command);
```

`update` takes `&Self::Model` and returns the next model plus a
single `Command`. To run multiple commands, use
`Command::batch([..])`.

Events arrive in the order they were produced. Coalescable
high-frequency events (`PointerMove`, `Resize`) are batched by
the renderer: only the latest value per source is delivered
before the next non-coalescable event. See
[events.md](events.md) for the full taxonomy and the coalescing
rules.

Ordering guarantees within a single update cycle:

1. `update` runs to completion and returns a next model.
2. Commands returned from `update` are dispatched to their
   executor (sync commands run immediately, async tasks spawn
   on the tokio runtime, effects go to the renderer).
3. `view` runs against the post-update model.
4. The resulting tree is diffed against the previous tree and
   patched.
5. `subscribe` runs and the subscription diff is applied.

Steps 2 through 5 happen before the next event is dequeued. A
panic in `update` is caught by the runner: the model reverts to
its pre-update state, the error is logged, and the loop
continues. The consecutive-panic counter is shared with `view`
guarding so a sustained crash stream surfaces through the same
frozen-UI overlay.

Match events with `event.widget_match()` for widget-scoped
helpers, or destructure `Event` variants directly for keyboard,
pointer, window, or async events:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Click("save")) => {
            next.dirty = false;
            (next, Command::focus("editor"))
        }
        Some(Submit("search", value)) => {
            next.query = value.to_string();
            (next, Command::none())
        }
        _ => (next, Command::none()),
    }
}
```

A catch-all arm prevents unhandled events from panicking the
update. The runner will absorb the panic, but logs stay quieter
without one.

## Subscribe

```rust
fn subscribe(model: &Self::Model) -> Vec<Subscription>;
```

Called after every successful `update`. The runner diffs the
returned list against the currently active set, starts new
subscriptions, and stops removed ones. Because `subscribe` is a
function of the model, subscriptions are conditional by
default: include a timer when a mode is active, omit it
otherwise, and the runner handles the transition.

```rust
fn subscribe(model: &Self::Model) -> Vec<Subscription> {
    let mut subs = vec![Subscription::on_key_press()];
    if model.auto_save && model.dirty {
        subs.push(Subscription::every(Duration::from_secs(1), "auto_save"));
    }
    subs
}
```

See [subscriptions.md](subscriptions.md) for constructors, rate
limiting, window scoping, and identity rules.

## Settings and window config

`settings()` returns a `plushie::settings::Settings` record
read once at startup. It configures renderer-wide defaults:
font, default text size, antialiasing, vsync, scale factor,
theme, fonts to load, default event rate, widget config, and
the list of `required_widgets`. In direct mode the settings
drive the iced daemon; in wire mode they are serialized and
sent before the first snapshot.

```rust
fn settings() -> Settings {
    Settings {
        default_font: Some("monospace".to_string()),
        default_text_size: Some(14.0),
        theme: Some(Theme::Dark),
        ..Settings::default()
    }
}
```

`window_config(&model)` returns a `WindowConfig` merged into
every window node the view declares. Fields cover title, size,
position, min/max size, maximized, fullscreen, visibility,
resizability, decorations, transparency, blur, level, and
`exit_on_close_request`. Per-window props set on a `window(..)`
builder in `view` override the base config.

See [configuration.md](configuration.md) for the complete
`Settings` and `WindowConfig` field tables and for the
`PLUSHIE_*` environment variables that cross-cut both modes.

## Renderer exit (wire mode)

When the renderer subprocess exits, the wire runner classifies
the exit and delivers an `ExitReason` to
`App::handle_renderer_exit`. The method takes `&mut Self::Model`
so the app can clear renderer-side state (scroll positions,
cursor positions, animation progress) before the next restart
attempt. Direct mode never calls this hook: the renderer and
the app share a process, so there is no separate exit to
classify.

```rust
fn handle_renderer_exit(model: &mut Self, reason: ExitReason) {
    log::warn!("renderer exited: {}", reason.label());
    model.pending_uploads.clear();
}
```

`ExitReason` lives at `plushie::settings::ExitReason`:

| Variant | Meaning |
|---|---|
| `Crash { message, code }` | Renderer subprocess failed. `message` describes the I/O or panic; `code` is the exit code when reapable. |
| `ConnectionLost` | Pipe closed cleanly without a final message. |
| `Shutdown` | Renderer exited at our request (typically after `Command::Exit`). |
| `HeartbeatTimeout` | No message received within `RestartPolicy::heartbeat_interval`. |
| `MaxRestartsReached { last_reason }` | Auto-restart gave up. Delivered after the policy is exhausted. |
| `RendererSwap` | Dev hot-reload requested a renderer swap. Does not count against the restart budget. |

`RestartPolicy` governs the restart loop:

```rust
fn restart_policy() -> RestartPolicy {
    RestartPolicy {
        max_restarts: 5,
        restart_delay: Duration::from_millis(100),
        heartbeat_interval: Some(Duration::from_secs(30)),
    }
}
```

Defaults: five consecutive restarts, 100 ms base delay with
exponential backoff (`restart_delay * 2.pow(attempt)`), and a
thirty-second heartbeat. Set `max_restarts: 0` to disable
auto-restart entirely; the first renderer crash then returns
`Error::RendererExit(ExitReason::Crash { .. })` from
`plushie::run`.

### State preserved across restarts

When the wire runner respawns the renderer after an exit:

1. `handle_renderer_exit` is called with the classified reason.
2. A fresh subprocess is spawned and the hello handshake is
   re-run.
3. The cached `Settings` are re-sent.
4. The current tree is re-sent as a full snapshot (not a
   patch); the new renderer has no memory of the prior one.
5. All open windows are re-opened via `WindowSync` against the
   fresh baseline.
6. The full subscription set is replayed as `Subscribe` ops.
7. In-flight effects are flushed with
   `EffectResult::RendererRestarted` so the app can re-issue or
   give up on them.

The app's `Model` survives untouched across restarts: the
runner holds it in-process. Renderer-side widget state (text
editor cursor, scroll offset, transient animation state) is
reset because the new renderer process starts clean.

When `max_restarts` is exhausted, `handle_renderer_exit` fires
once more with `ExitReason::MaxRestartsReached { last_reason }`,
pending effects are drained as `EffectResult::Shutdown`, and
`plushie::run` returns `Err(Error::RendererExit(..))`.

## Hot reload

The optional `dev` feature (`plushie = { features = ["dev"] }`)
adds a widget-crate watcher that rebuilds the custom renderer
via `cargo plushie build` and publishes a `SwapRenderer`
control signal when the rebuild completes. The wire runner
observes the signal, exits the current session with
`ExitReason::RendererSwap`, rediscovers the binary, and
respawns against the freshly built renderer. Swaps skip the
backoff delay and do not count against `max_restarts`.

```rust
fn main() -> plushie::Result {
    plushie::dev::watch_renderer::<MyApp>()
}
```

The watcher only tracks widget crate sources. The app binary
itself cannot replace itself from within; use `cargo-watch` or
`cargo plushie run --watch` outside the process for app-source
reload.

Direct mode has no equivalent hot-reload path: widget
implementations link at compile time into the app binary.

## Graceful shutdown

Three paths bring the runner down cleanly:

- **`Command::Exit`**: terminates the app from `update`. Direct
  mode flushes in-flight effects as `EffectResult::Shutdown`
  before calling `iced::exit()`. Wire mode sends the shutdown
  request and returns `Ok(())` once the renderer acknowledges.
- **Last window closed**: closing the final window produces a
  `WindowEvent` the app can observe. If nothing re-opens a
  window before the runner spins again, the iced daemon (direct)
  or the wire runner (after the renderer exits with
  `ExitReason::Shutdown`) returns `Ok(())`.
- **Max restarts reached** (wire): the policy is exhausted and
  the runner returns `Err(Error::RendererExit(..))`. Pending
  effects are drained with `EffectResult::Shutdown` before the
  function returns.

`plushie::run::<A>()` itself returns `plushie::Result`. Match on
`Error::RendererExit(reason)` if the app needs to distinguish a
classified renderer failure from other runner errors (binary
not found, handshake mismatch, I/O failure).

## See also

- [Events](events.md)
- [Commands](commands.md)
- [Subscriptions](subscriptions.md)
- [Direct vs wire](direct-vs-wire.md)
- [Configuration](configuration.md)
