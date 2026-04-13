# Core Widget Guide

Build an iced widget once, use it everywhere: directly in Rust
applications, and across every plushie-powered SDK (Elixir, Gleam,
and any future host language).

## The two-crate pattern

A reusable widget is two crates:

```
my-gauge/               depends on iced (via plushie-iced)
  src/lib.rs            the Widget impl (rendering, layout, events, a11y)
  Cargo.toml

my-gauge-plushie/         depends on plushie-widget-sdk + my-gauge
  src/lib.rs            PlushieWidget wrapper (prop parsing, event bridging)
  Cargo.toml
```

**The widget crate** (`my-gauge`) is a pure iced widget. It knows
nothing about plushie, JSON, protocols, or host SDKs. A Rust
developer imports it and uses it like any iced widget:

```rust
use my_gauge::gauge;

fn view(&self) -> Element<Message> {
    gauge(self.battery_level)
        .width(200)
        .color(Color::from_rgb(0.2, 0.8, 0.3))
        .into()
}
```

**The widget crate** (`my-gauge-plushie`) wraps the widget for
plushie's protocol. It parses JSON props, constructs the widget, and
bridges events. Every host SDK gets the widget through this single
wrapper, no per-language duplication:

```rust
use plushie_widget_sdk::prelude::*;
use my_gauge::gauge;

pub struct GaugeWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for GaugeWidget {
    fn type_names(&self) -> &[&str] { &["gauge"] }

    fn render<'a>(&'a self, node: &'a TreeNode, ctx: &RenderCtx<'a, R>) -> Element<'a, Message, Theme, R> {
        let value = node.prop_f32("value").unwrap_or(0.0);
        let width = plushie_core::types::Length::extract(node.props(), "width")
            .map(|l| iced_convert::length(&l))
            .unwrap_or(Length::Fixed(100.0));
        let color = node.prop_color("color")
            .unwrap_or(ctx.theme.palette().primary.base.color);

        gauge(value).width(width).color(color).into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(GaugeWidget)
    }
}
```

An Elixir developer uses it:

```elixir
gauge(id: "battery", value: 0.75, color: "#4CAF50")
```

A Gleam developer uses it the same way. A Rust developer uses the
widget crate directly without the wrapper. One widget, every
platform.

## Why two crates?

Separation of concerns. The widget crate has zero plushie knowledge
-- it depends only on iced. This means:

- **Testable in isolation.** Test the widget with iced's test
  harness. No protocol, no JSON, no plushie runtime needed.
- **Usable outside plushie.** Any iced application can use it. The
  widget isn't locked to plushie's ecosystem.
- **Clean API.** The widget has typed Rust parameters (`f32`,
  `Color`, `Length`), not `&Value` JSON blobs. The widget
  wrapper handles the JSON-to-typed conversion.

The widget wrapper is intentionally thin. It parses props,
constructs the widget, and maybe bridges events. The real logic
lives in the widget crate.

## Part 1: The iced widget crate

### Dependencies

```toml
[package]
name = "my-gauge"
version = "0.1.0"
edition = "2024"

[dependencies]
iced = { package = "plushie-iced", version = "0.6" }
```

**Note:** Use `plushie-iced` (the fork), not upstream `iced`. plushie
and all its SDKs use this fork. Using a different iced version
causes type mismatches at compile time.

If you're building a widget that should also work with upstream
iced, you can use Cargo features to switch between the two. But
for plushie ecosystem widgets, `plushie-iced` is the standard.

### The Widget trait

Every iced widget implements the `Widget` trait:

```rust
// Simplified signatures; see iced::advanced::widget::Widget
// for the full trait with all type parameters.
pub trait Widget<Message, Theme, Renderer> {
    fn size(&self) -> Size<Length>;           // size hint
    fn layout(&mut self, tree, renderer, limits) -> layout::Node;
    fn draw(&self, tree, renderer, theme, style, layout, cursor, viewport);
    fn update(&mut self, tree, event, layout, cursor, renderer, shell, viewport);
    fn operate(&mut self, tree, layout, renderer, operation);
    fn mouse_interaction(&self, tree, layout, cursor, viewport, renderer) -> Interaction;
    // ... plus tag(), state(), overlay()
}
```

`size()`, `layout()`, and `draw()` are required. Everything else
has defaults.

**Call order per frame:** `layout()` -> `draw()` -> `update()`
(for each event) -> `operate()` (for a11y/focus queries).

