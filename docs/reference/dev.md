# Dev mode

Dev mode is a watcher loop that rebuilds the custom renderer when
widget sources change and surfaces build status through an in-tree
overlay. It is gated behind the `dev` Cargo feature so production
builds carry none of its dependencies.

Enable it in the app crate:

```toml
[dependencies]
plushie = { version = "0.7.0", features = ["dev"] }
```

And switch `main`:

```rust
fn main() -> plushie::Result {
    plushie::dev::watch_renderer::<MyApp>()
}
```

`watch_renderer` is a drop-in for `plushie::run`. When no widget
crates are declared in the app's cargo metadata it returns
`plushie::run::<A>()` directly, so apps without native widgets pay
nothing for keeping the dev entry point wired up.

## What gets watched

Discovery runs off `cargo metadata`. Every dep whose `Cargo.toml`
declares a `[package.metadata.plushie.widget]` table is registered
as a widget crate. The watcher monitors each crate's `src/`
directory plus its `Cargo.toml`, debounces bursts of file events,
and reruns `cargo plushie build` when the debounce window expires.

The default debounce window is 250 ms. Tune it through
`WatchOpts::debounce` and pass the opts to
`watch_renderer_with_opts`:

```rust
use std::time::Duration;
use plushie::dev::{watch_renderer_with_opts, WatchOpts};

fn main() -> plushie::Result {
    let opts = WatchOpts {
        debounce: Duration::from_millis(500),
        release: false,
        overlay: None,
    };
    watch_renderer_with_opts::<MyApp>(opts)
}
```

The watcher does **not** track the app's own source. Replacing the
running binary requires an outer process; use `cargo-watch` or the
`cargo plushie run --watch` convenience wrapper for that side of
the loop.

## The rebuild lifecycle

1. **Debounce.** Events from `notify` collect into a single rebuild
   request once the debounce window passes without new events.
2. **Build.** `cargo plushie build` regenerates the renderer
   workspace under `target/plushie-renderer/` and compiles it.
   Output streams to stderr and, when an overlay handle is wired
   in, into the in-tree status banner.
3. **Swap.** On success the watcher publishes a
   `dev::ControlSignal::SwapRenderer`. The wire runner drains the
   queue at the next event-loop iteration, terminates the current
   `plushie-renderer` subprocess, and respawns against the freshly
   built binary.
4. **State preserved.** Across the swap the app's `Model`,
   subscriptions, and in-flight effects survive. The renderer
   re-syncs the view tree from the next snapshot the SDK emits.
5. **Failure.** When the build fails, the overlay surfaces the
   compiler output and the running renderer is left alone. The
   watcher retries on the next file change.

## In-tree rebuild overlay

The `dev::overlay` module exposes a `RebuildingOverlay` that
renders the watcher's status (`Building`, `Success`, `Failed`) at
the top of every window. Wire it into `App::view`:

```rust
use plushie::dev::{DevOverlayHandle, RebuildingOverlay, overlay::inject};

fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
    let body: ViewList = /* the real app tree */;
    inject(body, &model.overlay).into()
}
```

The overlay handle is shared with the watcher via
`WatchOpts::overlay`. The watcher pushes status updates; `inject`
renders the banner at view time. `Success` auto-dismisses after a
short delay so it doesn't clutter the screen during normal work.

## Restart policy on the renderer side

Auto-restart on renderer crashes is a wire-mode resilience feature
that lives outside the dev module but composes with it. Override
the default through `App::restart_policy`:

```rust
use plushie::settings::RestartPolicy;
use std::time::Duration;

impl App for MyApp {
    fn restart_policy() -> RestartPolicy {
        RestartPolicy {
            max_restarts: 10,
            restart_delay: Duration::from_millis(250),
            heartbeat_interval: Some(Duration::from_secs(15)),
        }
    }
    /* ... */
}
```

| Field | Default | Purpose |
|---|---|---|
| `max_restarts` | 5 | Consecutive auto-restart attempts before giving up. `0` disables auto-restart. |
| `restart_delay` | 100 ms | Base for exponential backoff: actual delay is `restart_delay * 2.pow(restart_count)`. |
| `heartbeat_interval` | 30 s | Watchdog window. If no wire message arrives within the interval, the runner triggers a restart. `None` disables. |

The hook fires on every restart attempt with the matching
`ExitReason`, then once more with `ExitReason::MaxRestartsReached`
when the limit is hit.

## Related references

- `plushie::cli` (see [CLI flags](cli.md)) layers reserved
  `--plushie-*` flags on top of `dev::watch_renderer`.
- `cargo-plushie` (see [CLI commands](cli-commands.md)) drives
  the same renderer build the watcher invokes.
