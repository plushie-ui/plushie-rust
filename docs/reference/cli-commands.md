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
| [`cargo plushie tools`](#cargo-plushie-tools) | Check or sync project-local native tools |
| [`cargo plushie run`](#cargo-plushie-run) | Build the custom renderer, then run the app |
| [`cargo plushie package`](#cargo-plushie-package) | Package command group |
| [`cargo plushie package portable`](#cargo-plushie-package-portable) | Build a portable launcher from a package manifest |
| [`cargo plushie package check`](#cargo-plushie-package-check) | Check a package manifest or portable launcher |
| [`cargo plushie package manifest validate`](#cargo-plushie-package-manifest-validate) | Validate a manifest file without a payload archive |
| [`cargo plushie package assemble`](#cargo-plushie-package-assemble) | Build a wire-mode Rust app payload and manifest |
| [`cargo plushie default-icons`](#cargo-plushie-default-icons) | Write bundled default app icons |
| [`cargo plushie new-widget`](#cargo-plushie-new-widget) | Scaffold a native widget crate |
| [`cargo plushie init`](#cargo-plushie-init) | Scaffold a plushie app crate |
| [`cargo plushie doctor`](#cargo-plushie-doctor) | Print a diagnostic report |

## Installation

Pre-1.0, pin the exact patch version. The workspace ships every
crate at the same version, so the cargo-plushie version must match
the `plushie` version in the app's `Cargo.toml`.

```bash
cargo install cargo-plushie --version 0.7.1 --locked
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
| `--features <LIST>` | string | Cargo features to enable while resolving the app graph |
| `--no-default-features` | bool | Disable default features while resolving the app graph |
| `--all-features` | bool | Enable all features while resolving the app graph |

```bash
cargo plushie build --release
```

Native-widget discovery runs off `cargo metadata`. Feature flags are
passed through to that metadata call so feature-gated widget
dependencies match the app graph being packaged. Every dep whose
`Cargo.toml` carries a `[package.metadata.plushie.widget]` table is
registered automatically. An explicit allowlist under
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
| `--required-version <VERSION>` | string | Exact plushie-rust version to download without Cargo metadata |
| `--manifest-path <PATH>` | path | App crate manifest (default `./Cargo.toml`) |

```bash
cargo plushie download
```

The version is pinned to the app's `plushie-renderer-lib` version
(falling back to the `plushie` package version) as resolved from
`cargo metadata`. Release assets use platform-specific filenames,
but the installed project binary is always `bin/plushie-renderer`
(or `bin/plushie-renderer.exe` on Windows), alongside its `.sha256`
sidecar.

Every download is verified against a SHA-256 checksum fetched from
the same release. A mismatch deletes the file and aborts. There
is no flag to skip verification: the binary runs as a child of the
app, so silent corruption would be a supply-chain risk.

### Release mirrors

By default downloads come from GitHub releases. Set
`PLUSHIE_RELEASE_BASE_URL` to verify the same flow against another
release mirror. The mirror must expose assets as
`BASE/vVERSION/ARTIFACT` with checksum sidecars at
`BASE/vVERSION/ARTIFACT.sha256`.

Remote mirrors must use HTTPS. `file://` mirrors and loopback HTTP are
for local release verification before assets are uploaded.

The command refuses to run when native widgets are present in the
dep graph. The stock binary has no code for them, so a successful
download would hand back a renderer that rejects every widget
message. The error lists the offending crates; `cargo plushie
build` is the correct path for those projects.

## cargo plushie tools

Check or sync the project-local Plushie native tool set under `bin/`.
The same implementation is also available from the standalone
`plushie` binary, which is the intended entry point for non-Rust SDKs
after they bootstrap `bin/plushie`.

```bash
cargo plushie tools check [FLAGS]
cargo plushie tools sync [FLAGS]
```

| Flag | Command | Type | Description |
|---|---|---|---|
| `--required-version <VERSION>` | check, sync | string | Exact plushie-rust version expected by the SDK |
| `--manifest-path <PATH>` | check, sync | path | App crate manifest when resolving the required version through Cargo metadata |
| `--strict` | check | bool | Treat dirty or mixed-source tools as failures |
| `--json` | check | bool | Emit machine-readable output |
| `--force` | sync | bool | Allow replacing source-built, custom, or identity-less tools |

`tools check` probes `plushie`, `bin/plushie-renderer`, and
`bin/plushie-launcher` with `--version --json`. It reports missing,
unreadable, stale, cross-target, dirty, or mixed-provenance tools with
a fix command. Pre-1.0 checks use exact plushie-rust version equality,
because SDKs are pinned one-to-one with the Rust side.

`tools sync` verifies that the running `plushie` tool matches the
required version, then installs the matching `plushie`,
`plushie-renderer`, and `plushie-launcher` into `bin/` with stable
local filenames. A release-built `plushie` downloads release assets and
verifies their checksum sidecars. A source-built `plushie` builds the
same native tool set from the same plushie-rust checkout and copies
those binaries into `bin/`. Existing downloaded release tools are
replaced by default because the command is the user's explicit sync
intent. `--force` is only needed when replacing a likely intentional
non-download tool, such as a source-built binary.

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

## cargo plushie package

Package command group for assembling Rust wire-mode payloads, building
portable launchers, running Plushie-specific checks, and delegating
platform bundles to `cargo-packager`.

```bash
cargo plushie package assemble
cargo plushie package portable --manifest plushie-package.toml
cargo plushie package check --manifest plushie-package.toml --postcheck
cargo plushie package bundle --manifest plushie-package.toml --format appimage
```

## cargo plushie package portable

Build a portable Rust launcher from a Plushie package manifest and
payload archive.

```bash
cargo plushie package portable --manifest plushie-package.toml
```

| Flag | Type | Description |
|---|---|---|
| `--manifest <PATH>` | path | Plushie package manifest |
| `--lax-tools` | bool | Skip the strict managed-tool check; tools that are missing, dirty, mixed, or version-mismatched are allowed. Strict checking is on by default |
| `--out <PATH>` | path | Final launcher path (default `target/plushie/package/<app-id>`) |
| `--launcher <PATH>` | path | Reusable `plushie-launcher` binary to embed package data into |
| `--run-signing-hooks` | bool | Run signing hooks declared in the manifest |
| `--verbose` | bool | Print launcher template resolution |

This command is the shared portable wrapper step for standalone
binaries. SDKs
still own host-language packaging: an Elixir SDK packages a release, a
Gleam SDK packages a shipment, Python can package a PyInstaller payload,
and so on. `cargo plushie package portable` consumes the resulting payload archive
and manifest, appends both to a reusable `plushie-launcher` binary, and
writes one executable.

`host_sdk` is descriptive metadata. `cargo plushie package portable` does not
branch on it to build Mix releases, Gleam shipments, PyInstaller apps,
Node SEA executables, Ruby runtimes, Burrito payloads, or Rust app
binaries.

The manifest stores structured argv:

```toml
schema_version = 1
app_id = "com.example.notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "python"
plushie_rust_version = "0.7.1"
protocol_version = 1

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = ["PATH", "HOME", "LANG", "LC_ALL", "XDG_RUNTIME_DIR", "WAYLAND_DISPLAY", "DISPLAY"]

[renderer]
path = "bin/plushie-renderer"
kind = "stock"
source = "download"

[platform]
publisher = "Example Inc."
bundle_id = "com.example.notes"
icon = "assets/icon.png"

[updates]
channel = "stable"
feed_url = "https://example.com/notes/updates.json"

[[signing.hooks]]
phase = "after-launcher-build"
command = ["codesign", "--sign", "Developer ID Application: Example Inc.", "{launcher}"]

[payload]
archive = "payload.tar.zst"
hash = "sha256:..."
```

The first supported payload shape is one archive containing the renderer,
host payload, assets, metadata, and notices. Use conventional paths:
`bin/` for executable entry points, `host/` or a language-specific
runtime directory for host files, `assets/` for app assets, and
`licenses/` for third-party notices. The manifest path values must match
archive paths exactly. Split renderer and host archives are not part of
the initial launcher contract.

The portable launcher verifies the embedded archive hash, extracts it
into a content-addressed cache, rejects archive entries that can escape
the app package, sets executable permissions where needed, and starts
the packaged app command. Before launching the app command it clears the
ambient environment, sets `PLUSHIE_PACKAGE_DIR` to the extracted app
package directory, sets `PLUSHIE_BINARY_PATH` to the packaged renderer,
and forwards only names listed in `start.forward_env`.

```bash
bin/notes
```

The app command is responsible for starting or connecting to the
renderer through its SDK's normal entry point. This keeps the package
launcher language-neutral while still giving every SDK a deterministic
renderer path through `PLUSHIE_BINARY_PATH`.
`start.forward_env` cannot include launcher-owned package variables such
as `PLUSHIE_BINARY_PATH` or `PLUSHIE_PACKAGE_DIR`; those are always set
by the launcher.

`target` is a normalized package target such as `linux-x86_64`,
`darwin-aarch64`, or `windows-x86_64`. It must match the current build
host. Cross-target package manifests are rejected until target-aware
assembly is implemented. `payload.archive` is manifest-relative.
`renderer.path`, `start.working_dir`, and `start.command[0]` are
app-package-relative paths. Absolute paths and parent traversal are
rejected so a standalone package cannot silently point at a global
binary. The launcher resolves `start.command[0]` to an absolute path
inside the extracted app package and sets the command's working
directory to `start.working_dir`.

Payload archives are intentionally plain files and directories. Archive
entries must be relative paths under the payload root. Symlinks, hard
links, device files, sockets, FIFOs, and other special entries are
rejected by validation and again by the portable launcher before
extraction. SDK packagers that copy language runtimes should dereference
or remove runtime symlinks before archiving, and should not rely on tar
link entries to preserve runtime structure.

The launcher makes `renderer.path` and `start.command[0]` executable
after extraction on Unix platforms. Additional executable scripts or
nested launchers should be declared through the host entry point or
preserved by the SDK packager's archive mode; there is no broad
manifest-side permission table yet. Windows executability follows file
extension and loader behavior rather than Unix mode bits.

The optional `[renderer]` table records provenance for diagnostics and
SDK validation. `kind` is `stock` or `custom`; `source` is an SDK-defined
string such as `download` or `local-build`. Native-widget package
commands should write `kind = "custom"` and fail before packaging if
they would ship a stock renderer. The manifest can later grow
SDK-provided native widget metadata, but the current launcher only needs
renderer provenance and the payload-local renderer path.

The optional `[platform]`, `[updates]`, and `[signing]` tables reserve
one shared metadata shape for SDK packagers and later platform packaging
layers. `platform.icon` is payload-relative and must exist in the
archive when it is set. If `platform.icon` is missing,
`cargo plushie package portable` prints a warning and continues. SDK package
commands should add a real app icon or include Plushie's bundled defaults
before archiving the payload. Update metadata is descriptive; the
portable launcher does not download updates. Signing hooks are
structured argv declarations that `cargo plushie package portable` can run after
it copies the final launcher into place, but only when
`--run-signing-hooks` is passed. Hooks run from the package manifest
directory without shell wrapping, and `{launcher}` expands to the final
launcher path. Payload hash verification, update signing, feed
publishing, and platform signing are owned by cargo-packager and updater
tooling when `package bundle` is used.

Rust direct-mode apps do not need the shared wire launcher when they are
already a single native executable. They should use normal Cargo and
platform packaging for that case, while reusing the same metadata names
when a platform package, update feed, icon, or signing step needs them.

The reusable launcher template is resolved from `--launcher`,
`PLUSHIE_LAUNCHER_PATH`, `bin/plushie-launcher`, or a sibling of the
running `plushie` executable. This lets SDKs package stock-renderer apps
from a downloaded native tool set without compiling Rust.

After a successful launcher run, cache pruning keeps the active payload
and the most recent previous payload for the same app ID. Older payload
directories are removed. Cache and output names include a deterministic
app ID hash so different app IDs that sanitize to the same path segment
do not share package artifacts. Failed launches do not prune cache
entries. A relative `PLUSHIE_CACHE_DIR` is resolved against the
launcher's current working directory before payload paths are composed.
The launcher writes diagnostics to stderr with the app ID, app version,
payload hash, cache path, cache reuse status, renderer path, host path,
and host exit status.

## cargo plushie package bundle

Create a platform package through the `cargo-packager` library. The
command consumes a Plushie package manifest and either an existing
portable executable or a portable executable it builds first. Plushie
writes a minimal `Packager.toml` when `--config` is omitted, then calls
the packager API directly.

```bash
bin/plushie package bundle --manifest dist/plushie-package.toml --format appimage
bin/plushie package bundle --manifest dist/plushie-package.toml --portable dist/notes --format appimage
bin/plushie package bundle --manifest dist/plushie-package.toml --config Packager.toml
```

| Flag | Type | Description |
|---|---|---|
| `--manifest <PATH>` | path | Plushie package manifest |
| `--portable <PATH>` | path | Existing portable executable to bundle. Builds one when omitted |
| `--out-dir <PATH>` | path | Output directory for cargo-packager artifacts |
| `--format <FORMAT>` | string | cargo-packager format. Repeatable. Defaults to cargo-packager's platform default |
| `--config <PATH>` | path | Custom cargo-packager config. Plushie generates one when omitted |
| `--lax-tools` | bool | Skip the strict managed-tool check; tools that are missing, dirty, mixed, or version-mismatched are allowed. Strict checking is on by default |
| `--launcher <PATH>` | path | Reusable launcher template used when building the portable executable |
| `--run-signing-hooks` | bool | Run manifest signing hooks when building the portable executable |
| `--verbose` | bool | Print launcher template resolution |

The generated cargo-packager config uses the manifest app ID, app name,
version, bundle ID, publisher, and payload icon where available. More
advanced cargo-packager signing, notarization, installer, and updater
settings should live in a committed cargo-packager config and be passed
with `--config`.

Use `cargo plushie package check` to check the manifest, payload hash,
and archive safety without building a launcher. Use `--postcheck` to
build the launcher and run its extraction path with an isolated
`PLUSHIE_CACHE_DIR`. The `cargo plushie package check --postcheck` path
runs the portable artifact with `--postcheck`, scrubs development
renderer overrides, asserts first-extraction and cache-reuse
diagnostics, and exits before starting the renderer or GUI app. It does
not prove host-renderer readiness. Run the portable artifact normally
under an artifact postcheck harness when that stronger signal is needed.

```bash
cargo plushie package check --manifest dist/plushie-package.toml
cargo plushie package check --manifest dist/plushie-package.toml --postcheck
```

## cargo plushie package check

Check a package manifest, payload, or portable launcher.

```bash
cargo plushie package check --manifest plushie-package.toml
cargo plushie package check --manifest plushie-package.toml --postcheck
```

| Flag | Type | Description |
|---|---|---|
| `--manifest <PATH>` | path | Plushie package manifest |
| `--lax-tools` | bool | Skip the strict managed-tool check; tools that are missing, dirty, mixed, or version-mismatched are allowed. Strict checking is on by default |
| `--postcheck` | bool | Build the portable launcher and run the extraction/cache postcheck path |
| `--postcheck-timeout <SECONDS>` | integer | Maximum time for `--postcheck` to wait |
| `--launcher <PATH>` | path | Reusable `plushie-launcher` binary to use during `--postcheck` |
| `--verbose` | bool | Print launcher template resolution |

## cargo plushie package manifest validate

Validate a package manifest TOML file without requiring a payload archive.

```bash
cargo plushie package manifest validate plushie-package.toml
cargo plushie package manifest validate dist/plushie-package.toml
```

Parses and validates schema, required fields, `app_id` format, path safety,
target, protocol version, and all optional section rules (`[platform]`,
`[updates]`, `[signing]`). The payload archive does not need to exist.

Exits 0 when valid. Exits non-zero on any validation error; the error is
printed to stderr. Non-fatal issues (such as a missing platform icon) are
printed as warnings.

Useful for CI lint steps, IDE integration, and SDK packagers that want to
confirm the generated manifest is well-formed before assembling the payload
archive.

## plushie-launcher

Run a package manifest and sibling payload archive through the reusable
launcher runtime. This binary is shipped as a release asset for SDKs
that need package execution without compiling Rust code.

```bash
bin/plushie-launcher --manifest dist/package/plushie-package.toml
bin/plushie-launcher --manifest dist/package/plushie-package.toml --postcheck
./dist/notes
./dist/notes --postcheck
```

| Flag | Type | Description |
|---|---|---|
| `--manifest <PATH>` | path | Plushie package manifest. The payload archive path is resolved relative to this manifest |
| `--postcheck` | bool | Validate extraction and diagnostics without starting the host |
| `--version` | bool | Print human-readable launcher identity |
| `--json` | bool | Emit machine-readable identity when used with `--version` |

The reusable launcher consumes the same `plushie-package.toml` shape as
`cargo plushie package portable`. `package portable` copies the reusable
launcher, appends the package manifest and payload archive, and writes a
single executable. The same runtime also supports `--manifest PATH` for
debugging an assembled package directory without first writing the
portable artifact.

## cargo plushie package assemble

Build a Rust SDK app as a wire-mode host payload, assemble it with a
payload-local renderer, and write `plushie-package.toml`. Always builds
with the release profile. Prints a `cargo plushie package portable`
handoff line on success; use that command or `package bundle` for the
final artifact.

```bash
cargo plushie package assemble
```

| Flag | Type | Description |
|---|---|---|
| `--manifest-path <PATH>` | path | Rust app crate manifest (default `./Cargo.toml`) |
| `--bin <NAME>` | string | Cargo binary target when the package has multiple bins |
| `--app-id <ID>` | string | Package app ID. Defaults to metadata or package name |
| `--app-name <NAME>` | string | Optional human-readable app name |
| `--icon <PATH>` | path | App icon copied into the payload |
| `--out-dir <DIR>` | path | Directory for generated manifest and archive |
| `--package-config <PATH>` | path | Developer-owned source package config. Defaults to `plushie-package.config.toml` next to the app manifest when present |
| `--write-package-config` | bool | Write a package config template and exit before building |
| `--features <LIST>` | string | Additional host Cargo features |
| `--no-default-features` | bool | Disable default features for the host build |
| `--all-features` | bool | Enable all features for the host build |
| `--verbose` | bool | Print underlying cargo commands |

The command is the Rust SDK-owned preparation step for the shared
launcher. It first reuses `cargo plushie build` with the same feature
selection to produce a renderer binary, then builds the selected Rust
app binary with `plushie/wire` enabled. Pass a package `Cargo.toml`;
virtual workspace manifests are rejected until `package assemble` grows an
explicit package selector. The payload uses conventional paths:

```text
bin/<host>
bin/plushie-renderer
assets/<icon>
```

The generated manifest writes `host_sdk = "rust"`, the app version
from Cargo metadata, the local platform target, `protocol_version`,
`host_sdk_version`, `plushie_rust_version`, `[platform].icon`, and a
`[renderer]` table with `kind = "custom"` and `source = "local-build"`.
If `--icon` is omitted, the command writes Plushie's bundled default
icons into `assets/` before archiving and points `[platform].icon` at
the large PNG.

`package assemble` only assembles packages for the current build host. Cargo
cross-target builds, including `CARGO_BUILD_TARGET` and build-target
configuration that places the host binary under a target-triple
directory, are rejected until target-aware assembly is implemented.

`package assemble` reads `plushie-package.config.toml` next to the app
manifest when present. Pass `--package-config` to use another path. The
source config only owns host startup settings:

```toml
config_version = 1

[start]
working_dir = "."
command = ["bin/my-app"]
forward_env = ["PATH", "HOME", "LANG", "LC_ALL", "XDG_RUNTIME_DIR", "WAYLAND_DISPLAY", "DISPLAY"]
```

The command can write a template with real default values:

```bash
cargo plushie package assemble --write-package-config
```

On success the command prints the next step:

```
Build launcher with:
  cargo plushie package portable --manifest target/plushie/rust-package/plushie-package.toml
```

Inspect or precheck the generated files before building the launcher:

```bash
cargo plushie package check --manifest target/plushie/rust-package/plushie-package.toml
```

Direct-mode Rust apps do not need this launcher path when the app is a
single native executable. Build them with Cargo's release or dist
profile and hand that native binary to the platform package manager.
Use `package assemble` when the app uses wire/connect mode or needs the
shared payload lifecycle.

## cargo plushie default-icons

Write Plushie's bundled default app icon PNGs to a directory.

```bash
cargo plushie default-icons --out dist/payload/assets
```

| Flag | Type | Description |
|---|---|---|
| `--out <DIR>` | path | Directory to receive the bundled icon files |

SDK package commands can call this before payload archiving when an
app does not provide its own icon. The generated files are ordinary
payload assets, so package manifests should reference them with a
payload-relative `[platform].icon` path.

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
| `PLUSHIE_RUST_SOURCE_PATH` | `build`, `package assemble`, `init`, `new-widget`, `doctor` | Absolute path to a local plushie-rust checkout. Enables `[patch.crates-io]` redirects and wasm source resolution |
| `PLUSHIE_RELEASE_BASE_URL` | `download`, `tools sync` | Override release asset base URL for mirrored release verification. Remote mirrors must use HTTPS. `file://` and loopback HTTP are for local checks |
| `PLUSHIE_BINARY_PATH` | `run`, package launcher, `doctor` | Explicit renderer binary path; set by `run` for the child `cargo run` process, set by package launchers for the packaged app command, reported by `doctor` |
| `PLUSHIE_PACKAGE_DIR` | package launcher | Set for the packaged app command to the extracted app package directory |
| `PLUSHIE_MODE` | `doctor` | Reported in the diagnostic report; consumed by the SDK to force wire mode |
| `PLUSHIE_SOCKET` | `doctor` | Reported in the diagnostic report; consumed by the SDK for socket-mode rendering |
| `PLUSHIE_CACHE_DIR` | package launcher | Overrides the extraction cache root. Relative values are made absolute from the launcher's current working directory |
| `CARGO_TARGET_DIR` | `build`, `run`, `download`, `doctor`, `package`, `package assemble` | Overrides the `target/` directory used for renderer output, discovery, and Rust package assembly. Relative values are resolved from the cargo-plushie invocation directory |
| `CARGO` | `build`, `run`, `package`, `package assemble` | Overrides the `cargo` binary invoked for sub-builds (honours the rustup proxy) |

## See also

- [Direct vs wire](direct-vs-wire.md)
- [Configuration](configuration.md)
- [Custom widgets](custom-widgets.md)
- [Versioning](versioning.md)
