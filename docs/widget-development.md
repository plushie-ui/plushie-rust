# Widget Development

Three ways to build custom widgets for plushie, each for a different
situation.

## Canvas interactive shapes

Draw shapes from JSON. The renderer handles hit testing, hover
styles, keyboard navigation, drag, and accessibility locally.
Zero Rust code required.

Use this for: charts, diagrams, custom buttons, toggles, radio
groups, toolbars, and any widget where the visual is custom but
the interaction pattern is standard (click, hover, drag).

See the [interactive canvas shapes](protocol.md#interactive-canvas-shapes)
section in the protocol docs.

## Custom widgets

A Rust crate that implements `PlushieWidget` from `plushie-widget-sdk`.
Your host SDK handles compilation and binary generation. You write
the widget logic.

Use this for: application-specific widgets that need native
rendering performance, complex state management, or access to iced's
widget library beyond what canvas provides. Most custom Rust widgets
use this path.

The rest of this section is a full, copy-pasteable worked example.

### Cargo.toml

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2024"

[dependencies]
plushie = { version = "0.6", features = ["direct"] }
plushie-widget-sdk = "0.6"
serde_json = "1"
```

Widgets depend on `plushie-widget-sdk`, not `iced` directly. When
a widget needs iced types beyond the prelude, use
`plushie_widget_sdk::iced::*` so the iced version stays pinned to
the renderer's version. See the
[patch.crates-io guidance](#patchcrates-io-guidance) below.

### main.rs

```rust
use plushie_widget_sdk::prelude::*;
use plushie_widget_sdk::app::PlushieAppBuilder;
use plushie_widget_sdk::widget::widget_set::iced_widget_set;

#[derive(PlushieWidget)]
#[plushie_widget(type_name = "my_gauge")]
struct MyGauge;

impl<R: PlushieRenderer> PlushieWidgetRender<R> for MyGauge {
    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> PlushieElement<'a, R> {
        let value = node.prop_f32("value").unwrap_or(0.0);
        let color = ctx.theme.palette().primary.base.color;
        container(text(format!("{value:.0}%")).color(color))
            .padding(8)
            .into()
    }
}

fn main() -> iced::Result {
    plushie::run(
        PlushieAppBuilder::new()
            .widget_set(&iced_widget_set())
            .widget(MyGauge),
    )
}
```

The `#[derive(PlushieWidget)]` with `type_name` generates
`type_names` and `fresh_for_session`. `PlushieWidgetRender` holds
just the `render` body. `PlushieElement<'a, R>` is shorthand for
`iced::Element<'a, Message, iced::Theme, R>`.

Stateful widgets implement `PlushieWidget` directly so they can
override `prepare`, `handle_message`, `cleanup_stale`, etc. The
[Core Widget Guide](core-widget-guide.md) has a worked stateful
example.

### A test

```rust
use plushie_widget_sdk::prelude::*;
use plushie_widget_sdk::testing::*;
use serde_json::json;

#[test]
fn gauge_renders() {
    let widget = MyGauge;
    let node = node_with_props("g1", "my_gauge", json!({ "value": 50.0 }));
    let test = TestEnv::default();
    let ctx = test.render_ctx();
    let _element: PlushieElement<'_> =
        <MyGauge as PlushieWidget<iced::Renderer>>::render(&widget, &node, &ctx);
}
```

See the "[Testing your widget](#testing-your-widget)" section below
for helpers covering the full lifecycle.

### Troubleshooting

Common compile errors and fixes:

- **"the trait `PlushieWidget` is not implemented for your type"**: you
  derived `PlushieWidget` but forgot the `PlushieWidgetRender` impl.
  Either add `impl<R: PlushieRenderer> PlushieWidgetRender<R> for X`
  with a `render` body, or drop the derive and implement
  `PlushieWidget` directly.

- **"cannot find derive macro `PlushieWidget`"**: the prelude re-
  exports `PlushieWidget` as both the trait and the derive macro.
  If you imported `use plushie_widget_sdk::registry::PlushieWidget`
  by itself, add `use plushie_widget_sdk::PlushieWidget` too, or
  stick with `use plushie_widget_sdk::prelude::*`.

