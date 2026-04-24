# Testing

The plushie test harness lives in `plushie::test`. A `TestSession<A>`
drives an app's MVU loop in-process without a GPU, without a window
server, and without the wire protocol. Interactions, assertions, and
queries run through the normalized view tree that the real renderer
would receive, so tests catch the same ID, role, and prop issues the
production runtime sees. Tests are plain `#[test]` functions; run
them with `cargo test`.

## Starting a session

```rust
use plushie::prelude::*;
use plushie::test::TestSession;

let mut session = TestSession::<Counter>::start();
session.click("inc");
assert_eq!(session.model().count, 1);
```

`TestSession::<A>::start()` calls `A::init()`, renders the initial
view, and drains any async work the init command produced. The
session owns the model, the normalized tree, composite-widget state,
an effect-stub table, and a subscription manager. `model()` returns
a borrow of the current model; `model_mut()` returns a mutable
borrow for tests that reach past the update path.

`reset()` re-runs `init()` and discards everything the session
accumulated. `rerender()` rebuilds the tree from the current model
without dispatching an event, which is the right hook when a test
mutates model state through `model_mut()` and needs the tree to
catch up before the next assertion.

Integration tests that target the built crate live under `tests/` at
the crate root; unit tests that want access to private items stay in
`#[cfg(test)]` modules inside `src/`.

## Selectors

Every interaction and query accepts `impl Into<Selector>`. Bare
`&str` becomes an ID selector; richer forms come from the
`Selector` constructors in `plushie_core::selector`.

| Constructor | Signature | Matches |
|---|---|---|
| `Selector::id` | `(id: &str) -> Self` | Widget ID; splits `window#id` form |
| `Selector::id_in_window` | `(id: &str, window: &str) -> Self` | ID scoped to a specific window |
| `Selector::text` | `(text: &str) -> Self` | `content`, `label`, `value`, or `placeholder` |
| `Selector::role` | `(role: &str) -> Self` | Accessibility role |
| `Selector::label` | `(label: &str) -> Self` | Accessibility label |
| `Selector::focused` | `() -> Self` | Widget with keyboard focus |

```rust
use plushie_core::Selector;

session.click("save");                          // by ID
session.click(Selector::id("form/save"));       // scoped path
session.click(Selector::id("main#save"));       // window-qualified
session.click(Selector::role("button"));        // by role
session.click(Selector::label("Save document")); // by a11y label
```

ID matching accepts a bare local name, a trailing scoped path
(`"form/save"` matches `"main#form/save"`), or a fully qualified
`window#scope/id`. When a selector is ambiguous the first match in
tree order wins.

## Interactions

All interaction methods resolve the selector and synthesize the
widget event. They panic if the selector matches nothing in the
current tree, after listing the available IDs to aid debugging. Use
`find` when a non-panicking lookup is needed.

| Method | Signature | Event |
|---|---|---|
| `click` | `(selector)` | `WidgetEvent` with `EventType::Click` |
| `type_text` | `(selector, text: &str)` | `EventType::Input` |
| `toggle` | `(selector)` | `EventType::Toggle`, auto-flips current value |
| `set_toggle` | `(selector, checked: bool)` | `EventType::Toggle` with explicit value |
| `select` | `(selector, value: &str)` | `EventType::Select` |
| `submit` | `(selector)` | `EventType::Submit` with current `value` prop |
| `submit_with` | `(selector, text: &str)` | `EventType::Submit` with explicit text |
| `slide` | `(selector, value: f64)` | `EventType::Slide` |
| `paste` | `(selector, text: &str)` | `EventType::Paste` |
| `scroll` | `(selector, dx: f32, dy: f32)` | `EventType::Scroll` |
| `sort` | `(selector, column: &str, dir: SortDir)` | `EventType::Sort` |
| `pane_focus_cycle` | `(selector)` | `EventType::PaneFocusCycle` |

Keyboard input takes `impl Into<KeyPress>`. Combo strings
(`"Ctrl+s"`, `"Shift + Left_Arrow"`), typed `Key` enums, and
`(Key, KeyModifiers)` tuples all work.

| Method | Signature | Description |
|---|---|---|
| `press` | `(key: impl Into<KeyPress>)` | Key down, no release |
| `release` | `(key: impl Into<KeyPress>)` | Key up |
| `type_key` | `(key: impl Into<KeyPress>)` | Press then release |

`dispatch` is the escape hatch: it accepts a fully-formed `Event`
and runs it through the widget interception layer, update, command
execution, async drain, and re-render. Most tests never need it.

## Canvas interactions

Canvas widgets receive pointer events in their own coordinate space.
`button` accepts `"left"`, `"right"`, `"middle"`, or a typed
`MouseButton`. The touch variants carry a finger ID so multi-touch
gestures stay disambiguated.

