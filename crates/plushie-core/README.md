# plushie-core

Core types and wire protocol for [Plushie](https://github.com/plushie-ui/plushie-rust).
**Pre-1.0**

This crate defines the shared data types, wire protocol messages, and
property system used by all Plushie crates. It has no dependency on
iced or any GUI framework, making it suitable for host SDKs, tools,
and any code that needs to work with the Plushie wire format.

## What's in here

- **Protocol types** - `IncomingMessage`, `OutgoingEvent`, `TreeNode`,
  `PatchOp`, and all wire message structs
- **Property types** - Color, Padding, Length, Font, Border, Shadow,
  Style, A11y, canvas shapes, and the full type system
- **Settings** - renderer configuration, window config, protocol version
- **Animation** - easing curves shared between SDK and renderer
- **Scoped IDs** - structured ID parsing for hierarchical widget identity
- **Derive macros** - via `plushie-core-macros` (re-exported)

## Usage

Most users don't depend on this crate directly. It's re-exported by
`plushie` (Rust SDK) and `plushie-widget-sdk` (widget author SDK).

```toml
[dependencies]
plushie-core = "0.6"
```

## License

MIT OR Apache-2.0
