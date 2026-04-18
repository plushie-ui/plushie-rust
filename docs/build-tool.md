# Build Tool: cargo-plushie

`cargo-plushie` is the Cargo subcommand that drives plushie's build,
download, and scaffold flows. It sits between the app's `Cargo.toml`
and the renderer binary, handling the details that the SDK cannot
resolve at runtime: widget discovery, custom renderer generation,
stock binary fetch, and dev-loop orchestration.

Most apps only ever call `cargo plushie download` (for wire mode
without native widgets) or `cargo plushie build` (when native widgets
are present). The remaining subcommands are scaffolders and
diagnostics.

## Installation

```sh
cargo install cargo-plushie
```

The binary registers itself as a Cargo subcommand, so once installed,
both invocation shapes work:

```sh
cargo plushie <sub>     # via Cargo
cargo-plushie <sub>     # direct
```

## Quick start

### Wire mode without native widgets

For an app that uses only built-in widgets, fetch a prebuilt renderer
and run:

```sh
cargo plushie download
cargo run
```

`download` fetches a precompiled binary from GitHub releases pinned
to the exact `plushie-renderer-lib` version in the app's dep graph,
verifies its SHA-256, and installs it at
`target/plushie/bin/plushie-renderer-<os>-<arch>`. The SDK's wire-mode
discovery picks it up automatically (see
[Binary discovery](#binary-discovery)).

### App with a native widget

When the app's dep graph contains a crate carrying
`[package.metadata.plushie.widget]`, a stock binary is not enough:
the custom widget code has to be compiled into the renderer. Build
a widget-aware renderer, then run the app:

```sh
cargo plushie build
cargo run
```

`build` discovers widget crates via `cargo metadata`, writes a
generated workspace under `target/plushie-renderer/`, shells out to
`cargo build`, and leaves the widget-aware binary at
`target/plushie-renderer/target/<profile>/<app>-renderer`. Wire-mode
discovery finds that binary ahead of the downloaded stock one.

## Command reference

### `cargo plushie build`

Build a custom renderer binary wired to all native widgets in the
app's dep graph.

**Synopsis**

```
cargo plushie build [--release] [--verbose] [--manifest-path <path>]
cargo plushie build --wasm [--release] [--verbose] [--wasm-dir <path>] [--manifest-path <path>]
```

**Flags**

- `--release`: build with the `release` Cargo profile. Default is
  `debug`.
- `--verbose`: print the underlying `cargo build` command and stream
  its output through unchanged.
- `--manifest-path <path>`: path to the app's `Cargo.toml`. Defaults
  to `./Cargo.toml`.
- `--wasm`: build the `plushie-renderer-wasm` bundle via `wasm-pack`
  instead of producing a native custom renderer. Requires a resolvable
  plushie-rust source path (see [Metadata reference](#metadata-reference)).
- `--wasm-dir <path>`: output directory for the `wasm-pack` bundle.
  Defaults to `target/plushie/pkg/`. Only honored with `--wasm`.

**Behavior**

1. Runs `cargo metadata` against the app crate.
2. Filters every package in the resolved graph for a
   `[package.metadata.plushie.widget]` table. Each hit becomes a
   widget registration.
3. Runs three collision checks against the discovered widgets:
   duplicate `type_name`, shadowing a built-in widget name, and
   duplicate Cargo crate basenames (two widgets at sibling paths with
   the same directory name cannot coexist in a Cargo workspace).
4. Generates `target/plushie-renderer/Cargo.toml` and
   `target/plushie-renderer/src/main.rs`. The generator uses
   `write_if_changed` semantics: identical content skips the write so
   Cargo does not spuriously rebuild.
5. Invokes `cargo build` in the generated workspace. The resulting
   binary is named `<app>-renderer` by default (see `binary_name` in
   the [Metadata reference](#metadata-reference)).

The generated workspace links every widget crate with the `impl`
feature enabled, so the renderer compiles in the iced-based
implementations. The app crate itself keeps depending on the
iced-free stub.

**Example**

```sh
cargo plushie build --release
```

### `cargo plushie download`

Fetch a precompiled stock renderer for the host platform from GitHub
releases.

**Synopsis**

```
cargo plushie download [--force] [--manifest-path <path>]
```

**Flags**

- `--force`: overwrite an existing binary. Without this, a binary
  already present at the target path is preserved and the command
  exits successfully.
- `--manifest-path <path>`: path to the app's `Cargo.toml`. Defaults
  to `./Cargo.toml`.

**Behavior**

1. Runs `cargo metadata` to determine the expected renderer version.
   The version comes from `plushie-renderer-lib` in the dep graph, or
   (fallback) from `plushie`.
2. If any widget crate declares `[package.metadata.plushie.widget]`,
   aborts with a hard error. A stock binary cannot run a custom
   widget; downloading it would silently give the wrong result.
3. Derives the platform identifier (`linux-x86_64`, `darwin-aarch64`,
   `windows-x86_64`, etc.) and downloads
   `plushie-renderer-<os>-<arch>[.exe]` from
   `https://github.com/plushie-ui/plushie-rust/releases/download/v<version>/`.
4. Downloads the companion `.sha256` sidecar and verifies the
   digest. A mismatch aborts without installing.
5. Writes the binary to `target/plushie/bin/<download-name>` and
   marks it executable on Unix.

**Example**

```sh
cargo plushie download
cargo plushie download --force     # re-fetch even if present
```

### `cargo plushie run`

Build the custom renderer and run the app with `cargo run`. Thin
orchestrator on top of `build` + `cargo run`.

**Synopsis**

```
cargo plushie run [--watch] [--release] [--manifest-path <path>]
```

**Flags**

- `--watch`: re-run on app source changes. Requires `cargo-watch`
  (`cargo install cargo-watch`); if not installed, falls back to a
  single `cargo run` with a warning.
- `--release`: build with the `release` Cargo profile.
- `--manifest-path <path>`: path to the app's `Cargo.toml`. Defaults
  to `./Cargo.toml`.

**Behavior**

1. Runs `cargo plushie build` (honoring `--release`) so the renderer
   is up to date.
2. Either invokes `cargo run` once, or under `--watch`, runs
   `cargo watch -w src -s 'cargo plushie build && cargo run'` so that
   edits under `src/` trigger a renderer rebuild and an app restart.

The `--watch` flow watches the app crate's own `src/`. For
widget-crate watching inside the app process, use
`plushie::dev::watch_renderer::<A>()`. See
[Dev loop](#dev-loop).

**Example**

```sh
cargo plushie run
cargo plushie run --watch --release
```

### `cargo plushie new-widget`

Scaffold a new native widget crate with the conventional
`[package.metadata.plushie.widget]` layout and an `impl` feature
flag.

**Synopsis**

```
cargo plushie new-widget <name> [--path <path>]
```

**Arguments**

- `<name>`: kebab-case widget name (e.g. `my-gauge`). Becomes the
  Cargo package name, the widget `type_name` (snake-cased), and the
  PascalCase builder struct.

**Flags**

- `--path <path>`: destination directory. Defaults to `./native/<name>`.

**Behavior**

1. Validates the name: ASCII-only kebab-case, no leading or trailing
   dashes, no consecutive dashes, starts with a letter.
2. Fails if the snake-cased form would shadow a built-in widget name
   (e.g. `button`, `text`, `canvas`).
3. Fails if the destination already exists.
4. Writes `Cargo.toml` and `src/lib.rs` using the `widget!` macro,
   with an `impl` feature that gates the iced-based renderer code.
   Default features keep the crate iced-free.
5. When `PLUSHIE_SOURCE_PATH` is set, rewrites dependencies to path
   deps against the local checkout and emits a `.cargo/config.toml`
   forwarding the `plushie-iced` override so the scaffold compiles
   against the fork the same way the source workspace does.

See [Widget crate architecture](custom-widgets.md#widget-crate-architecture)
for the shape of the generated crate.

**Example**

```sh
cargo plushie new-widget my-gauge
cargo plushie new-widget my-gauge --path crates/my-gauge
```

### `cargo plushie init`

Scaffold a new plushie app crate.

**Synopsis**

```
cargo plushie init <name> [--path <path>]
```

**Arguments**

- `<name>`: kebab-case app name. Becomes the Cargo package name and a
  PascalCase `App` struct.

**Flags**

- `--path <path>`: destination directory. Defaults to `./<name>`.

**Behavior**

Scaffolds a Cargo crate with:

- `src/main.rs` wiring `plushie::cli::run::<MyApp>()` so the reserved
  `--plushie-*` flags work out of the box.
- `examples/plushie_script.rs` stub that routes the same entry point
  through `cargo run --example plushie_script`.
- `scripts/smoke.plushie` sample automation script.
- `[package.metadata.plushie]` marker in `Cargo.toml`.

When `PLUSHIE_SOURCE_PATH` is set, dependencies are rewritten to path
deps and a `.cargo/config.toml` forwards the `plushie-iced` override,
matching the behavior of `new-widget`.

**Example**

```sh
cargo plushie init my-app
cd my-app
cargo run
```

### `cargo plushie doctor`

Print a diagnostic report. Non-zero exit status if any critical issue
is detected.

**Synopsis**

```
cargo plushie doctor [--manifest-path <path>]
```

**Flags**

- `--manifest-path <path>`: path to the app's `Cargo.toml`. Defaults
  to `./Cargo.toml`.

**Behavior**

Gathers and prints, one per row, each with an `OK` / `WARN` / `FAIL`
severity marker:

- `rustc` toolchain version (fails if below the supported minimum).
- `cargo-plushie` version (matches the tool's own build).
- `host` (OS and architecture).
- `PLUSHIE_BINARY_PATH`, `PLUSHIE_SOURCE_PATH`, `PLUSHIE_MODE`,
  `PLUSHIE_SOCKET` environment variable values.
- `renderer`: the discovered renderer path (see
  [Binary discovery](#binary-discovery)).
- `arch`: binary architecture via `file(1)` on Unix, with a critical
  warning on host mismatch.
- `native widgets`: each widget crate discovered in the dep graph.
- `renderer-lib`: the app's expected renderer version.
- `version skew`: spawns the renderer with `--mock --json`, reads the
  hello line, and compares the reported version against
  `renderer-lib`. Mismatch is critical; the handshake would reject
  the pair at run time.

The command is read-only. It never launches the host app and never
spawns a renderer in a way that affects live sessions.

**Example**

```sh
cargo plushie doctor
```

## Metadata reference

Two metadata sections drive the build tool. Both live under the
Cargo-reserved `[package.metadata]` namespace so they are ignored by
other tooling.

### App crate: `[package.metadata.plushie]`

Optional. When present, fields refine the default behavior.

```toml
[package.metadata.plushie]
binary_name = "my-app-renderer"       # optional
source_path = "../plushie-rust"       # optional; also read from env
native_widgets = ["my-gauge"]         # optional explicit allowlist
app = true                            # marker for dev tooling
```

**Fields**

- `binary_name` (string, optional): override the custom renderer
  binary name. Without this, `cargo plushie build` derives
  `<app_name>-renderer` with underscores replaced by dashes.
- `source_path` (string, optional): path (relative to the manifest)
  to a plushie-rust checkout. Used in two places: rewriting
  dependencies in the generated renderer workspace to path deps, and
  locating `plushie-renderer-wasm` for `--wasm` builds. The
  `PLUSHIE_SOURCE_PATH` env var wins over this key.
- `native_widgets` (array of strings, optional): explicit allowlist
  of widget crates to compile into the renderer. When set, only the
  listed crates are registered; auto-discovery is skipped. Each name
  must be a direct dependency of the app crate and must declare a
  `[package.metadata.plushie.widget]` table; misspellings and
  non-plushie crates surface as a hard error rather than a silent
  omission. Leave this field unset (or empty) to use the default
  full auto-discovery.
- `app` (bool, optional): marker indicating this crate is a plushie
  app. Scaffolders set it so future workspace-wide tooling can find
  app crates without guessing.

### Widget crate: `[package.metadata.plushie.widget]`

Required on every crate that wants to be discovered as a native
widget.

```toml
[package.metadata.plushie.widget]
type_name = "my_gauge"
constructor = "my_gauge::factory::MyGaugeFactory::new()"
```

**Fields**

- `type_name` (string, required): the wire-protocol type name the
  renderer will use to route tree nodes to this widget. Must be
  snake_case, unique across every widget in the dep graph, and must
  not shadow a built-in.
- `constructor` (string, required): a Rust expression that produces
  an instance of the widget's renderer-side factory. The generator
  injects this into the generated `main.rs` as
  `.widget(<constructor>)`. Use a simple path call with no arguments
  (for example `my_gauge::factory::MyGaugeFactory::new()`). Malformed
  expressions surface as compile errors when the generated workspace
  builds; the build-tool currently does not pre-validate the shape.
  This Cargo.toml entry is the single source of truth: the `widget!`
  macro does not accept a `constructor` attribute.

## Binary discovery

The SDK's wire-mode runner locates a renderer using a four-step
chain. The first hit wins.

1. **`PLUSHIE_BINARY_PATH` environment variable.** Explicit intent:
   if the variable is set but the path does not resolve to an
   existing file, the runner errors without falling through. Use this
   to point at a binary outside the Cargo target directory (for
   example, a system-installed release).

2. **Custom build output:**
   `target/plushie-renderer/target/<profile>/<bin-name>` where
   `<bin-name>` is the resolved binary name from
   `cargo plushie build`. Both `release` and `debug` are tried, in
   that order.

3. **Downloaded stock binary:** `target/plushie/bin/<download-name>`
   where `<download-name>` is `plushie-renderer-<os>-<arch>[.exe]`.
   This is where `cargo plushie download` installs.

4. **`plushie-renderer` on `PATH`.** Catch-all for users who ran
   `cargo install plushie-renderer`.

If every step falls through, the runner returns
`Error::BinaryNotFound` with guidance that names all three install
paths.

`cargo plushie doctor` prints the resolved path under the `renderer`
row and mirrors the same chain, so a diagnostic run tells you which
step picked up.

## Mode precedence

`plushie::run::<A>()` (and `plushie::cli::run::<A>()`) selects between
direct and wire mode at startup. The precedence is:

1. **Socket connect.** `PLUSHIE_SOCKET` env or `--plushie-socket
   <path>` CLI argument. Attaches to a listen-mode renderer over the
   socket. The token (if any) is resolved from `--plushie-token`,
   then `PLUSHIE_TOKEN`, then a one-line JSON negotiation message
   read from stdin with a one-second timeout.

2. **Explicit spawn binary.** `PLUSHIE_BINARY_PATH` env. Spawns the
   specified binary in wire mode without further discovery.

3. **Forced mode.** `PLUSHIE_MODE` env or `--plushie-mode=<mode>`
   CLI. Accepts `direct` or `wire`. `wire` triggers the four-step
   discovery chain; `direct` falls through to the feature default.

4. **Feature default.** Compiled `direct` feature wins; otherwise
   the wire-spawn path with four-step discovery.

### Typical scenarios

- **Local GUI app (default).** App is built with the `direct` feature
  (the default). No env vars set. `plushie::run` stays in-process
  with iced.

- **Remote renderer over SSH.** The renderer listens on a Unix socket
  on the local machine; the app runs on the remote host with
  `PLUSHIE_SOCKET=/tmp/plushie.sock` exported and an SSH forward for
  the socket. `plushie::run` picks precedence step 1 and connects.

- **Renderer spawned the app.** The renderer opens a pipe to the app
  and sets `PLUSHIE_SOCKET` in the child's environment before exec.
  The app attaches back to the renderer without negotiating a binary
  of its own.

- **Dual-feature distribution (the pacman scenario).** A package
  ships a single binary compiled with both `direct` and `wire`. On
  GUI desktops, direct mode wins. On headless servers, the user
  exports `PLUSHIE_MODE=wire` and the SDK discovers a renderer on
  its own.

## Environment variable reference

| Variable              | Purpose                                                       | Precedence notes                              |
|-----------------------|---------------------------------------------------------------|-----------------------------------------------|
| `PLUSHIE_BINARY_PATH` | Absolute path to a renderer binary. Explicit intent.          | Highest in binary discovery; explicit failure if unreadable. |
| `PLUSHIE_SOURCE_PATH` | Absolute path to a plushie-rust checkout.                      | Used by `build --wasm`, generated renderer workspace, and scaffolders. Overrides `source_path` metadata. |
| `PLUSHIE_MODE`        | `direct` or `wire`.                                            | Precedence step 3. Superseded by `PLUSHIE_SOCKET` and `PLUSHIE_BINARY_PATH`. |
| `PLUSHIE_SOCKET`      | Path or host:port of an existing renderer listening socket.    | Precedence step 1, highest. Triggers wire-connect mode. |
| `PLUSHIE_TOKEN`       | Auth token presented during socket handshake.                  | Falls back to stdin JSON negotiation when unset. |
| `CARGO_TARGET_DIR`    | Standard Cargo setting. Honored by the build tool and the SDK. | Redirects `target/plushie-renderer/` and `target/plushie/bin/`. |

## Reserved CLI flags

`plushie::cli::run::<A>()` parses flags prefixed with `--plushie-`.
Any `--plushie-*` flag the easy path does not recognize is a hard
error pointing the user at `--plushie-help`. Flags without the
prefix pass through untouched, so user-owned CLIs can coexist.

| Flag                      | Action                                                     |
|---------------------------|------------------------------------------------------------|
| `--plushie-help`          | Print the reserved-flag summary and exit.                  |
| `--plushie-mode=<mode>`   | Force `direct` or `wire` (see [Mode precedence](#mode-precedence)). |
| `--plushie-socket <path>` | Attach to a listen-mode renderer over socket.              |
| `--plushie-token <token>` | Token presented during socket handshake.                   |
| `--plushie-script <path>` | Execute a `.plushie` automation script against a headless TestSession and exit. Honors the header's `backend:` field (`mock`, `headless`, `windowed`). |
| `--plushie-replay <path>` | Replay a `.plushie` script against a live windowed renderer. Forces `backend: windowed` regardless of the header so the user can watch the replay. |
| `--plushie-inspect`       | Emit a pretty-JSON snapshot of the initial view tree and exit. |

Apps that need their own CLI can skip the easy path entirely and
call `plushie::run`, `plushie::run_connect`, or the
`plushie::automation::cli` helpers directly. The easy path is a thin
wrapper over those primitives.

## Dev loop

Two options for keeping the renderer in sync while iterating.

### External: `cargo plushie run --watch`

Runs `cargo-watch` outside the app process, watching `src/` and
chaining `cargo plushie build && cargo run` on every change. Works
for any app shape; the app restarts fresh on every rebuild.

```sh
cargo plushie run --watch
```

Requires `cargo install cargo-watch`. A missing `cargo-watch` falls
back to a single `cargo run` with a warning on stderr.

### Internal: `plushie::dev::watch_renderer`

In-process file watcher gated behind the `dev` Cargo feature. Wires
into the app's `main` and restarts the renderer subprocess in place,
preserving Model state. Best when the app is long-running and the
Model is expensive to rebuild.

```toml
[dependencies]
plushie = { version = "0.6", features = ["dev"] }
```

```rust
fn main() -> plushie::Result {
    plushie::dev::watch_renderer::<MyApp>()
}
```

`watch_renderer` reads the app's cargo metadata, watches each widget
crate's `src/` and `Cargo.toml`, and re-runs `cargo plushie build`
after a debounce window (250 ms by default). Status is pushed to an
optional `DevOverlayHandle`, which the runtime tree walker picks up
and renders as a slim status bar at the top of every window. See
`plushie::dev::RebuildingOverlay` for the overlay states
(Rebuilding, Success, Failed, Frozen) and their dismiss behavior.

The in-process watcher does not watch the app's own source: the
running binary cannot be swapped from the inside. Combine with
`cargo plushie run --watch` if the app crate is also under active
development.

## Troubleshooting

Most wire-mode setup problems are one `cargo plushie doctor` run
away from a diagnosis. Start there.

- **"renderer binary not found".** None of the four discovery steps
  resolved. Run `cargo plushie download` (stock renderer) or
  `cargo plushie build` (widget-aware custom renderer), or set
  `PLUSHIE_BINARY_PATH` to a known-good binary.

- **"cannot download: native widgets declared".** `download` refuses
  to install a stock binary when widget crates are present, because
  the stock binary does not know how to render them. Use
  `cargo plushie build` instead.

- **"sha256 mismatch".** The downloaded binary did not match its
  sidecar. Rerun `cargo plushie download --force`; persistent
  mismatches usually mean a truncated download or a CDN serving a
  stale file.

- **"version skew".** The doctor row says the app expects one
  version but the binary reports another. Remove
  `target/plushie-renderer/` and `target/plushie/bin/` and rebuild
  from scratch. The handshake rejects mismatched protocol versions.

- **`--wasm` fails with "unable to locate plushie-renderer-wasm
  source".** Set `PLUSHIE_SOURCE_PATH` to a plushie-rust checkout,
  or add `source_path` under `[package.metadata.plushie]`. WASM
  builds have no registry path today; the source crate must be on
  disk.