### A complete gauge widget

```rust
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Widget, tree};
use iced::{Color, Element, Length, Size, Rectangle, Theme, mouse};

/// A circular gauge that displays a value from 0.0 to 1.0.
pub struct Gauge {
    value: f32,
    color: Color,
    width: Length,
    height: Length,
}

impl Gauge {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            color: Color::from_rgb(0.2, 0.6, 1.0),
            width: Length::Fixed(100.0),
            height: Length::Fixed(100.0),
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Gauge
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size { width: self.width, height: self.height }
    }

    fn layout(
        &mut self,
        _tree: &mut widget::Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let bg = theme.palette().background.weak.color;

        // Background track
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: iced::Border {
                    radius: (bounds.height / 2.0).into(),
                    ..Default::default()
                },
                ..renderer::Quad::default()
            },
            iced::Background::Color(bg),
        );

        // Filled portion
        let filled_width = bounds.width * self.value;
        if filled_width > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        width: filled_width,
                        ..bounds
                    },
                    border: iced::Border {
                        radius: (bounds.height / 2.0).into(),
                        ..Default::default()
                    },
                    ..renderer::Quad::default()
                },
                iced::Background::Color(self.color),
            );
        }
    }

    fn operate(
        &mut self,
        _tree: &mut widget::Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        use iced::advanced::widget::operation::accessible::{Accessible, Role};

        operation.accessible(
            None,
            layout.bounds(),
            &Accessible {
                role: Role::Meter,
                label: Some("Gauge"),
                ..Accessible::default()
            },
        );
    }
}

/// Convenience constructor.
pub fn gauge(value: f32) -> Gauge {
    Gauge::new(value)
}

/// Into Element conversion.
impl<'a, Message: 'a, Renderer> From<Gauge>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(widget: Gauge) -> Self {
        Self::new(widget)
    }
}
```

This widget works in any iced application. No plushie dependency.

### Layout

`layout()` returns a `layout::Node` describing the widget's size.
For leaf widgets (no children), `layout::atomic(limits, width, height)`
handles the constraint resolution.

For widgets with children, compute child layouts and position them:

```rust
fn layout(&mut self, tree, renderer, limits) -> layout::Node {
    let child_limits = limits.width(Length::Fill);
    let child_layout = self.child
        .as_widget_mut()
        .layout(&mut tree.children[0], renderer, &child_limits);

    let child_size = child_layout.bounds().size();
    let padding = 10.0;
    let node_size = Size::new(
        child_size.width + padding * 2.0,
        child_size.height + padding * 2.0,
    );

    layout::Node::with_children(
        node_size,
        vec![child_layout.move_to(Point::new(padding, padding))],
    )
}
```

### Drawing

Use `renderer.fill_quad()` for rectangles; it's batched (hundreds
of quads in one GPU draw call). For text, use `renderer.fill_text()`.
For complex paths or gradients, use `canvas::Frame`.

### Events

`update()` receives all iced events. Call `shell.capture_event()`
to stop propagation, `shell.publish(message)` to emit messages:

```rust
fn update(&mut self, _tree, event, layout, cursor, _renderer, shell, _viewport) {
    if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
        if cursor.is_over(layout.bounds()) {
            shell.publish(MyMessage::Clicked);
            shell.capture_event();
        }
    }
}
```

### Widget state

Widgets that need mutable state across frames declare it via
`tag()` and `state()`:

```rust
fn tag(&self) -> tree::Tag {
    tree::Tag::of::<MyState>()
}

fn state(&self) -> tree::State {
    tree::State::new(MyState::default())
}
```

Access in other methods: `tree.state.downcast_ref::<MyState>()`.

### Accessibility

`operate()` exposes the widget to screen readers and other AT:

```rust
fn operate(&mut self, _tree, layout, _renderer, operation) {
    operation.accessible(None, layout.bounds(), &Accessible {
        role: Role::Meter,
        label: Some("Battery level"),
        ..Accessible::default()
    });
}
```

For focusable widgets, also call `operation.focusable()` with a
state that implements the `Focusable` trait.

---

## Part 2: The plushie widget wrapper

The wrapper crate bridges your iced widget to plushie's protocol.
It's intentionally thin: just prop parsing and event bridging.

### Dependencies

