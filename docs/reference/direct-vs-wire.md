# Direct vs wire

Plushie ships two runner modes behind a single feature-agnostic
entry point. `plushie::run::<A>()` dispatches at compile time to
either an in-process iced daemon (direct) or a subprocess renderer
that speaks the wire protocol over stdin/stdout (wire). Both
modes share the same engine, the `plushie-renderer-lib` crate, so
the widget surface, event taxonomy, commands, and subscriptions
behave identically.

## Modes at a glance

| Mode | Feature | Renderer lives in | Footprint | Survives renderer crash |
|---|---|---|---|---|
| Direct | `direct` (default) | app process | smallest runtime | n/a (single process) |
| Wire | `wire` | `plushie-renderer` subprocess | smallest SDK compile | yes, via `RestartPolicy` |

Direct wins when both features are enabled. Force wire at runtime
via `plushie::run_with_renderer(path)`, `plushie::run_spawn()`, or
the `PLUSHIE_MODE=wire` env / `--plushie-mode wire` CLI flag, all
honoured inside `plushie::run`.

## Direct mode

Direct mode embeds the renderer directly in the application
binary. `plushie::run::<A>()` boots an iced daemon
(`runner/direct.rs`) that owns a `plushie_renderer_lib::App`.
Events are delivered through an in-process queue sink. There is no
wire encoding, no subprocess, and no stdin/stdout handshake.

Enable (it is the default):

```toml
[dependencies]
plushie = "0.7.1"
```

Direct-mode dependencies add `rfd` (file dialogs), `arboard`
(clipboard), `notify-rust` (notifications), `parking_lot`, and
`plushie-widget-sdk`. Platform effects resolve inside the app
through `DirectEffectHandler` (`runner/effects.rs`).

