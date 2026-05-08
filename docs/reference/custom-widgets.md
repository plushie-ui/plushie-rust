# Custom widgets

A custom widget is a renderer-side widget type that ships outside the
built-in iced set. Authors implement the [`PlushieWidget`] trait from
the `plushie-widget-sdk` crate and register the type with the renderer,
which then dispatches `render`, `prepare`, message handling, widget
ops, and per-instance subscriptions uniformly alongside every built-in.

Reach for a custom widget when the built-in catalog and the canvas
primitives cannot express what the view needs: a bespoke GPU-drawn
gauge, a new input device, a control whose per-frame behaviour depends
on renderer-side state. Anything that can be composed from existing
widgets is better written as a reusable Rust function (or a composite
`plushie::widget::Widget`) than as a new wire type.

Custom widgets plug into both runner modes, but with different
bundling stories. See [Direct vs wire](direct-vs-wire.md).

## Crate layout

The widget-author surface lives in `plushie-widget-sdk`. Pair it with
`plushie-core` (the derive macros emit `::plushie_core::*` paths) and,
optionally, the iced re-export for widgets that build their UI from
iced primitives directly.

```toml
[dependencies]
plushie-core = "0.7.0"
plushie-widget-sdk = "0.7.0"
```

The prelude covers almost every type a widget author names:

```rust
use plushie_widget_sdk::prelude::*;
```

It re-exports the trait, the derive macros, `RenderCtx`, the widget
subscription types, the prop extraction helpers, the wire-aware domain
types (`Color`, `Length`, `Font`, `Padding`, `Theme`), and a curated
set of iced constructors. For anything else iced-specific, reach into
`plushie_widget_sdk::iced`; a direct `iced` dependency is never needed.

## Authoring a widget

The minimum viable widget is a unit struct that declares its wire
type name and renders. The `PlushieWidget` derive fills in the
boilerplate (`type_names`, `fresh_for_session`) and delegates `render`
to a sibling [`PlushieWidgetRender`] impl:

```rust
use plushie_widget_sdk::prelude::*;

#[derive(PlushieWidget, Default)]
#[plushie_widget(type_name = "gauge")]
pub struct Gauge;

impl PlushieWidgetRender for Gauge {
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a>,
    ) -> PlushieElement<'a> {
        let value = prop_f32(&node.props, "value").unwrap_or(0.0);
        let label = text(format!("{:.0}%", value * 100.0));
        container(label).padding(8).into()
    }
}
```

Stateful widgets skip the derive and implement `PlushieWidget`
directly so the `fresh_for_session` contract (return an instance with
no session-specific state) stays explicit. The two required methods
are `type_names` and `render`; every other method on the trait has a
default.

## The `PlushieWidget` trait

[`PlushieWidget<R>`] is generic over the renderer backend. `R` defaults
to `iced::Renderer`; the mock backend substitutes `()`. Both are
covered by the [`PlushieRenderer`] sealed trait alias and no external
type satisfies it.

| Method | Signature | Purpose |
|---|---|---|
| `type_names` | `(&self) -> &[&str]` | Wire type name(s) this widget handles |
| `namespace` | `(&self) -> &str` | Config routing key for `Settings.widget_config` |
| `render` | `(&'a self, &'a TreeNode, &RenderCtx<'a, R>) -> Element<'a, Message, Theme, R>` | Build the iced element for this node |
| `prepare` | `(&mut self, &TreeNode, &str, &Theme)` | Per-frame mutable update, keyed by `(window_id, node_id)` |
| `handle_message` | `(&mut self, &Message) -> HandleResult` | Take responsibility for a widget message |
| `cleanup_stale` | `(&mut self, &HashSet<(String, String)>)` | Drop per-instance state for nodes no longer in the tree |
| `init` | `(&mut self, &InitCtx<'_>)` | Receive namespaced config from the host's Settings message |
| `infer_a11y` | `(&self, &TreeNode) -> Option<A11yOverrides>` | Inject a11y annotations when none were declared |
| `handle_widget_op` | `(&mut self, &str, &str, &Value) -> Option<Vec<OutgoingEvent>>` | Receive app-issued widget operations (focus, scroll, custom commands) |
| `event_specs` | `(&self) -> Vec<EventSpec>` | Declare emitted-event shapes for runtime validation |
| `command_specs` | `(&self) -> Vec<CommandSpec>` | Declare accepted-command shapes for runtime validation |
| `subscriptions` | `(&self, &TreeNode, &SubscribeCtx<'_>) -> Vec<WidgetSubscription>` | Request renderer-driven subscriptions while the node is in the tree |
| `fresh_for_session` | `(&self) -> Box<dyn PlushieWidget<R>>` | Produce a session-isolated instance with no carried-over state |

