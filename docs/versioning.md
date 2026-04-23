# Versioning policy

## Overview

plushie-rust ships every crate in the workspace at one workspace
version. One release equals one version equals one coherent set of
compatible crates. The crates covered by this single version are:

- `plushie`
- `plushie-core`
- `plushie-core-macros`
- `plushie-widget-sdk`
- `plushie-renderer-lib`
- `plushie-renderer`
- `plushie-renderer-wasm`
- `cargo-plushie`

Every release bumps `[workspace.package].version` in the root
`Cargo.toml` once, and every crate picks up the new number through
`version.workspace = true`. There is no scenario where a partial
release ships a subset of crates at a different version.

Pre-1.0, breaking changes may land in any minor bump (`0.X.0`).
Patch releases (`0.X.Y`) stay backwards-compatible. Every release
notes explicit breakages under a "Breaking changes" heading in
`CHANGELOG.md`.

## PLUSHIE_RUST_VERSION

`PLUSHIE_RUST_VERSION` is the single number that identifies a
plushie-rust release. Host SDKs pin to this version for three
purposes:

- downloading the matching prebuilt `plushie-renderer` binary,
- installing the matching `cargo-plushie` on the developer machine,
- emitting matching plushie-crate version strings into the
  generated Cargo.toml.

A host SDK that declares `PLUSHIE_RUST_VERSION = "0.6.1"` is
promising that every plushie-rust artifact it touches (the renderer
binary, the build tool, the generated dependencies) comes from the
`0.6.1` release.

## SDK version vs plushie-rust version

The host SDKs (plushie-elixir, plushie-gleam, plushie-python,
plushie-ruby, plushie-typescript) each have their own independent
semver. They track `PLUSHIE_RUST_VERSION` separately, in whatever
form their ecosystem expects (an Elixir module attribute, a Python
package constant, etc.).

Two axes, two bumps:

- SDK-only fixes (dialyzer fixes, host-language refactors, docs,
  new high-level APIs built on top of the existing protocol) bump
  the SDK version only. `PLUSHIE_RUST_VERSION` stays the same.
- plushie-rust upgrades (new renderer widgets, protocol additions,
  renderer bug fixes) bump `PLUSHIE_RUST_VERSION` inside the SDK,
  and typically bump the SDK version as well.

## Compatibility rule

An SDK release's `PLUSHIE_RUST_VERSION` must exactly match the
plushie-rust release it targets. No semver range, no
`~> 0.6`-style fuzzy pin. We bump the exact version.

Rationale: the renderer binary, the generated Cargo.toml deps, and
the protocol messages travel together. A single mismatched version
puts an SDK out of sync with itself. Forcing exact-match removes
that entire class of "mostly works, except for the one message that
changed" bugs.

## Drift example

A concrete timeline showing how the two axes evolve independently:

1. plushie-rust `0.6.1` releases.
2. plushie-elixir `0.6.3` ships with `PLUSHIE_RUST_VERSION = "0.6.1"`.
3. plushie-elixir `0.6.4` ships a dialyzer fix. Still
   `PLUSHIE_RUST_VERSION = "0.6.1"` - no renderer change.
4. plushie-rust `0.6.2` releases (renderer bug fix).
5. plushie-elixir `0.6.5` bumps to `PLUSHIE_RUST_VERSION = "0.6.2"`
   to pick up that renderer fix.

The SDK number kept moving between steps 2 and 3 without touching
the plushie-rust pin. Step 5 is the moment the SDK opts in to the
newer plushie-rust.

## Wire protocol versioning

The wire protocol has its own version (`protocol_version` in hello
and Settings) that is independent of `PLUSHIE_RUST_VERSION`. Hello
currently also emits `protocol` as a legacy alias for host SDKs that
still read the old field.
The rules:

- Intra-minor renderer releases (e.g. `0.6.1` to `0.6.2`) MUST NOT
  break protocol compatibility. An older SDK speaking an older
  protocol version must keep working against a newer patch
  renderer.
- Minor or major bumps (e.g. `0.6.x` to `0.7.0`) MAY bump the
  protocol version. When they do, the CHANGELOG breaking-changes
  section describes the break and its migration.

The handshake lets an SDK detect a mismatch early and fail with a
clear error rather than producing corrupt behaviour downstream.
