# CLI commands

`cargo-plushie` is a Cargo subcommand binary that drives the
renderer build, download, scaffold, and diagnostic flows. Once
installed on `PATH`, Cargo dispatches `cargo plushie <sub>` to it
automatically. Direct invocation as `cargo-plushie <sub>` works
too: the binary normalises both argv shapes before parsing.

| Subcommand | Purpose |
|---|---|
| [`cargo plushie build`](#cargo-plushie-build) | Build a custom renderer with bundled native widgets |
| [`cargo plushie download`](#cargo-plushie-download) | Download a precompiled stock renderer |
| [`cargo plushie run`](#cargo-plushie-run) | Build the custom renderer, then run the app |
| [`cargo plushie new-widget`](#cargo-plushie-new-widget) | Scaffold a native widget crate |
| [`cargo plushie init`](#cargo-plushie-init) | Scaffold a plushie app crate |
| [`cargo plushie doctor`](#cargo-plushie-doctor) | Print a diagnostic report |

## Installation

Pre-1.0, pin the exact patch version. The workspace ships every
crate at the same version, so the cargo-plushie version must match
the `plushie` version in the app's `Cargo.toml`.

```bash
cargo install cargo-plushie --version 0.6.1 --locked
```

Cargo resolves `cargo <sub>` by looking for a `cargo-<sub>` binary
on `PATH`. The install target is `$CARGO_HOME/bin/cargo-plushie`,
which is already on `PATH` after a standard rustup install. No
shell config is needed.

During local development against a plushie-rust checkout, invoke
from the source tree instead:

```bash
cargo run -p cargo-plushie --release --quiet -- <sub> [flags]
```

## cargo plushie build

Generate a renderer workspace that bundles every native widget in
the app's dep graph, then compile it with `cargo build`.

```bash
cargo plushie build [FLAGS]
```

| Flag | Type | Description |
|---|---|---|
| `--release` | bool | Build with the `release` Cargo profile |
| `--verbose` | bool | Print the underlying cargo command and stream its output |
| `--manifest-path <PATH>` | path | App crate manifest (default `./Cargo.toml`) |
| `--wasm` | bool | Build the `plushie-renderer-wasm` bundle via `wasm-pack` |
| `--wasm-dir <PATH>` | path | Output directory for the wasm bundle (default `target/plushie/pkg/`) |

```bash
cargo plushie build --release
```

Native-widget discovery runs off `cargo metadata`. Every dep whose
`Cargo.toml` carries a `[package.metadata.plushie.widget]` table
is registered automatically. An explicit allowlist under
`[package.metadata.plushie].native_widgets` in the app manifest
narrows the set; listed crates must be direct deps of the app and
must carry the widget metadata table, otherwise the build fails
with the offending name.

Before invoking cargo, the command runs collision checks against
the built-in widget set, the detected type names, and the detected
crate basenames. A collision fails the build with a specific
error pointing at the crate responsible.

The generated workspace lands under
`target/plushie-renderer/`, and the compiled binary under
`target/plushie-renderer/target/<profile>/`. The SDK's wire
discovery picks the binary up from there with no extra wiring;
see [Direct vs wire](direct-vs-wire.md) for the discovery chain.

When `PLUSHIE_RUST_SOURCE_PATH` is set (or
`[package.metadata.plushie].source_path` points at a local
checkout), the command writes a `.cargo/config.toml` sibling that
adds `[patch.crates-io]` redirects for every plushie-rust crate.
Without this, mixing registry crates with local crates produces
type-mismatch errors at compile time.

The `--wasm` path shells out to `wasm-pack build --target web`
against the `plushie-renderer-wasm` crate. `wasm-pack` must be on
`PATH`. When no local source path is configured, the crate is
fetched from crates.io and compiled in place; a local checkout is
only needed when native widgets require source-level patching.

## cargo plushie download

Fetch a precompiled stock renderer from GitHub releases.

```bash
cargo plushie download [FLAGS]
```

| Flag | Type | Description |
|---|---|---|
| `--force` | bool | Overwrite an existing binary |
| `--manifest-path <PATH>` | path | App crate manifest (default `./Cargo.toml`) |

```bash
cargo plushie download
```

The version is pinned to the app's `plushie-renderer-lib` version
(falling back to the `plushie` package version) as resolved from
`cargo metadata`. The binary name follows
`plushie-renderer-<os>-<arch>[.exe]` and installs to
`target/plushie/bin/`, alongside its `.sha256` sidecar.

Every download is verified against a SHA-256 checksum fetched from
the same release. A mismatch deletes the file and aborts. There
is no flag to skip verification: the binary runs as a child of the
app, so silent corruption would be a supply-chain risk.

The command refuses to run when native widgets are present in the
dep graph. The stock binary has no code for them, so a successful
download would hand back a renderer that rejects every widget
message. The error lists the offending crates; `cargo plushie
build` is the correct path for those projects.

## cargo plushie run

Build the custom renderer, then launch the app.

```bash
cargo plushie run [FLAGS]
```

| Flag | Type | Description |
|---|---|---|
| `--watch` | bool | Delegate to `cargo-watch` and rebuild on app-src change |
| `--release` | bool | Build with the `release` Cargo profile |
| `--manifest-path <PATH>` | path | App crate manifest (default `./Cargo.toml`) |

```bash
cargo plushie run --watch
```

Step one delegates to the same flow as `cargo plushie build`, so
widget discovery and collision checks share one code path. Step
two pins `PLUSHIE_BINARY_PATH` to the binary that was just
produced for the requested profile, then `exec`s `cargo run` (or
`cargo watch -w src -s 'cargo plushie build && cargo run'` when
`--watch` is set and `cargo-watch` is installed).

Pinning the binary path matters: the SDK's wire discovery probes
`release/` before `debug/` regardless of the current `cargo run`
profile. Without the explicit pin, a stale `release/` binary plus
a debug `cargo run` would launch the wrong renderer.

When `--watch` is requested but `cargo-watch` is not installed,
the command prints a one-line hint and falls through to a single
`cargo run`.

## cargo plushie new-widget

Scaffold a native widget crate.

```bash
cargo plushie new-widget <NAME> [FLAGS]
```

| Flag | Type | Description |
|---|---|---|
| `<NAME>` | string | Kebab-case widget name (e.g. `my-gauge`) |
| `--path <PATH>` | path | Destination directory (default `./native/<name>`) |

```bash
cargo plushie new-widget star-rating
```

The name becomes the Cargo package name, the wire-protocol
`type_name` (snake-cased), and a PascalCase builder struct inside
`src/lib.rs`. The scaffolded manifest declares the
`[package.metadata.plushie.widget]` table that `cargo plushie
build` reads during discovery.

If `PLUSHIE_RUST_SOURCE_PATH` is set, the scaffold emits path
dependencies pointing at the checkout so local edits to the SDK
flow straight into the new widget crate.

The command refuses to write over an existing destination and
refuses to scaffold a widget whose `type_name` would shadow a
built-in widget. Both errors are caught before any files are
written.

## cargo plushie init

Scaffold a plushie app crate with a wired `main.rs`, an
automation-script example, and a sample `.plushie` script.

```bash
cargo plushie init <NAME> [FLAGS]
```

| Flag | Type | Description |
|---|---|---|
| `<NAME>` | string | Kebab-case app name (e.g. `my-app`) |
| `--path <PATH>` | path | Destination directory (default `./<name>`) |

```bash
cargo plushie init plushie-pad
cd plushie-pad
cargo run
```

The generated crate builds in direct mode by default and prints a
next-step hint for switching to the custom renderer via
`cargo plushie run --watch`. When `PLUSHIE_RUST_SOURCE_PATH` is
set, the scaffold wires path deps through to the checkout and
drops a `.cargo/config.toml` that forwards the workspace's
`plushie-iced` patches.

## cargo plushie doctor

Print a diagnostic report and exit non-zero on any critical
finding.

```bash
cargo plushie doctor [FLAGS]
```

| Flag | Type | Description |
|---|---|---|
| `--manifest-path <PATH>` | path | App crate manifest (default `./Cargo.toml`) |

```bash
cargo plushie doctor
```

The report is read-only. It never starts the app, never modifies
files, and the version probe talks to the renderer over `--mock
--json` (the protocol-only stub path) bounded by a 5-second
timeout. Each row carries a severity: `OK`, `WARN`, or `FAIL`. A
`FAIL` on any row sets exit code 1, which CI pipelines can treat
as a hard gate.

Rows cover:

- `rustc` version and whether it meets the minimum supported
  toolchain (currently `1.92`).
- `cargo-plushie` version and host triple.
- `PLUSHIE_BINARY_PATH`, `PLUSHIE_RUST_SOURCE_PATH`,
  `PLUSHIE_MODE`, `PLUSHIE_SOCKET`.
- Renderer discovery: first hit from the SDK's wire discovery
  chain, or a `FAIL` row with install hints when nothing resolves.
- Binary architecture from `file(1)` on Unix, compared against
  the host arch.
- Detected native widgets (crate name and registered type name).
- Declared `plushie-renderer-lib` version.
- Version skew: launches the discovered binary with `--mock
  --json`, reads the `hello` line, and compares `version` to the
  app's expected renderer-lib version.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | `doctor` detected a critical issue, or any subcommand failed (argument parse, cargo invocation, network, collision, io) |

`build`, `run`, and `download` propagate the exit status of the
underlying cargo invocation, so a failed `cargo build` surfaces as
a non-zero exit from `cargo plushie build`.

## Environment variables

| Variable | Read by | Description |
|---|---|---|
| `PLUSHIE_RUST_SOURCE_PATH` | `build`, `init`, `new-widget`, `doctor` | Absolute path to a local plushie-rust checkout. Enables `[patch.crates-io]` redirects and wasm source resolution |
| `PLUSHIE_BINARY_PATH` | `run`, `doctor` | Explicit renderer binary path; set by `run` for the child `cargo run` process, reported by `doctor` |
| `PLUSHIE_MODE` | `doctor` | Reported in the diagnostic report; consumed by the SDK to force wire mode |
| `PLUSHIE_SOCKET` | `doctor` | Reported in the diagnostic report; consumed by the SDK for socket-mode rendering |
| `CARGO_TARGET_DIR` | `build`, `run`, `download`, `doctor` | Overrides the `target/` directory used for renderer output and discovery |
| `CARGO` | `build`, `run` | Overrides the `cargo` binary invoked for sub-builds (honours the rustup proxy) |

## See also

- [Direct vs wire](direct-vs-wire.md)
- [Configuration](configuration.md)
- [Custom widgets](custom-widgets.md)
- [Versioning](versioning.md)
