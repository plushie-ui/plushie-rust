# Testing

Plushie ships with a test harness, `TestSession<A>`, that drives an
app's MVU loop in-process. No GPU, no window server, no wire protocol,
no tokio runtime to stand up by hand. Interactions synthesise widget
events, commands execute inline, and every query runs against the
same normalized view tree the real renderer would receive.

This chapter is a practical tour: unit-testing a counter, an
end-to-end run through the to-do app, async and effect tests, and
subscription diffs. For the full API surface see the
[testing reference](../reference/testing.md).

## Setting up

Tests are plain `#[test]` functions. Pull in the prelude for app
construction and `TestSession` from `plushie::test`:

```rust
use plushie::prelude::*;
use plushie::test::TestSession;

#[test]
fn counter_starts_at_zero() {
    let session = TestSession::<Counter>::start();
    assert_eq!(session.model().count, 0);
}
```

`TestSession::<A>::start()` runs `A::init()`, renders the initial
view, and drains any async work that the init command queued. The
session owns the model, the normalized tree, composite-widget state,
an effect-stub table, and a subscription manager. From there, every
interaction and assertion is a method call on `session`.

Unit tests live inside a `#[cfg(test)] mod tests { ... }` block in
`src/`. Reach for them when a test needs a private helper. Anything
that exercises the full MVU cycle belongs under `tests/` at the
crate root, one file per binary. Both shapes use the same harness.

## The counter app

Here is the counter from chapter 2, with a small set of unit tests
wedged into the same file:

```rust
use plushie::prelude::*;

#[derive(Clone)]
struct Counter {
    count: i32,
}

impl App for Counter {
    type Model = Self;

    fn init() -> (Self, Command) {
        (Counter { count: 0 }, Command::none())
    }

    fn update(model: &Self, event: Event) -> (Self, Command) {
        let mut next = model.clone();
        match event.widget_match() {
            Some(Click("inc")) => next.count += 1,
            Some(Click("dec")) => next.count -= 1,
            _ => {}
        }
        (next, Command::none())
    }

    fn view(model: &Self, _widgets: &mut WidgetRegistrar) -> ViewList {
        window("main")
            .title("Counter")
            .child(
                column()
                    .padding(16)
                    .spacing(8.0)
                    .child(text(&format!("Count: {}", model.count)).id("count"))
                    .child(row().spacing(8.0).children([
                        button("inc", "+"),
                        button("dec", "-"),
                    ])),
            )
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie::test::TestSession;

    #[test]
    fn increment_increases_count() {
        let mut session = TestSession::<Counter>::start();
        session.click("inc");
        assert_eq!(session.model().count, 1);
        session.assert_text("count", "Count: 1");
    }

    #[test]
    fn multiple_clicks_accumulate() {
        let mut session = TestSession::<Counter>::start();
        session.click("inc");
        session.click("inc");
        session.click("dec");
        assert_eq!(session.model().count, 1);
    }
}
```

`session.model()` returns a borrow of the current model. If a test
needs to reach past `update` and mutate model state directly, use
`session.model_mut()`. Mutations made that way are not visible in
the tree until the next dispatch or an explicit `session.rerender()`.

## Selectors

Every interaction and query accepts `impl Into<Selector>`. A bare
`&str` is an ID selector; richer forms come from the `Selector`
constructors:

```rust
use plushie::automation::Selector;

session.click("save");                              // by ID
session.click(Selector::id("form/save"));           // scoped path
session.click(Selector::id("main#save"));           // window-qualified
session.click(Selector::text("Save"));              // by visible text
session.click(Selector::role("button"));            // by a11y role
session.click(Selector::label("Save document"));    // by a11y label
session.press_enter_on(Selector::focused());        // whatever has focus
```

ID matching accepts a bare local name, a trailing scoped path
(`"form/save"` matches `"main#form/save"`), or a fully qualified
`window#scope/id`. When a selector matches more than one widget, the
first match in tree order wins. When it matches nothing, the
interaction panics and prints the available IDs so the failure
points at the drift rather than a stack deep inside the harness.

## Interactions

Interactions resolve the selector, synthesise the widget event, run
`update`, execute the returned command, drain async work, and
re-render. By the time a method returns, the next assertion will see
the resulting state.

```rust
session.click("save");
session.type_text("editor", "Hello");
session.submit("search");
session.toggle("auto_save");
session.set_toggle("auto_save", true);
session.select("theme", "dark");
session.slide("volume", 0.75);
session.paste("editor", "pasted content");
session.scroll("log", 0.0, -40.0);
```

