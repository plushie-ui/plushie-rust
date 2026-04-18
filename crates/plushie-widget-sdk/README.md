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

#[derive(PlushieWidget)]
#[plushie_widget(type_name = "my_gauge")]
struct MyGauge;

impl<R: PlushieRenderer> PlushieWidgetRender<R> for MyGauge {
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> PlushieElement<'a, R> {
        // Build an iced Element from the node's props
        todo!()
    }
}
```

The `PlushieWidget` derive generates `type_names` and
`fresh_for_session`. The author implements `PlushieWidgetRender::render`
with the body. `PlushieElement<'a, R>` is the shorthand for
`iced::Element<'a, Message, iced::Theme, R>`.

For full control (stateful widgets, custom lifecycle hooks, multiple
type names) skip the derive and implement `PlushieWidget` directly:

```rust
impl<R: PlushieRenderer> PlushieWidget<R> for MyGauge {
    fn type_names(&self) -> &[&str] { &["my_gauge"] }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> PlushieElement<'a, R> {
        todo!()
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(MyGauge)
    }
}
```

For iced types not in the prelude, use `plushie_widget_sdk::iced::*`
instead of adding a direct `iced` dependency. This avoids version
conflicts.

## Dependencies

Widget crates need **both** `plushie-widget-sdk` and `plushie-core`
as direct dependencies:

```toml
[dependencies]
plushie-widget-sdk = "0.6"
plushie-core = "0.6"
```

The widget derive macros (`#[derive(WidgetEvent)]`,
`#[derive(WidgetCommand)]`, `#[derive(WidgetProps)]`,
`#[derive(PlushieWidget)]`) emit code that references
`::plushie_core::*` paths. Without a direct `plushie-core` dep the
generated code will not resolve.

## iced stability

`plushie-widget-sdk` re-exports iced as a transitive dependency.
iced surfaces may change on any plushie minor release. For stable
semantics, prefer prelude names and `iced_convert::*` conversions;
reach into `plushie_widget_sdk::iced::*` only for iced-specific
constructs that are not covered by the prelude.

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
