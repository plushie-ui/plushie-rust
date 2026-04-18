# Release guide

This document covers the mechanics of shipping a new plushie-rust
release: what to check, in what order to publish, and how to recover
from a mid-release failure. For the conceptual versioning policy
(what `PLUSHIE_RUST_VERSION` means, how it relates to host SDK
versions, and the protocol-compatibility rule) see
[versioning.md](versioning.md).

## Pre-release checklist

1. `git fetch origin && git status` - working tree clean, branch in
   sync with `origin/main`.
2. `just preflight` - all checks pass.
3. `Cargo.lock` resolves `windows 0.62` for `gpu-allocator` (see the
   Windows pin notes in [../CONTRIBUTING.md](../CONTRIBUTING.md)).
4. `CHANGELOG.md` has an `[Unreleased]` section with the changes for
   this release.

## Preparing the release commit

1. Update `CHANGELOG.md`: rename `[Unreleased]` to
   `[x.y.z] - YYYY-MM-DD`. List breaking changes first if it is a
   minor or major bump.
2. Bump `[workspace.package].version` in the root `Cargo.toml`.
3. Bump the internal path-dep versions in `[workspace.dependencies]`
   (the `plushie-core`, `plushie-core-macros`, `plushie-widget-sdk`,
   `plushie-renderer-lib`, `plushie-renderer`, `plushie-renderer-wasm`
   entries). Per-crate manifests use `.workspace = true`, so no
   per-crate edits are required.
4. `just preflight` again.
5. Commit as `release: prepare x.y.z`.
6. Tag `vx.y.z` and push (handled manually, not by an agent).

## Publish order

Every crate in the workspace is intended to publish to crates.io
under one shared version. Publish in dependency order so each crate
can resolve its dependencies from the registry:

1. `plushie-core-macros` - proc-macro crate, no internal deps.
2. `plushie-core` - depends on `plushie-core-macros`.
3. `plushie-widget-sdk` - depends on core, core-macros.
4. `plushie-renderer-lib` - depends on core, widget-sdk.
5. `plushie-renderer` - depends on core, widget-sdk, renderer-lib.
6. `plushie-renderer-wasm` - depends on core, widget-sdk,
   renderer-lib.
7. `plushie` - depends on core, core-macros, widget-sdk (optional),
   renderer-lib (optional).
8. `cargo-plushie` - depends on core only. At runtime it invokes
   `cargo build` inside a generated workspace; it does not compile
   any plushie-rust crate into its own binary. Safe to publish last.

After each `cargo publish`, wait for crates.io to index the new
version before publishing the next crate. A short `sleep 30` between
steps is usually enough; if the next `cargo publish` reports
`no matching package named ...`, wait longer and retry.

## Dry-run before publishing

Run `cargo publish -p <crate> --dry-run --locked` for each crate in
the publish order. Dry-runs for dependent crates will fail until
their dependency version is actually on crates.io, which is expected
and not a blocker - the real `cargo publish` on those crates will
succeed once earlier steps in the order complete.

If a dry-run fails for any reason besides the "version not on
crates.io" case above (missing README, invalid license field,
path-only dependency on a non-published crate, etc.), fix the crate
and commit before starting the real publish.

## Recovering from a partial publish

`cargo publish` is idempotent per version: once a version is
published, it cannot be re-uploaded. If publishing step 4 succeeds
and step 5 fails, fix the cause and resume from step 5 without
touching the earlier crates. Do not bump the workspace version to
paper over a broken crate - yank the broken version from crates.io
instead (`cargo yank --version x.y.z -p <crate>`) and publish a
patch release with the fix.
