# plushie-widget-sdk

Widget SDK for [Plushie](https://github.com/plushie-ui/plushie-rust).
Build custom native widgets in Rust. **Pre-1.0**

This crate provides the `PlushieWidget` trait and everything needed to
implement custom widgets that render via iced. Widgets built with this
SDK work across all Plushie host SDKs (Elixir, Gleam, Python, Ruby,
TypeScript, Rust).

Also contains all built-in widget implementations that ship with the
renderer.

## Quick start

```rust
use plushie_widget_sdk::prelude::*;

struct MyGauge;

impl PlushieWidget for MyGauge {
    fn type_names(&self) -> &[&str] { &["my_gauge"] }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a>,
    ) -> Element<'a, Message> {
        // Build an iced Element from the node's props
        todo!()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget> {
        Box::new(MyGauge)
    }
}
```

For iced types not in the prelude, use `plushie_widget_sdk::iced::*`
instead of adding a direct `iced` dependency. This avoids version
conflicts.

## Features

- **PlushieWidget trait** - three required methods, optional lifecycle
  hooks for init, prepare, message handling, cleanup
- **Canvas engine** - reusable canvas composition with layer caching,
  interactive shapes, hit testing, and keyboard navigation
- **Built-in widgets** - all standard Plushie widget implementations
- **Prop helpers** - typed extraction of widget properties from tree nodes
- **Testing utilities** - node factories and render context builders
  for widget tests

## Documentation

See the `PlushieWidget` trait documentation in `src/registry.rs`
for the full API reference.

## License

MIT OR Apache-2.0
