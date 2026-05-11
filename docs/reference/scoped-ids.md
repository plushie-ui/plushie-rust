# Scoped IDs

A scoped ID is the canonical wire string for a widget, built from
the chain of enclosing named containers: `window#scope/path/id`.
The top-level enum `plushie_core::ScopedId` parses and rebuilds
that format; the `plushie::ui` builders supply the raw ID, and
normalization composes the scoped form before the diff runs.

Scoping lets the same local ID live in many places at once. "The
delete button in file A" and "the delete button in file B" are
both `button("delete", "x")` at the source level; the enclosing
container's ID distinguishes them on the wire and in events.

## Auto-IDs from `#[track_caller]`

Layout and display constructors generate an auto-ID from the
call site. The helper in `plushie::ui` is:

```rust
#[track_caller]
pub(crate) fn auto_id(prefix: &str) -> String {
    let loc = std::panic::Location::caller();
    format!("auto:{prefix}:{}:{}", loc.file(), loc.line())
}
```

Every zero-argument constructor (`column()`, `row()`, `container()`,
`stack()`, `grid()`, `scrollable()`, `keyed_column()`, `pin()`,
`floating()`, `responsive()`, `space()`, `text()`, `rich_text()`,
`rule()`, `progress_bar(..)`, `image(..)`, `svg(..)`, `markdown(..)`,
`qr_code(..)`) carries `#[track_caller]` and seeds its `id` field
with `auto_id("column")`, `auto_id("text")`, and so on. The
resulting string (for example `auto:column:src/ui.rs:42`) is
stable across re-renders of the same call site and unique between
different call sites in different source positions. Normalization
treats every ID that starts with `auto:` as transparent: it is
not scope-prefixed, it does not create a scope for its children,
and it is exempt from duplicate-ID detection.

That last point matters for loops. If a helper function contains
`let layout = column().spacing(8.0);` and a caller invokes the
helper inside a `for` loop, every iteration gets the same auto-ID
because the source location is identical. This is intentional:
the auto-ID does not need to be unique (it is transparent), and a
stable value keeps tree diffing deterministic.

## Explicit IDs

Every widget exposes `.id(&str)` to override the auto-ID:

```rust
use plushie::prelude::*;

column()
    .id("toolbar")
    .spacing(8.0)
    .children([
        button("save", "Save").into(),
        button("load", "Load").into(),
    ])
```

An explicit ID creates a scope. Children of `toolbar` are
scope-prefixed: the save button is wired as `toolbar/save`.
Auto-ID containers between `toolbar` and the button are
transparent, so a wrapping `row()` in the tree does not appear in
the path.

Explicit IDs must be non-empty, must not contain `/` (the scope
separator) or `#` (the window qualifier), and must stay under
1024 bytes. Violations surface as `widget_id_invalid` diagnostics;
duplicate explicit IDs among siblings surface as `duplicate_id`.
See [built-in widgets](built-in-widgets.md) for per-widget
`.id()` availability.

## Required IDs

Stateful and interactive widgets take the ID as the first
positional argument because their identity drives event routing
and renderer-side state:

```rust
pub fn window(id: &str) -> WindowBuilder
pub fn pane_grid(id: &str) -> PaneGridBuilder
pub fn text_input(id: &str, value: &str) -> TextInputBuilder
pub fn text_editor(id: &str, content: &str) -> TextEditorBuilder
pub fn checkbox(id: &str, checked: bool) -> CheckboxBuilder
pub fn toggler(id: &str, is_toggled: bool) -> TogglerBuilder
pub fn radio(id: &str, value: &str, selected: Option<&str>) -> RadioBuilder
pub fn slider(id: &str, range: (f32, f32), value: f32) -> SliderBuilder
pub fn vertical_slider(id: &str, range: (f32, f32), value: f32)
    -> VerticalSliderBuilder
pub fn pick_list(id: &str, options: &[&str], selected: Option<&str>)
    -> PickListBuilder
pub fn combo_box(id: &str, options: &[&str], value: &str) -> ComboBoxBuilder
pub fn button(id: &str, label: &str) -> ButtonBuilder
pub fn pointer_area(id: &str) -> PointerAreaBuilder
pub fn sensor(id: &str) -> SensorBuilder
pub fn tooltip(id: &str, tip: &str) -> TooltipBuilder
pub fn overlay(id: &str) -> OverlayBuilder
pub fn themer(id: &str) -> ThemerBuilder
pub fn canvas(id: &str) -> CanvasBuilder
pub fn group(id: &str) -> GroupBuilder
pub fn interactive(id: &str) -> GroupBuilder
```

A stateful widget without a stable ID cannot preserve its
renderer-side state (cursor position, selection, scroll offset,
open/closed state, pane geometry) across renders. Taking the ID
as a positional argument makes the omission a compile error.

## ID uniqueness and collisions

Uniqueness is checked per-scope, not globally. Two siblings with
the same explicit ID produce a `duplicate_id` diagnostic; the
same local ID in two different scopes is safe:

```rust
column().id("form-a").child(button("save", "Save"))  // form-a/save
column().id("form-b").child(button("save", "Save"))  // form-b/save
```

Auto-IDs bypass duplicate detection because call-site equality is
the usual case (a helper rendered inside a loop). Explicit empty
IDs (`.id("")`) are treated as unauthored and get an
`empty_id` diagnostic so the widget is still visible to tooling.

