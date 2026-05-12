# The Development Loop

The pad has a layout but the preview pane is empty. In this chapter
we bring it to life and, along the way, work out what "iteration"
looks like for a Rust Plushie app. Rust is not a scripting language:
there is no `Code.compile_string` equivalent, no `Module.create` at
runtime, no `eval`. Every piece of the pad that appears to be "live
code" is actually a module that was compiled into the pad binary
ahead of time and selected at runtime.

This chapter covers the iteration loop itself, the experiment
gallery pattern the pad uses in place of runtime compilation,
watch-mode rebuilds through `cargo plushie run --watch`, the
dev-mode renderer swap for wire-mode apps, and the debugging
techniques that carry you through the rest of the guide series.

## Edit, compile, run

Plushie apps iterate the same way every Rust binary iterates:

```bash
cargo run
```

Save a source file, run `cargo run`, the compiler rechecks the
crate, and Cargo launches the new binary. There is no background
recompilation, no hot module replacement, no shared REPL. If the
code does not compile, nothing runs. If the code compiles, you get
a fresh process with a fresh model.

This is the opposite trade-off from the scripting-language SDKs.
Elixir, Gleam, and the TypeScript hosts can take source code typed
into a text editor at runtime, compile it in process, and render
the result without restarting the app. Rust cannot do that from
inside itself. The compiler is not linked into the binary, macro
expansion happens at build time, and the produced machine code is
laid out before `main` starts.

The upside is everything the type system gives you: compile-time
checks on widget IDs that flow through typed state, on event
matching, on the shape of every `Command` constructor. A broken
view never reaches the renderer because it never links. The loop
is slower per iteration than an eval-based one, but the set of
mistakes that can survive a compile is much smaller.

Two things make the loop feel fast in practice:

- `cargo plushie run --watch` rebuilds and relaunches on every
  source change, so you edit and save instead of context-switching
  to a terminal.
- The experiment gallery pattern, covered below, gives the pad a
  catalogue of view fragments that swap in at runtime without a
  rebuild, so the thing being iterated on is usually small and
  self-contained.

## The experiment gallery

The pad cannot compile user-typed source, so it does the next best
thing: it bakes a catalogue of example views into the binary and
picks one to render. Each entry in the catalogue is a plain Rust
module that implements a small trait the demo defines for itself
(`Experiment` is a user-defined name, not part of plushie):

```rust
use plushie::prelude::*;

pub trait Experiment: Send {
    fn name(&self) -> &'static str;
    fn source(&self) -> &'static str;
    fn view(&self) -> View;
    fn update(&mut self, _event: &Event) -> bool { false }
}
```

`name` is the sidebar label. `view` builds a view fragment that the
pad drops into its preview pane. `update` is called with every
event scoped under `preview/`, so each experiment owns whatever
transient state it cares about. `source` is the interesting one:

```rust
fn source(&self) -> &'static str {
    include_str!("hello.rs")
}
```

`include_str!` reads the `.rs` file at compile time and pins it
into the binary as a `&'static str`. The source pane shows exactly
the bytes the compiler saw, so there is no way for the displayed
source to drift out of sync with the running code. The pad cannot
`eval` that text, but it can display it next to the rendered
result, which is the part the reader actually wanted.

The catalogue itself is a `Vec<Box<dyn Experiment>>`:

```rust
pub fn build_gallery() -> Vec<Box<dyn Experiment>> {
    vec![
        Box::new(hello::Hello::default()),
        Box::new(counter::Counter::default()),
        Box::new(list::ListExperiment::default()),
        Box::new(canvas::CanvasExperiment::default()),
        Box::new(form::Form::default()),
    ]
}
```

The pad stores the gallery on its model and tracks a `selected`
index. Sidebar buttons carry IDs like `pick_0`, `pick_1`; clicking
one sets the index and `view` picks up the new experiment on the
next render.

### Adding a new experiment

Adding a demo to the gallery is three steps. There is no macro, no
registry, no config file: it is all plain Rust.

First, create the module. Copy `src/experiments/hello.rs` to
`src/experiments/stopwatch.rs` and fill it in:

