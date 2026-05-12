# State management

A counter or a todo list fits in a single struct with half a dozen
fields. Real apps grow past that. Panes remember their splits, lists
remember selections, editors remember history, a sidebar remembers
which screen is active. The model turns into a large record that a
single `update` has to keep coherent.

This chapter covers how to keep a growing model readable: splitting
it into sub-structs, updating nested fields on a local next model, deriving
values instead of caching them, wiring navigation and selection, and
adding undo/redo. The SDK ships a small set of helpers in
`plushie::route`, `plushie::selection`, `plushie::undo`,
`plushie::query`, and `plushie::state` that handle the patterns that
tend to be tedious or easy to get wrong. Everything else is plain
Rust: structs, enums, and the borrow checker.

## Organising the model

Start flat. A handful of fields on one struct is easy to read and
easy to move. `update` matches on the event and touches the field it
cares about:

```rust
struct Counter {
    count: i32,
    last_changed_at: Option<std::time::Instant>,
}
```

Split into sub-structs when two things happen at once: fields start
clustering by subject (`editor_source`, `editor_dirty`,
`editor_cursor`), and a helper in `view` needs several fields from
the same cluster but nothing from outside it. That is the signal to
group:

```rust
struct Pad {
    editor: Editor,
    sidebar: Sidebar,
    settings: Settings,
}

struct Editor {
    source: String,
    dirty: bool,
    cursor: usize,
}

struct Sidebar {
    filter: String,
    collapsed: bool,
}
```

The payoff is twofold. View helpers take `&Editor` instead of
`&Pad` and become testable without building a whole application
model. `update` arms can clone the current model into `next`, then
call small helpers that edit a narrow slice of `next`.

Resist nesting more than two levels deep. `model.workspace.editor.
buffer.cursor` is a hint that either the grouping is wrong or one of
the inner structs is itself a self-contained subject that should
live under a different parent.

## Updating nested state

Rust's borrow checker keeps nested updates honest. `update` receives
`&Self::Model`, clones or rebuilds the parts that change, and returns
the next model:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Input(id, value)) if id == "editor" => {
            next.editor.source = value.to_string();
            next.editor.dirty = true;
        }
        Some(Click(id)) if id == "toggle-sidebar" => {
            next.sidebar.collapsed = !model.sidebar.collapsed;
        }
        _ => {}
    }
    (next, Command::none())
}
```

When the arm gets wider than a few lines, factor it into a helper
that takes a mutable borrow of the sub-struct:

```rust
fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Input(id, value)) if id == "editor" => {
            update_editor_input(&mut next.editor, value.to_string());
        }
        _ => {}
    }
    (next, Command::none())
}

fn update_editor_input(editor: &mut Editor, value: String) {
    editor.source = value;
    editor.dirty = true;
    editor.cursor = editor.source.len();
}
```

The helper borrows `&mut Editor` from the local next model for the
duration of the call. The rest of the model is untouched, so the
compiler allows another simultaneous borrow elsewhere in the same arm
if needed. If two helpers need disjoint sub-structs at once,
destructure the next model first:

```rust
let Pad { editor, sidebar, .. } = &mut next;
sync_sidebar_with_editor(sidebar, editor);
```

This split borrow is explicit and local. It avoids the runtime cost
of a `RefCell` and makes the data dependency readable at the call
site.

## Derived state

When one field can be computed from others, compute it. Caching it
on the model means keeping the cache in sync, and the cache goes
stale the first time someone forgets.

```rust
// Don't: dirty_count duplicates what todos already know.
struct Bad {
    todos: Vec<Todo>,
    dirty_count: usize,
}

// Do: derive on read.
struct Good {
    todos: Vec<Todo>,
}

impl Good {
    fn dirty_count(&self) -> usize {
        self.todos.iter().filter(|t| t.dirty).count()
    }
}
```

Call `model.dirty_count()` from the view. The runtime runs `view`
after every update, so deriving on read is as fresh as a cached
field would be, and it cannot drift. Cache only when a derivation
shows up in a profile as a real cost, and even then, prefer to cache
into a field that is only ever written in one place (a single helper
that the whole update path routes through).

The same rule applies to list projections. A `Vec<Todo>` plus a
predicate is enough; there is no need for a separate
`Vec<ActiveTodo>` unless the filtered list is used across many
frames and the source list is huge.

## Routing and navigation

Multi-screen apps switch between views. The simplest switch is a
plain enum matched in `view`:

```rust
enum Screen {
    List,
    Detail(String),
    Settings,
}

