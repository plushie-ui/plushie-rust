# Composition patterns

Plushie views are plain Rust values. A widget builder returns a
typed builder; container setters like `.child(..)` and
`.children(..)` accept `impl Into<View>`; the runtime collapses the
top-level `ViewList` into a single wire tree. Composition falls out
of the type system: a helper function returns a builder or a
`View`, and any number of those compose into a larger tree.

This page covers the recurring shapes: helpers as components, ID
scoping across helper boundaries, passing model state down,
dispatching events back up, memoising expensive subtrees,
conditional rendering, keyed lists, multi-window trees, error
fallbacks, and when to reach for a real custom widget instead.

## Functions as components

A helper function is a component. It takes the data it needs, calls
the built-in builders, and returns something that converts into a
`View`. Two idiomatic return shapes cover almost every case.

Return `impl Into<View>` when the caller will chain more setters or
when several helpers share a site that already expects a builder:

```rust
use plushie::prelude::*;

fn primary_button(id: &str, label: &str) -> impl Into<View> {
    button(id, label)
        .style(Style::primary())
        .padding(Padding::all(8))
}
```

Return a concrete `View` when the helper builds a subtree and the
caller just drops it into a parent:

```rust
fn todo_row(todo: &TodoItem) -> View {
    container()
        .id(&todo.id)
        .child(
            row()
                .spacing(8.0)
                .child(checkbox("toggle", todo.done))
                .child(text(&todo.text))
                .child(button("delete", "x")),
        )
        .into()
}
```

Both shapes compose identically at the call site, because
`.child(..)` accepts `impl Into<View>` and container builders
already implement `From` into `View`. The difference is how much
freedom the caller has: `impl Into<View>` still exposes builder
setters; `View` is final.

Within a helper, prefer short pipelines over intermediate bindings.
When a helper exceeds roughly a screenful, extract another helper.

## Scoping IDs across helper calls

Every interactive widget needs a stable ID. When a helper that
emits interactive widgets is called from many places, the local
IDs inside the helper repeat, so the helper must be wrapped in a
scope. The canonical shape is an outer container with an
explicit `.id(..)` derived from the item's own identifier:

```rust
fn todo_row(todo: &TodoItem) -> View {
    container()
        .id(&todo.id)
        .child(
            row()
                .spacing(8.0)
                .child(checkbox("toggle", todo.done))
                .child(button("delete", "x")),
        )
        .into()
}
```

The delete button in `todo_row(todo)` with `todo.id == "t1"` is
wired as `t1/delete`. The inner `row()` is auto-ID and does not
appear in the path. A helper that has no stateful children (all
auto-ID layout and display widgets) does not need an outer scope:
auto-IDs disambiguate leaves by call-site source location.

See [Scoped IDs](scoped-ids.md) for the full rules on auto-IDs,
explicit IDs, and scope rewriting.

## Lifting model state

Helpers take a borrow of the model or a slice of it. Use
`&Model` when the helper needs broad read access; pass narrow
types when the helper's surface is small. Narrow types make the
helper reusable and keep it testable without a full model.

```rust
struct TodoItem { id: String, text: String, done: bool }

// Broad borrow: the whole model.
fn list_view(model: &TodoApp) -> View {
    column()
        .id("list")
        .spacing(4.0)
        .children(model.todos.iter().map(todo_row))
        .into()
}

// Narrow borrow: just the piece this helper needs.
fn todo_row(todo: &TodoItem) -> View {
    container()
        .id(&todo.id)
        .child(
            row()
                .spacing(8.0)
                .child(checkbox("toggle", todo.done))
                .child(text(&todo.text))
                .child(button("delete", "x")),
        )
        .into()
}
```

`column().children(..)` accepts any `IntoIterator` whose items
convert to `View`, so the `.iter().map(..)` form flows in directly
without collecting into a `Vec`.

View functions are pure and take the model by shared reference.
Do not hand a helper `&mut Model` just to nudge derived state;
compute derived values up front in `view`, or precompute them in
`update` and store them on the model.

## Dispatching events from helpers

Helpers only produce view nodes. Event handling stays in `update`
and routes by scoped ID. The item ID placed on the outer wrapper
becomes the first entry of `event.scope()` on any event emitted
from inside the helper:

