# plushie-rust - Development Tasks
#
# Run `just` to see available recipes.
# Run `just preflight` before pushing to catch CI failures locally.

set shell := ["bash", "-euo", "pipefail", "-c"]

export RUSTFLAGS := "-D warnings"

default:
    @just --list

# === CI Preflight ===

preflight: check check-release clippy fmt test test-examples test-wire test-wire-dev test-feature-combos doc
    @echo ""
    @echo "All preflight checks passed!"

# === Individual Checks ===

check:
    cargo check --workspace --all-targets
    cargo check -p plushie --all-targets --features wire
    # Feature-permutation spot checks: catch regressions in the
    # wire-only, direct-only, and no-feature builds before CI does.
    cargo check -p plushie --no-default-features --features direct --all-targets
    cargo check -p plushie --no-default-features --features wire --all-targets
    cargo check --workspace --no-default-features --all-targets

check-release:
    cargo check --workspace --release

clippy:
    cargo clippy --workspace --all-targets
    cargo clippy -p plushie --all-targets --features wire

doc:
    cargo doc --workspace --no-deps

fmt:
    cargo fmt --check

test:
    cargo nextest run --workspace --profile ci

test-examples:
    cargo test -p plushie --examples

test-wire:
    cargo test -p plushie --features wire --test wire_mode
    cargo test -p plushie --features wire --test wire_connect
    cargo test -p plushie --features wire --test wire_image_ops
    cargo test -p plushie --features wire --test automation_replay_windowed

# Hot-reload integration test exercises the wire+dev feature combo
# (control-signal-driven renderer swap). Runs against the
# plushie-renderer binary built earlier in the preflight chain.
test-wire-dev:
    cargo nextest run -p plushie --features "wire,dev" --test wire_hot_reload --profile ci

test-feature-combos:
    cargo nextest run -p plushie --no-default-features --test no_runner_features --profile ci

test-cargo:
    cargo test --workspace

# === Build Variants ===

build:
    cargo build --workspace

build-release:
    cargo build --release --workspace

coverage:
    #!/usr/bin/env bash
    if command -v cargo-llvm-cov &>/dev/null; then
        cargo llvm-cov --workspace --html
    elif command -v cargo-tarpaulin &>/dev/null; then
        cargo tarpaulin --workspace --out html
    else
        echo "Install cargo-llvm-cov or cargo-tarpaulin for coverage." >&2
        exit 1
    fi

# === Development Helpers ===

format:
    cargo fmt

test-filter pattern:
    cargo nextest run --workspace -- {{pattern}}

test-crate crate:
    cargo nextest run -p {{crate}}

clean:
    git clean -fdX

docs:
    cargo doc --workspace --open

# === Watch Mode ===

watch-check:
    cargo watch -x 'check --workspace --all-targets'

watch-test:
    cargo watch -x 'nextest run --workspace'

# === Dependency Health ===

audit:
    cargo audit

outdated:
    cargo outdated --workspace
