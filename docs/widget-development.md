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

See the `PlushieWidget` trait docs in `crates/plushie-widget-sdk/src/registry.rs`.

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
