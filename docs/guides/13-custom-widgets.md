# Custom Widgets

As an app grows, `view` gets crowded. A sidebar, a rating block, a
preview pane: each is a self-contained piece of UI with its own
rendering and, sometimes, its own state. Plushie offers a progression
of ways to extract reusable pieces, from a plain helper function up to
a renderer-side widget written in Rust.

This chapter walks the four levels in order. Pick the lowest one that
fits: the later levels cost more, and premature adoption costs more
than premature deferral.

## Levels of composition

| Level | When to reach for it | Wire type? | Lives in |
|---|---|---|---|
| Helper function | Shape over existing widgets, no internal state | No | App code |
| Canvas-based widget | Custom drawing, no new input kinds | No | App code |
| Composite `Widget` | Pure-Rust widget with its own state and events | No | App code |
| Native `PlushieWidget` | Custom rendering or new input semantics | Yes (new type name) | Separate crate |

The boundaries between the first three are app-author territory: all
three compile into the app binary and need no renderer changes. The
fourth is renderer territory, which means a wire type name, a widget
crate, and either compile-time linking (direct mode) or a bundled
renderer (wire mode). See [Direct vs wire](../reference/direct-vs-wire.md)
for the dual-mode story.

## Helper functions as components

A helper that returns `impl Into<View>` or `View` is already a
component. It takes the data it needs, composes built-in builders,
and slots into a parent's `.child(..)` call. Chapter 7 used these
inline; [composition patterns](../reference/composition-patterns.md)
covers the full shape, including ID scoping and model lifting.

```rust
use plushie::prelude::*;

fn primary_button(id: &str, label: &str) -> impl Into<View> {
    button(id, label)
        .style(Style::primary())
        .padding(Padding::all(8))
}

fn section(title: &str, body: View) -> View {
    column()
        .spacing(12.0)
        .child(text(title).size(18.0))
        .child(body)
        .into()
}
```

Events from widgets inside a helper flow through to `update` by their
scoped ID, unchanged. A helper that carries no state and emits no
events of its own is usually the right answer.

## Canvas-based widgets

When the visual does not match any built-in widget but needs no new
input semantics, reach for [canvas](12-canvas.md). A canvas-returning
helper is still just a helper: the canvas draws, the parent handles
interaction.

```rust
use plushie::prelude::*;

fn gauge(id: &str, value: f32, max: f32) -> View {
    let pct = (value / max).clamp(0.0, 1.0);
    let angle = 180.0 * pct;

    canvas(id)
        .width(120.0)
        .height(70.0)
        .child(layer("bg").child(
            path(vec![arc(60.0, 60.0, 50.0, 180.0.into(), 0.0.into())])
                .stroke(Color::hex("#ddd"))
                .stroke_width(8.0)
                .stroke_cap(LineCap::Round),
        ))
        .child(layer("value").child(
            path(vec![arc(
                60.0,
                60.0,
                50.0,
                180.0.into(),
                (180.0 + angle).into(),
            )])
            .stroke(Color::hex("#3b82f6"))
            .stroke_width(8.0)
            .stroke_cap(LineCap::Round),
        ))
        .child(layer("label").child(
            canvas_text(40.0, 55.0, &format!("{:.0}%", pct * 100.0))
                .size(16.0)
                .fill(Color::hex("#333")),
        ))
        .into()
}
```

Canvas covers most bespoke visuals: progress rings, sparklines, colour
swatches, data visualisations. Embed SVG with `canvas_svg` when the
shape is easier to draw in a vector editor than to code.

Interactive canvas regions use `interactive(id)`, which wraps a group
in a focusable, clickable hit target. Events from an interactive
region arrive as ordinary widget events scoped under the canvas ID,
so `update` sees a `Click` the same way it would for a button.

## Composite widgets with the `Widget` trait

A helper stops being enough when the widget needs internal state that
the parent app should not manage: hover tracking, expansion state, a
scroll offset the host never reads. The `plushie::widget::Widget`
trait is the next step. It is pure Rust, links into the app binary,
and introduces no new wire type.

