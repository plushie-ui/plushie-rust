# Deployment

The earlier guides assumed `cargo run`. That's the loop for
writing an app. Shipping it is a different exercise. This chapter
covers two tracks that tend to get conflated and shouldn't be.

The first is packaging a native Rust Plushie app for distribution:
a release binary, stripped symbols, signed installers on each
platform. Direct mode and wire mode diverge here because wire mode
ships a second binary alongside the app.

The second is the WASM renderer. `plushie-renderer-wasm` compiles
the renderer (not the app SDK) to `wasm32-unknown-unknown`, which
is useful for browser-hosted apps driven from another language.
A Rust app itself does not compile to `wasm32` today, and the
chapter is explicit about why and what works in its place.

## Native packaging

A release build is the starting point for every distribution
path. Run it from the app crate:

```bash
cargo build --release
```

The output lands at `target/release/<app-name>` (plus `.exe` on
Windows). That is a runnable binary on the host it was built for.
For a smaller artifact, strip debug symbols in a profile override:

```toml
[profile.release]
strip = "symbols"
lto = "thin"
codegen-units = 1
```

`lto = "thin"` and a single codegen unit trade compile time for a
few percent of binary size and startup speed. Measure before
committing to either; both hurt iteration noticeably.

Cross-compiling to another triple requires the toolchain target
installed (`rustup target add <triple>`) and, for anything that
links C, a matching C toolchain. For Linux to Windows,
`cargo-xwin` handles the linker setup. For Linux to macOS,
`cargo-zigbuild` with Zig as the linker is the usual path. Both
are third-party and outside the scope of this guide.

### Installer formats

Platform-native installers are owned by external tooling:

- **Debian / Ubuntu**: `cargo-deb` reads `[package.metadata.deb]`
  and produces a `.deb` from the release binary plus a
  `.desktop` entry and any declared assets.
- **RPM**: `cargo-generate-rpm` reads
  `[package.metadata.generate-rpm]` and produces an `.rpm`.
- **Windows MSI**: `cargo-wix` wraps WiX Toolset. Ship a `.wxs`
  template in the repo and let `cargo wix` stamp in the version
  from `Cargo.toml`.
- **macOS .app bundle and .dmg**: `cargo-bundle` produces a
  `.app`. Turning that into a notarised, signed `.dmg` is
  Apple's tooling (`codesign`, `notarytool`, `create-dmg`).

Signing keys live outside the repository. Inject them through CI
secrets, not `Cargo.toml`. The Plushie SDK does not enumerate
platform signing commands; each platform's documentation is the
reference.

## Direct mode: single binary

Direct mode embeds the renderer into the app binary. `cargo build
--release` produces exactly one executable, and that executable
has no runtime dependency on a sibling `plushie-renderer` file.
Install it, run it, done.

Dependencies that ship alongside (fonts loaded at startup,
platform libraries like `libwayland-client.so`) follow the
platform's usual rules. `rfd`, `arboard`, and `notify-rust` link
statically in a release build on Linux, so they do not add
runtime shared-object requirements.

For a single-file distribution, direct mode is the simpler
target. No wire-mode discovery logic runs, no `PLUSHIE_BINARY_PATH`
needs to be set, and nothing fails because the renderer is on a
different filesystem path than expected.

## Wire mode: binary plus renderer

Wire mode needs two executables: the app, and the `plushie-renderer`
subprocess the app launches. The SDK resolves the renderer through
the discovery chain documented in
[direct vs wire](../reference/direct-vs-wire.md). A shipped app
cannot rely on `cargo plushie build` output under `target/`: that
path only exists at development time.

The practical options are:

1. **Install the renderer next to the app.** Ship both binaries
   in the same package. At startup, set `PLUSHIE_BINARY_PATH` to
   the installed renderer path before calling `plushie::run`, or
   call `plushie::run_with_renderer(path)` directly. Either
   bypasses discovery entirely.
2. **Use a custom renderer.** Apps with native widgets must build
   a renderer with `cargo plushie build --release` and package
   the resulting binary from
   `target/plushie-renderer/target/release/`. The stock renderer
   has no code for custom widgets.
3. **Rely on a PATH install.** Viable for developer tooling,
   unreliable for end-user distribution. `cargo plushie download`
   fetches a stock binary, but the path it installs to
   (`target/plushie/bin/`) is not on an end user's `PATH`.

