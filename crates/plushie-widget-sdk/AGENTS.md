# plushie-widget-sdk

The public SDK for writing custom Plushie widgets in Rust. Widget
authors depend on this crate to implement the `PlushieWidget` trait.
Host SDKs (Elixir, Gleam, Python, Ruby, TypeScript) auto-detect native
widgets and compile them into the renderer binary.

Also contains all built-in widget implementations that ship with the
renderer.

## Quick reference

```
cargo test -p plushie-widget-sdk             # run all tests
cargo clippy -p plushie-widget-sdk           # lint
cargo doc -p plushie-widget-sdk --open       # rustdocs
```

## Project layout

```
crates/plushie-widget-sdk/
  src/
    lib.rs               # public re-exports and module guide
    a11y.rs              # accessibility types and wrapping
    shared_state.rs      # SharedState, style override caching, hash utilities
    validate.rs          # prop validation schemas
    registry.rs          # PlushieWidget trait, WidgetRegistry, WidgetSet
    render_ctx.rs        # RenderCtx for widget render dispatch
    canvas_engine.rs     # reusable canvas composition engine
    app.rs               # PlushieAppBuilder for registering widgets
    prop_helpers.rs      # public prop extraction helpers for widget authors
    prelude.rs           # common re-exports for widget authors
    testing.rs           # test factory helpers for widget authors
    message.rs           # Message enum, keyboard/mouse serialization helpers
    runtime.rs           # renderer-internal re-exports (Message, ThemeChrome, ...)
    theming.rs           # theme resolution, custom palette parsing, hex colors
    image_registry.rs    # in-memory image handle storage
    fonts.rs             # loaded-font registry: tracks family names registered with iced
    iced_convert.rs      # iced type conversions
    svg_guard.rs         # SVG decode guard: bounded pre-parse with a wall-clock timeout
    animation/           # renderer-side animation engine
      mod.rs easing.rs timed.rs spring.rs color.rs ghost.rs
    widget/              # tree node -> iced widget rendering
      mod.rs             # module declarations, re-exports, render() entry point
      widget_set.rs      # IcedWidgetSet, iced_widget_set()
      render.rs          # main render dispatch: maps TreeNode to iced Element
      helpers.rs         # internal prop/style parsing
      overlay.rs         # iced overlay widget implementation
      canvas/            # canvas infrastructure
        mod.rs types.rs program.rs shapes.rs interaction.rs validation.rs tests.rs
      *_widget.rs        # one file per built-in widget (one PlushieWidget impl each)
    protocol/            # extended protocol types (re-exports plushie-core)
      mod.rs outgoing_ext.rs
  tests/
    doc_examples.rs          # ensures doc code examples compile
    doc_examples_stateful.rs # end-to-end template for testing a stateful widget lifecycle
    builtin_type_names.rs    # drift check: BUILTIN_TYPE_NAMES const vs actual widget set
```

The pure renderer state engine (`Core`), retained UI tree, and wire
codec live in the sibling `plushie-renderer-engine` crate. Widget
authors do not depend on it; the renderer crates do.

## PlushieWidget trait

Required (three methods):

1. `type_names()` - returns the widget type strings this impl handles
2. `render()` - maps a TreeNode to an `iced::Element` for rendering
3. `fresh_for_session()` - produces a fresh instance for
   `--max-sessions > 1`

Optional lifecycle hooks:

- `namespace()` - prefix for type matching (e.g. `"myapp"`)
- `init()` - called when the Settings message arrives; receives
  per-namespace config
- `prepare()` - runs during `apply()` (mutable context) to populate
  state that `render()` reads immutably
- `handle_message()` - processes widget-specific iced Messages
- `handle_widget_op()` - handles widget ops (focus, scroll, etc.)
- `cleanup_stale()` - teardown for widget nodes no longer present in
  the tree (receives the set of live `(window_id, node_id)` keys)
- `infer_a11y()` - auto-infer accessibility properties from node props
- `event_specs()` - declare event families this widget can emit
- `command_specs()` - declare commands this widget accepts

See the trait documentation in `src/registry.rs` for the full API.

## Key patterns

**prepare / render split.** Stateful widgets (text_editor, markdown)
need mutable state across renders, but iced's `view()` is `&self`.
`prepare()` runs during `apply()` (mutable) to populate factory-owned
state. `render()` reads it immutably via `&'a self`. No `RefCell`.

**Canvas layer caching.** Per-layer `canvas::Cache`. `prepare()` hashes
each layer's shape JSON; only changed layers clear the cache. Layers
with active hover/pressed interaction bypass the cache for style
overrides. Background drawn uncached. Tooltip and focus ring overlays
drawn uncached on top.

**Canvas interactive shapes.** Shapes with an `interactive` field get
renderer-local hit testing, hover/pressed styles, keyboard navigation,
drag, tooltips, and accessibility. Zero wire round-trip for visual
feedback. Groups (`"type": "group"`) are the composability mechanism.

**Canvas composition over containment.** Canvas handles custom visuals
and interaction primitives but does NOT replace iced's widget system.
Complex components compose canvas with built-in widgets (e.g.
stack(canvas + text_input) for custom inputs).

**A11y auto-inference.** Image/SVG `alt` props flow into accessible
labels. Text input/editor `placeholder` flows into accessible
descriptions. Explicit a11y props override inferred values.

**StyleMap preset base.** A `"base"` field names a preset to extend.
The parsed style starts from the preset's defaults, then remaining
fields override individual properties.

**Tree walker.** Per-frame passes over the retained tree (widget
prepare, animation-descriptor scan) run as `TreeTransform` impls
driven by `plushie_core::tree_walk::walk`. `WidgetRegistry::prepare_and_scan`
composes the prepare and scan transforms through a single depth-first
traversal. Add new per-node passes as additional `TreeTransform`
impls and thread them through the same walker rather than introducing
a fresh recursion. See the `plushie_core::tree_walk` rustdoc for the
pattern and a worked example.

## Writing custom widgets

1. Create a Rust crate that depends on `plushie-widget-sdk`.
2. Import `plushie_widget_sdk::prelude::*`.
3. Implement the `PlushieWidget` trait.
4. For iced types not in the prelude (e.g. `canvas::Path`), use
   `plushie_widget_sdk::iced::*` instead of adding a direct `iced`
   dependency. This avoids version conflicts.

The `plushie_widget_sdk::testing` module provides `node()`,
`node_with_props()`, `node_with_children()`, and render context
builders so widget tests don't import half the crate.

Widgets automatically get accessibility support through composition.
The renderer's a11y layer wraps all widget output with `A11yOverride`,
so widget authors don't need to implement accesskit integration
themselves.

## Related crates

- plushie - Rust app SDK (direct + wire rendering modes)
- plushie-core - core types, wire protocol (no iced dependency)
- plushie-renderer-engine - pure state engine, retained tree, wire codec
- plushie-renderer-lib - shared renderer logic
- plushie-renderer - native renderer binary