```rust
use plushie::prelude::*;

use super::Experiment;

#[derive(Default)]
pub struct Stopwatch {
    elapsed_ms: u64,
    running: bool,
}

impl Experiment for Stopwatch {
    fn name(&self) -> &'static str {
        "stopwatch"
    }

    fn source(&self) -> &'static str {
        include_str!("stopwatch.rs")
    }

    fn view(&self) -> View {
        let label = format!("{:.1}s", self.elapsed_ms as f64 / 1000.0);
        column()
            .spacing(12.0)
            .padding(16)
            .child(text(&label).id("elapsed").size(28.0))
            .child(button("toggle", if self.running { "Stop" } else { "Start" }))
            .into()
    }

    fn update(&mut self, event: &Event) -> bool {
        if let Some(Click("toggle")) = event.widget_match() {
            self.running = !self.running;
            return true;
        }
        false
    }
}
```

Second, declare the module in `src/experiments/mod.rs`:

```rust
pub mod stopwatch;
```

Third, add an entry to `build_gallery`:

```rust
vec![
    Box::new(hello::Hello::default()),
    Box::new(stopwatch::Stopwatch::default()),
    // ...
]
```

Run `cargo run`. The sidebar now shows a `stopwatch` row. The
source pane shows the contents of `stopwatch.rs`. Clicking the
button toggles `running`.

This pattern is the Rust answer to the Elixir pad's "type code and
press Save" flow. The compile step is still there, it just moved
out of the app and into the build system. The trade-off is an
explicit one: pay the build cost, keep the type checker on your
side.

## Watch mode