```rust
use plushie::prelude::*;
use plushie::event::WidgetMatch::*;

fn update(model: &mut TodoApp, event: Event) -> Command {
    match event.widget_match() {
        Some(Click("delete")) => {
            if let Some(todo_id) = event.scope().and_then(|s| s.first()) {
                model.todos.retain(|t| &t.id != todo_id);
            }
        }
        Some(Toggle("toggle", _)) => {
            if let Some(todo_id) = event.scope().and_then(|s| s.first())
                && let Some(item) = model.todos.iter_mut().find(|i| &i.id == todo_id)
            {
                item.done = !item.done;
            }
        }
        _ => {}
    }
    Command::none()
}
```

`event.scope()` returns `Option<&[String]>`, with the nearest
ancestor first and the window last. Deeper helpers get a longer
scope chain; a match can narrow on any prefix. See
[Events](events.md) for the full `widget_match` surface.

When a helper needs to emit an event type that does not belong in
the widget catalog at all, the answer is a real custom widget, not
a new event variant. See the closing section.

## Memoising expensive subtrees

`memo(key, deps, view_fn)` wraps a subtree in a `__memo__` marker
so normalisation can reuse the cached subtree when the deps hash
is unchanged. The signature, from `plushie::ui::memo`:

```rust
pub fn memo<D: Hash>(
    key: impl Into<String>,
    deps: D,
    view_fn: impl FnOnce() -> View,
) -> View
```

The `view_fn` always runs (the view function is pure, so the SDK
cannot skip it without re-hashing the deps first). What a cache
hit avoids is re-walking the subtree through normalisation,
scoped-ID rewrites, and tree diff. Use it for subtrees that are
large and rarely change:

```rust
column().children([
    memo("header", (model.user_id, model.revision), || {
        expensive_header(&model)
    }),
    text(&model.dynamic_text).into(),
])
```

`key` identifies the memo call site; pick a stable string per
distinct memo in the tree. `deps` is any `Hash` value: a tuple, a
`&str`, a `u64`, a custom type that derives `Hash`. Avoid hashing
floats unless the bit-level identity is intentional.

`memo` is a micro-optimisation. Start without it. Reach for it
when a profile shows normalisation or diff time dominating a
specific render.

## Conditional rendering

A view branch can return different subtrees by matching on model
state. Because helpers return `View` (or a builder that converts
to one), a `match` or `if` expression works anywhere a single
child is expected, as long as all arms return the same type.

### Match-returned views

```rust
fn main_content(model: &App) -> View {
    match model.route.current() {
        "/list" => list_view(model),
        "/edit" => edit_view(model),
        _ => not_found(),
    }
}
```

All arms return `View`. For builder arms, add `.into()` on each.

### Optional subtrees

A subtree that only sometimes appears can be expressed with
`Option<View>` passed through `Into<ViewList>`, or by pre-building
a child list that conditionally includes the node:

```rust
fn toolbar(model: &App) -> View {
    let mut r = row().spacing(4.0)
        .child(button("save", "Save"))
        .child(button("cancel", "Cancel"));
    if model.show_advanced {
        r = r.child(button("advanced", "Advanced"));
    }
    r.into()
}
```

At the top level, `ViewList::from(Option<View>)` is defined, so a
`view` function that returns nothing during a loading state can
yield `ViewList::new()` or an `Option<View>` that is `None`.

### Multi-window `ViewList`

`App::view` returns `impl Into<ViewList>`. The common shapes:

```rust
fn view(model: &Self::Model, _widgets: &mut WidgetRegistrar) -> ViewList {
    match model.mode {
        Mode::Single => window("main").child(main(model)).into(),
        Mode::Detached => vec![
            window("main").child(main(model)).into(),
            window("detail").child(detail(model)).into(),
        ]
        .into(),
        Mode::Blank => ViewList::new(),
    }
}
```

A single window, a `Vec<View>` of peer windows, a fixed-size
array, an `Option<View>`, or `()` all convert in. The runtime
collapses a single entry into the root directly and wraps
multiple entries under a synthetic container.

## Keyed lists and stable identity