```toml
[package]
name = "my-gauge-plushie"
version = "0.1.0"
edition = "2024"

[dependencies]
plushie-widget-sdk = "0.6"
my-gauge = { path = "../my-gauge" }
```

### The wrapper

```rust
use plushie_widget_sdk::prelude::*;
use my_gauge::gauge;

pub struct GaugeWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for GaugeWidget {
    fn type_names(&self) -> &[&str] { &["gauge"] }

    fn render<'a>(&'a self, node: &'a TreeNode, ctx: &RenderCtx<'a, R>) -> Element<'a, Message, Theme, R> {
        let value = node.prop_f32("value").unwrap_or(0.0);
        let color = node.prop_color("color")
            .unwrap_or(ctx.theme.palette().primary.base.color);
        let width = plushie_core::types::Length::extract(node.props(), "width")
            .map(|l| iced_convert::length(&l))
            .unwrap_or(Length::Fixed(100.0));
        let height = plushie_core::types::Length::extract(node.props(), "height")
            .map(|l| iced_convert::length(&l))
            .unwrap_or(Length::Fixed(100.0));

        gauge(value)
            .color(color)
            .width(width)
            .height(height)
            .into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(GaugeWidget)
    }
}
```

That's the entire wrapper. The widget logic, layout, drawing,
and accessibility are all in the widget crate. The wrapper just
translates JSON props to typed parameters.

### What the wrapper handles

| Concern | Where |
|---------|-------|
| Layout, drawing, events, a11y | Widget crate (`my-gauge`) |
| Prop parsing (JSON -> types) | Wrapper crate (`my-gauge-plushie`) |
| Event bridging (plushie Message -> host) | Wrapper crate |
| State management | Wrapper crate (if needed) |
| Compilation, binary generation | Host SDK (automatic) |

### Events from your widget

If your iced widget emits messages via `shell.publish()`, the
wrapper catches them in `handle_message()` and translates to
`OutgoingEvent`:

```rust
fn handle_message(&mut self, msg: &Message) -> Option<Vec<OutgoingEvent>> {
    if let Message::Event { id, family, .. } = msg {
        if family == "click" {
            return Some(vec![
                OutgoingEvent::widget_event("gauge_clicked", id, None)
            ]);
        }
    }
    None
}
```

For high-frequency events (continuous value changes), set a
`CoalesceHint`:

```rust
OutgoingEvent::widget_event("value_changed", id, data)
    .with_coalesce(CoalesceHint::Replace)
```

### Commands to your widget

Host SDKs can send commands to your widget at runtime, bypassing
the normal tree update cycle. This is useful for high-frequency
data (pushing plot points to a chart) or imperative operations
(scrolling to a position, clearing state).

Commands arrive through `handle_widget_op()` on the PlushieWidget
trait. On the wire, they use the unified command format:

```json
{"type": "command", "id": "gauge-1", "family": "set_value", "value": 72.0}
```

The `family` string identifies the operation. The `value` carries
the payload (or `null` for operations with no data).

#### Typed command enums with `#[derive(WidgetCommand)]`

For Rust SDK users, `#[derive(WidgetCommand)]` generates type-safe
command construction with automatic family naming and value
encoding:

```rust
use plushie_core::WidgetCommand;

#[derive(WidgetCommand)]
enum GaugeCommand {
    /// Set gauge to a value immediately.
    SetValue(f32),
    /// Reset gauge to zero.
    Reset,
    /// Set the display range.
    SetRange { min: f32, max: f32 },
}
```

The derive macro converts variant names to `snake_case` family
strings and encodes payloads automatically:

| Variant | Wire family | Wire value |
|---------|-------------|------------|
| `SetValue(72.0)` | `"set_value"` | `72.0` |
| `Reset` | `"reset"` | `null` |
| `SetRange { min: 0.0, max: 100.0 }` | `"set_range"` | `{"min": 0.0, "max": 100.0}` |

Use `Command::widget()` with the typed enum:

```rust
use plushie::command::Command;

// Type-safe: compiler checks the variant and payload types
Command::widget("temp-gauge", GaugeCommand::SetValue(72.0))
```

For dynamic or untyped usage, `Command::send()` accepts raw
family and value:

```rust
// Low-level: no compile-time type checking on the payload
Command::send("temp-gauge", "set_value", serde_json::json!(72.0))
```

### Spec validation

The PlushieWidget trait provides `event_specs()` and
`command_specs()` for runtime validation of payloads. The renderer
validates emitted event payloads and incoming command payloads
against these specs and logs warnings on mismatch.

