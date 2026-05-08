# Getting Started

This chapter takes you from an empty directory to a running Plushie
window. We install the Rust toolchain, create a crate, add `plushie`
as a dependency, pick a rendering mode, and run the smallest app
that puts pixels on the screen.

## Prerequisites

Plushie needs a recent stable Rust toolchain. The minimum supported
version is `1.92`. If you do not already have rustup, install it
from [rustup.rs](https://rustup.rs/) and then pin a toolchain:

```bash
rustup install stable
rustup default stable
rustc --version
```

Plushie runs on Linux, macOS, and Windows. Direct mode compiles
against the native graphics stack; wire mode spawns a separate
renderer binary. Both are covered below.

Once Plushie is installed you can verify your environment at any
time with:

```bash
cargo plushie doctor
```

The doctor report checks `rustc` against the minimum, resolves the
renderer binary, and prints any skew it detects. See [CLI
commands](../reference/cli-commands.md) for the full set of
subcommands.

## Creating a project

Use Cargo to scaffold a new binary crate:

```bash
cargo new plushie-hello
cd plushie-hello
```

Open `Cargo.toml` and add `plushie` as a dependency. Pre-1.0
releases can break between patch versions, so pin the exact
version:

```toml
[package]
name = "plushie-hello"
version = "0.1.0"
edition = "2021"

[dependencies]
plushie = "=0.7.0"
```

Fetch the dependency once so `cargo check` is fast later:

```bash
cargo fetch
```

## Choosing a mode

Plushie has two rendering modes behind the same `plushie::run`
entry point. Pick one before you write `main.rs`.

| Mode | Feature | Renderer lives in | When to pick |
|---|---|---|---|
| Direct | `direct` (default) | app process | desktop app, simplest setup |
| Wire | `wire` | `plushie-renderer` subprocess | crash isolation, remote or browser rendering, custom widgets |

Direct mode is the default and requires nothing extra: `cargo run`
is enough. Wire mode needs a `plushie-renderer` binary available
to the app at runtime. The full trade-off matrix, including socket
mode and WASM renderers, is in [direct vs
wire](../reference/direct-vs-wire.md).

Most first-time apps should start in direct mode. You can switch
later without touching your `App` impl; the surface is identical.

## Writing main.rs

Replace the generated `src/main.rs` with a minimal app that
displays a single line of text:

```rust
use plushie::prelude::*;

struct Hello;

impl App for Hello {
    type Model = ();

    fn init() -> (Self::Model, Command) {
        ((), Command::none())
    }

    fn update(_model: &mut Self::Model, _event: Event) -> Command {
        Command::none()
    }

    fn view(_model: &Self::Model, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Hello")
            .child(text("Hello from Plushie"))
            .into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<Hello>()
}
```

A quick tour of what is on screen:

- `use plushie::prelude::*` pulls in the `App` trait, the widget
  constructors (`window`, `text`, and friends), `Command`,
  `Event`, and the `ViewList` / `View` types.
- `impl App for Hello` wires the type into the Elm loop. The
  runtime owns the `Model` and calls `init`, `update`, and `view`
  for you.
- `Model = ()` is fine when there is no state to track. A real
  app puts its state here.
- `view` returns a `ViewList`. The `.into()` call turns a single
  window builder into a one-element list; multi-window apps
  return a `Vec<View>` instead.
- `plushie::run::<Hello>()` boots the runner selected by the
  enabled feature. The return type is `plushie::Result`, which
  carries a typed [`Error`](../reference/app-lifecycle.md) enum on
  failure.

## Running in direct mode

With `direct` enabled (the default), `cargo run` is all it takes:

```bash
cargo run
```

A native window opens with the title "Hello" and the text "Hello
from Plushie" inside. Close the window or press `Ctrl+C` in the
terminal to stop the app.

Direct mode keeps iced in-process. There is no subprocess, no
wire encoding, and no external binary to install. This is the
path every Plushie desktop example in the later chapters assumes
unless it says otherwise.

## Switching to wire mode

Wire mode moves the renderer into a separate process and talks to
it over stdin/stdout in MessagePack. To build an app against wire
mode, swap the feature set in `Cargo.toml`:

```toml
[dependencies]
plushie = { version = "=0.7.0", default-features = false, features = ["wire"] }
```

You also need a `plushie-renderer` binary. Install `cargo-plushie`
once, pinned to the same version as the SDK:

```bash
cargo install cargo-plushie --version 0.7.0 --locked
```

Then either download a precompiled stock renderer or build one
from source. Downloading is faster and works for apps that use
only built-in widgets:

```bash
cargo plushie download
```

The binary lands under `target/plushie/bin/` and the SDK's
discovery chain picks it up automatically. For apps that bundle
[custom widgets](../reference/custom-widgets.md), build a custom
renderer instead:

```bash
cargo plushie build --release
```

Either way, launch the app the same way:

```bash
cargo run
```

`plushie::run` walks the discovery chain (`PLUSHIE_BINARY_PATH`,
custom build output, downloaded stock binary, `plushie-renderer`
on `PATH`) and spawns the first hit. To point at an explicit
binary instead, set `PLUSHIE_BINARY_PATH` or call
`plushie::run_with_renderer(path)` from `main`.

`cargo plushie run` is a shortcut that builds the custom renderer
and starts the app in one step. Combined with `--watch`, it
rebuilds the renderer when app source changes.

For socket mode, WASM renderers, and the full discovery algorithm,
see [direct vs wire](../reference/direct-vs-wire.md) and
[configuration](../reference/configuration.md).

## What's next

You now have a Plushie app running in the mode of your choice.
The next chapter walks through building something real: state,
events, and a view that reacts to input.

Next: [Your first app](03-your-first-app.md)