fn view(model: &Self, widgets: &mut WidgetRegistrar) -> ViewList {
    let body: View = match &model.screen {
        Screen::List => list_view(model).into(),
        Screen::Detail(id) => detail_view(model, id).into(),
        Screen::Settings => settings_view(model).into(),
    };
    window("main").child(body).into()
}
```

That is all a routing layer strictly needs: an enum in the model,
arms in `update` that reassign it, and a match in `view`. Linear
history (a browser-style back button) is where a dedicated helper
starts to pay off. `plushie::route::Route` keeps a stack of path
strings and optional parameters:

```rust
use plushie::route::Route;
use serde_json::json;
use std::collections::HashMap;

let mut route = Route::new("/list");
route.push("/detail");
route.push_with_params("/settings", {
    let mut p = HashMap::new();
    p.insert("tab".into(), json!("theme"));
    p
});

route.current();       // "/settings"
route.params().get("tab");  // Some(Value::String("theme"))
route.can_go_back();   // true

route.pop();
route.current();       // "/detail"
```

`Route` is plain data. Store it on the model, update it on the next
model from `update`, and read from it in `view`:

```rust
struct App {
    route: Route,
}

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Click(id)) if id == "open-detail" => {
            next.route.push("/detail");
        }
        Some(Click(id)) if id == "back" => {
            next.route.pop();
        }
        _ => {}
    }
    (next, Command::none())
}

fn view(model: &Self, widgets: &mut WidgetRegistrar) -> ViewList {
    let body: View = match model.route.current() {
        "/list" => list_view(model).into(),
        "/detail" => detail_view(model).into(),
        _ => not_found_view().into(),
    };
    window("main").child(body).into()
}
```

The root entry cannot be popped, so `pop()` on a single-entry stack
returns `false` and leaves the stack alone. `replace_top` swaps the
current route without growing the stack: useful when a screen
transitions to another screen of the same logical level (for
example, login to signup on an auth flow).

Parameters are `HashMap<String, serde_json::Value>`. Keep them
small: IDs, indices, a filter string. Anything larger belongs on
the model proper, not in route parameters.

## Selection

Lists and tables almost always need a concept of selection. Single
selection (radio-style), multi selection (checkbox-style), and range
selection (shift-click) are common enough that
`plushie::selection::Selection` covers them directly:

```rust
use plushie::selection::{Selection, SelectionMode};

let order = vec!["a".into(), "b".into(), "c".into(), "d".into()];
let mut sel = Selection::new(SelectionMode::Multi, order);

sel.select("a");
sel.toggle("c");
sel.is_selected("c");  // true
sel.count();           // 2

sel.range_select("b"); // selects from anchor "c" to "b" inclusive
sel.clear();
```

`Selection` holds the mode, the selected set, an anchor for range
operations, and an ordered list of candidate IDs (the order is what
`range_select` uses to walk from anchor to target). Update it in
place from the usual events:

```rust
match event.widget_match() {
    Some(Press(id, press)) if id.starts_with("item-") => {
        if press.modifiers.shift {
            model.selection.range_select(id);
        } else if press.modifiers.control || press.modifiers.command {
            model.selection.toggle(id);
        } else {
            model.selection.select(id);
        }
    }
    _ => {}
}
```

`WidgetMatch::Press` carries a `PointerPress` with the modifier
state at the moment the button went down, which is what a shift- or
ctrl-click wants. Plain `Click` fires on release and does not carry
modifiers.

When the underlying data changes (items added, removed, reordered),
rebuild the selection with a fresh order vector. Selected IDs that
no longer appear in the order list are still valid entries in the
selected set, so stale IDs do not cause a panic; they just never
match. If the app treats removed IDs as a bug, prune them explicitly
after the data update.

A simple `Option<String>` or `HashSet<String>` is fine if the
selection logic is a single obvious line. Reach for `Selection`
when the code starts branching on mode, tracking an anchor by hand,
or reinventing range walks.

## Undo and redo

Editors, canvases, and form-heavy apps expose undo. The naive
approach, cloning the entire model on every edit, works and is fine
for small models. It stops being fine when the model is large or
when the undo stack needs labels, coalescing of rapid edits, or
bounded size. `plushie::undo::UndoStack` covers all three.

`UndoStack` is generic over a `Clone + Send + 'static` type. It
stores the current value plus an undo and redo stack of
`UndoCommand` entries. Each command carries an `apply` function and
an `undo` function: applying goes forward, undoing calls the reverse
function on the current state. The snapshot-style `push(new_state)`
is sugar on top that builds the apply and undo closures for you.

```rust
use plushie::undo::{UndoStack, UndoCommand};

let mut stack = UndoStack::new("".to_string());
stack.push("hello".to_string());
stack.push("hello world".to_string());

stack.current();    // "hello world"
stack.undo();
stack.current();    // "hello"
stack.redo();
stack.current();    // "hello world"
```

