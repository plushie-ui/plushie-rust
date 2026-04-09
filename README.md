# plushie-renderer

A standalone native GUI renderer driven by a simple wire protocol over
stdin/stdout. Send it a tree of UI nodes as MessagePack or JSON, get
native desktop windows. Send updates, get events back.

Built for the [Plushie Elixir toolkit](https://github.com/plushie-ui/plushie-elixir),
but the renderer doesn't know or care what language is on the other
end. Any language that can spawn a process and write bytes to its stdin
can use it.

## Why

Native desktop GUIs typically require writing in a specific language --
Swift for macOS, C# for Windows, C++ for Qt or GTK. Languages without
mature GUI bindings are left choosing between Electron (ships an entire
browser), web view wrappers, or complex FFI.

This project separates rendering from application logic. The renderer
is a standalone binary that speaks a simple protocol. Your application
handles state and events; the renderer handles pixels and platform
integration. They talk over stdio.

## How it works

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

The [protocol](docs/protocol.md) has two directions:

**In** (your app -> renderer): UI tree snapshots, incremental patches,
window operations, widget commands, subscription management, platform
effect requests (file dialogs, clipboard, notifications).

**Out** (renderer -> your app): User interaction events (clicks, input,
key presses, scroll, touch), effect responses, window lifecycle events.

A UI tree is a nested structure of nodes. Each node has an id, a type,
props, and children. Here's a minimal window with a label and a button,
as JSON:

```json
{
  "id": "main",
  "type": "window",
  "props": { "title": "Counter" },
  "children": [
    {
      "id": "col",
      "type": "column",
      "props": { "padding": 20, "spacing": 10 },
      "children": [
        {
          "id": "label",
          "type": "text",
          "props": { "content": "Count: 0" },
          "children": []
        },
        {
          "id": "inc",
          "type": "button",
          "props": { "label": "+" },
          "children": []
        }
      ]
    }
  ]
}
```

When a user clicks the button, the renderer sends back:

```json
{ "type": "event", "family": "click", "id": "inc" }
```

Your app handles the event, rebuilds the tree, and sends it again.
There are two ways to update the UI:

- **Snapshot** -- Send the full tree every time. No diffing, no
  bookkeeping. Build your tree, serialize it, send it. This is the
  simplest approach and works well for small-to-medium UIs (forms,
  dashboards, tools, dialogs). A minimal client can be built in an
  afternoon using nothing but snapshots.

- **Patch** -- Diff the new tree against the previous one and send
  only the changes (prop updates, insertions, removals). More
  efficient for large trees or high-frequency updates. Requires the
  client to implement tree diffing -- the
  [Plushie](https://github.com/plushie-ui/plushie-elixir) toolkit does
  this, for example.

Start with snapshots. Add patching later if you need the performance.

### Wire format

The renderer supports two encodings:

- **JSON** -- One JSON object per line. Works everywhere, human-readable,
  no extra libraries needed. Performant for most use cases.

- **MessagePack** -- Binary encoding with a 4-byte big-endian length
  prefix. Better when sending binary data (images, canvas pixel
  buffers) or when serialization overhead matters at scale.

The format is auto-detected from the first byte of stdin, or can be
forced with `--json` or `--msgpack`.

## Capabilities

**30+ built-in widget types** covering layout (column, row, container,
stack, grid, scrollable, pane grid), display (text, rich text, image,
SVG, markdown, progress bar, QR code), input (text input, text editor
with syntax highlighting, checkbox, radio, toggle, slider, pick list,
combo box), and interactive wrappers (button, tooltip, mouse area,
canvas with drawing primitives).

**Multi-window.** Declare window nodes in the tree; the renderer opens
and closes them automatically. Each window has independent title, size,
position, theme, and scale factor.

**Accessibility.** Built-in [accesskit](https://accesskit.dev)
integration exposes the widget tree to screen readers and assistive
technology on all platforms.

**Theming.** Named themes (light, dark, and iced built-ins) plus custom
palettes defined as JSON objects with hex color fields. Per-window theme
overrides.

**Platform effects.** Native file dialogs, clipboard read/write, and OS
notifications -- requested over the protocol, results delivered as
events.

**Custom widgets.** The widget SDK (plushie-ext) lets you write new
widget types in Rust without forking the renderer. Custom widgets range
from simple render-only widgets to full interactive components with
their own state, event handling, and lifecycle management.

## Use cases

**Language communities without native GUI options.** If your language
can write JSON to stdout and read it back, you can build a toolkit on
top. The [Plushie](https://github.com/plushie-ui/plushie-elixir) toolkit for
Elixir was the first; Python, Go, Ruby, Node.js, or anything else
could follow the same pattern.

**Framework authors.** Building a GUI framework for your language? This
gives you a rendering backend with 30+ widgets, accessibility,
multi-window, and theming without writing platform code.

**Tool builders.** Have an existing CLI application and want to add a
GUI mode? The renderer can be bundled alongside your binary and driven
over stdio.

**Agent and AI tooling.** The tree format is plain JSON with a small
vocabulary (id, type, props, children). Language models and autonomous
agents can generate and update interfaces directly, creating dynamic
UIs tailored to the task at hand without pre-built templates.

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

### Run

The renderer reads a Settings message from stdin on startup, then
enters its event loop. In practice, your host library spawns it as a
child process and manages the communication. For manual
experimentation, you can pipe JSON:

    echo '{"type":"settings","settings":{}}' | cargo run -- --json

This starts the renderer in JSON mode with default settings. It will
wait for further messages on stdin (snapshots, patches, etc.).

## Project structure

This workspace contains four crates:

- **plushie-ext** -- Library crate and public SDK. Wire protocol,
  tree management, widget rendering, theming, platform effects, and the
  `PlushieWidget` trait for custom widgets. Widget authors depend on
  this crate.

- **plushie-renderer-lib** -- Shared renderer logic that compiles to
  both native and wasm32. Contains the App struct, effect handler trait,
  event processing, and window management.

- **plushie-renderer** -- Binary crate. Wires plushie-ext into an
  `iced::daemon` application. Handles stdin/stdout I/O, window
  lifecycle, and the iced event loop.

- **plushie-renderer-wasm** -- WASM entry point (cdylib). Runs the
  renderer in the browser via wasm-bindgen.

## Capabilities included

All capabilities are compiled in by default -- no feature flags to
manage. The binary includes all 30+ widget types, accessibility,
file dialogs, clipboard, notifications, and both non-GUI modes.

Headless mode (`--headless`) and mock mode (`--mock`) are runtime
flags that don't require a special build.

## Development

Install [just](https://just.systems) and
[cargo-nextest](https://nexte.st), then:

    just preflight      # Run all CI checks (check, clippy, fmt, test)
    just check          # Fast compile check
    just test           # Run tests
    just build-release  # Optimized release build

See `just --list` for all available recipes.

Both tools can also be installed via cargo:

    cargo install cargo-binstall        # Fast binary installer (one-time)
    cargo binstall cargo-nextest just   # Install dev tools

## Status

Early stage. The protocol and widget API are functional but not yet
stable -- breaking changes between versions are expected. The wire
protocol includes a version handshake so host libraries can detect
incompatibilities.

The first toolkit built on this renderer is
[Plushie](https://github.com/plushie-ui/plushie-elixir), a desktop GUI
framework for Elixir.

## Documentation

- [Protocol reference](docs/protocol.md) -- Wire format, message types,
  encoding, startup handshake
- [Plushie](https://github.com/plushie-ui/plushie-elixir) -- Elixir desktop
  GUI toolkit built on this renderer, with documentation of the tree
  format, event model, and UI builder DSL

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