| Method | Signature | Description |
|---|---|---|
| `canvas_press` | `(selector, x: f32, y: f32, button: impl Into<MouseButton>)` | Pointer press at `(x, y)` |
| `canvas_release` | `(selector, x: f32, y: f32, button: impl Into<MouseButton>)` | Pointer release |
| `canvas_move` | `(selector, x: f32, y: f32)` | Pointer move |
| `canvas_touch_press` | `(selector, x: f32, y: f32, finger: u64)` | Touch press |
| `canvas_touch_release` | `(selector, x: f32, y: f32, finger: u64)` | Touch release |
| `canvas_touch_move` | `(selector, x: f32, y: f32, finger: u64)` | Touch move |

## Finding elements

`find` and `find_all` return `Option<Element>` and `Vec<Element>`
respectively. Neither panics, so they are the correct tool for
conditional assertions.

| Method | Signature | Description |
|---|---|---|
| `find` | `(selector) -> Option<Element<'_>>` | First match |
| `find_all` | `(selector) -> Vec<Element<'_>>` | All matches in tree order |
| `find_focused` | `() -> Option<Element<'_>>` | Currently focused widget |
| `text_content` | `(selector) -> Option<String>` | Visible text of a match |
| `prop_str` | `(selector, key: &str) -> Option<String>` | String prop |
| `prop` | `(selector, key: &str) -> Option<Value>` | Raw JSON prop |

`Element<'a>` wraps a `TreeNode` with typed accessors:

| Method | Returns | Description |
|---|---|---|
| `id` | `&str` | Fully qualified ID |
| `widget_type` | `&str` | Type name (e.g. `"button"`) |
| `children` | `Vec<Element<'a>>` | Direct children |
| `text` | `Option<&'a str>` | Visible text |
| `prop_str` | `Option<&'a str>` | String prop |
| `prop_f32` | `Option<f32>` | Numeric prop |
| `prop_bool` | `Option<bool>` | Bool prop |
| `a11y` | `Option<Value>` | Raw a11y object |
| `inferred_role` | `String` | Explicit a11y role or widget-type fallback |
| `is_focused` | `bool` | Focus state from the tree |
| `is_disabled` | `bool` | Disabled state from the tree |

## Assertions

All assertions panic on mismatch with a descriptive message. Most
accept `impl Into<Selector>`.

| Method | Signature | Description |
|---|---|---|
| `assert_exists` | `(selector)` | Selector matches at least one widget |
| `assert_not_exists` | `(selector)` | Selector matches nothing |
| `assert_text` | `(selector, expected: &str)` | Visible text equals `expected` |
| `assert_prop` | `(selector, key: &str, expected: &Value)` | Named prop equals value |
| `assert_role` | `(selector, expected: &str)` | `Element::inferred_role` equals `expected` |
| `assert_a11y` | `(selector, expected: &Value)` | Resolved a11y contains every key in `expected` |
| `assert_no_diagnostics` | `()` | No normalization warnings accumulated |

`assert_model` is available when `A::Model: PartialEq + Debug`:

```rust
session.assert_model(&Counter { count: 3 });
```

`assert_a11y` runs against `resolved_a11y`, which layers widget-SDK
inference (e.g. `placeholder` falling through to `description`,
`alt` falling through to `label`) on top of the explicit `a11y`
prop. The resolved value matches what AccessKit receives from the
renderer. See [accessibility.md](accessibility.md) for the inference
rules.

```rust
session.assert_a11y("email", &json!({"required": true}));
```

## Async and effects

Async work queued by `Command::task`, `Command::stream`, and
`Command::send_after` runs automatically after every interaction.
Completed task results are stored keyed by tag, and the harness
drives them to completion using a current-thread tokio runtime. A
panicking task resolves to an `AsyncEvent` error payload rather than
unwinding the harness.

| Method | Signature | Description |
|---|---|---|
| `run_pending_async` | `()` | Drain queued async/stream tasks |
| `cancel_pending` | `(tag: &str)` | Drop queued tasks with a tag before they run |
| `pending_async_count` | `() -> usize` | Number of tasks still queued |
| `await_async` | `(tag: &str, timeout: Duration) -> Option<&Result<Value, Value>>` | Lookup result by tag |

`run_pending_async` is called automatically during `start` and after
every `dispatch`. Tests invoke it explicitly only when they queue
tasks, mutate the model, and need to trigger the drain without
dispatching an event.

Platform effects (file dialogs, clipboard, notifications) are
intercepted by the stub table, keyed by `EffectKind`. Ops without a
stub are recorded on the `issued_ops` buffer so tests can still
assert what the app requested.

| Method | Signature | Description |
|---|---|---|
| `register_effect_stub` | `(kind: EffectKind, response: EffectResult)` | Install a stub for an effect kind |
| `unregister_effect_stub` | `(kind: EffectKind)` | Remove a stub |
| `issued_ops` | `() -> &[RendererOp]` | Renderer ops the app requested |
| `drain_issued_ops` | `() -> Vec<RendererOp>` | Take ownership and clear the buffer |

```rust
use plushie::event::EffectResult;

session.register_effect_stub(
    EffectKind::FileOpen,
    EffectResult::FileOpened { path: "/tmp/test.txt".into() },
);
```

Stubs apply to every effect of their kind, regardless of tag. See
[commands.md](commands.md) for the full effect catalog.

## Strict diagnostics