## Loops and list rendering

The canonical shape for list items is to give the container an
ID derived from the item's own identifier. Each item becomes a
scope, and the inner widgets keep clean local names:

```rust
column()
    .id("list")
    .children(model.todos.iter().map(|todo| {
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
    }))
```

The delete button for a todo with id `t1` is wired as
`list/t1/delete`. The wrapping `row()` is auto-ID and does not
appear in the path. In the event handler, the item ID shows up
in the scope chain:

```rust
use plushie::event::WidgetMatch::*;

match event.widget_match() {
    Some(Click("delete")) => {
        if let Some(item_id) = event.scope().and_then(|s| s.first()) {
            model.todos.retain(|t| &t.id != item_id);
        }
    }
    _ => {}
}
```

`event.scope()` returns the reversed ancestor chain (nearest
parent first, window last), so `scope[0]` is always the immediate
enclosing named container.

## Reusable components

A helper function that returns a `View` can be called many times
from the same parent. Any stateful or interactive widget inside
needs an ID, and those IDs have to stay unique when the helper
is reused. The pattern is to take the discriminator as an
argument and wrap the helper's output in an outer container with
that ID:

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

Because `container().id(&todo.id)` creates a scope, the inner
`toggle` and `delete` IDs are re-used without conflict across
every call. Auto-ID layout widgets inside the helper
(`row()` here) are transparent, so the published path stays
`<todo.id>/toggle` rather than leaking the wrapper.

When the helper has no stateful children (it is all auto-ID
layout and display widgets), no outer scope is needed; each
auto-ID already disambiguates the leaf nodes by source location.

## Event routing by ID path

When a widget emits an event, the runtime routes it by the
canonical scoped ID. The renderer sends the full
`window#scope/path/id` string; the SDK parses it into
`plushie_core::ScopedId` and stores it on the `WidgetEvent` as
`scoped_id`:

```rust
pub struct ScopedId {
    pub id: String,              // local name, e.g. "delete"
    pub scope: Vec<String>,      // reversed ancestors, e.g. ["t1", "list"]
    pub window_id: Option<String>,
    pub full: String,            // canonical wire ID
}
```

## Multi-window Synthetic Root

When `App::view` returns multiple top-level windows, the Rust SDK
wraps them in an internal `auto:root` container before normalization
and diffing. Like every `auto:` ID, this wrapper is transparent: it
does not create a user-visible scope segment, does not participate in
duplicate-ID diagnostics, and should not be targeted by selectors.
Single-window apps do not need this wrapper because their view is
promoted directly to the tree root.

Matching patterns in order from least to most specific:

```rust
match event.widget_match() {
    Some(Click("save")) => /* any save button */,
    Some(Click("save")) if event.scope().is_some_and(|s| s.first().is_some_and(|p| p == "settings-form")) =>
        /* save button under `settings-form` */,
    _ => {}
}
```

Commands that target a widget use the same slash-joined format:
`Command::focus("form/email")`, and the window-qualified form
`Command::focus("settings#email")`. See [commands](commands.md) for
the full list.

A11y cross-references (`labelled_by`, `described_by`,
`error_message`, `active_descendant`, and each `radio_group`
entry) go through the same scope-rewrite pass. A bare reference
inside a named container resolves to the scoped form; a reference
that already contains `/` or `#` passes through unchanged.
Unresolved references surface as `a11y_ref_unresolved`
diagnostics and are otherwise non-fatal.

## Debugging ID collisions

Normalization emits diagnostics rather than panicking, so the
renderer keeps running even when IDs are wrong. The variants to
watch for in `plushie_core::diagnostic::Diagnostic`:

| Diagnostic | Trigger |
|---|---|
| `DuplicateId { id, .. }` | Two siblings with the same scoped ID |
| `WidgetIdInvalid { reason: "reserved_char", .. }` | User ID contains `/` or `#` |
| `WidgetIdInvalid { reason: "too_long", .. }` | User ID exceeds 1024 bytes |
| `EmptyId { type_name }` | An explicit `.id("")` on a real widget |
| `A11yRefUnresolved { .. }` | A11y cross-reference does not match any declared ID |

In `#[test]` code, `TestSession` exposes `allow_diagnostics` and
`assert_no_diagnostics`; the default is strict, so any of the
above will fail the test with the diagnostic payload in the
panic message. The typed `reason` string on
`WidgetIdInvalid` lets a match expression branch on the specific
failure mode.

When a test selector fails to match, the scoped-ID format is the
first thing to check. `Selector::id("save")` matches trailing
segments, so a widget wired as `main#form/save` is found by
`"save"`, `"form/save"`, or `"main#form/save"`. Ambiguity across
windows is an error; qualify with the window prefix
(`"settings#save"`) or use `Selector::id_in_window`.

## See also

- [Events](events.md) for `WidgetEvent`, `ScopedId`, and the
  full `widget_match` surface.
- [Built-in widgets](built-in-widgets.md) for which constructors
  take an ID argument and which expose `.id()`.
- [Composition patterns](composition-patterns.md) for helper
  functions, memoization, and view reuse across call sites.
- [Testing](testing.md) for selector syntax, diagnostic
  assertions, and session-level ID resolution.