```rust
use plushie::prelude::*;
use plushie::widget::{EventResult, Widget};

struct StarRating;

#[derive(WidgetEvent)]
enum StarRatingEvent {
    /// User selected a rating.
    Select(u64),
}

#[derive(Default)]
struct StarState {
    hover: Option<usize>,
}

impl Widget for StarRating {
    type State = StarState;
    type Props = UntypedProps;

    fn view(id: &str, props: &UntypedProps, state: &Self::State) -> View {
        let rating = props
            .0
            .get("rating")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let display = state.hover.unwrap_or(rating);

        row()
            .id(id)
            .spacing(4.0)
            .children((0..5).map(|i| {
                let filled = i < display;
                let label = if filled { "\u{2605}" } else { "\u{2606}" };
                button(&format!("star-{i}"), label).style(if filled {
                    Style::warning()
                } else {
                    Style::text()
                })
            }))
            .into()
    }

    fn handle_event(event: &Event, state: &mut Self::State) -> EventResult {
        match event.widget_match() {
            Some(Click(id)) if id.starts_with("star-") => {
                if let Ok(n) = id["star-".len()..].parse::<usize>() {
                    state.hover = None;
                    EventResult::emit_event(StarRatingEvent::Select((n + 1) as u64))
                } else {
                    EventResult::Ignored
                }
            }
            Some(Enter(id, _)) if id.starts_with("star-") => {
                state.hover = id["star-".len()..].parse::<usize>().ok().map(|n| n + 1);
                EventResult::Consumed
            }
            Some(Exit(..)) => {
                state.hover = None;
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }
}
```

`State: Default` seeds the per-instance state; the runtime keeps the
slot alive for as long as the widget appears in the tree. `Props`
extracts from the tree node via `FromNode`. `UntypedProps` reads raw
JSON; a `#[derive(WidgetProps)]` struct gives compile-time checked
fields, described later.

`handle_event` intercepts events emitted by the widget's own subtree
before they reach `App::update`. The return values:

| Return | Effect |
|---|---|
| `EventResult::Emit { family, value }` | Emit a new widget event to the parent. Original is replaced. |
| `EventResult::emit_event(expr)` | Shorthand: derive the family and value from a `WidgetEvent`. |
| `EventResult::Consumed` | Handled; nothing reaches the parent. |
| `EventResult::Ignored` | Pass through to the parent unchanged. |

Mutate `state` directly inside the arm. There is no separate
`UpdateState` variant: any branch can mutate state and return any
other result.

### Using a composite widget in a view

`App::view` takes a `&mut WidgetRegistrar` as its second argument.
Pass it to `WidgetView::<W>::new(id).register(widgets)` at the point
in the tree where the widget should appear.

```rust
use plushie::widget::WidgetView;

fn view(model: &Self, widgets: &mut WidgetRegistrar) -> ViewList {
    window("main")
        .child(
            WidgetView::<StarRating>::new("stars")
                .prop("rating", model.rating as u64)
                .register(widgets),
        )
        .into()
}
```

Emitted events arrive at `App::update` as ordinary widget events.
Match on `event.as_widget()` when the family is the widget's own
(`"select"`, `"toggle"`) and not part of the built-in
`WidgetMatch` vocabulary:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    if let Some(w) = event.as_widget() {
        if w.scoped_id.id == "stars" {
            if let Some(n) = w.value.as_u64() {
                next.rating = n as usize;
            }
        }
    }
    (next, Command::none())
}
```

Composite widgets can also declare subscriptions via `subscribe(props,
state) -> Vec<Subscription>` and cache expensive expansions via
`cache_key(props, state) -> Option<u64>`. Both are opt-in; the default
impls return empty and `None`.

## Native widgets with `PlushieWidget`

Composite widgets cover everything that can be expressed as a tree of
built-in widgets plus internal state. When that is not enough, the
next level is a native widget: a new wire type, implemented in Rust
against the `plushie-widget-sdk` crate, and plugged into the renderer.
Reach for this when any of the following hold:

- The widget needs custom GPU drawing below the widget tree.
- The widget needs input semantics the built-in events cannot express.
- The widget owns renderer-side state that cannot be derived from the
  app model (a virtualised list's scroll cache, a canvas with its own
  input tracking).
- The widget will ship as a reusable crate across multiple apps.

A native widget is packaged as a separate crate so it can be listed as
a dependency of both the app and, in wire mode, the bundled renderer.
For the full surface, see
[custom widgets reference](../reference/custom-widgets.md).

### Scaffolding a widget crate

`cargo plushie new-widget` scaffolds a widget crate with the right
`Cargo.toml` metadata, a PascalCase widget struct, and a paired
factory for renderer registration.

```bash
cargo plushie new-widget star-rating
```

The name becomes the Cargo package (`star-rating`), the wire
`type_name` (`star_rating`), and the builder struct (`StarRating`).
The generated `Cargo.toml` declares
`[package.metadata.plushie.widget]` so `cargo plushie build` finds
the widget automatically during renderer bundling.

If `PLUSHIE_RUST_SOURCE_PATH` is set, the scaffold emits path deps
pointing at a local `plushie-rust` checkout so SDK edits reach the
new crate immediately. See
[CLI commands](../reference/cli-commands.md) for the flag reference.

### Implementing `PlushieWidget`

The minimum viable widget declares a wire type name and renders. The
derive fills in `type_names`, `fresh_for_session`, and delegates
`render` to a sibling `PlushieWidgetRender` impl:

```rust
use plushie_widget_sdk::prelude::*;