```rust
use plushie_core::{EventSpec, CommandSpec, PayloadSpec, ValueType};

impl<R: PlushieRenderer> PlushieWidget<R> for GaugeWidget {
    // ... type_names, render, clone_for_session ...

    fn event_specs(&self) -> Vec<EventSpec> {
        vec![
            EventSpec {
                family: "value_changed".into(),
                payload: PayloadSpec::Value(ValueType::Float),
            },
            EventSpec {
                family: "gauge_clicked".into(),
                payload: PayloadSpec::None,
            },
        ]
    }

    fn command_specs(&self) -> Vec<CommandSpec> {
        vec![
            CommandSpec {
                family: "set_value".into(),
                payload: PayloadSpec::Value(ValueType::Float),
            },
            CommandSpec {
                family: "reset".into(),
                payload: PayloadSpec::None,
            },
            CommandSpec {
                family: "set_range".into(),
                payload: PayloadSpec::Fields {
                    fields: vec![
                        ("min".into(), ValueType::Float),
                        ("max".into(), ValueType::Float),
                    ],
                    required: vec!["min".into(), "max".into()],
                },
            },
        ]
    }
}
```

When using `#[derive(WidgetCommand)]`, the derive macro also
generates `command_specs()` on the enum type, so you can delegate:

```rust
fn command_specs(&self) -> Vec<CommandSpec> {
    GaugeCommand::command_specs()
}
```

### Testing

Test the widget crate and wrapper crate independently:

**Widget crate:** Standard iced widget testing. Construct the
widget, verify it doesn't panic with various inputs.

**Wrapper crate:** Use plushie's test helpers:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use plushie_widget_sdk::testing::*;
    use serde_json::json;

    #[test]
    fn renders_with_props() {
        let widget = GaugeWidget;
        let node = node_with_props("g1", "gauge", json!({
            "value": 0.75,
            "color": "#4CAF50"
        }));

        let test = TestEnv::default();
        let ctx = test.render_ctx();

        let _element = widget.render(&node, &ctx);
    }

    #[test]
    fn renders_with_no_props() {
        let widget = GaugeWidget;
        let node = node("g1", "gauge");

        let test = TestEnv::default();
        let ctx = test.render_ctx();

        let _element = widget.render(&node, &ctx); // should use defaults
    }
}
```

### Publishing

Publish both crates. The widget crate is useful to Rust/iced
developers directly. The plushie wrapper crate is used by host SDKs:

```
crates.io:
  my-gauge           the iced widget (Rust developers use this)
  my-gauge-plushie   the plushie wrapper (SDKs reference this)
```

Host SDK authors add the plushie wrapper to their widget list.
The SDK's build system compiles it into the renderer binary
automatically.

---

## Adding a widget to plushie's standard set

If your widget is general-purpose enough to ship with every plushie
installation (like text_input, slider, or canvas), it can be added
to plushie-widget-sdk instead of distributed as a separate crate.

This is a contribution to the plushie project, not the normal
distribution path:

| What | Where |
|------|-------|
| The iced widget (if new to iced) | `plushie-iced` fork |
| The render function | `crates/plushie-widget-sdk/src/widget/` |
| The validate schema | `crates/plushie-widget-sdk/src/widget/validate.rs` |
| Message variants (if new) | `crates/plushie-widget-sdk/src/message.rs` |
| OutgoingEvent constructors | `crates/plushie-widget-sdk/src/protocol/outgoing.rs` |
| Message wiring | `crates/plushie-renderer-lib/src/emitters.rs` |
| Dispatch table entry | `crates/plushie-widget-sdk/src/widget/render.rs` |

The plushie-iced fork stays close to upstream iced. Only add to the
fork for: new accessible roles, Widget trait extensions, or bug
fixes not yet upstream. plushie-specific code (prop parsing, event
emission, validation) belongs in plushie-widget-sdk.

## Further reading

- [PlushieWidget](../crates/plushie-widget-sdk/src/registry.rs) trait docs for
  building application-specific widgets (simpler, no iced Widget trait)
- [Widget Development](widget-development.md) for the decision
  framework
- iced widget examples in the
  [iced repository](https://github.com/iced-rs/iced)
- plushie-widget-sdk rustdocs (`cargo doc --open` in the plushie-rust workspace)
