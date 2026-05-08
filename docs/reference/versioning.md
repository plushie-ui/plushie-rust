# Versioning

Plushie ships as a single Cargo workspace that releases all its
crates together on a shared version. This page is the authoritative
reference for what that version covers, how the wire protocol
versions independently, how pinning works in a consumer's
`Cargo.toml`, and how host SDKs in other languages align their
renderer binaries with the Rust crates.

## Workspace version

The root `Cargo.toml` declares `[workspace.package].version` and
every workspace crate inherits it with `version.workspace = true`.
The workspace members that share this version:

- `plushie` (app SDK)
- `plushie-core`
- `plushie-core-macros`
- `plushie-widget-sdk`
- `plushie-renderer-lib`
- `plushie-renderer` (binary)
- `plushie-renderer-wasm`
- `cargo-plushie`

Internal path deps in `[workspace.dependencies]` are pinned to the
same string (for example `plushie-core = { version = "0.7.0", path
= "crates/plushie-core" }`), so a release bump is a single edit to
the workspace `version` plus the matching entries, and publishes
go out as one batch.

Pre-1.0 the project follows a conservative cadence: minor bumps
may break APIs, patch bumps may not. Post-1.0 it will follow
SemVer strictly.

## What the version covers

The workspace version pins three surfaces at once:

- **The Rust API.** Every item under `plushie::*`, `plushie_core::*`,
  `plushie_widget_sdk::*`, and the derive macros
  (`plushie_core_macros`) moves together. A patch release to
  `plushie` implies a patch release of everything else.
- **The widget surface.** `plushie::ui::*` builders, prop types,
  event variants, `Command` constructors, and `Subscription`
  constructors all live under this version. Adding a widget is a
  minor bump; renaming a setter is a breaking change.
- **The `cargo-plushie` CLI.** The subcommand tree (`build`,
  `download`, `run`, `new-widget`, `init`, `doctor`) and the
  manifest conventions it reads (`[package.metadata.plushie]`,
  native widget metadata under `[package.metadata.plushie.widget]`)
  are versioned with the SDK. Upgrading the workspace version
  means installing the matching `cargo-plushie`.

The workspace version does not cover the wire protocol. See the
next section.

## Wire protocol version

The wire protocol carries its own independent integer version in
the `hello` handshake and in the `Settings` message. The Rust
constant is `plushie_core::protocol::PROTOCOL_VERSION` (currently
`1`).

Two rules keep host SDKs in multiple languages able to talk to a
Rust renderer:

- **Patch releases never change the wire protocol.** A patch bump
  of the workspace version ships the same `PROTOCOL_VERSION` as
  the previous patch. This is what lets a downstream host SDK pin
  a renderer binary at a specific patch and trust that newer
  patches remain drop-in.
- **`PROTOCOL_VERSION` bumps are minor or major changes.** The
  constant increments only when a wire shape changes in a way
  that older hosts can't parse, and the release notes call out
  the bump.

On startup `runner/wire.rs` reads the renderer's `hello` message,
pulls `protocol_version` out of it, and compares against the SDK's
`PROTOCOL_VERSION`. A mismatch is fatal: the SDK returns an error
rather than speaking a protocol it doesn't understand. A separate
check compares the renderer's reported crate version against
`plushie::RENDERER_VERSION`; that comparison is advisory and
produces a warning log when the strings differ.

## Pinning `plushie` in Cargo.toml

Pre-1.0, pin the exact patch version. Cargo's default caret
requirement would allow `^0.7.0` to roll forward to `0.8.0`, which
at pre-1.0 is a potentially breaking change. Use an `=` requirement:

```toml
[dependencies]
plushie = "=0.7.0"
```

For a wire-only build, the same pin applies with the feature
swap:

```toml
[dependencies]
plushie = { version = "=0.7.0", default-features = false, features = ["wire"] }
```

Once the project reaches 1.0 the standard caret requirement
becomes safe and this guidance relaxes to `plushie = "1"`.

## Downloaded vs built renderer binaries

There are two ways to obtain a `plushie-renderer` binary, and the
workspace version governs both.

**Downloaded stock renderer.** `cargo plushie download` fetches a
precompiled renderer from the GitHub releases page. The version is
resolved from the app's dep graph: `cargo-plushie` runs `cargo
metadata`, finds the `plushie-renderer-lib` package (falling back
to `plushie`), and reads its version. The download URL is
`https://github.com/plushie-ui/plushie-rust/releases/download/v{version}/...`
and the binary lands at `target/plushie/bin/`. There is no
`--version` flag: changing the version means changing the
`Cargo.toml` pin.

**Custom-built renderer.** `cargo plushie build` generates a
workspace under `target/plushie-renderer/` that bundles the app's
native widget crates and builds a renderer from source. The
generated `Cargo.toml` pins every plushie-rust crate to the app's
effective workspace version (read from `cargo metadata`), so the
custom renderer links against exactly the same crate versions the
SDK is using. The resulting binary lives under
`target/plushie-renderer/target/<profile>/` and wire-mode discovery
picks it up automatically.

Either way, the renderer binary's `CARGO_PKG_VERSION` should match
the SDK's `RENDERER_VERSION`. Running a mismatched pair logs a
warning at handshake time and works when the protocol version
agrees, but it is not a supported configuration.

## Host-SDK pinning

Host SDKs in other languages (Gleam, Elixir, TypeScript) do not
consume the Rust crates directly. They pin a renderer-binary
version in their own manifest through an environment variable or
config entry commonly named `PLUSHIE_RUST_VERSION`, then install
the matching `cargo-plushie` to build or download a renderer at
that version.

The host-SDK pin plays the same role as the Rust `Cargo.toml`
pin: it names a workspace version, and every artifact the host
needs (the renderer binary, the WASM bundle, the protocol
semantics) comes out of that single version. The `hello` handshake
enforces compatibility at runtime, so a host that pins
`PLUSHIE_RUST_VERSION=0.7.0` is compatible with any Rust app that
pins `plushie = "=0.7.0"` in its `Cargo.toml`.

## Upgrade guidance

- Patch upgrades (`0.7.0 -> 0.7.1`): bump the pin, rebuild,
  re-run. No code changes expected. Wire protocol unchanged.
- Minor upgrades pre-1.0 (`0.7.x -> 0.8.0`): read the
  `CHANGELOG.md` entry. Breaking API changes are listed first.
  Rebuild the renderer binary (or re-download) so its version
  matches the SDK; mixing a `0.7` renderer with a `0.8` SDK
  produces a `hello` warning at best and a protocol-version error
  at worst.
- When upgrading, `cargo plushie doctor` checks for version skew
  between the SDK's `RENDERER_VERSION`, the renderer binary
  resolved via `wire_discovery`, and the `cargo-plushie` tool
  itself, and flags any mismatch.

## See also

- [Direct vs wire](direct-vs-wire.md)
- [Wire protocol](wire-protocol.md)
- [CLI commands](cli-commands.md)
- [Configuration](configuration.md)