#[derive(PlushieWidget, Default)]
#[plushie_widget(type_name = "star_rating")]
pub struct StarRating;

impl PlushieWidgetRender for StarRating {
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        _ctx: &RenderCtx<'a>,
    ) -> PlushieElement<'a> {
        let value = node.prop_f32("value").unwrap_or(0.0) as usize;
        let max = prop_u32(&node.props, "max").unwrap_or(5) as usize;
        let size = node.prop_f32("size").unwrap_or(24.0);

        let mut stars = row![].spacing(4);
        for i in 1..=max {
            let label = if i <= value { "\u{2605}" } else { "\u{2606}" };
            stars = stars.push(text(label).size(size));
        }
        stars.into()
    }
}
```

Stateful widgets skip the derive and implement `PlushieWidget`
directly, so the `fresh_for_session` contract (return a fresh instance
with no per-session state) stays explicit. The trait has two required
methods (`type_names` and `render`); every other method has a default.

Three hooks are worth naming on the first pass:

- `prepare(&mut self, node, node_id, theme)` runs once per frame per
  instance, keyed by `(window_id, node_id)`. Use it to update
  renderer-side state before `render` reads it.
- `handle_message(&mut self, msg) -> HandleResult` intercepts iced
  messages. Return `HandleResult::Fallthrough` to let the registry do
  generic Click / Input / Toggle conversion; `HandleResult::emit(evts)`
  when the widget owns the message.
- `handle_widget_op(&mut self, node_id, op, payload)` receives
  app-issued `Command::widget` dispatches. Return `Some(events)` to
  emit follow-up events; `None` when the op is unhandled.

### Props with `#[derive(WidgetProps)]`

`WidgetProps` turns a plain struct into both a prop extractor and an
app-side fluent builder.

```rust
use plushie_widget_sdk::prelude::*;

#[derive(WidgetProps)]
#[widget(name = "star_rating")]
pub struct StarRating {
    /// Current value (0..=max).
    pub value: f32,
    /// Maximum rating.
    pub max: u32,
    /// Star glyph size in pixels.
    pub size: f32,
}

// Inside render:
let props = StarRatingProps::from_node(node);
let value = props.value.unwrap_or(0.0);
```

Field doc comments carry over to both the generated props struct and
the builder setter doc, so field-level documentation is written once.
Wire-aware domain types (`Color`, `Length`, `Font`, `Padding`,
`Theme`) use `T::extract(&node.props, key)` directly.

### Events with `#[derive(WidgetEvent)]`

Declare the typed event set the widget emits. Variant names become
snake_case family strings on the wire.

```rust
#[derive(WidgetEvent)]
pub enum StarRatingEvent {
    ValueChanged(f32),
    Cleared,
}
```

| Variant shape | Wire encoding |
|---|---|
| `Cleared` (unit) | `("cleared", PropValue::Null)` |
| `ValueChanged(f32)` (single tuple) | `("value_changed", PropValue::F32(v))` |
| `Change { x: f32, y: f32 }` (named) | `("change", PropValue::Object({x, y}))` |

Multi-field tuple variants are rejected; use named fields when a
variant carries more than one value.

### Commands with `#[derive(WidgetCommand)]`

Mirror of `WidgetEvent` for app-issued commands. Generates a
`WidgetCommandEncode` impl so the app can build typed commands that
reach `handle_widget_op` as `(family, Value)`.