- **"wrong number of type arguments"** on `Element`: the trait
  method returns `Element<'a, Message, Theme, R>`, four
  parameters. Use `PlushieElement<'a, R>` from the prelude instead.

- **"two versions of `iced` in the dependency graph"**: you added a
  direct `iced` dependency. Delete it; use
  `plushie_widget_sdk::iced::*` for advanced types.

- **widget registers but doesn't render**: check the `type_name` in
  your derive attribute matches the `type` field in the host's
  widget call. Collisions with a built-in type name panic at
  registration time now; `.widget_override()` opts in to replacement.

### Adding a built-in widget to the workspace

Internal contributors adding a widget to the iced set follow the
same external-author flow plus a few workspace-specific deltas:

1. **File placement.** One widget per file in
   `crates/plushie-widget-sdk/src/widget/`. Follow the naming
   `foo_widget.rs` used by the existing 38 widgets.
2. **Registration.** Add a `register` call in
   `crates/plushie-widget-sdk/src/widget/widget_set.rs` so the
   widget ships with the iced set.
3. **Prop validation schema.** Add an entry to
   `crates/plushie-widget-sdk/src/validate.rs` so the renderer can
   warn on invalid prop shapes.
4. **Role auto-population.** Add a row to the a11y normalizer's
   role table if the widget defaults to a specific role
   (button, checkbox, etc.).
5. **Automation fallback.** If the widget surfaces a label, update
   `crates/plushie-widget-sdk/src/a11y.rs` so screen-reader
   inspection picks up the right text.
6. **Inline tests.** Widget files historically carry their tests
   in-module via `#[cfg(test)] mod tests { ... }`.
7. **Protocol doc.** Add the widget's prop table to
   `docs/protocol.md` under the widget reference.

See any recent built-in widget file for a template.

## Reusable iced widgets

An iced widget that works directly in Rust applications AND across
every plushie SDK. You build the widget once as a standard iced
widget, then add a thin `PlushieWidget` wrapper for plushie
compatibility.

Use this for: widgets you want to share across the ecosystem,
a chart library, a date picker, a color wheel. Rust developers
use the widget directly. Elixir, Gleam, and other SDK users get
it through plushie without any per-language widget code.

See the [Core Widget Guide](core-widget-guide.md).

## Decision framework

| Need | Approach | Rust needed? |
|------|----------|-------------|
| Custom visuals, standard interaction | Canvas interactive shapes | no |
| Custom visuals + text editing | Compose canvas + `text_input` | no |
| Custom visuals + scrolling | Compose canvas + `scrollable` | no |
| Custom visuals + dropdown | Compose canvas + `overlay` | no |
| Application-specific native widget | `PlushieWidget` | yes (basic) |
| Reusable widget (Rust + all SDKs) | iced widget + `PlushieWidget` wrapper | yes (intermediate) |
| Maximum rendering performance | `PlushieWidget` | yes |

**Start with canvas.** Most custom widgets can be built from
canvas interactive shapes composed with built-in widgets. Move to
a custom widget only when canvas can't do what you need.

## Testing your widget

`plushie_widget_sdk::testing` is the canonical test harness for
widget authors. The pieces:

- `TestEnv::default()` builds a ready-to-use environment with the
  iced widget set registered. Override fields via struct-update
  syntax to change the theme, text defaults, or registry.
- `node`, `node_with_props`, `node_with_children`, and
  `node_with_props_and_children` construct test tree nodes.
- `TestEnv::render_ctx()` returns a `RenderCtx` bound to the
  environment's state so you can drive `widget.render(...)` in a
  test.
- `TestEnv::prepare_and_render(&mut widget, &node, window_id)`
  runs the `prepare` + `render` phases together, which is almost
  always what a stateful widget's test needs.
- `TestEnv::handle_message_events(&mut widget, &msg)` runs
  `handle_message` and returns the emitted events flattened to a
  `Vec<OutgoingEvent>`.

