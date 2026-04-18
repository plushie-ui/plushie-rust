# plushie-rust

[![CI](https://github.com/plushie-ui/plushie-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/plushie-ui/plushie-rust/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/plushie.svg)](https://crates.io/crates/plushie)
[![docs.rs](https://docs.rs/plushie/badge.svg)](https://docs.rs/plushie)
[![MSRV](https://img.shields.io/badge/MSRV-1.92-blue.svg)](rust-toolchain.toml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Rust workspace for [Plushie](https://github.com/plushie-ui). **Pre-1.0**

**Versioning policy.** Pre-1.0, breaking changes may land in any minor
bump (`0.X.0`). Patch releases (`0.X.Y`) stay backwards-compatible.
Every release notes explicit breakages under a "Breaking changes"
heading in `CHANGELOG.md`.

Build native desktop apps in Rust with the Elm architecture, or use
the standalone renderer binary to power GUI frameworks in any language.
Rendering is handled by [iced](https://github.com/iced-rs/iced).

SDKs are available for
[Rust](crates/plushie/),
[Elixir](https://github.com/plushie-ui/plushie-elixir),
[Gleam](https://github.com/plushie-ui/plushie-gleam),
[Python](https://github.com/plushie-ui/plushie-python),
[Ruby](https://github.com/plushie-ui/plushie-ruby), and
[TypeScript](https://github.com/plushie-ui/plushie-typescript).

## How it works

The renderer is a standalone binary driven by a simple wire protocol
over stdin/stdout. Send it a tree of UI nodes as MessagePack or JSON,
get native desktop windows. Send updates, get events back.

```
  Your app (any language)
       |          ^
       | stdin    | stdout
       | trees    | events
       v          |
  plushie-renderer (Rust binary)
       |
  Native windows via iced
       |
  Desktop (Linux, macOS, Windows)
```

The Rust SDK can also run the renderer in-process (no subprocess),
sharing the same API for both modes.

### Direct vs wire in the Rust SDK

`plushie::run::<App>()` is feature-agnostic. Which runner actually
fires depends on the feature flags enabled at build time:

- **Built-in widgets only**: any mode. The default (`direct`) runs
  in-process with no setup. `--features wire --no-default-features`
  auto-discovers the stock `plushie-renderer` binary via
  `PLUSHIE_BINARY_PATH` then `PATH`.
- **Custom `PlushieWidget` impls**: build a custom renderer crate that
  registers your widgets, install that binary instead, and point the
  SDK at it with `PLUSHIE_BINARY_PATH=/path/to/my-renderer` or call
  `plushie::run_with_renderer(path)` directly.
- **WebAssembly**: direct mode runs in-browser today. A Web Worker +
  postMessage transport for WASM wire mode is tracked in the backlog.

The [protocol reference](docs/protocol.md) documents the full wire
format, message types, and startup handshake.

## Crates

All crates live under `crates/`:

| Crate | Description |
|-------|-------------|
| [plushie](crates/plushie/) | Rust SDK for building desktop apps |
| [plushie-widget-sdk](crates/plushie-widget-sdk/) | Widget SDK for custom native widgets |
| [plushie-renderer](crates/plushie-renderer/) | Standalone renderer binary |
| [plushie-core](crates/plushie-core/) | Core types and wire protocol (no iced) |
| [plushie-core-macros](crates/plushie-core-macros/) | Derive macros for types and widgets |
| [plushie-renderer-lib](crates/plushie-renderer-lib/) | Shared renderer logic (native + wasm32) |
| [plushie-renderer-wasm](crates/plushie-renderer-wasm/) | WASM entry point via wasm-bindgen |

## Features

- **Built-in widgets** - layout, input, display, and interactive
  widgets out of the box
- **Canvas** - shapes, paths, gradients, transforms, and interactive
  elements for custom 2D drawing
- **Themes** - dark, light, nord, catppuccin, tokyo night, and more,
  with custom palettes and per-widget style overrides
- **Animation** - renderer-side transitions, springs, and sequences
  with no wire traffic per frame
- **Multi-window** - declare windows in the tree; the renderer manages
  open, close, and per-window theming automatically
- **Platform effects** - native file dialogs, clipboard, OS
  notifications
- **Accessibility** - keyboard navigation, screen readers, and focus
  management via [AccessKit](https://accesskit.dev). Platform
  requirements:
  - **Linux**: `at-spi2-core` package plus a running D-Bus session.
    Orca or another AT-SPI2 client handles the actual announcements.
  - **macOS**: first run prompts the user for accessibility
    permissions; VoiceOver picks up the app after that.
  - **Windows**: UIA is built into the OS; no extra setup.
- **Custom widgets** - implement `PlushieWidget` in Rust for full
  control over rendering, state, and event handling
- **Three modes** - windowed (default), headless (tiny-skia, no
  display), mock (protocol-only, fast testing)
- **Session multiplexing** - concurrent test sessions over a single
  renderer process

## Getting started

### Prerequisites

**Linux (Debian/Ubuntu):**

    sudo apt-get install build-essential pkg-config cmake \
      libxkbcommon-dev libwayland-dev libx11-dev \
      libfontconfig1-dev libfreetype-dev

**Linux (Arch):**

    sudo pacman -S base-devel pkgconf cmake \
      libxkbcommon wayland libx11 fontconfig freetype2

**macOS:**

    xcode-select --install

**Windows:** No additional dependencies.

### Build and test

    cargo build
    cargo test

### Run the Rust SDK examples

    cargo run -p plushie --example counter
    cargo run -p plushie --example todo

### Run the renderer manually

    echo '{"type":"settings","settings":{}}' | cargo run -p plushie-renderer -- --json

## Development

Install [just](https://just.systems) and
[cargo-nextest](https://nexte.st), then:

    just preflight      # all CI checks (check, clippy, fmt, test)
    just check          # fast compile check
    just test           # run tests
    just build-release  # optimized release build

See `just --list` for all available recipes.

## Status

Pre-1.0. The protocol and widget API are functional but not yet
stable. The wire protocol includes a version handshake so host
libraries can detect incompatibilities.

## Documentation

- [Protocol reference](docs/protocol.md) - wire format, message
  types, encoding, startup handshake
- [Widget development](docs/widget-development.md) - building custom
  widgets
- [Custom widgets](docs/custom-widgets.md) - native widget crates and
  the `PlushieWidget` trait
- [Core widget guide](docs/core-widget-guide.md) - reusable iced
  widgets that work across every SDK
- [Build tool](docs/build-tool.md) - `cargo plushie` reference,
  binary discovery, and the dev loop
- [WASM build](docs/wasm-build.md) - building for WebAssembly

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