Direct mode does not compile to `wasm32`: several transitive deps
are native-only. For a browser target, see [WASM
renderer](#wasm-renderer).

Custom widgets in direct mode link their `PlushieWidget` impls at
compile time. Register them with the widget set before launch.

## Wire mode

Wire mode spawns the `plushie-renderer` binary and talks to it
over stdin/stdout. The app process owns the Elm loop, encodes
view trees as patches, and consumes decoded events. The renderer
owns iced, the GPU context, window lifecycles, and platform
effects.

Enable wire (and drop direct if you want a wire-only build):

```toml
[dependencies]
plushie = { version = "0.7.1", default-features = false, features = ["wire"] }
```

Wire mode adds `rmp-serde` (MessagePack) and `base64`. It does
not pull in `rfd`, `arboard`, or `notify-rust`: effect requests
are serialized and the renderer handles them.

The renderer is a separate process, so the SDK survives a
renderer crash. `App::restart_policy()` controls auto-restart;
`App::handle_renderer_exit(&mut model, reason)` fires on every
unexpected exit, then once more with `ExitReason::MaxRestartsReached`
when the policy is exhausted. Neither hook fires in direct mode
(the app and renderer share a process).

### Binary discovery

`plushie::run` resolves the renderer in this order (first hit
wins), mirroring `runner/wire_discovery.rs`:

1. `PLUSHIE_BINARY_PATH` env. Explicit; a missing file fails
   fast rather than falling through.
2. Custom build output under
   `target/plushie-renderer/target/{release,debug}/`, populated
   by `cargo plushie build`. Release profile is preferred.
3. Downloaded stock binary at
   `bin/plushie-renderer(.exe)`,
   populated by `cargo plushie download`.
If nothing resolves, `plushie::run` returns `Error::BinaryNotFound`
with guidance naming every install path. On Unix the discovered
binary is passed to `file(1)` and an architecture mismatch is
logged as a warning; discovery does not abort on a mismatch
because the binary may still run under emulation.

Socket mode (`plushie::run_connect`, or `PLUSHIE_SOCKET` /
`--plushie-socket`) connects to an existing renderer over a
Unix domain socket or TCP socket instead of spawning one. The
discovery chain does not apply.

### Custom widgets

Custom `PlushieWidget` implementations must be compiled into the
renderer binary. Use `cargo plushie build` to generate a workspace
that bundles the widget crates into a custom `plushie-renderer`.
The resulting binary lives under
`target/plushie-renderer/target/<profile>/` where discovery step
two will find it automatically.

Alternatively, pass an explicit path to
`plushie::run_with_renderer(path)` and skip discovery entirely.

## What is identical, what differs

Everything in the app-facing surface is identical between modes
unless the table below lists it.

| Area | Direct | Wire |
|---|---|---|
| Widget builders, `ui::*` | identical | identical |
| `Event` variants, `widget_match()` | identical | identical |
| `Command` constructors | identical | identical |
| `Subscription` constructors, `.max_rate`, `.for_window` | identical | identical |
| `App::settings`, `App::window_config`, themes | identical | identical |
| `TestSession` harness | identical | identical |
| File dialogs, clipboard, notifications | `rfd` / `arboard` / `notify-rust` in-process | renderer handles the request |
| Custom widgets | linked at compile time | bundled into a custom renderer binary |
| `App::handle_renderer_exit`, `App::restart_policy` | never fires | fires on subprocess exit |
| Wire encoding on every render | none | MessagePack frame per patch |
| Can drive a remote display | no | yes (socket mode) |
| `wasm32` target | no | renderer-side only, see below |

SDK-local commands (`Command::Async`, `Command::Stream`,
`Command::SendAfter`, `Command::Cancel`) execute in the app
process in both modes. Wire mode uses a private two-worker
tokio runtime by default; apps that already own a runtime can
call `plushie::run_wire_with_runtime(path, handle)` to share it.

## Cargo feature flags

| Feature | Default | Pulls in | Notes |
|---|---|---|---|
| `direct` | yes | `plushie-renderer-lib`, `rfd`, `arboard`, `notify-rust`, `parking_lot`, `plushie-widget-sdk` | native only |
| `wire` | no | `rmp-serde`, `base64` | works without iced |
| `dev` | no | `notify`, `cargo_metadata` | watch-mode, dev overlay |

Valid combinations:

- `direct` only: native app, in-process renderer.
- `wire` only: smallest SDK, requires a `plushie-renderer`
  binary at runtime.
- `direct` + `wire`: both runners compiled in. `plushie::run`
  picks direct by default; `PLUSHIE_MODE=wire`, `PLUSHIE_SOCKET`,
  `PLUSHIE_BINARY_PATH`, `run_with_renderer`, `run_spawn`, and
  `run_connect` force wire.
- Neither: `plushie::run` returns `Error::NoRunnerFeature` at
  runtime. There is no link-time guard, so the failure is
  visible only when `run` is called.

```toml
[dependencies]
plushie = { version = "0.7.1", default-features = false, features = ["wire"] }
```

Version pinning: pre-1.0, pin the exact patch version.
`plushie`, `plushie-core`, `plushie-renderer-lib`, and
`plushie-renderer` release as a single workspace; the renderer
binary version must match the SDK version. The SDK exposes the
version it was built against as `plushie::RENDERER_VERSION` and
logs a warning if the wire `hello` message reports a different
value.

## Performance profile

Direct mode has no wire encoding step. A view refresh walks the
tree, applies renderer-side caches, and hands iced an
`Element<'_, Message, Theme, Renderer>` directly.

Wire mode serializes each patch to MessagePack and writes it to
the renderer's stdin; events come back the same way. The extra
cost is per-render and per-event, not per-frame (iced drives its
own frame loop inside the renderer). The upside: the renderer
can run on a different host (socket mode) or in a browser (WASM
renderer), and a renderer crash does not terminate the app.

Do not treat the mode choice as a raw performance decision
without measurement. Most apps are bound by view construction
in Rust code, which is identical in both modes.

## WASM renderer

The `plushie-renderer-wasm` crate compiles
`plushie-renderer-lib` to `wasm32-unknown-unknown` with
`wasm-bindgen`. It is the renderer, not the app SDK. The Rust
app SDK is native-only today: `rfd`, `arboard`, and
`notify-rust` do not build for `wasm32`.

Host SDKs written in other languages (Gleam, Elixir, JavaScript)
can drive the WASM renderer from the browser via the wire
protocol over a JavaScript transport. A Rust app can reach the
same WASM renderer too, but only if the app runs outside the
browser and speaks the wire protocol through some browser-hosted
bridge (websocket, shared worker, etc.). There is no current
path for building a pure Rust plushie app that runs as WASM in
the browser with its renderer.

Build a WASM renderer with `cargo plushie build --wasm`. See
`crates/plushie-renderer-wasm/src/lib.rs` for the `PlushieApp`
JavaScript API.

## Choosing a mode

Desktop-only native app with no crash-isolation requirement:
direct mode. It is the default for a reason: smallest friction,
no external binary, no subprocess overhead, and custom widgets
link at compile time.

Need crash isolation, remote rendering, or a renderer shared
with a host SDK in another language: wire mode. Pair it with a
`RestartPolicy` suited to the app's tolerance and an
`handle_renderer_exit` hook that persists anything the renderer
was holding for you.

Custom renderer (bundled widgets, patched iced, forked renderer
binary): wire mode with `plushie::run_with_renderer` pointing at
the custom binary, or direct mode with the custom widget crates
listed as dependencies and registered before launch.

Browser target: host SDK + WASM renderer. Not Rust + WASM.

## Standalone packaging

Direct-mode Rust apps already build to a native executable. If the
app has no external payload, the first supported standalone artifact
is the release or dist-profile binary itself. This is the path used
by the direct-mode demo smoke: build the app, run it from a different
working directory, and verify it starts under the same display setup
as other SDK artifacts.

Use the shared package launcher when a Rust app needs more than the
single executable can carry cleanly:

- Payload files such as images, fonts, migrations, or generated data.
- Shared platform metadata such as app identity, icon, publisher, or
  update channel.
- Signing, notarization, installer, or update hooks that should use
  the same manifest shape as wire SDKs.
- A cache-managed payload lifecycle where a replaced binary must
  extract and run the matching embedded payload.

Wire-mode Rust apps follow the same shape as other wire SDKs. Use
`cargo plushie package assemble` to build the host with wire support,
assemble a payload containing the host executable and a payload-local
`plushie-renderer`, write `plushie-package.toml`, and hand it to
`cargo plushie package portable` for a self-extracting artifact or
`cargo plushie package bundle` for a platform package. Socket-mode flags
(`--plushie-socket`, `--plushie-token`) are the Rust CLI spelling of the
same renderer-parent contract represented by `PLUSHIE_SOCKET` and
`PLUSHIE_TOKEN` in other SDK payloads.

## See also

- [Wire protocol](wire-protocol.md)
- [Custom widgets](custom-widgets.md)
- [CLI commands](cli-commands.md)
- [App lifecycle](app-lifecycle.md)
- [Configuration](configuration.md)
