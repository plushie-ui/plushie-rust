# plushie (Rust SDK)

The Rust SDK for building plushie desktop apps. Same role as the
plushie packages in Elixir, TypeScript, Python, Ruby, and Gleam.

## Quick reference

```
cargo run -p plushie --example counter    # run counter example
cargo run -p plushie --example todo       # run todo example
cargo test -p plushie                     # run all tests
cargo clippy -p plushie -- -D warnings   # lint
```

## Project layout

```
plushie/
  src/
    lib.rs              App trait, View type, run()/run_wire()/run_connect() entry points
    event.rs            Event enum, WidgetMatch typed matching, typed pointer/key/scroll data
    command.rs          Command enum, builder methods, re-exports op types from plushie-core
    subscription.rs     Subscription types with fluent configuration
    types.rs            Re-exports from plushie-core (Color, Padding, Style, etc.) + SDK-specific types
    settings.rs         Re-exports Settings, WindowConfig, ExitReason from plushie-core
    widget.rs           Composite Widget trait + EventResult + WidgetRegistrar
    prelude.rs          Curated re-exports for app developers
    test.rs             TestSession for headless MVU testing
    cli.rs              Zero-config CLI entry point and mode-dispatch for app main()
    error.rs            Error enum covering all SDK-surface failure modes
    derive_support.rs   Helper traits used by widget derive macros (WidgetEvent, WidgetProps)
    selection.rs        Single/multi/range selection for lists and tables
    undo.rs             Function-based undo/redo with bounded size, labels, coalescing
    route.rs            Navigation stack with parameters
    query.rs            Composable query pipeline (filter, search, sort, paginate, group)
    state.rs            Path-based state container with revision tracking and transactions
    automation/         Production-capable automation primitives
      mod.rs            Re-exports Selector from plushie-core, Element
      element.rs        Typed TreeNode wrapper with text/a11y/role accessors
      file.rs           .plushie automation file parser
      runner.rs         Script executor against TestSession
      cli.rs            CLI-dispatch helpers for --plushie-script, replay, and run subcommands
      runner_wire.rs    Windowed automation runner: drives a real renderer subprocess over the wire
    ui/                 View builders
      mod.rs            View type, builder infrastructure, prop helpers
      layout.rs         window, column, row, container, stack, grid, etc.
      display.rs        text, rich_text, space, rule, image, etc. (auto-ID leaf nodes)
      input.rs          text_input, text_editor, checkbox, slider, etc. (ID required)
      interactive.rs    button, pointer_area, sensor, tooltip, etc. (ID required)
      canvas.rs         canvas, layer, group, rect, circle, line, etc.
      table.rs          table builder with columns, sorting, selection, rich cells
      memo.rs           memo() view caching via __memo__ marker node
    animation/          Declarative animation descriptors
      mod.rs            Re-exports Transition, Spring, Sequence, Easing from plushie-core
      tween.rs          SDK-side interpolation: timed (easing) + spring (physics) modes
    runner/             Execution backends
      mod.rs            Feature-gated module declarations
      direct.rs         In-process rendering via iced::daemon (feature = "direct")
      wire.rs           Subprocess rendering via stdin/stdout (feature = "wire")
      bridge.rs         Wire protocol I/O (spawn, framing, codec)
      env.rs            Environment whitelist passed to the wire-mode renderer subprocess
      socket.rs         Socket transport adapter for run_connect() (pre-existing Unix socket)
      wire_discovery.rs Renderer binary discovery for wire mode (env, cargo-plushie, PATH)
      effect_tracker.rs Effect lifecycle tracking (wire IDs, timeouts, one-per-tag)
      queue_sink.rs     In-process event sink for direct mode
      event_bridge.rs   Renderer event -> SDK Event conversion (used by both runners)
    runtime/            Shared event loop internals
      mod.rs            Module declarations, shared view/widget-expand logic
      normalize.rs      Tree normalization (scope prefixing, ID validation)
      tree_diff.rs      LIS-based tree diffing for wire mode patches
      subscriptions.rs  Subscription lifecycle diffing and management
      memo_cache.rs     Memoization cache for __memo__ subtrees (owned by runner, direct + test)
      view_errors.rs    Consecutive-error tracking and frozen-UI overlay for repeated view/update panics
      widget_view_cache.rs  Widget view cache for composite widgets that opt in via Widget::cache_key
      windows.rs        Multi-window lifecycle synchronization
    dev/                Dev-mode tooling (live-reload, in-tree rebuild overlay)
      mod.rs            Module declarations
      dev_overlay.rs    Event-interception glue that wires the overlay into the running app
      overlay.rs        In-tree status bar + detail drawer rendered inside the live app
      watch.rs          File-system watcher that triggers renderer rebuilds and surfaces progress
  tests/
    a11y_test.rs               Accessibility inference and tree shape
    app_test.rs                Full MVU integration tests via TestSession
    async_contract_test.rs     Command::task lifecycle and error delivery
    automation_test.rs         Selector + .plushie automation runner
    automation_replay_windowed.rs  automation::cli::replay against a real renderer subprocess
    command_test.rs            Command construction and matching
    derive_event_test.rs       WidgetEvent and WidgetCommand derive macro tests
    derive_widget_test.rs      WidgetProps derive macro integration tests
    doc_examples.rs            Ensures rustdoc code examples compile
    event_test.rs              Event matching, WidgetMatch, value accessors
    golden_test.rs             Golden-file tree hash / snapshot parity
    multi_window_test.rs       Multi-window open/close, window-qualified IDs
    no_runner_features.rs      Behaviour when neither direct nor wire feature is enabled
    subscription_test.rs       Subscription lifecycle and diffing
    touch_test.rs              Touch / pointer kind dispatch via TestSession
    tree_diff_proptest.rs      Property tests for tree diff -> patch -> apply
    types_test.rs              Property type construction and From impls
    ui_test.rs                 View builder -> TreeNode conversion
    util_test.rs               Selection, UndoStack, Route, Query
    widget_intercept_test.rs   Composite widget event interception
    widget_test.rs             Composite Widget trait behavioral tests
    wire_connect.rs            Integration test for run_connect (socket-based renderer)
    wire_hot_reload.rs         Dev-mode hot-reload: wire runner with a mock renderer
    wire_image_ops.rs          Wire-mode integration tests for image command builders
    wire_load_font.rs          Wire-mode integration tests for typed LoadFont message
    wire_mode.rs               Wire-mode handshake and exit behaviour
  examples/
    counter.rs          Minimal counter (button click handling, layout basics)
    todo.rs             Todo list (text_input, scoped IDs, dynamic lists, filter)
    clock.rs            Timer-driven clock (Subscription::every, Timer event)
    gallery.rs          Widget gallery (common widget types showcase)
    notes.rs            Notes app (Route, UndoStack, Selection working together)
    shortcuts.rs        Keyboard shortcut logger (on_key_press subscription)
    async_fetch.rs      Async command demo (Command::task, loading state)
    rate_plushie.rs     App rating page (composite widgets, form validation)
    color_picker.rs     HSV color picker (composite Widget trait demo)
```