`handle_message` returns [`HandleResult`]: `Fallthrough` lets the
registry run its generic message-to-event conversion (Click, Input,
Toggle, and friends); `Handled(events)` takes responsibility and the
registry emits the supplied events as-is. `HandleResult::consume()`
and `HandleResult::emit(events)` are the usual shorthands.

`handle_widget_op` is how app-issued `Command::widget` calls reach the
widget. The `node_id` is the full wire ID from the operation, `op`
names the family, and `payload` is the JSON value the app sent.
Return `Some(events)` to emit follow-up events from the dispatch, or
`None` to signal "I don't handle this op".

## `RenderCtx`

[`RenderCtx<'a, R>`] is `Copy`; render methods can clone it freely and
pass a variant down to child nodes. The fields:

| Field | Type | Use |
|---|---|---|
| `caches` | `&'a SharedState` | Renderer-side caches (style hashes, override memoization) |
| `images` | `&'a ImageRegistry` | Image handles the host has registered |
| `theme` | `&'a Theme` | Active iced theme for the current window |
| `registry` | `&'a WidgetRegistry<R>` | Dispatch to other widgets (via helpers, not directly) |
| `default_text_size` | `Option<f32>` | Host-provided default text size, in pixels |
| `default_font` | `Option<iced::Font>` | Host-provided default font |
| `window_id` | `&'a str` | Wire id of the enclosing window |
| `scale_factor` | `f32` | DPI scale factor for the window |

Helpers: `ctx.render_child(node)` dispatches one child through the
main pipeline; `ctx.render_children(node)` renders every child;
`ctx.with_theme(theme)` returns a context scoped to an override theme;
`ctx.with_window_id(id)` rebinds the window context for a nested
subtree.

## Derive macros

Three derives cover the common shapes. All live in the prelude.

### `#[derive(WidgetProps)]`

Declares the widget's props as a plain Rust struct. The derive emits
a companion `{Name}Props` struct whose fields are `Option<T>`, with a
`from_node(&TreeNode)` that extracts typed values via
`PlushieType::extract`. Widget crates can also generate a fluent
builder the app side uses in `view`.

```rust
use plushie_widget_sdk::prelude::*;

#[derive(WidgetProps)]
#[widget(name = "gauge")]
pub struct Gauge {
    /// Current value; clamped at render time.
    pub value: f32,
    /// Full-scale value for the gauge.
    pub max: f32,
}

// In render:
let props = GaugeProps::from_node(node);
let value = props.value.unwrap_or(0.0);
```

Field doc comments carry over to both the generated Props struct and
the builder setter doc, so field-level documentation only needs to be
written once.

### `#[derive(WidgetEvent)]`

Declares the typed event set the widget emits. The derive implements
`WidgetEventEncode`, which turns each variant into a
`(family, PropValue)` pair for wire transport. Variant names become
snake_case family strings.

| Variant shape | Wire encoding |
|---|---|
| `Cleared` (unit) | `("cleared", PropValue::Null)` |
| `Select(u64)` (single tuple) | `("select", PropValue::U64(v))` |
| `Change { x: f32, y: f32 }` (named) | `("change", PropValue::Object({x, y}))` |

```rust
#[derive(WidgetEvent)]
pub enum GaugeEvent {
    ValueChanged(f32),
    Cleared,
}
```

Multi-field tuple variants are rejected; use named fields when a
variant carries more than one value.