An installer for a wire-mode app copies the renderer into the
same platform-native location as the app binary (`/usr/bin/`,
`Program Files\<App>\`, `Applications/<App>.app/Contents/MacOS/`)
and the app's startup code points at it:

```rust
use std::path::PathBuf;

fn main() -> Result<(), plushie::Error> {
    let renderer = installed_renderer_path();
    plushie::run_with_renderer::<MyApp>(renderer)
}

fn installed_renderer_path() -> PathBuf {
    std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("exe dir")
        .join(if cfg!(windows) { "plushie-renderer.exe" } else { "plushie-renderer" })
}
```

Version pinning applies. The renderer version must match the SDK
version the app was built against, surfaced as
`plushie::RENDERER_VERSION`. Shipping a mismatched pair produces
a handshake warning at best and a wire-format crash at worst. The
release workflow that builds the app binary should build (or
download) the renderer in the same step and package both as one
artifact.

### Crash isolation as a deployment concern

Wire mode's split process model survives renderer crashes with
the `RestartPolicy` configured on the app. That matters for
long-running desktop software more than it does for a tool that
opens briefly and quits. If the policy is doing real work in
production, `App::handle_renderer_exit` is the hook that logs the
crash, persists the model, or shows a recovery UI. See
[app lifecycle](../reference/app-lifecycle.md) for the exit-hook
contract.

## The WASM renderer crate

`crates/plushie-renderer-wasm/` builds the renderer for the
browser. It is the same code as `plushie-renderer-lib` compiled
to `wasm32-unknown-unknown` and wrapped in `wasm-bindgen` glue.
The widget surface is identical to the native renderer; the
differences are in the platform effects (no `rfd`, no `arboard`,
no `notify-rust`, no multi-window) and the transport (a
JavaScript callback instead of stdout).

What the crate is not: an SDK. It does not let you write a
Plushie app in Rust and run it in the browser. The `plushie`
crate, which hosts `App::update` and the Elm loop, is native-only
because its default-on dependencies (`rfd`, `arboard`,
`notify-rust`, a tokio runtime via `wire`) do not target
`wasm32`. See [Why Rust apps don't compile to WASM
today](#why-rust-apps-dont-compile-to-wasm-today) for the
long version.

## Building the WASM renderer

`cargo plushie build --wasm` shells out to `wasm-pack build
--target web` against the `plushie-renderer-wasm` crate.
`wasm-pack` must be on `PATH`. When no local source path is
configured, the crate is fetched from crates.io and compiled in
place; no local checkout is required for stock builds.

```bash
# Stock build - fetches plushie-renderer-wasm from crates.io:
cargo plushie build --wasm

# Against a local checkout via env var:
PLUSHIE_RUST_SOURCE_PATH=~/projects/plushie-rust \
    cargo plushie build --wasm

# Or via package metadata:
cargo plushie build --wasm
```

```toml
[package.metadata.plushie]
source_path = "../plushie-rust"
```

The output is a pair of files:

- `plushie_renderer_wasm.js` (wasm-bindgen JavaScript glue)
- `plushie_renderer_wasm_bg.wasm` (compiled binary)

Default destination is `target/plushie/pkg/`. Override with
`--wasm-dir` for direct placement into a static-asset tree:

```bash
cargo plushie build --wasm --wasm-dir priv/static/wasm
```

A release build passes `--release` through:

```bash
cargo plushie build --wasm --release
```

`wasm-pack` must be on `PATH`. Install it from
<https://rustwasm.github.io/wasm-pack/>. The Rust
`wasm32-unknown-unknown` target must be installed too (`rustup
target add wasm32-unknown-unknown`), though `wasm-pack` prompts
if it is missing.

See [CLI commands](../reference/cli-commands.md#cargo-plushie-build)
for the full flag list.

## Loading it in a browser

The glue module exposes a default `init` function and a
`PlushieApp` class. `init` fetches and instantiates the `.wasm`
binary; the class constructor starts an iced daemon inside the
browser and hands back a handle for sending wire-protocol
messages into the renderer.

```html
<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8" />
  <title>Plushie in the browser</title>
  <style>
    body { margin: 0; overflow: hidden; }
    canvas { width: 100vw; height: 100vh; display: block; }
  </style>
</head>
<body>
  <canvas id="plushie-canvas"></canvas>
  <script type="module">
    import init, { PlushieApp } from "./wasm/plushie_renderer_wasm.js";

    await init();

    const settings = JSON.stringify({
      protocol_version: 1,
      fonts: [],
    });

    const app = new PlushieApp(settings, (event) => {
      // Route renderer event to the app host (see below).
    });

    // Later, push a wire-protocol message into the renderer:
    // app.send_message(JSON.stringify({ type: "snapshot", tree: ... }));
  </script>
</body>
</html>
```

`PlushieApp` takes two constructor arguments. The first is a JSON
settings object: the WASM renderer requires a
`protocol_version` field matching
`plushie_widget_sdk::protocol::PROTOCOL_VERSION`, and missing or
mismatched values fail fast with a JS exception. The second is a
callback invoked once per outbound event, called with a JSON
string.

`send_message(json)` pushes an incoming message (Snapshot, Patch,
Settings, Subscribe, Effect response, etc.) into the renderer.
The wire format is always JSON here; MessagePack is native-only.

Serve the `.wasm` file with `Content-Type: application/wasm` for
streaming compilation. Gzip or brotli on the `.wasm` body
dominates first-load page weight; enable it at the edge. See
[wire protocol](../reference/wire-protocol.md) for the message
schema the callback and `send_message` exchange.

## Driving it from JavaScript

For a pure-browser app with no server component, the host for the
Elm loop is plain JavaScript. The loop owns the model, calls
`update` on events, calls `view` to produce a tree, diffs against
the previous tree, and pushes patches into `app.send_message`.
The callback handed to the `PlushieApp` constructor is where
events come back out.

```javascript
let model = { count: 0 };

function view(m) {
  return {
    type: "window",
    id: "main",
    children: [
      { type: "text", id: "count", label: `Count: ${m.count}` },
      { type: "button", id: "inc", label: "+" },
    ],
  };
}

function update(m, event) {
  if (event.type === "widget_click" && event.id === "inc") {
    return { ...m, count: m.count + 1 };
  }
  return m;
}

const app = new PlushieApp(JSON.stringify({ protocol_version: 1 }), (raw) => {
  const event = JSON.parse(raw);
  model = update(model, event);
  app.send_message(JSON.stringify({ type: "snapshot", tree: view(model) }));
});

app.send_message(JSON.stringify({ type: "snapshot", tree: view(model) }));
```

This is the minimum viable host. A real JS host needs tree
diffing, subscription management, command execution, and event
routing. Writing that host from scratch is a substantial project.
In practice, the WASM renderer is paired with a host SDK that
already implements the loop.

## Driving it from another language's SDK

This is the pattern the Gleam and Elixir SDKs use for the
browser. The host SDK compiles to JavaScript or WebAssembly, runs
in the same page as the renderer, and drives the `PlushieApp` via
`send_message`. The SDK provides `update`, `view`, subscriptions,
and commands in its native language; the renderer handles layout
and paint.

The Gleam SDK, for example, compiles a full Plushie app (Elm loop
and widget tree and pure types) to JavaScript via Gleam's JS
target. In a browser page, the generated JS code imports the
WASM renderer module, constructs a `PlushieApp`, and drives it
from the JS-compiled Gleam runtime. Both app and renderer live in
the same browser tab. No server, no WebSocket.

Rust cannot do this today because the Rust SDK does not compile
to `wasm32`. A Rust app author reaching for a browser target
picks a different host SDK (Gleam, Elixir with LiveView, plain
JS) and keeps the renderer story (the `plushie-renderer-wasm`
bundle) identical. The widget surface, event taxonomy, and wire
protocol are the same regardless of host.

## Driving it from a remote Rust app

A Rust app that wants to render into a browser without rewriting
in another language has one path: run the app process on a
server (or locally outside the browser), keep the renderer in the
browser, and bridge the two with a WebSocket.

The shape looks like this:

```
  Browser                        Server / host machine
  -------                        ---------------------
  PlushieApp (WASM) <--JSON-->  WebSocket <--JSON-->  plushie::run_connect
  renders to canvas              (text frames)         (Rust app, wire mode)
```

On the server side, a Unix or TCP socket listens for connections
and `plushie::run_connect` (or `PLUSHIE_SOCKET=<addr>`) wires the
SDK's wire transport to it. On the browser side, JavaScript
accepts incoming frames and calls `app.send_message(frame)` for
each one, plus forwards the renderer's event callback back over
the same WebSocket.

The browser-side glue is a small amount of hand-written JS:
accept a wire frame from the WebSocket, push it into
`PlushieApp`, read the event callback, push events back out to
the WebSocket. The SDK does not ship this glue; the wire
protocol and the `PlushieApp` API are stable enough to let an app
author write it in a few dozen lines.

This setup is wire mode with a WebSocket-shaped transport
instead of stdin/stdout. Every SDK feature that works in wire
mode works here: crash isolation (the WebSocket disconnects; the
SDK reconnects per `RestartPolicy`), subscriptions, commands,
custom widgets (bundled into the WASM renderer via the same
`cargo plushie build --wasm` path, with widget crates declared as
path deps of the `plushie-renderer-wasm` build).

The Rust app still runs natively. The browser is a display.

## Why Rust apps don't compile to WASM today

The short answer: three default-on dependencies of the `plushie`
crate (`rfd`, `arboard`, `notify-rust`) are native-only. `rfd`
binds platform file dialogs (GTK / AppKit / Win32). `arboard`
binds platform clipboard APIs. `notify-rust` binds DBus on Linux
and NSUserNotification on macOS. None compile to `wasm32`, and
the `direct` feature pulls all three in unconditionally.

Even with `default-features = false, features = ["wire"]`, the
wire feature pulls a tokio runtime for async commands. Tokio's
default multi-threaded runtime does not run on `wasm32` (no
threads, no `mio`). A single-threaded runtime works on `wasm32`
with feature flags, but the SDK does not currently gate on that.

Beyond the direct dependencies, the widget set loaded by the app
process compiles against `plushie-widget-sdk`, which assumes
native iced types in places. Making the whole SDK truly
wasm-portable would mean splitting the crate into
`plushie-core` (pure, wasm-safe, already present) and
`plushie-host` (native platform glue), pushing every platform
call behind a trait, and making the `App` trait object-safe
enough for a browser-hosted Elm loop to drive it via JS
interop. That work hasn't been done.

Other language SDKs do not have this problem because their host
runtimes (BEAM compiled to JS, JavaScript, etc.) already run in
the browser and re-implement the Elm loop in that environment.
Gleam in particular has a target-aware compiler: the same
codebase produces BEAM modules and JavaScript modules from the
same source.

## Future directions

None of this is immutable. A plausible path to browser-hosted
Rust apps:

- Split `plushie` into `plushie-core` (already exists, pure) and
  `plushie-runner-native`. The app-facing API (`App` trait,
  `Command`, `Event`, widget builders) moves into
  `plushie-core`.
- Add `plushie-runner-web`, a crate that implements the runner
  loop against browser APIs. `rfd` stubs to `<input
  type="file">`. `arboard` stubs to `navigator.clipboard` (with
  gesture gating). `notify-rust` stubs to the Notifications API.
- Gate tokio behind a `native-async` feature; use
  `wasm-bindgen-futures::spawn_local` for `Command::task` on
  `wasm32`.
- Bundle the app crate and the WASM renderer into one `.wasm`
  binary with a single `init()` entry point.

That's a substantial refactor. The `plushie-core` split already
done makes it less speculative than it would be otherwise, but
it is not on any near-term path. For now, a Rust app that needs
a browser target pairs with a host SDK in another language.

## Summary

Native packaging is a release build plus the platform's
installer tooling, which lives outside the SDK. Direct mode
distributes one binary; wire mode distributes the app plus the
renderer, pinned to matching versions, with `PLUSHIE_BINARY_PATH`
or `run_with_renderer` bypassing the discovery chain at startup.

The `plushie-renderer-wasm` crate is the renderer compiled for
the browser. Build it with `cargo plushie build --wasm`, serve
the output as static assets, load the JS glue, construct a
`PlushieApp`, drive it with JSON wire messages. Useful paired
with a host SDK in another language, or with a remote Rust app
over a WebSocket bridge. Not yet useful for pure Rust apps in
the browser; that path requires a crate-level refactor that has
not been done.

## See also

- [Direct vs wire](../reference/direct-vs-wire.md)
- [CLI commands](../reference/cli-commands.md)
- [Configuration](../reference/configuration.md)
- [App lifecycle](../reference/app-lifecycle.md)
- [Wire protocol](../reference/wire-protocol.md)