Typing `cargo run` by hand after every edit gets old. The
`cargo-plushie` CLI provides a `--watch` flag that delegates to
[`cargo-watch`](https://github.com/watchexec/cargo-watch) for
you:

```bash
cargo plushie run --watch
```

The command checks for `cargo-watch` on `PATH`; if it is missing,
it prints a hint and falls through to a single `cargo run` so the
invocation still succeeds. Install the watcher once:

```bash
cargo install cargo-watch
```

Under the hood the command runs `cargo watch -w src -s 'cargo
plushie build && cargo run'`, which reruns the build and the app
on every change under `src/`. For a direct-mode app the
`cargo plushie build` step is a fast no-op (the renderer lives
in-process, so there is nothing extra to build). For a wire-mode
app with native widgets, it rebuilds the custom
`plushie-renderer` binary if any of the bundled widget crates
changed. Either way, `PLUSHIE_BINARY_PATH` is pinned to the freshly
built binary so the SDK's wire discovery picks it up.

Watch mode restarts the whole process. The model is lost on every
rebuild because the model is owned by the app process and the app
process is the thing that got replaced. This is the inverse of the
Elixir SDK's hot-reload, which preserves model state across code
swaps. It is one of the costs of the Rust iteration model, and the
main reason the pad uses the experiment gallery rather than
trying to rebuild in place.

See [`cargo plushie run`](../reference/cli-commands.md#cargo-plushie-run)
for the full flag table and the exact command chain.

## Dev-mode renderer swap (wire mode)

Wire-mode apps have one extra trick the direct runner cannot do:
hot-swap the renderer subprocess without restarting the app. The
optional `dev` Cargo feature ships a widget-crate watcher that
rebuilds the custom renderer and signals the wire runner to swap
in the new binary.

Enable the feature in `Cargo.toml`:

```toml
[dependencies]
plushie = { version = "0.7.1", default-features = false, features = ["wire", "dev"] }
```

Then wire the watcher into `main` in place of the normal `run`
call:

```rust
fn main() -> plushie::Result {
    plushie::dev::watch_renderer::<MyApp>()
}
```

When `watch_renderer` detects a source change inside any widget
crate listed under `[package.metadata.plushie]`, it shells out to
`cargo plushie build`, waits for the rebuild to finish, and
publishes a `SwapRenderer` control signal. The wire runner drains
the signal on its next event-loop iteration, exits the current
renderer subprocess with `ExitReason::RendererSwap`, and respawns
against the fresh binary. The `Model`, subscription set, and
pending effects all survive. The swap skips the restart backoff and
does not count against `RestartPolicy::max_restarts`. See
[app lifecycle](../reference/app-lifecycle.md#hot-reload) for the
full sequence.

Two limits to know about:

- The watcher tracks widget crate sources only. The app binary
  cannot replace itself from inside a running process. For
  app-source edits, use `cargo plushie run --watch` outside the
  process.
- Direct mode has no equivalent path. Widget implementations link
  at compile time into the app binary, so there is no subprocess
  to swap. If you need renderer-level hot-swap, the app needs to
  run in wire mode.

## Debugging a running app

When something on screen does not look right, three tools catch
most of the cases before you reach for a debugger.

**Logs.** Plushie uses the `log` crate. Drop a quick
`log::info!("selected = {}", model.selected);` into `update` and
pick up `env_logger` or any other `log` backend at the top of
`main`:

```rust
fn main() -> plushie::Result {
    env_logger::init();
    plushie::run::<PadApp>()
}
```

Run with `RUST_LOG=plushie=debug,my_crate=trace cargo run` to
filter the output. The SDK itself logs renderer lifecycle,
subscription diffs, and view panics at `info` and `warn` level, so
turning up the SDK's own log level often reveals what the runtime
saw before your code noticed.

Plain `println!` works too; `cargo run` sends stdout to the
terminal unless you are in wire mode and have attached stdin/stdout
to the renderer directly. When in doubt, prefer `log::` over
`println!` so the output stays routable.

**An event log pane in the app itself.** The pad's fourth pane is
a rolling buffer of every event the pad received, debug-printed
and truncated to fit. `update` pushes a new entry on every event:

```rust
fn update(model: &Self::Model, event: Event) -> (Self::Model, Command) {
    let mut next = model.clone();
    // ...
    next.push_log(format!("{event:?}"));
    (next, Command::none())
}
```

The result is a live trace of what the user is producing, visible
without leaving the app. It is the fastest way to answer "did that
click actually reach `update`?" or "what does a `Resize` look
like?", which are exactly the questions that come up when you are
still building up intuition for the event taxonomy.

A trimmed version of the pattern:

```rust
const EVENT_LOG_CAPACITY: usize = 20;

fn push_log(&mut self, entry: String) {
    self.event_log.insert(0, entry);
    if self.event_log.len() > EVENT_LOG_CAPACITY {
        self.event_log.truncate(EVENT_LOG_CAPACITY);
    }
}
```

Newest events at the top, bounded so it cannot grow unchecked.
Wrap the buffer in a `scrollable` and render it below the main
content.

**`cargo plushie doctor`.** Before you start chasing a bug that
looks like an environment issue (renderer not launching, wrong
architecture, version mismatch), run the diagnostic:

```bash
cargo plushie doctor
```

The report covers `rustc` version, `cargo-plushie` version, every
`PLUSHIE_*` environment variable, renderer discovery, binary
architecture, detected native widgets, and version skew between
the app and the renderer. A `FAIL` row exits non-zero, so CI
pipelines can gate on it too. See
[`cargo plushie doctor`](../reference/cli-commands.md#cargo-plushie-doctor)
for the full row set.

## Fast feedback for layout work

Layout iteration has its own rhythm. You are not changing logic,
you are changing padding, spacing, alignment, and length
specifiers, and you want to see each change on screen without
re-triggering the state that got you to an interesting layout in
the first place.

Watch mode restarts the app, which means it resets the model. For
a counter showing `count = 7` that took seven clicks to reach, a
rebuild drops you back to `count = 0`. Two patterns work around
that.

The first is the experiment gallery itself: each experiment's
`Default` impl sets a useful starting state. `Form::default`
preseeds `volume: 25.0`; `Counter::default` starts at zero but the
bump-and-bump-again cost is tiny. Authoring experiments so they
open in an "interesting" state means a rebuild drops you straight
back into that state.

The second is the test harness. `TestSession<A>` drives the same
MVU loop the real runner does, without a GPU or a window server.
For a layout tweak that only matters in a specific model shape,
drive the app to that shape once in a test and assert on the
rendered tree:

```rust
use plushie::test::{TestSession, assert_tree_hash};

#[test]
fn preview_at_step_seven_matches_golden() {
    let mut session = TestSession::<PadApp>::start();
    for _ in 0..7 {
        session.click("preview/inc");
    }
    assert_tree_hash(&session, "counter_at_seven", "tests/golden");
}
```

Set `PLUSHIE_UPDATE_SNAPSHOTS=1` to rewrite the golden after an
intentional change, then flip it off and run `cargo test` on a
loop while you iterate. The test compiles and runs in a fraction
of a second and gives you a stable, scripted checkpoint that
survives rebuilds. See [testing](../reference/testing.md) for the
full harness surface.

## What's next

The pad now has a working preview, a real iteration loop, and the
debugging affordances to back it up. The next chapter steps back
from tooling and into the language of widget interaction: the
event types Plushie produces, how `Event::widget_match` carves
them into friendly shapes, and the patterns for responding to
them. Continue to [Events](05-events.md).