Keyboard input takes `impl Into<KeyPress>`. Combo strings, typed
`Key` enums, and `(Key, KeyModifiers)` tuples all work:

```rust
session.press("Ctrl+s");
session.type_key("Escape");
session.type_key(Key::ArrowLeft);
```

`press` sends a key-down without a release; `release` sends the
up; `type_key` does both. For anything more exotic, `session.dispatch(event)`
takes a fully-formed `Event` and runs it through the same pipeline
that the synthesised variants use.

## Assertions

Assertions panic on mismatch with descriptive messages. Most accept
`impl Into<Selector>`:

```rust
session.assert_exists("count");
session.assert_not_exists("error_banner");
session.assert_text("count", "Count: 3");
session.assert_role("save", "button");
session.assert_model(&Counter { count: 3 });
session.assert_a11y("email", &json!({"required": true}));
```

`assert_model` is available when `A::Model: PartialEq + Debug`. For
internal state that is not worth implementing `PartialEq` for, reach
into `session.model()` and use plain `assert_eq!` on the field.

`assert_a11y` matches against `resolved_a11y`, which layers widget
inference (a `placeholder` falling through to `description`, `alt`
falling through to `label`) on top of the explicit `a11y` prop. The
resolved value matches what AccessKit receives from the renderer.
See [accessibility.md](../reference/accessibility.md) for the
inference rules.

When a non-panicking lookup is needed, `session.find(selector)`
returns `Option<Element<'_>>` and `session.find_all(selector)`
returns `Vec<Element<'_>>`. `Element` carries typed accessors
(`widget_type`, `text`, `prop_str`, `prop_f32`, `is_focused`, ...)
for conditional assertions.

## Testing the to-do app end-to-end

The to-do app from chapter 6 is a good end-to-end target: text
input, submit-to-add, scoped IDs for item-level events, and
conditional rendering by filter. A single test exercises most of it:

```rust
use plushie::prelude::*;
use plushie::test::TestSession;

#[test]
fn add_complete_and_filter_flow() {
    let mut session = TestSession::<TodoApp>::start();

    session.type_text("new_todo", "Buy milk");
    session.submit("new_todo");
    session.type_text("new_todo", "Write tests");
    session.submit("new_todo");

    assert_eq!(session.model().todos.len(), 2);
    session.assert_exists("todo_1/toggle");
    session.assert_exists("todo_2/toggle");

    session.set_toggle("todo_2/toggle", true);
    assert!(session.model().todos[0].done);

    session.click("filter_active");
    session.assert_exists("todo_1/label");
    session.assert_not_exists("todo_2/label");

    session.click("filter_done");
    session.assert_not_exists("todo_1/label");
    session.assert_exists("todo_2/label");
}
```

A few things to notice. Scoped item IDs (`"todo_1/toggle"`) flow
through selectors unchanged: whatever path the view produced is
what the test matches. `set_toggle` takes an explicit value; plain
`toggle` auto-flips the current one. After the filter click, the
tree no longer contains the hidden rows, so `assert_not_exists` is
the correct assertion, not `assert_prop("visible", false)`.

Tests that touch focus can assert through `find_focused()`:

```rust
session.submit("new_todo");
let focused = session.find_focused().expect("something should be focused");
assert_eq!(focused.id(), "app/new_todo");
```

The submit branch returned `Command::focus("app/new_todo")`, which
the harness executed before the assertion ran.

## Async and effects

`TestSession` runs async tasks inline on a current-thread tokio
runtime. The drain happens automatically after every interaction,
so a click that kicks off a `Command::task` is visible on the next
assertion without polling.

For platform effects (file dialogs, clipboard, notifications), the
harness has a stub table keyed by `EffectKind`. Install a response
before triggering the effect, and the stub replies instead of the
real platform:

```rust
use plushie::event::{EffectKind, EffectResult};
use plushie::test::TestSession;

#[test]
fn import_loads_a_file() {
    let mut session = TestSession::<Pad>::start();
    session.register_effect_stub(
        EffectKind::FileOpen,
        EffectResult::FileOpened { path: "/tmp/hello.rs".into() },
    );

    session.click("import");
    assert!(session.model().active_path.is_some());
}
```

Stubs apply to every effect of their kind regardless of tag. An
effect kind without a stub is recorded on the `issued_ops` buffer
so the test can still assert what the app requested:

