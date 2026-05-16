# Packaging and distribution

`cargo plushie package-rust assemble` turns a Rust Plushie app into a
self-contained payload that ships with its own renderer. The shared
`cargo plushie package portable` and `cargo plushie package bundle`
steps then wrap that payload into a portable single-file executable or
an OS-native installer (AppImage, `.dmg`, `.msi`). The recipient does
not need Rust, Cargo, or anything else installed.

When the artifact runs, the launcher extracts the payload to a
content-addressed cache, starts the packaged host binary, and the host
spawns its renderer from inside the extracted payload. The flow is the
same as a wire-mode `cargo run`, just with the host and renderer paths
pinned to the payload instead of `target/`.

This is the wire-mode packaging path. Direct-mode apps are already a
single native executable; see [Direct-mode apps](#direct-mode-apps)
below.

## The packaging pipeline

A packaged Rust app moves through three stages:

1. **Host build and payload assembly.** `cargo plushie package-rust
   assemble` builds the renderer (reusing `cargo plushie build`),
   builds the app binary with the `plushie/wire` feature enabled,
   assembles both into a payload directory alongside the icon, archives
   the payload, and writes a complete `plushie-package.toml`. This
   stage is Rust-specific.
2. **Optional assemble step.** Other SDKs hand a partial manifest to
   `cargo plushie package assemble` for completion. The Rust SDK does
   not need this step: `package-rust assemble` produces a complete
   manifest directly. The cross-SDK schema is the same either way.
3. **Artifact build.** `cargo plushie package portable` produces a
   self-extracting single-file executable. `cargo plushie package
   bundle` produces OS-native installers via
   [cargo-packager](https://github.com/crabnebula-dev/cargo-packager).
   Both consume the same completed manifest and are shared across
   every Plushie SDK.

## cargo plushie package-rust assemble

Stage 1 of the pipeline. The command builds the renderer, builds the
host binary with wire support, assembles the payload, and writes the
manifest. Always release profile. See
[CLI commands](cli-commands.md#cargo-plushie-package-rust-assemble) for
the full flag reference.

```bash
cargo plushie package-rust assemble
cargo plushie package portable --manifest target/plushie/rust-package/plushie-package.toml
```

The `app_id` is a reverse-DNS identifier in the
`namespace.[subnamespace.]app` form (`dev.example.notes`,
`com.acme.invoice`). When `--app-id` is omitted, the command falls
back to `[package.metadata.plushie.app_id]` in the app's `Cargo.toml`,
then to the Cargo package name. The shared package step validates the
format during precheck.

The output directory defaults to
`target/plushie/rust-package/`. Pass `--out-dir` to override.
Repeated runs overwrite the payload directory and archive in place.

Virtual workspace manifests are rejected: pass a package `Cargo.toml`
explicitly with `--manifest-path` when the app lives inside a
workspace. Cross-target host builds are also rejected for now;
`CARGO_BUILD_TARGET` and target-triple build configuration cause an
early error.

## The payload

`target/plushie/rust-package/payload-root/` is the directory that gets
archived:

```
target/plushie/rust-package/
  plushie-package.toml             # complete manifest
  payload.tar.zst                  # archived payload (sha256 + size recorded)
  payload-root/
    bin/
      <app>                        # release-profile host binary
      plushie-renderer             # payload-local renderer copy (.exe on Windows)
    assets/
      <icon>                       # app icon from --icon, or bundled default
```

The host binary is the app itself. There is no `bin/start_host` wrapper
script: the Rust host is already a native executable that knows how to
talk wire. The launcher invokes it directly, with
`PLUSHIE_BINARY_PATH` pointing at the payload-local
`bin/plushie-renderer`. The packaged app never reaches out to the
system `PATH` or a download cache; everything it needs is inside the
extracted payload.

The renderer is always packaged as `kind = "custom"`. `cargo plushie
package-rust assemble` reuses `cargo plushie build`, which bundles
every native widget in the app's dependency graph into the renderer
binary. A stock renderer cannot carry custom widget code, so even apps
with no native widgets ship the build path's output for consistency.

## Source layout

Packaging adds project-owned files that belong in version control and
generated files that do not. Knowing which is which avoids accidentally
committing release artifacts.

| Path | What it is | Commit or gitignore |
|---|---|---|
| `Cargo.toml` | Crate manifest. May carry `[package.metadata.plushie]` keys (`app_id`, `app_name`). | Already committed. |
| `plushie-package.config.toml` | Optional developer-owned package config: start command, forward_env, assets. | Commit. |
| `package_assets/` | Optional project-owned files copied verbatim into the payload root. | Commit. |
| `bin/plushie-renderer` etc. | Managed tools synced by `cargo plushie tools sync` (renderer, launcher, plushie CLI). Platform-specific. | Gitignore. |
| `target/` | Standard Cargo build output; includes `target/plushie/rust-package/` and `target/plushie/package/`. | Already gitignored by default. |

A minimum `.gitignore` for a packaging-enabled project looks like:

```
/bin/
/target/
```

`cargo plushie download`, `cargo plushie package-rust assemble`, and
the shared portable step each check whether their output path is
gitignored when run inside a git repository and print a one-paragraph
warning when it is not. The command still succeeds; the warning is
just a nudge.

## Host build

`cargo plushie package-rust assemble` shells to `cargo build` with the
release profile, the selected binary target, and the `plushie/wire`
feature appended to whatever `--features` was passed in. The build
honours `CARGO`, `CARGO_TARGET_DIR`, and the rustup proxy in the usual
way.

The host build does not need a `[profile.release]` table to package
successfully, but a tuned profile is worth setting once and forgetting:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"
```

`strip = "symbols"` is the Rust equivalent of running `strip(1)` on
the binary. It removes debug symbols and shrinks the artifact
substantially. cargo-plushie does not re-strip after building; the
profile is the right place for that knob.

For multi-binary packages, pass `--bin <name>` to select which target
gets packaged. Without it, a package with multiple bins fails with a
specific error pointing at `--bin`.

Cross-compiling to a different target than the build host is not
supported yet. Build each target on a matching runner.

## Renderer selection

The renderer comes from `cargo plushie build` and is always custom-
bundled. This matches `cargo plushie run`'s discovery chain: the
binary lands under `target/plushie-renderer/target/release/`, and
`package-rust assemble` copies it from there into the payload.

To package against a different renderer source path (for example a
local plushie-rust checkout), set `PLUSHIE_RUST_SOURCE_PATH` to that
directory before invoking the command. The build step picks it up
through the same `[patch.crates-io]` mechanism documented under
[`cargo plushie build`](cli-commands.md#cargo-plushie-build).

## Bundled assets

A packaged app needs two kinds of files beyond the binary itself: the
icon and other OS-bundle metadata that cargo-plushie reads from the
manifest, and runtime assets that your app loads at startup (fonts,
images, data files). Each has a different home.

### App-loaded assets

Rust apps commonly embed runtime assets at compile time:

```rust
const INTER: &[u8] = include_bytes!("../assets/inter.ttf");
const ICON: &[u8] = include_bytes!("../assets/window-icon.png");
```

Compile-time embedding does not need a packaging step. The bytes are
already in the host binary, packaged or not.

For assets that need to live on disk at runtime (large data files,
files the app reads relative to its executable), resolve relative to
the host binary's directory:

```rust
let exe = std::env::current_exe()?;
let data_dir = exe.parent().expect("binary parent").join("data");
```

The launcher places the host binary under `bin/<app>` in the extracted
payload, so a sibling layout like `bin/<app>` plus `data/...` is the
expected shape. Put those files under [`package_assets/`](#package-
level-assets) so they end up in the payload root.

### Package-level assets

Files that need to live inside the payload at a known location go in a
`package_assets/` directory next to `plushie-package.config.toml`.
cargo-plushie copies the contents verbatim into the payload root
during assembly:

```
my-app/
├── Cargo.toml
├── plushie-package.config.toml
└── package_assets/
    ├── icon.png                # ends up at payload/icon.png
    └── fonts/
        └── extra.ttf           # ends up at payload/fonts/extra.ttf
```

The convention is zero-config: if `package_assets/` exists, it is
used. To use a different directory name, set `[assets].dir` in the
package config:

```toml
[assets]
dir = "branding"
```

Asset files overwrite SDK-generated payload files when names collide,
so a `package_assets/bin/plushie-renderer` would replace the bundled
renderer. Use this for overrides, not by accident; the default layout
has no overlap.

### Icon

`package-rust assemble` looks for an explicit `--icon PATH` first. The
named file is copied into the payload at `assets/<basename>` and
recorded under `[platform].icon`. If `--icon` is omitted, the bundled
default icon is written to `assets/default-app-icon-512.png` and
recorded there.

**Format:** PNG with RGBA alpha channel for transparency.

**Dimensions:** square aspect ratio, 512x512 minimum. cargo-packager
scales this single source down for `.ico` (16/32/48/64/128/256) and
up or down for `.icns` (16/32/64/128/256/512/1024). Provide 1024x1024
or larger when the same icon will be used for retina displays or
high-DPI Windows installers.

A custom icon kept under `package_assets/` and referenced through
`[platform].icon` works too. The shared assemble step is the canonical
path for that flow; for `package-rust assemble`, `--icon` is the
shorter spelling.

The manifest schema accepts a single icon path. Multi-size sources
and per-platform `.icns`/`.ico` overrides are not yet supported.

## The managed tool set

The shared portable and bundle steps rely on a small set of managed
tools installed locally:

| File | Role |
|---|---|
| `cargo-plushie` (on `PATH`) | The cargo subcommand. Orchestration entry point: owns `build`, `download`, `package-rust assemble`, and the shared `package` subcommands. Installed with `cargo install cargo-plushie`. |
| `bin/plushie` | The standalone Plushie CLI binary. Installed under the project's `bin/` by `cargo plushie tools sync`. |
| `bin/plushie-renderer` | The stock renderer cache. `package-rust assemble` ignores this in favor of the custom build, but `cargo plushie tools` still tracks it for parity with other SDKs. |
| `bin/plushie-launcher` | The shared launcher template that `package portable` embeds the payload into. |

For Rust projects working from a plushie-rust checkout, every entry
can be built straight from source with `cargo run -p cargo-plushie --
<sub>`. For projects depending on the published crates, install
cargo-plushie matching the SDK version:

```bash
cargo install cargo-plushie --version <X.Y.Z> --locked
cargo plushie tools sync
```

`cargo plushie tools sync` resolves the matching renderer, launcher,
and standalone `plushie` CLI versions from `cargo metadata` and
installs them under `bin/`. See
[`cargo plushie tools`](cli-commands.md#cargo-plushie-tools) for the
full flow.

The shared portable and bundle commands run a strict-tools check by
default: they verify that the launcher, renderer, and cargo-plushie
itself all match the SDK-pinned version. Pass `--lax-tools` to bypass
the check; this is for local experimentation, not release builds.

## The manifest

`package-rust assemble` writes a complete `plushie-package.toml`. A
typical manifest looks like:

```toml
schema_version = 1
app_id = "dev.example.notes"
app_name = "Notes"
app_version = "0.1.0"
target = "linux-x86_64"
host_sdk = "rust"
host_sdk_version = "0.7.1"
plushie_rust_version = "0.7.1"
protocol_version = 1

[start]
working_dir = "."
command = ["bin/notes"]
forward_env = ["PATH", "HOME", "LANG", "LC_ALL", "XDG_RUNTIME_DIR", "WAYLAND_DISPLAY", "DISPLAY"]

[renderer]
path = "bin/plushie-renderer"
kind = "custom"

[platform]
icon = "assets/default-app-icon-512.png"

[payload]
archive = "payload.tar.zst"
hash = "sha256:..."
size = 12345678
```

The schema is shared across every Plushie SDK. The Rust SDK fills
`host_sdk = "rust"`, sets both `host_sdk_version` and
`plushie_rust_version` to the depended-on `plushie` crate version,
and records `kind = "custom"` because the renderer is built from the
same workspace as the app's native widgets.

When the depended-on `plushie` version is not visible to `cargo
metadata` (for example a standalone app with no `plushie` dep yet),
both `host_sdk_version` and `plushie_rust_version` fall back to the
cargo-plushie crate version. The crates release together, so the
fallback still reflects the SDK the artifact was built against.

## Package config

Optional developer-owned defaults live in
`plushie-package.config.toml` next to the app's `Cargo.toml`. Generate
a template with:

```bash
cargo plushie package-rust assemble --write-package-config
```

The template lays out every supported field with sensible defaults:

```toml
config_version = 1

[start]
working_dir = "."
command = ["bin/my-app"]
forward_env = [
  "PATH",
  "HOME",
  "LANG",
  "LC_ALL",
  "XDG_RUNTIME_DIR",
  "WAYLAND_DISPLAY",
  "DISPLAY",
]

# [assets]
# dir = "package_assets"
```

`[start].working_dir` is relative to the extracted payload root.
`[start].command` is a structured argv; the first element is the
packaged host executable. The default value matches the binary name
cargo-plushie picks during assembly, so the default rarely needs
editing.

`[start].forward_env` is the list of environment variable **names**
copied from the parent process into the host process at launch time.
Names only; values are never logged or recorded. The defaults cover
the variables a typical Linux GUI app needs. Add more entries when
your app reads additional environment, for example `RUST_LOG` during
development.

The source config takes precedence over a `[start]` already present
in the partial manifest, so editing the config file changes what the
next `package-rust assemble` writes.

Use `--package-config PATH` to point at a config file outside the
default location.

## Forwarded environment

The package launcher does not blanket-inherit the user's environment.
It builds the host process environment from two closed sources:

- Launcher-owned variables: `PLUSHIE_BINARY_PATH` points at the
  payload-local renderer, `PLUSHIE_PACKAGE_DIR` points at the
  extracted payload root, `PLUSHIE_PACKAGE_READY_FILE` is used by the
  launcher to coordinate startup readiness, plus the small set of
  other internal coordination variables the launcher sets itself.
- The names listed in `[start].forward_env`.

Variables outside both sets are dropped. This matches the renderer
environment allowlist the SDK uses to bound its renderer subprocess,
and gives packaged apps a predictable, narrow runtime environment
regardless of where the launcher is invoked from.

`forward_env` cannot include the launcher-owned package variables;
the launcher rejects entries that would shadow them.

## Building artifacts

Once the manifest is complete, the same payload feeds two artifact
shapes. Both commands are shared across every Plushie SDK; the Rust
SDK does not own them.

### Portable single-file executable

```bash
cargo plushie package portable \
  --manifest target/plushie/rust-package/plushie-package.toml
```

Produces a self-extracting executable wrapping `plushie-launcher` and
the archived payload. Output lands under `target/plushie/package/` by
default; pass `--out PATH` to override. The artifact is content-
addressed by the payload hash, so two builds of the same inputs
produce a byte-identical executable.

The launcher extracts the payload to a per-user cache directory keyed
by the payload hash. Repeated runs of the same artifact reuse the
extraction. See [CLI commands](cli-commands.md#cargo-plushie-package-portable)
for the cache lifecycle.

### OS-native installers

```bash
cargo plushie package bundle --manifest <path> --format appimage
cargo plushie package bundle --manifest <path> --format dmg
cargo plushie package bundle --manifest <path> --format nsis
```

Delegates to [cargo-packager](https://github.com/crabnebula-dev/cargo-packager)
for AppImage (Linux), `.app` and `.dmg` (macOS), and `nsis` and
`wix` (Windows; these are cargo-packager format names, producing
`.exe` and `.msi` installers respectively). Format availability
depends on the runner: Apple formats need a macOS runner, Windows
formats need a Windows runner.

## Distribution

Artifacts are version-named and shipped with SHA-256 sidecars in the
same layout the SDK uses to fetch its own managed tools:

```
BASE/vVERSION/ARTIFACT
BASE/vVERSION/ARTIFACT.sha256
```

GitHub releases match this layout naturally. Other hosting works the
same way: any HTTPS endpoint that serves
`BASE/vVERSION/ARTIFACT` and `BASE/vVERSION/ARTIFACT.sha256` is
usable.

For local release verification, point `PLUSHIE_RELEASE_BASE_URL` at a
`file://` directory or a loopback HTTP server before assets are
uploaded. The download flow accepts both schemes alongside the
default HTTPS; remote mirrors must use HTTPS.

## Continuous integration

The following GitHub Actions workflow builds a portable artifact per
target on a `v*` tag push and uploads everything to a GitHub release
with SHA-256 sidecars. Drop it in at `.github/workflows/release.yml`
and edit the marked lines for your app:

```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write          # for uploading release assets

jobs:
  package:
    name: Package (${{ matrix.target }})
    runs-on: ${{ matrix.runner }}
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: linux-x86_64
            runner: ubuntu-latest
          - target: darwin-x86_64
            runner: macos-13
          - target: darwin-aarch64
            runner: macos-14
          - target: windows-x86_64
            runner: windows-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo registry and target
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: cargo-${{ matrix.target }}-${{ hashFiles('Cargo.lock') }}

      # EDIT: pin to the cargo-plushie / plushie version this app uses
      - name: Install cargo-plushie
        run: cargo install cargo-plushie --version 0.7.1 --locked

      - name: Sync managed tools
        run: cargo plushie tools sync

      - name: Assemble the Rust package payload
        # EDIT: set --app-id and --app-name to match your project
        run: |
          cargo plushie package-rust assemble \
            --app-id dev.example.notes \
            --app-name "Notes"

      - name: Build the portable artifact
        run: |
          cargo plushie package portable \
            --manifest target/plushie/rust-package/plushie-package.toml

      - name: Compute SHA-256 sidecar
        shell: bash
        run: |
          cd target/plushie/package
          for f in *; do
            if [ -f "$f" ] && [[ "$f" != *.sha256 ]]; then
              shasum -a 256 "$f" | awk '{print $1}' > "$f.sha256"
            fi
          done

      - name: Upload to release
        uses: softprops/action-gh-release@v2
        with:
          files: |
            target/plushie/package/*
          generate_release_notes: true
```

The workflow runs four parallel jobs, one per supported target. Each
installs the toolchain, fetches dependencies, installs cargo-plushie,
syncs the managed tools, assembles the payload, produces the portable
artifact, computes a SHA-256 sidecar, and uploads both files to the
release that the tag push creates.

Lines to tweak for your project:

- The matrix runner labels (`macos-13` for Intel macOS, `macos-14`
  for Apple Silicon). GitHub-hosted runner labels change over time;
  pin or update as needed. Add `ubuntu-24.04-arm` (or use a
  self-hosted runner) for Linux aarch64.
- The cargo-plushie version. Pin it to the version your app's
  `Cargo.toml` depends on; pre-1.0 the workspace ships every crate
  at the same version.
- The `cargo plushie package-rust assemble` arguments: `--app-id`,
  `--app-name`, and `--bin` if the package has multiple bins.
- Release notes: set `generate_release_notes` to `false` and add
  `body` (or `body_path`) when you write release notes by hand.

To also build OS-native installers, add a second matrix entry that
calls `cargo plushie package bundle --format <fmt>` instead of
`package portable`, and adjust the upload glob accordingly. Apple
formats need a macOS runner with valid signing identities; Windows
formats need a Windows runner with the appropriate SDKs.

For private hosting, replace the upload step with whatever pushes the
artifact and sidecar to your release endpoint. Any service that
exposes the assets at `BASE/vVERSION/ARTIFACT` plus
`BASE/vVERSION/ARTIFACT.sha256` works with the download flow.

## Signing

`plushie-package.toml` carries a `[[signing.hooks]]` block: a list of
commands that run after the artifact is built. Pass
`--run-signing-hooks` to `package portable` or `package bundle` to
invoke them. Hooks are opt-in so release builds run them and local
experimentation does not.

Each hook is a structured argv. Use them for macOS notarization,
Windows code signing, Linux checksum attestation, or whatever else
the target platform needs. Plushie does not hold signing keys; the
hook commands do.

## Updates

`plushie-package.toml` reserves an `[updates]` block for update
channel metadata. The schema is in place. The runtime side that
consumes it, planned around
[cargo-packager-updater](https://github.com/crabnebula-dev/cargo-packager),
is not yet shipped.

## Direct-mode apps

Direct-mode Rust apps build to a single native executable that
already embeds the renderer. They do not need the shared launcher
path. The release-profile binary itself is the artifact:

```bash
cargo build --release
```

Use platform packaging (cargo-deb, cargo-rpm, cargo-packager pointed
at the bare binary, or the platform's native tooling) for those
artifacts. The shared `[platform]`, `[updates]`, and `[[signing]]`
metadata names still apply when a platform package needs them, but
there is no payload archive, no extraction step, and no
`PLUSHIE_BINARY_PATH` indirection.

Switch to the shared launcher path when a direct-mode app needs:

- Payload files (images, fonts, migrations, generated data) that
  cannot be `include_bytes!`-embedded cleanly.
- A cache-managed payload lifecycle where a replaced binary must
  extract and run the matching embedded payload.
- The same manifest shape as wire-mode SDKs (cross-language fleet,
  shared signing and update infrastructure).

To switch, drop the `direct` feature, enable `wire`, and run
`package-rust assemble`. See
[Direct vs wire](direct-vs-wire.md#standalone-packaging) for the
trade-offs.

## Host-first versus renderer-parent

Packaging is host-first. The launcher starts the host binary and the
host spawns its own renderer.

A separate renderer-parent flow exists for development and embedding
hosts. The renderer starts first, binds a Unix socket, and spawns
the host command with `PLUSHIE_SOCKET` pointing at it:

```bash
plushie-renderer --listen \
  --exec-bin target/release/my-app
```

The renderer sets `PLUSHIE_SOCKET` (and `PLUSHIE_TOKEN`) in the
spawned child's environment, pointing at the bound socket. Extra
literal arguments can be appended with repeated `--exec-arg <value>`
pairs, but no placeholder substitution is performed: the renderer
does not expand a `{socket}` token, so the socket path is only
available via the env var, not as a CLI value.

`plushie::run` detects `PLUSHIE_SOCKET` (or an explicit
`--plushie-socket <path>`) and
connects to the existing renderer instead of spawning one. The same
entry point is what the packaged host binary calls when started by
the launcher, so driving a packaged app from an external renderer is
possible but requires adding `PLUSHIE_SOCKET` and `PLUSHIE_TOKEN` to
`[start].forward_env` so the launcher passes them through. This is
not a default-on configuration.

## See also

- [CLI commands](cli-commands.md)
- [Direct vs wire](direct-vs-wire.md)
- [Configuration](configuration.md)
- [Versioning](versioning.md)
- [Wire protocol](wire-protocol.md)
- [cargo-packager](https://github.com/crabnebula-dev/cargo-packager) -
  the bundle backend for OS-native installers