## Architecture

- **Elm architecture**: init() creates model, update() handles events
  and returns commands, view() builds the UI tree.
- **Three modes**: direct (in-process iced, default), wire (subprocess
  renderer via stdin/stdout), and connect (attach to a pre-existing
  Unix socket). Same App API for all three.
- **View builders**: Typed functions that produce TreeNode-based View
  values. Auto-IDs via #[track_caller] for display/layout widgets.
  Interactive widgets require explicit IDs.
- **Event matching**: WidgetMatch enum for typed pattern matching on
  widget events. Pointer events carry PointerPress/Move/Scroll structs,
  keyboard events carry KeyData with typed Key enum, scroll position
  carries ScrollPosition. Full Event enum for mixed event types.
- **Typed core types**: Angle (dual-storage, degrees on wire),
  PointerKind (Mouse/Touch/Pen), CustomTheme (52 shade builders),
  EventType (centralized family mapping), A11y (Option<bool> fields,
  merge method). All in plushie-core, shared with the renderer.
- **TestSession**: Headless MVU testing without rendering. Exercises
  the full init/update/view cycle. Assertions for text, role, a11y,
  model, diagnostics. Touch simulation. Tree hash/snapshot.
- **Tree walker**: `runtime::prepare_tree` composes widget expansion
  and ID normalization through `plushie_core::tree_walk` so both
  passes share a single depth-first traversal. New per-node passes
  (future a11y rewrites, analytics hooks, etc.) land as additional
  `TreeTransform` impls in that module rather than extra recursions.
  See the `plushie_core::tree_walk` rustdoc for the pattern.

## Feature flags

- `direct` (default): In-process rendering. Pulls in plushie-renderer-lib
  and iced.
- `wire`: Subprocess rendering. Pulls in tokio. Can coexist with direct.

## Testing and automation

TestSession provides headless MVU testing with typed inputs.
Interactions accept Selectors (or bare strings) and KeyPress
(combo strings like "Ctrl+s", Key enum, or tuples):

```rust
use plushie::prelude::*;
use plushie::test::TestSession;

let mut session = TestSession::<Counter>::start();
session.click("inc");
session.click(Selector::role("button"));
session.press("Ctrl+s");
session.press(Key::Enter);
session.canvas_press("canvas", 10.0, 20.0, MouseButton::Right);

session.register_effect_stub(
    EffectKind::FileOpen,
    EffectResult::FileOpened { path: "/tmp/test.txt".into() },
);

let elem = session.find(Selector::role("heading")).unwrap();
assert_eq!(elem.inferred_role(), "heading");
session.assert_text("display", "1");
session.assert_role("display", "label");
session.assert_a11y("heading", &serde_json::json!({"role": "heading"}));
session.assert_no_diagnostics();

let all_buttons = session.find_all(Selector::role("button"));
let hash = session.tree_hash();       // stable u64 for regression
let snapshot = session.tree_snapshot(); // pretty JSON for snapshots
```

The automation module (`plushie::automation`) provides production-
capable primitives (Selector, Element, .plushie file parser/runner)
usable outside tests (agents, accessibility harnesses, scripting).

All tests are in the top-level `tests/` directory and exercise the
public API surface.

## Related crates

- plushie-widget-sdk: Widget SDK for widget authors (PlushieWidget trait)
- plushie-renderer-lib: Shared renderer logic
- plushie-renderer: Default renderer binary