```rust
session.click("copy");
let ops = session.drain_issued_ops();
assert!(ops.iter().any(|op| matches!(op,
    RendererOp::Effect { kind: EffectKind::ClipboardWrite, .. })));
```

When a test needs to wait for a specific tagged task (for example
one queued by a later model mutation, outside the auto-drain path),
`await_async` looks up the result:

```rust
use std::time::Duration;

session.click("fetch");
let result = session
    .await_async("fetch_result", Duration::from_millis(500))
    .expect("fetch_result did not complete");
assert!(result.is_ok());
```

`run_pending_async()` drives the queue explicitly. It's only needed
when a test mutates the model through `model_mut()` and has queued
tasks without dispatching an event.

## Subscriptions and animation

`App::subscribe` runs after every dispatch, but the harness exposes
the lifecycle hooks directly so a test can verify that a model
change produced the expected subscribe and unsubscribe ops:

```rust
use plushie::subscription::SubOp;

#[test]
fn toggling_listen_subscribes_to_key_events() {
    let mut session = TestSession::<Shortcuts>::start();
    assert!(session.active_subscriptions().is_empty());

    session.model_mut().listen_keys = true;
    session.advance_subscriptions();

    let ops = session.last_subscription_ops();
    assert!(matches!(ops[0], SubOp::Subscribe { ref kind, .. } if kind == "on_key_press"));
    assert_eq!(session.active_subscriptions().len(), 1);
}
```

`advance_subscriptions()` diffs the current subscription set
against the previous one, records ops, and makes them available
through `last_subscription_ops()`. No renderer ticks are involved.

Animation needs a frame clock that the harness does not run
automatically. Tests that touch animated props drive frames
explicitly:

```rust
session.click("start_fade");
session.advance_frame(0);
session.advance_frame(500_000);   // microseconds
session.advance_frame(1_000_000);
```

For tests that only care about the post-animation state, jump
straight to it:

```rust
session.click("start_fade");
session.skip_transitions();
session.assert_prop("panel", "opacity", &json!(1.0));
```

`skip_transitions()` advances by ten seconds, which settles every
timed transition and spring the SDK ships.

## Strict diagnostics

Sessions start in strict mode: every `duplicate_id`, reserved-
character, or dispatch-depth warning accumulates on the session.
`Drop` panics the test if any are still present at teardown, so a
regression in ID generation or scope composition fails the first
test that renders the broken view.

Tests that are specifically about diagnostic paths opt out:

```rust
let mut session = TestSession::<MyApp>::start().allow_diagnostics();
// ... exercise the diagnostic path ...
assert!(session.has_diagnostic(DiagnosticKind::DuplicateId));
```

To assert that a particular change did not regress diagnostics,
call `session.assert_no_diagnostics()` in the middle of a test. The
strict-mode drop check catches everything at teardown; the explicit
call catches it at the point the test writer expects.

## Integration tests under `tests/`

Integration tests live under `crates/<crate>/tests/`. Each file
compiles as its own binary, so it can only call public items of the
crate under test. This is the right home for anything that
exercises the full MVU cycle across modules: interactions,
assertions, async, subscriptions, golden trees.

A typical layout:

```
crates/my_app/
  src/
    lib.rs
    pad.rs
  tests/
    pad_flow.rs
    pad_import_export.rs
    common/
      mod.rs
```

`tests/common/mod.rs` is a shared helper module (fixture data,
builder shortcuts). Tests import it with `mod common;`. Unit tests
stay in `#[cfg(test)] mod tests { ... }` blocks inside `src/` for
private helpers; integration tests exercise the public crate
surface the same way a downstream consumer would.

A tree-hash snapshot pins the normalized structure of a rendered
view to a golden file:

```rust
use plushie::test::{TestSession, assert_tree_hash};

#[test]
fn counter_after_increment_matches_golden() {
    let mut session = TestSession::<Counter>::start();
    session.click("inc");
    assert_tree_hash(&session, "counter_after_inc", "tests/golden");
}
```

The directory path resolves relative to `CARGO_MANIFEST_DIR`, so
`cargo test` finds the same golden file regardless of where it was
invoked from. Set `PLUSHIE_UPDATE_SNAPSHOTS=1` to rewrite the stored
hash when the UI intentionally changes.

## What's next

The app is covered: widgets, events, subscriptions, commands,
async, effects, and now tests. The remaining chapter looks at
patterns for carrying state that outlives any single widget or
screen. [Chapter 16](16-shared-state.md) covers shared state.