For rapid sequential edits (a typing burst, a drag), coalescing
merges them into one undo step. Commands with the same key within
the time window are composed into a single entry, so one Ctrl+Z
reverses the whole burst:

```rust
let cmd = UndoCommand::new(
    |s: &String| format!("{s}a"),
    |s: &String| s.strip_suffix('a').unwrap_or(s).to_string(),
)
.label("type")
.coalesce("typing", 500);

stack.apply(cmd);
```

Wire undo into the update loop the same way:

```rust
struct Editor {
    history: UndoStack<String>,
}

fn update(model: &Self, event: Event) -> (Self, Command) {
    let mut next = model.clone();
    match event.widget_match() {
        Some(Input(id, value)) if id == "editor" => {
            let prev = next.history.current().clone();
            next.history.apply(
                UndoCommand::new(
                    move |_| value.clone(),
                    move |_| prev.clone(),
                )
                .coalesce("typing", 500),
            );
        }
        Some(KeyPress(_, kp)) if kp.key == Key::Z && kp.modifiers.command => {
            if kp.modifiers.shift {
                next.history.redo();
            } else {
                next.history.undo();
            }
        }
        _ => {}
    }
    (next, Command::none())
}
```

`stack.current()` gives an immutable borrow; `stack.current_mut()`
gives a mutable one that bypasses the history (useful for transient
view state that should not be undoable). The stack is bounded; the
default is 100 entries, overridable with `with_max_size`.

## Filtering and sorting lists

Two approaches work for filtered and sorted lists: compute on the
fly during `view`, or cache an index. On-the-fly computation is the
default. It keeps the model small and the update path simple:

```rust
fn visible(model: &Self) -> Vec<&Todo> {
    let needle = model.filter.to_lowercase();
    let mut list: Vec<&Todo> = model.todos.iter()
        .filter(|t| t.text.to_lowercase().contains(&needle))
        .collect();
    list.sort_by_key(|t| t.created_at);
    list
}
```

For larger collections or more complex pipelines (filter plus
search plus multi-field sort plus pagination), `plushie::query`
bundles the pipeline into one composable value:

```rust
use plushie::query::{Query, SortDir};

let result = Query::new(&model.todos)
    .filter(|t| !t.archived)
    .search(&model.filter, |t| vec![t.text.as_str(), t.tag.as_str()])
    .sort_by(vec![
        (SortDir::Desc, Box::new(|a, b| a.priority.cmp(&b.priority))),
        (SortDir::Asc, Box::new(|a, b| a.created_at.cmp(&b.created_at))),
    ])
    .page(model.page)
    .page_size(25)
    .run();

result.entries;  // Vec<Todo> for this page
result.total;    // total matches before pagination
```

The pipeline runs in a fixed order: filter, search, sort, paginate,
group. Each step is optional. `run` returns a `QueryResult<T>` with
the page entries, total count, and optional groups.

Cache an index (a `Vec<usize>` of matching positions, or a
precomputed sort order) only when the view re-renders often and the
source collection is large enough that the filter shows up as a
cost. Invalidating the cache on every mutation of the source list
is the usual failure mode; if you cache, centralise the mutation
path so the invalidation happens in exactly one place.

## When the model grows too big

A healthy model fits on one screen of code. When it stops doing
that, there are three common refactors.

**Extract sub-models.** The fields cluster by subject and the
cluster already has a natural name. Pull them into their own struct
(`Editor`, `Sidebar`, `Settings`). The cost is low: each call site
becomes `model.editor.source` instead of `model.editor_source`.
The gain is that helpers can take the sub-struct directly.

**Move transient state off the model.** Not every piece of state
belongs in the app model. Text input values before commit, hover
states driven by `Move` events, and animation progress that the
renderer owns, belong in the widget tree or renderer-side
animations, not in the model. Look for fields that change on every
frame and never affect what else the app does: they are candidates.

**Split the app.** The hardest refactor, but sometimes the right
one: the model is actually two apps pretending to be one. Separate
the windows, give each its own model, and coordinate through
commands. The runtime already supports multiple top-level windows;
see the
[windows and layout reference](../reference/windows-and-layout.md)
for the split.

When none of these apply, `plushie::state::State` offers a
path-based container with revision tracking and transactions. It is
useful for configuration stores and shared settings where the keys
are data rather than code (a user-editable settings screen, a
plugin registry). It is not a substitute for a typed struct when
the shape is known at compile time.

## What's next

A well-organised model makes tests easy to write because helpers
take narrow borrows and run without a UI. The next chapter covers
[testing](15-testing.md): the `TestSession` harness, asserting on
the model, simulating events, and stubbing effects.