Sessions start in strict mode: every `duplicate_id`, reserved-
character, or dispatch-depth warning accumulates, and `Drop` panics
the test if any are still present at teardown. Opt out for tests
that exercise diagnostic paths directly.

| Method | Signature | Description |
|---|---|---|
| `allow_diagnostics` | `() -> Self` | Disable the drop-time panic (chain from `start`) |
| `diagnostics` | `() -> Vec<String>` | Rendered diagnostic strings |
| `typed_diagnostics` | `() -> Vec<Diagnostic>` | Structured variants for variant-shape assertions |
| `has_diagnostic` | `(kind: DiagnosticKind) -> bool` | Any diagnostic of a given kind |
| `drain_diagnostics` | `() -> Vec<String>` | Take ownership and clear |

```rust
let mut session = TestSession::<MyApp>::start().allow_diagnostics();
```

## Advancing animation

The harness does not run a frame clock automatically. Tests that
touch animated props drive frames explicitly.

| Method | Signature | Description |
|---|---|---|
| `advance_frame` | `(timestamp: u64)` | Dispatch an `AnimationFrame` at the given timestamp |
| `skip_transitions` | `()` | Advance by 10s to settle every timed transition and spring |

## Subscriptions

`App::subscribe` runs after every dispatch, but tests can call the
lifecycle hooks directly to verify that a model change produced the
expected `Subscribe`/`Unsubscribe` ops.

| Method | Signature | Description |
|---|---|---|
| `advance_subscriptions` | `()` | Diff the current subscription set and record ops |
| `active_subscriptions` | `() -> &[Subscription]` | Subscriptions live as of the last advance |
| `last_subscription_ops` | `() -> &[SubOp]` | Ops from the most recent diff |

## Accessing model and tree

| Method | Returns | Description |
|---|---|---|
| `model` | `&A::Model` | Current model |
| `model_mut` | `&mut A::Model` | Mutable model (requires a later `rerender`) |
| `tree` | `&TreeNode` | Normalized view tree |
| `tree_hash` | `String` | Canonical SHA-256 hex of the tree |
| `tree_snapshot` | `String` | Pretty-printed JSON of the tree |

`assert_tree_hash(&session, name, dir)` is a free function that
compares `tree_hash()` against a golden file under `dir`. The
directory path is resolved relative to `CARGO_MANIFEST_DIR`, so it
refers to the same on-disk location regardless of where `cargo test`
was invoked from. Set `PLUSHIE_UPDATE_SNAPSHOTS=1` to rewrite the
stored hash when the UI intentionally changes.

```rust
use plushie::test::{TestSession, assert_tree_hash};

let mut session = TestSession::<Counter>::start();
session.click("inc");
assert_tree_hash(&session, "counter_after_inc", "tests/golden");
```

## Multi-window scopes

`session.window(window_id)` returns a `WindowScope` that routes
interactions to a specific window. The scope also exposes synthetic
lifecycle events.

```rust
let mut session = TestSession::<MyApp>::start();
session.window("modal").opened();
session.window("modal").click("close");
session.window("modal").closed();
```

| Method | Signature | Description |
|---|---|---|
| `click` | `(selector)` | Click scoped to this window |
| `type_text` | `(selector, text: &str)` | Text input scoped to this window |
| `opened` | `()` | Synthesize `WindowEvent::Opened` |
| `closed` | `()` | Synthesize `CloseRequested` then `Closed` |
| `resized` | `(w: f32, h: f32)` | Synthesize `Resized` with new dimensions |
| `focused` | `()` | Synthesize `Focused` |
| `unfocused` | `()` | Synthesize `Unfocused` |

## Widget test harness

`WidgetTestSession<W>` hosts a single composite widget inside an
auto-generated `window > column` app and records every event the
widget emits. It is the right tool for widget-authoring tests that
don't need a full app.

```rust
use plushie::test::WidgetTestSession;

let mut session = WidgetTestSession::<StarRating>::start("stars");
session.click("star_3");
let (family, value) = session.last_event().expect("widget emitted");
assert_eq!(family, "select");
```

`start_with_props(id, props)` seeds the widget with initial props.
`events()`, `last_event()`, and `drain_events()` read the recording.
`session()` and `session_mut()` return the underlying `TestSession`
for the full interaction and assertion surface.

## Integration tests vs unit tests

Integration tests live under `crates/<crate>/tests/`. Each file
compiles as its own binary, so it can only call public items of the
crate under test. Use integration tests for anything that exercises
the full MVU cycle: interactions, assertions, async, subscriptions,
golden trees.

Unit tests stay inside `src/` as `#[cfg(test)] mod tests { ... }`
blocks. Reach for them when a test needs access to a private helper
or a private type; everything else should live under `tests/` for
faster compile times and cleaner isolation.

## See also

- [Events](events.md) - event taxonomy and matching
- [Commands](commands.md) - command types, async, effect catalog
- [Subscriptions](subscriptions.md) - subscription diffing and ops
- [Accessibility](accessibility.md) - role/label inference and resolved a11y
- [Built-in widgets](built-in-widgets.md) - widget constructors and props
