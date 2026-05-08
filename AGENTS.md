# plushie-rust

Rust workspace for Plushie. Contains the Rust app SDK, the widget SDK
for custom widget authors, and the renderer binary. Six host SDKs
drive the renderer over stdin/stdout: Rust (in wire mode), Elixir,
Gleam, Python, Ruby, TypeScript.

## Stewardship

Direction, trust model, goals, and explicit non-goals are captured
in `docs/stewardship/`. That directory is the authority on what
work the project takes on and what it declines. The summary below
is enough for routine work; pull the relevant doc when an axis is
in play. Use `docs/stewardship/triage.md` as the routing tool when
the answer is not self-evident.

Pre-1.0: no backcompat, right design wins, rename across SDKs is
fine. Post-1.0: stability obligations begin (Hyrum's Law).
plushie-rust = protocol authority. plushie-elixir = canonical
API-shape reference. Cross-SDK parity audited in sibling
`plushie-sdk-parity/`. Wire change = six-SDK change.

### Disciplines (non-negotiable)

Tests through real renderer; cross-SDK claims verified by reading
source on each side; design before code at boundaries (wire,
public API, widget trait); clarity is the bar; no half-built
features; local cleanup not scope creep; no legacy shims pre-1.0.

### Goals

Wire protocol correctness; cross-SDK concept parity (semantics
converge, syntax diverges per language); lightweight by default;
panic isolation between widgets; fault tolerance across the wire
(renderer crash auto-recovers + state re-syncs, app exception
reverts, neither side takes the other down); structural host
protection on renderer-to-host channel.

### Non-goals (declined, not deprioritized)

Browser-grade isolation of arbitrary remote hosts; DoS-proofness
at any cost; backcompat before 1.0; coverage targets as a metric;
micro-optimization at cost of readability; refactoring without a
forcing function; per-Rust API ergonomics that diverge from
cross-SDK shape; API stability hardening pre-1.0 (single 1.0
sweep, not piecemeal); defending against speculative deployment
shapes (server-side WASM, multi-tenant hosting,
browser-as-arbitrary-host, etc.).

### Trust model

Asymmetric. Renderer-to-host = closed and typed (fixed enum of
event variants + structured responses; no opaque blobs, no eval,
no "run on host"). Host structurally protected today; the remote-
rendering use case relies on it. Host-to-renderer = broad by
design (file paths, fonts/images/SVG, screenshots, effects,
`--exec`); bounding it is the capability/manifest roadmap. Wire =
byte-stream agnostic; confidentiality + integrity delegated to
the outer transport. Same-access (user attacking themselves) is
out of scope.

### Resilience

Things-go-wrong axis, not adversary axis. Panic isolation per
widget; session isolation per thread; defensive parsing at
boundaries (reject + structured error, never crash); hard caps as
correctness bounds (`MAX_TREE_DEPTH=256`, `MAX_LOADED_FONTS=256`,
`MAX_FONT_BYTES=16MiB`, `MAX_IMAGES=4096`, 64MiB wire cap); clean
exit on broken transport. Fail-fast on programming-error invariant
violations, unrecoverable init, memory-corruption indicators.
Degrade gracefully on user-facing input.

### Performance

Lightweight = baseline, not optimization-after-fact. Don't do
unnecessary work in the first place; cost compounds. Worth doing
without benchmark (readability preserved/improved): consolidate
redundant work, right data structure, avoid clear unnecessary
alloc, localized cleanup-as-optimization. Need benchmark first
(readability cost real): clever encoding, big-O without realistic
N, optimization on idle paths. Numeric direction: 16.67ms frame
budget at a few hundred to ~1000 nodes; sub-ms wire round-trip on
local pipe; startup <2s; resident <200MiB; idle CPU = no
measurable work.

### Test discipline

Integration spine: tests exercise real renderer (default backend
in every SDK runs actual binary). Pure-language mock that diverges
from real binary = worse than no test. Three modes (cross-SDK
contract): mock (default, protocol-only), headless (tiny-skia),
windowed (full iced + display). Mocking acceptable only for forced
renderer crash sim, malformed wire bytes the codec rejects
pre-typed, test infra. Direct + wire dual coverage when paths
could meaningfully diverge. Tests as documentation; slow tests =
slow code; failing test before fix.

### Simplicity

Clarity = constraint, not aspiration. Reader-cost compounds.
Readability wins ties. Abstraction earns its place: 3 similar
lines > premature abstraction; 3rd use earns consideration not
commitment; single-user abstraction = costume; "we might need this
someday" = reason not to extract. Local complexity > global.
Cohesion across file > brevity of any one file. Functional flavor
(concept-level; Rust idiom on syntax): pure where possible,
immutable preferred, sum types over flag-state-machines, errors-
as-values, composition over inheritance. Comments answer
why-not-what.

### Common shapes -> outcomes

- "mock the renderer for speed" -> decline
- "add `#[non_exhaustive]` / API hardening" -> decline; 1.0 sweep
- "this is O(n) on a hot path" -> need realistic N
- "split this large module" -> need forcing function
- "harden against malicious host" -> defer to capability-manifest
- "wire should encrypt / sign" -> outer transport's job
- "consolidate N redundant traversals" -> do
- "extract this single-use helper" -> decline; costume
- "this panic should be a structured error" -> usually do
- "this graceful path should panic on bad input" -> usually no
- "rename field across SDKs" -> route through parity workflow

## Before committing

Run `just preflight`. It mirrors CI exactly: check, clippy, fmt, test.

## Commit hygiene

Every commit should be self-contained and functional. Preflight
should pass at each commit, not just at the tip.

Commits after `origin/main` are unpublished and can be freely
amended, squashed, or reordered to keep the history clean. Run
`git fetch origin` first to ensure the boundary is current. Use
`--amend` to fold small fixes into the commit they belong to
rather than creating "fix the fix" commits. If a later commit
fixes a bug introduced by an earlier unpublished commit, squash
them together.

Never amend or rebase commits that are already on `origin/main`.

## Commit messages

Commit messages should describe what changed and why. Do not include:
- Counts of any kind (findings, files, tests, items). If the
  content is listed, the reader can count. Counts add noise.
- Ticket, review, or tracking IDs (R-001, PROJ-123, etc.)
- References to this file

More broadly, think carefully before including counts anywhere
(code comments, docs, log messages). If the count is derivable
from the surrounding content, it doesn't add value.

## Writing style

Do not use `--` (double dash) as a separator or em-dash substitute
in prose, docs, comments, or bullet lists. Use a single `-` for
list item separators and reword sentences to avoid inline dashes
(use commas, periods, colons, or parentheses instead). `--` should
only appear as part of CLI flag names (e.g. `--watch`, `--release`).

## Releasing

1. Update `CHANGELOG.md`: rename `[Unreleased]` to `[x.y.z] - YYYY-MM-DD`
   (or add a new section if there's no Unreleased). List breaking changes
   first if it's a minor/major bump.
2. Bump the workspace version in the root `Cargo.toml` (`[workspace.package].version`).
3. Bump the internal path-dep versions in the root `Cargo.toml`
   `[workspace.dependencies]` table (the `plushie-core`,
   `plushie-core-macros`, `plushie-widget-sdk`, `plushie-renderer-engine`,
   `plushie-renderer-lib`, `plushie-renderer`, `plushie-renderer-wasm`
   entries). Per-crate manifests already use `.workspace = true`, so
   no per-crate edits are required.
4. Run `just preflight` to verify.
5. Commit as `release: prepare x.y.z`.
6. Tag and push (handled manually, not by the agent).

### Windows / gpu-allocator pin

`Cargo.lock` pins `gpu-allocator` against `windows` 0.62 so `wgpu-hal`
resolves cleanly on Windows builds. `cargo update` will silently drop
this pin if the transitive graph allows a newer `windows` version.
Before releasing, confirm the lockfile still resolves `windows 0.62`
for `gpu-allocator` and that the Windows cross-compile check in CI
passes. If the pin is lost, re-apply via `cargo update -p windows --precise 0.62.x`
and commit the lockfile change.

## Quick reference

```
just preflight                     # run all CI checks locally
just check                         # fast compile check
just clippy                        # lint (same flags as CI)
just test                          # run tests (nextest, CI profile)
just test-cargo                    # run tests (plain cargo test)
just test-filter <pattern>         # run matching tests (nextest)
just test-crate <crate>            # run tests for one crate (nextest)
just build                         # debug build (workspace)
just build-release                 # optimized release build (workspace)
# Shipping artifacts go through the `dist` profile rather than
# `release`: `cargo build --profile dist` or
# `cargo build --profile dist -p <crate>`. `dist` inherits from
# `release` and adds `lto = true`, `codegen-units = 1`, `strip`,
# and `opt-level = 3` (wasm overrides to `"z"` for size). Local
# `cargo run --release` and `cargo test --release` stay fast.
just format                        # auto-format
just fmt                           # check formatting (CI mode)
just watch-check                   # cargo watch: check on save
just watch-test                    # cargo watch: test on save
just docs                          # build and open rustdocs
just audit                         # cargo audit
just outdated                      # cargo outdated
just coverage                      # code coverage (llvm-cov or tarpaulin)
# Environment variables (not commands):
RUST_LOG=plushie_renderer=debug       # verbose binary logging
RUST_LOG=plushie_widget_sdk=debug     # verbose widget SDK logging
```

Nextest config: `.config/nextest.toml` (slow-timeout, CI profile).

## Crate map

All crates live under `crates/`:

| Crate | Type | Audience | Purpose |
|-------|------|----------|---------|
| `plushie` | lib+bin | App developers | Rust SDK: Elm architecture, direct + wire + connect rendering modes |
| `plushie-core` | lib | Internal | Core types, wire protocol, Selector, Key/KeyPress, no iced dependency |
| `plushie-core-macros` | proc-macro | Internal | Derive macros for types and widgets |
| `plushie-widget-sdk` | lib | Widget authors | `PlushieWidget` trait, canvas engine, built-in widget impls |
| `plushie-renderer-engine` | lib | Internal | Pure state engine (`Core`), retained tree, wire codec |
| `plushie-renderer-lib` | lib | Internal | Shared renderer logic (native + wasm32), `EffectHandler` trait |
| `plushie-renderer` | bin | All SDKs | Native renderer binary: iced daemon, stdin/stdout I/O |
| `plushie-renderer-wasm` | cdylib | Web | WASM entry point via wasm-bindgen |
| `cargo-plushie` | bin | App developers | Cargo subcommand: build and download renderer binaries |

Dependency flow:

```
plushie-core-macros
        |
   plushie-core
     /        \
plushie    plushie-widget-sdk
     \        |
      \  plushie-renderer-engine
       \      |
   plushie-renderer-lib
     /        \
plushie-renderer  plushie-renderer-wasm
```

`plushie` (the Rust SDK) and `plushie-widget-sdk` are the two public
crates. Everything else is internal or a binary entry point. See
`crates/plushie/CLAUDE.md` and `crates/plushie-widget-sdk/CLAUDE.md`
for per-crate details.

## Architecture

- **stdin/stdout protocol.** The renderer binary reads messages from
  stdin (MessagePack by default, JSONL available) and writes events to
  stdout. All log output goes to stderr.
- **iced::daemon.** The renderer runs as an `iced::daemon` application,
  which supports multiple windows without forcing a default window and
  keeps running when all windows are closed (important for the
  stdin-driven model).
- **Tree rendering.** A retained tree of UI nodes is maintained in
  `Core` (`engine.rs`). Snapshots replace the full tree; patches
  update it incrementally. The widget mapper in `widget/` walks the
  tree and maps each node type to an iced widget.
- **Multi-window.** The host drives window open/close via tree nodes.
  The renderer maintains bidirectional `window_id <-> window::Id` maps.
- **Three modes.** Windowed (default, full iced rendering), `--headless`
  (real rendering via tiny-skia, no display server), `--mock`
  (protocol-only, no rendering, stub screenshots). The hello message
  reports the mode so SDKs can adapt.
- **Session multiplexing.** Headless and mock modes support concurrent
  sessions via `--max-sessions N`. Each session runs in its own thread
  with isolated state. Messages are dispatched by the `session` field.
- **Rust SDK modes.** The `plushie` crate supports three modes: direct
  (in-process iced rendering, default), wire (subprocess renderer via
  stdin/stdout), and connect (attach to a pre-existing Unix socket).
  Same App API for all three.

## Non-obvious patterns

**prepare / render split.** Stateful widgets (text_editor, markdown)
require mutable state that persists across renders, but iced's
`view()` only has `&self`. `PlushieWidget::prepare()` runs during
`apply()` (mutable context) to populate factory-owned state.
`render()` in `view()` reads it immutably. No `RefCell` needed.

**Canvas layer caching.** Canvas widgets use per-layer `canvas::Cache`
for efficient re-rendering. `prepare()` hashes each layer's shape
JSON; only changed layers clear the cache. Layers with active
hover/pressed interaction bypass the cache for style overrides.

**pending_tasks drain.** Widget ops return iced `Task`s, but `apply()`
doesn't return them. They're pushed to `Vec<Task<Message>>` on the
App and drained via `Task::batch` after each `update()` call.

**rmp-serde workaround.** rmp-serde can't deserialize internally-tagged
enums from external msgpack producers. `Codec::decode` routes through
`rmpv::Value` then `serde_json::Value` as intermediates. See `codec.rs`.

**Custom themes.** JSON objects with hex color fields parsed into
`iced::Theme::custom()` with a `Palette`. Optional `base` field
selects a starting palette to override.

**Canvas interactive shapes.** Shapes with an `interactive` field get
renderer-local hit testing, hover/pressed styles, keyboard navigation,
drag, tooltips, and accessibility with zero round-trip. Groups are the
primary composability mechanism. The `interactive` field means "this
is a semantic element" carrying both interaction config and a11y.

**Overlay flip and align.** The overlay widget's `flip` prop auto-flips
when content would overflow the viewport. `align` controls cross-axis
alignment (start/center/end).

**EffectHandler trait.** Platform effects are abstracted behind
`EffectHandler` in plushie-renderer-lib. Native: `NativeEffectHandler`
(rfd, arboard, notify-rust). WASM: `WebEffectHandler` (stubs).

**Platform sleep.** `emitter.rs` cfg-gates between `tokio::time::sleep`
(native) and `wasmtimer::tokio::sleep` (wasm32).

**A11y auto-inference.** Image/SVG `alt` props flow into accessible
labels. Text input/editor `placeholder` props flow into accessible
descriptions. Explicit a11y props override inferred values.

**StyleMap preset base.** StyleMap objects can include a `"base"` field
naming a preset to extend, letting hosts override individual properties
without restating everything.

**Prop validation mode.** Debug builds always validate; release builds
opt in via `validate_props: true` in Settings. `OnceLock<bool>`, set
once per process.

## No feature flags (renderer crates)

The renderer crates compile all capabilities unconditionally. No Cargo
feature flags. Headless (`--headless`) and mock (`--mock`) are runtime
flags.

The `plushie` SDK crate does have feature flags: `direct` (default,
in-process rendering) and `wire` (subprocess rendering).

## Protocol version handshake

The renderer reads a Settings message from stdin on startup, then emits
a `hello` message on stdout confirming the protocol version and wire
codec. See `docs/reference/wire-protocol.md` for the full specification.

## iced fork (plushie-iced)

The renderer depends on a fork of iced. Cargo.toml dependencies use
`package = "plushie-iced"` aliases so source code still writes
`use iced::*`. The fork source lives in the `plushie-iced` sibling repo.

Local development: a gitignored `.cargo/config.toml` (in this repo
root) with a `[patch.crates-io]` table redirects every `plushie-iced*`
crate the workspace resolves to the sibling checkout. CI and `cargo
publish` resolve from crates.io because the config file is not checked
in. To work against an in-flight plushie-iced branch, clone it
alongside plushie-rust and write the config file with one patch entry
per resolved `plushie-iced-*` crate (see the committed example in this
repo's history for the full list).

```
# .cargo/config.toml (gitignored)
[patch.crates-io]
plushie-iced      = { path = "../plushie-iced" }
plushie-iced-core = { path = "../plushie-iced/core" }
# one entry per plushie-iced-* crate in Cargo.lock
```

`[patch.crates-io]` replaces the older `paths = [...]` form, which
cargo warns about (and will eventually reject) whenever the sibling
checkout's transitive dep graph differs from the registry version.

To go back to the crates.io version, delete the file. Cargo reruns
dep resolution automatically on the next build.

## Related repositories

These are expected as sibling directories (e.g. `../plushie-elixir/`):

- plushie-elixir - Elixir SDK (reference implementation)
- plushie-gleam - Gleam SDK
- plushie-python - Python SDK
- plushie-typescript - TypeScript SDK
- plushie-ruby - Ruby SDK
- plushie-iced - vendored iced fork
- plushie-demos - demo apps for all SDKs