### `#[derive(WidgetCommand)]`

Mirror of `WidgetEvent` for incoming commands. Generates a
`WidgetCommandEncode` impl so app code can build typed commands that
reach `handle_widget_op` as `(family, Value)`.

```rust
#[derive(WidgetCommand)]
pub enum GaugeCommand {
    Reset,
    SetValue(f32),
    SetRange { min: f32, max: f32 },
}
```

## Props

Values travel over the wire as `PropValue` and land in
`node.props` as a typed `Props` map. Read them with the helpers from
`plushie_widget_sdk::prop_helpers` (glob-imported by the prelude):
`prop_str`, `prop_f32`, `prop_f64`, `prop_u32`, `prop_u64`, `prop_usize`,
`prop_i32`, `prop_i64`, `prop_bool`, `prop_bool_default`,
`prop_range_f32`, `prop_range_f64`, `prop_f32_array`, `prop_f64_array`,
`prop_str_array`, `prop_animated_f32`, `prop_animated_color`.

For wire-aware domain types (`Color`, `Length`, `Font`, `Padding`,
`Theme`) call `T::extract(&node.props, key)` directly; the trait
bound is `PlushieType` from `plushie_core::types`, re-exported by the
prelude.

Wire encoding happens in the host SDK: the widget crate only consumes
props, never encodes them.

## Events

Widgets emit [`OutgoingEvent`] values. Direct construction is rarely
necessary; the common path is to build events from a `WidgetEvent`
derive and pass them through `HandleResult::emit` or as the return
value of `handle_widget_op`. Each outgoing event carries a family,
a payload, and an optional coalesce hint that lets high-frequency
emitters (animation ticks, pointer streams) deduplicate on the way
back to the app.

The registry's default message conversion handles Click, Input,
Toggle, Select, and the rest of the generic widget vocabulary. Only
override `handle_message` when the widget needs stateful processing
(cursor position, drag state) that the generic path does not carry.

## Commands

App-issued commands reach the widget through `handle_widget_op`.
Route on the `op` argument and deserialise `payload` using the typed
command enum:

```rust
fn handle_widget_op(
    &mut self,
    _node_id: &str,
    op: &str,
    payload: &Value,
) -> Option<Vec<OutgoingEvent>> {
    match op {
        "reset" => {
            self.value = 0.0;
            None
        }
        "set_value" => {
            if let Some(v) = payload.as_f64() {
                self.value = v as f32;
            }
            None
        }
        _ => None,
    }
}
```

Return `Some(events)` when a command should emit follow-up events in
the same dispatch cycle (for example, acknowledging that a reset
completed). Return `None` for silent commands.

## Direct mode

In direct mode the app binary links the widget crate at compile time
and the in-process iced daemon runs the widget directly. The compiled
widget and the compiled app share one process; no wire encoding
happens.

Today the stock `plushie::run` entry point wires in the built-in iced
widget set and nothing else. Apps that need custom widgets in direct
mode construct a `PlushieAppBuilder` themselves and hand it to a
renderer that accepts one (see [Direct vs wire](direct-vs-wire.md)
for the full shape and the entry points that honour it).

```rust
use plushie_widget_sdk::app::PlushieAppBuilder;
use plushie_widget_sdk::runtime::iced_widget_set;

let builder = PlushieAppBuilder::new()
    .widget_set(&iced_widget_set())
    .widget(Gauge);
```

`.widget(w)` panics on type-name collision; use `.widget_override(w)`
to deliberately shadow an existing registration, and `.widget_set(s)`
to add a whole bundle at once.

## Wire mode

Wire mode spawns an external renderer, so the widget code has to live
inside that renderer. `cargo plushie build` reads the app's dep graph,
finds every crate whose `Cargo.toml` carries a
`[package.metadata.plushie.widget]` table, and generates a
`plushie-renderer` workspace under `target/plushie-renderer/`. The
generated `main.rs` wires each widget into a `PlushieAppBuilder` and
hands it to `plushie_renderer::run`:

```rust
use plushie_widget_sdk::app::PlushieAppBuilder;

fn main() -> plushie_widget_sdk::iced::Result {
    let builder = PlushieAppBuilder::new()
        .widget(my_gauge::factory::GaugeFactory::new());
    plushie_renderer::run(builder)
}
```

The SDK's wire discovery (`target/plushie-renderer/target/<profile>/`)
picks the resulting binary up automatically; see
[CLI commands](cli-commands.md) for the build flow and flags.

The stock renderer published on GitHub Releases has no code for
custom widgets. `cargo plushie download` refuses to run when any
native widgets are present in the dep graph, because the resulting
binary would reject every one of their widget messages.

## Testing

`plushie_widget_sdk::testing` ships a minimal harness for widget unit
tests. [`TestEnv`] owns the pieces needed to construct a
[`RenderCtx`]: a `SharedState`, an `ImageRegistry`, a `Theme`, and a
`WidgetRegistry` pre-populated with the iced widget set. All fields
are public so tests can customise before calling `render_ctx()`.

```rust
use plushie_widget_sdk::prelude::*;
use plushie_widget_sdk::testing::*;

#[test]
fn renders_label_with_percent() {
    let env = TestEnv::default();
    let mut widget = Gauge::default();
    let node = node_with_props(
        "gauge-1",
        "gauge",
        serde_json::json!({ "value": 0.75, "max": 1.0 }),
    );

    let _element = env.prepare_and_render(&mut widget, &node, "main");
}
```

Node constructors: `node`, `node_with_props`, `node_with_children`,
`node_with_props_and_children`. Helpers for driving a widget end to
end: `TestEnv::prepare_and_render` (runs `prepare` then `render` with
correct borrow ordering) and `TestEnv::handle_message_events` (runs
`handle_message` and flattens the `HandleResult` to a plain
`Vec<OutgoingEvent>`).

For app-level tests that exercise the widget through the normal Elm
loop, use `plushie::test::TestSession` as described in the
[Testing reference](testing.md). The widget-sdk harness is narrower:
it never spins up a renderer process.

## Bundling and metadata

A widget crate ships its own `Cargo.toml` metadata so tooling can
discover it without the app declaring a registry by hand. The
`[package.metadata.plushie.widget]` table is the source of truth:

```toml
[package]
name = "my-gauge"
version = "0.1.0"
edition = "2024"

[package.metadata.plushie.widget]
type_name = "gauge"
constructor = "my_gauge::factory::GaugeFactory::new()"

[features]
impl = ["dep:plushie-widget-sdk"]

[dependencies]
plushie-core = "0.7.0"
plushie-core-macros = "0.7.0"
plushie-widget-sdk = { version = "0.7.0", optional = true }
```

`type_name` is the wire type registered with the renderer.
`constructor` is a zero-arg Rust expression that produces the
registered value (often a factory, not the builder struct used in
`App::view`). `cargo plushie build` splices the constructor into the
generated `main.rs`.

The `impl` feature pattern keeps the stub crate iced-free so any
plushie app can declare the widget in its view tree, even if the
renderer-side implementation is only compiled into the custom
`plushie-renderer` binary.

## Scaffolding

`cargo plushie new-widget <name>` produces a working widget crate in
one step:

```bash
cargo plushie new-widget star-rating
```

The scaffold emits a kebab-case Cargo package, a snake_case wire
type name, and a PascalCase builder plus a paired `Factory` struct
under an `impl` feature. The generated `render` body reads typed
props, applies a clamp, and renders a padded container: enough to
run against the custom renderer unmodified, and enough of a
template for authors to evolve into the real widget.

The command refuses to scaffold over an existing destination and
refuses a type name that would shadow a built-in widget. When
`PLUSHIE_RUST_SOURCE_PATH` is set it emits path dependencies
pointing at the local `plushie-rust` checkout so SDK edits reach the
new crate immediately; see [CLI commands](cli-commands.md) for the
flag reference.

## See also

- [Direct vs wire](direct-vs-wire.md)
- [CLI commands](cli-commands.md)
- [Built-in widgets](built-in-widgets.md)
- [Wire protocol](wire-protocol.md)
- [Events](events.md)