```rust
#[derive(WidgetCommand)]
pub enum StarRatingCommand {
    Reset,
    SetValue(f32),
    SetRange { min: f32, max: f32 },
}
```

Route on `op` inside `handle_widget_op`; deserialise `payload` with
`serde_json` when the variant carries data.

## Using the widget in an app

Native widgets reach the app through two paths depending on runner
mode.

### Direct mode

In direct mode the widget crate is linked into the app binary. The
stock `plushie::run` entry point only wires the built-in iced widget
set, so apps that use custom widgets construct a `PlushieAppBuilder`
themselves and hand it to a direct-mode runner that accepts one:

```rust
use plushie_widget_sdk::app::PlushieAppBuilder;
use plushie_widget_sdk::runtime::iced_widget_set;

let builder = PlushieAppBuilder::new()
    .widget_set(&iced_widget_set())
    .widget(StarRating);
```

`.widget(w)` panics on type-name collision; `.widget_override(w)`
deliberately shadows an existing registration. See
[direct vs wire](../reference/direct-vs-wire.md) for the complete
entry-point shape and which runners honour a custom builder.

### Wire mode

Wire mode spawns an external renderer, so widget code has to live
inside that renderer. `cargo plushie build` reads the app's dep
graph, finds every crate with `[package.metadata.plushie.widget]`,
generates a workspace under `target/plushie-renderer/`, and compiles
a custom `plushie-renderer` binary with the widgets registered.

```bash
cargo plushie build --release
```

The SDK's binary discovery picks the resulting binary up
automatically from `target/plushie-renderer/target/<profile>/`, so
the app runs with `cargo run` as usual. `cargo plushie run` chains
the two steps: build the custom renderer, then exec `cargo run`
with the discovered binary pinned via `PLUSHIE_BINARY_PATH`.

The stock binary fetched by `cargo plushie download` carries no code
for custom widgets and `download` refuses to run when any native
widgets are present in the dep graph.

### Referencing the widget from `view`

On the app side, either call the builder generated by
`WidgetProps` or construct a raw tree node. The builder is usually
what `view` wants:

```rust
window("main")
    .child(
        StarRating::builder("rating")
            .value(model.rating)
            .max(5)
            .size(28.0),
    )
    .into()
```

Events from the widget arrive at `update` as
`Event::Widget { family: "value_changed", .. }`. Match with
`event.widget_match()` on `WidgetMatch::Custom` to extract the family
and payload:

```rust
use plushie::prelude::*;
use WidgetMatch::*;

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    if let Some(Custom { id: "rating", family: "value_changed", value }) =
        event.widget_match()
    {
        if let Some(v) = value.as_f64() {
            next.rating = v as f32;
        }
    }
    (next, Command::none())
}
```

## Testing a custom widget

`plushie_widget_sdk::testing::TestEnv` ships a harness for widget
unit tests. It owns the pieces needed to construct a `RenderCtx` and
exposes `prepare_and_render` and `handle_message_events` helpers so
the test drives the same sequence the renderer would.

```rust
use plushie_widget_sdk::prelude::*;
use plushie_widget_sdk::testing::*;
use serde_json::json;

#[test]
fn renders_five_stars() {
    let env = TestEnv::default();
    let mut widget = StarRating::default();
    let node = node_with_props(
        "rating-1",
        "star_rating",
        json!({ "value": 3.0, "max": 5, "size": 24.0 }),
    );

    let _element = env.prepare_and_render(&mut widget, &node, "main");
}
```

Node constructors: `node`, `node_with_props`, `node_with_children`,
`node_with_props_and_children`. For app-level tests that exercise the
widget through the normal Elm loop, use `plushie::test::TestSession`
as described in the [testing reference](../reference/testing.md). The
widget-sdk harness is narrower: it never spins up a renderer process.

Composite widgets built on `plushie::widget::Widget` are tested
through the same `TestSession` path. The session drives `App::view`
the same way the runtime does, so a click on a widget's internal
button flows through `handle_event` into `update` without ceremony.

## What's next

Plushie apps that reuse widgets across tabs, panes, or windows hit
the same recurring question: where does shared state live? The next
chapter looks at [state management](14-state-management.md) patterns
for routing, selection, undo history, and search, built on the
helpers in `plushie::state`.