A `column` diffs its children by position: inserting at the front
shifts every downstream index, which invalidates renderer state
on stateful widgets and forces re-layout. `keyed_column` diffs by
each child's ID, so a stable ID on each row keeps state pinned to
its row across reorders, insertions, and removals:

```rust
keyed_column()
    .spacing(4.0)
    .children(model.todos.iter().map(todo_row))
```

Because `todo_row` already scopes itself by `todo.id`, the outer
wrapper's ID is already the diff key. No additional work is
needed.

Give every list item an ID that is stable across renders and
unique among siblings. If items can change identity (a draft item
that gains a real server-side ID once saved), accept one diff
churn at the transition and keep the new ID stable afterwards.

## Shared widgets across windows

A view helper does not know which window it lives under. Calling
the same helper from two different window builders scopes its
output beneath each window independently, because scope paths are
anchored at the window ID.

```rust
fn view(model: &Self::Model, _widgets: &mut WidgetRegistrar) -> ViewList {
    let toolbar = || row().spacing(4.0)
        .child(button("save", "Save"))
        .child(button("cancel", "Cancel"));

    vec![
        window("main").child(column().child(toolbar()).child(main(model))).into(),
        window("inspector").child(column().child(toolbar()).child(inspector(model))).into(),
    ]
    .into()
}
```

The two Save buttons have the same local ID (`save`) but different
scoped paths (`main#save` vs `inspector#save`). Dispatch either by
matching on scope, or route on `event.as_widget().map(|w| &w.scoped_id.window_id)`.

Interactive widgets that must survive a window close and reopen
(editor state, scroll position) need IDs stable across those
events. The renderer keys its state by scoped ID, so reusing the
same window ID and widget ID is enough.

## Error boundary and fallback patterns

Plushie has no runtime-level error boundary: view functions are
pure and cannot panic on the hot path without taking down the
runtime. The pattern is an explicit fallback branch that checks
the model for error state before rendering the normal tree:

```rust
fn main_content(model: &App) -> View {
    if let Some(err) = &model.fatal {
        return error_screen(err);
    }
    if model.loading {
        return loading_screen();
    }
    normal_tree(model)
}

fn error_screen(msg: &str) -> View {
    container()
        .id("error")
        .center(true)
        .child(
            column()
                .spacing(12.0)
                .child(text("Something went wrong").size(18.0))
                .child(text(msg))
                .child(button("retry", "Retry")),
        )
        .into()
}

fn loading_screen() -> View {
    container()
        .id("loading")
        .center(true)
        .child(text("Loading...").size(16.0))
        .into()
}
```

Keep fatal flags and recoverable errors on the model, set them
from `update` in response to `Event::CommandError`,
`Event::Async` with an `Err` result, or `Event::System` with a
`RecoveryFailed` or `SessionError` variant. See
[Events](events.md) for the system-event taxonomy.

For windows that should survive their own content failing (an
inspector that could not build its subtree from a half-loaded
model), wrap the window's body in a fallback helper the same way.

## When to reach for custom widgets

Helpers are enough when the composition is a shape over existing
widgets. Reach for a real custom widget (via `plushie-widget-sdk`)
when any of the following hold:

- The widget owns renderer-side state that cannot be derived from
  the app model (a canvas with its own input tracking, a
  virtualised list with scroll and cursor).
- The widget needs a render pass below the tree boundary
  (custom drawing, custom hit testing, custom layout).
- The widget emits event kinds that do not fit an existing
  `WidgetMatch` variant and carries fields the match surface
  cannot express.
- The widget will be reused across multiple apps or crates and
  needs its own prop, event, and command types with full
  derive-macro support.

A helper that starts simple and keeps accreting model state and
event kinds is a signal to promote it to a custom widget. Until
then, helpers and `memo` cover almost every shape.

## See also

- [Scoped IDs](scoped-ids.md) for auto-IDs, explicit IDs, and how
  scope paths compose across helpers.
- [Events](events.md) for `widget_match`, `event.scope()`, and the
  full event taxonomy.
- [Built-in widgets](built-in-widgets.md) for the constructors and
  setters these helpers call.
- [Windows and layout](windows-and-layout.md) for window
  configuration and the multi-window tree shape.
- [Themes and styling](themes-and-styling.md) for `Style`,
  `Border`, `Shadow`, and the prop types shared across helpers.