`crates/plushie-widget-sdk/tests/doc_examples.rs` holds the
canonical stateless templates. A matching
`doc_examples_stateful.rs` drives a stateful widget through the
full `prepare -> render -> handle_message -> handle_widget_op ->
cleanup_stale` lifecycle; copy it as the starting point for your
own stateful widget tests.

## Development loop

**Rust apps.** The `plushie` crate's `direct` feature renders
in-process. `cargo run` (or `cargo watch -x run`) gives a full
rebuild-and-relaunch cycle in seconds for a small widget. Unit
tests with `cargo test` or `cargo watch -x test` drive the render
pipeline without needing a display server.

**Host SDK apps (Elixir, Gleam, Python, Ruby, TypeScript).** The
renderer binary is rebuilt by the host SDK's build system and
relaunched each time. The exact loop differs per SDK (most follow
the renderer's `mix`, `gleam run`, or language-native build tool),
but the shape is the same: make a change, let the host SDK rebuild
the renderer, restart the host process. Iteration is slower than
direct mode but the widget code itself is identical.

**Test loops.** For pure widget tests
(`cargo test -p my-widget-crate`), `cargo watch -x 'test -p my-widget-crate'`
keeps the loop fast. Tests run without requiring the full app to
boot.

## Rust toolchain

The plushie workspace pins its toolchain in `rust-toolchain.toml`
(currently 1.92). Widget crates that depend on `plushie-widget-sdk`
inherit that MSRV. Match the workspace toolchain when:

- Building against a local `plushie-iced` checkout (ABI safety).
- Running `cargo clippy` in CI (lint behaviour tracks the pin).

Nightly-only Rust features in widget code will not compile on the
pinned stable release, so avoid them unless you are ready to bump
the workspace too.

## `[patch.crates-io]` guidance

The plushie workspace root's `Cargo.toml` has a `[patch.crates-io]`
section pointing at a sibling `plushie-iced` checkout during
development. Widget crates that consume `plushie-widget-sdk`
should:

- Depend on `plushie-iced` (not upstream `iced`) when a direct iced
  dep is unavoidable.
- Prefer `plushie_widget_sdk::iced::*` for advanced iced types
  (`canvas::Path`, `advanced::Layout`). It is the re-export of the
  exact iced version the renderer uses.
- Not add a `[patch.crates-io]` entry of their own unless they are
  actively co-developing the fork. Patches applied only in a
  consumer crate do not propagate to its dependencies.

## Accessibility for custom widgets

### Focus-visible pattern

Custom focusable widgets should mirror the focus-visible pattern the
built-in widgets inherit from the iced fork: a focus ring is visible
whenever focus arrived via keyboard, and hidden when focus arrived via
mouse press. The two rules that keep this correct:

1. On any mouse press inside the widget, clear any focus-visible
   state you track. Mouse interaction produces focus without a ring.
2. On any keyboard key down that changes focus (Tab, arrow keys for
   composite widgets), set focus-visible. Keyboard users need the
   ring.

Canvas interactive elements get this for free via
`canvas::program::ProgramState`. If you write a widget outside the
canvas system, follow the same pattern: store a `focus_visible: bool`
in your widget state, clear it on mouse press, restore it on keyboard
press, and paint a focus indicator only when it's true.

### Dynamic a11y state (`busy`)

Widgets that go through a rapid value-change phase (a slider mid-drag,
a live-updating progress bar, a realtime chart) should toggle
`a11y.busy` during the change and clear it when settled. Screen
readers suppress announcements while busy is set, so assistive tech
doesn't chatter through an intermediate value.

The built-in slider sets busy automatically during drag, which is
fork-level behaviour. Custom drag-driven widgets should mirror it:

- Set `a11y.busy = true` when the drag begins (or the first value
  event arrives for a realtime widget).
- Clear `a11y.busy` (absent / `null`) when the drag ends (or the
  stream goes idle).

Authoring this from the widget builder is enough. The `A11yOverride`
layer propagates the busy flag to the AccessKit node.
