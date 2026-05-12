# Accessibility

Plushie integrates with platform accessibility services via
[AccessKit](https://github.com/AccessKit/accesskit): VoiceOver on
macOS, AT-SPI / Orca on Linux, UI Automation / NVDA / JAWS on
Windows. Most accessibility semantics are inferred automatically
from widget types, so correct roles, labels, and state ship without
extra work. The types live in `plushie_core::types::a11y`,
re-exported as `plushie::types`. Direct and wire modes share the
same AccessKit integration through `plushie-renderer-lib`, so this
page applies identically to both.

## Accessible by default

Built-in widgets expose accessibility metadata automatically: a
button announces itself as a button, a checkbox tracks its checked
state, a slider exposes its numeric value and range. Widget-SDK
`infer_a11y` implementations fill in a role and, where applicable,
a default `label` or `description` derived from the widget's
content before the node reaches AccessKit.

Layout containers (`column`, `row`, `container`, `stack`, `grid`,
`keyed_column`, `space`) map to `Role::GenericContainer` and are
filtered out of the platform accessibility tree. Screen reader
users navigate through the semantic content (buttons, text,
inputs) without encountering intermediate layout wrappers.

When overrides are needed (custom canvas controls, widgets with
context-dependent labels, relationship annotations), the `A11y`
struct is available on every widget via `.a11y(&a11y)`.

## The `A11y` struct

`A11y` lives in `plushie_core::types::a11y` and is re-exported from
`plushie::types`. It is a plain-data builder: every field is
`Option`, and setters return `Self`. Use `A11y::new()` to construct
an empty value and chain setters, or `A11y::with_description(..)`
for the common "description only" case used by
`infer_a11y` fallbacks.

```rust
use plushie::types::{A11y, HasPopup, Live, Orientation, Role};

let a = A11y::new()
    .role(Role::Button)
    .label("Save document")
    .description("Save the current document to disk")
    .live(Live::Polite);
```

### Fields

| Field | Type | Purpose |
|---|---|---|
| `role` | `Option<Role>` | Override the inferred role |
| `label` | `Option<String>` | Accessible name |
| `description` | `Option<String>` | Longer description read after the label |
| `hidden` | `Option<bool>` | Exclude from the accessibility tree |
| `expanded` | `Option<bool>` | Disclosure state |
| `required` | `Option<bool>` | Form field is required |
| `level` | `Option<usize>` | Heading level (1 through 6) |
| `live` | `Option<Live>` | Live-region politeness |
| `busy` | `Option<bool>` | Suppress announcements during updates |
| `invalid` | `Option<bool>` | Form validation error state |
| `modal` | `Option<bool>` | Dialog is modal |
| `read_only` | `Option<bool>` | Value is readable but not editable |
| `mnemonic` | `Option<char>` | Keyboard mnemonic |
| `toggled` | `Option<bool>` | Toggle / checked state |
| `selected` | `Option<bool>` | Selection state |
| `value` | `Option<String>` | Current value for assistive technology |
| `orientation` | `Option<Orientation>` | Layout orientation hint |
| `disabled` | `Option<bool>` | Disabled state override |
| `position_in_set` | `Option<usize>` | 1-based position in a group |
| `size_of_set` | `Option<usize>` | Total items in the group |
| `labelled_by` | `Option<String>` | ID of a widget that labels this one |
| `described_by` | `Option<String>` | ID of a widget that describes this one |
| `error_message` | `Option<String>` | ID of a widget showing the validation error |
| `active_descendant` | `Option<String>` | Currently active descendant ID |
| `radio_group` | `Option<Vec<String>>` | Peer IDs for an explicit radio group |
| `has_popup` | `Option<HasPopup>` | Popup kind, if any |

Decode clamps `level` to 1..=6. Values outside that range become
`None`. `mnemonic` takes a single `char`; on decode, only the first
character of the wire string is used.

### Constructors and merging

```rust
impl A11y {
    pub fn new() -> Self;
    pub fn with_description(description: impl Into<String>) -> Self;
    pub fn merge(base: &A11y, overrides: &A11y) -> A11y;
}
```

`A11y::merge` builds the effective accessibility record used by the
renderer: widget-inferred defaults are the `base`, user-provided
props are the `overrides`, and non-`None` fields in `overrides`
win. Fields the user did not specify fall through to the default.
The test harness exposes the merged view through
`TestSession::resolved_a11y`.

## Role taxonomy

`Role` is a `#[derive(PlushieEnum)]` enum that maps one-to-one to
iced's `accessible::Role`. Wire values are snake_case strings;
several variants accept aliases on decode.

**Interactive:** `Button`, `CheckBox`, `ComboBox`, `Link`,
`MenuItem`, `RadioButton` (alias `"radio"`), `Slider`, `Switch`,
`Tab`, `TextInput`, `MultilineTextInput` (alias `"text_editor"`),
`TreeItem`.

**Structure:** `GenericContainer` (aliases `"container"`,
`"generic"`), `Group`, `Heading`, `Label`, `List`, `ListItem`,
`ColumnHeader`, `Row` (wire `"table_row"`, alias `"row"`), `Cell`
(wire `"table_cell"`, alias `"cell"`), `Table`, `Tree`,
`RadioGroup`.

**Landmarks:** `Navigation`, `Region`, `Search`.

**Status:** `Alert`, `AlertDialog`, `Dialog`, `Status`, `Meter`,
`ProgressIndicator` (alias `"progress_bar"`).

**Other:** `Canvas`, `Document`, `Image`, `Menu`, `MenuBar`,
`ScrollBar`, `ScrollView`, `Separator`, `StaticText`, `TabList`,
`TabPanel`, `Toolbar`, `Tooltip`, `Window`.

Unknown role strings decode to `None` rather than a default, so
typos surface as a missing override.

## Live regions

```rust
pub enum Live { Polite, Assertive }
```

The absence of a `live` value (the `None` case on `Option<Live>`)
means "not a live region": the AccessKit node carries no live
attribute. There is no `Off` variant; demoting a region back to
non-live is done by clearing the field.

| Value | Behaviour | Use for |
|---|---|---|
| `Live::Polite` | Announced after current speech finishes | Status messages, counters, progress updates |
| `Live::Assertive` | Interrupts current speech immediately | Error messages, critical alerts |

```rust
use plushie::prelude::*;
use plushie::types::{A11y, Live, Role};

text(&model.status_message)
    .a11y(&A11y::new().live(Live::Polite));

text(&model.error)
    .a11y(&A11y::new().live(Live::Assertive).role(Role::Alert));
```

Reserve `Live::Assertive` for urgent context. Rapid updates on an
assertive region cause announcement storms. Prefer `Live::Polite`
for anything that updates more than once per user action. Do not
apply `live` to static content; screen readers re-announce on every
tree rebuild even when the content did not change.

## `HasPopup`

```rust
pub enum HasPopup { Listbox, Menu, Dialog, Tree, Grid }
```

Indicates the type of popup a widget triggers when activated. Set
it alongside `expanded` on controls that open a menu, listbox, or
dialog.

```rust
A11y::new()
    .role(Role::Button)
    .label("Options")
    .has_popup(HasPopup::Menu)
    .expanded(model.menu_open);
```

## Labels vs descriptions

Screen readers resolve a widget's accessible name in this order:

1. **Direct label** - the explicit `a11y.label(..)` value, or a
   widget-SDK `infer_a11y` default derived from a label-like prop
   (`button`'s `label`, `text`'s content, `image`'s `alt`).
2. **Labelled-by** - if no direct label, the renderer follows
   `labelled_by` to a sibling widget. For roles that support
   name-from-contents (button, checkbox, radio, link), descendant
   text is used automatically.
3. **No name** - the screen reader announces only the role.

A **label** is the short accessible name announced on focus. A
**description** is a longer string read after the name; use it to
add supplementary guidance without bloating the accessible name.
`placeholder` on `text_input`, `text_editor`, `combo_box`, and
`pick_list` flows into `description` automatically when `A11y`
does not set one.

Interactive widgets that fall off the bottom of the resolution
order emit a
[`Diagnostic::MissingAccessibleName`](events.md) during
normalization. Treat it as an authoring bug.

### Cross-references

`labelled_by`, `described_by`, `error_message`, `active_descendant`,
and each entry in `radio_group` hold scoped widget IDs. Tree
normalization resolves them relative to the current scope, so a
bare `"label"` inside scope `"form"` rewrites to `"form/label"`.
Unresolved refs emit [`Diagnostic::A11yRefUnresolved`](events.md)
with the offending `key`, the raw `value`, and an `is_member` flag
that is true when the reference appeared inside a collection (e.g.
a `radio_group` entry). The ID is left as-is on the wire.

## Attaching `A11y` to a widget

Every builder in `plushie::ui` exposes an `a11y` setter with the
same signature:

```rust
pub fn a11y(self, a11y: &A11y) -> Self
```

The builder takes the argument by reference, wire-encodes it, and
stores the result on the node's props. This applies uniformly to
layout, display, input, interactive, canvas, memo, and table
widgets.

```rust
use plushie::prelude::*;
use plushie::types::{A11y, Role};

let save = button("save", "Save")
    .a11y(&A11y::new().description("Save the current document"));

let email_label = A11y::new().labelled_by("email-label");
let email = text_input("email", &model.email)
    .placeholder("Email address")
    .a11y(&email_label);
```

### Form field labelling

Three idiomatic shapes:

```rust
// Direct label.
text_input("email", &model.email)
    .placeholder("Email address")
    .a11y(&A11y::new().label("Email address"));

// Cross-widget labelled_by.
text("Email address").id("email-label");
text_input("email", &model.email)
    .a11y(&A11y::new().labelled_by("email-label"));

// Description for additional context.
text_input("password", &model.password)
    .a11y(&A11y::new()
        .label("Password")
        .described_by("password-hint"));
text("Must be at least 8 characters").id("password-hint");
```

`text_input::required(true)` and `text_input::validation(..)` flow
into `required`, `invalid`, and `error_message` automatically, so
validation metadata does not need a separate `A11y` record.

### Canvas annotations

Canvas shapes are opaque to accessibility without explicit
annotations. Mark an interactive group with a role and label, and
set `focusable(true)` to add it to the Tab order:

```rust
use plushie::prelude::*;
use plushie::types::{A11y, Role};

interactive("save-btn")
    .on_click(true)
    .focusable(true)
    .a11y(&A11y::new().role(Role::Button).label("Save experiment"))
    .children([
        rect(0.0, 0.0, 100.0, 36.0).fill(Color::rgb(0.23, 0.51, 0.96)).into(),
        canvas_text(50.0, 11.0, "Save").fill(Color::WHITE).size(14.0).into(),
    ]);
```

Without `focusable(true)` the group responds to mouse clicks but is
invisible to keyboard navigation and screen readers.

## Keyboard navigation and focus

Built-in keyboard navigation:

| Key | Behaviour |
|---|---|
| Tab / Shift+Tab | Cycle focus through focusable widgets |
| Space / Enter | Activate the focused widget |
| Arrow keys | Navigate within sliders, lists, and similar widgets |
| F6 / Shift+F6 | Cycle focus between `pane_grid` panes |
| Ctrl+Tab | Escape the current focus scope |
| Escape | Close popups, dismiss modals |

Focus rings follow the focus-visible pattern: they appear on
keyboard navigation and not on mouse clicks.

### Focus commands

Programmatic focus lives on `Command`. See
[commands.md](commands.md) for the full catalog.

| Function | Purpose |
|---|---|
| `Command::focus(id)` | Move focus to a specific widget |
| `Command::focus_next()` | Move to the next focusable widget |
| `Command::focus_previous()` | Move to the previous focusable widget |
| `Command::focus_next_within(scope)` | Next focusable widget inside a subtree, wrapping at the boundary |
| `Command::focus_previous_within(scope)` | Previous focusable widget inside a subtree, wrapping at the boundary |
| `Command::find_focused(tag)` | Query which widget currently has focus |
| `Command::focus_window(id)` | Bring a window to the front and give it input focus |

`Command::focus` targets canvas elements via their scoped path
(`"canvas/element"`).

## Screen reader announcements

For one-shot announcements that do not correspond to a visible
widget, push text directly with `Command::announce`:

```rust
use plushie::prelude::*;
use plushie::types::Live;

pub fn update(model: &Model, event: Event) -> (Model, Command) {
    // ...
    (model.clone(), Command::announce("Document saved", Live::Polite))
}
```

`Command::announce_text(text)` is shorthand for
`Command::announce(text, Live::Polite)`. The renderer currently
routes all announcements through iced's assertive channel; the
politeness argument is preserved in the wire message for forward
compatibility.

Assistive technology actions (e.g. VoiceOver "activate") produce
the same `WidgetEvent` as direct interaction, so no special
handling is required in `update`. See [events.md](events.md) for
focus, blur, and diagnostic event variants.

## Testing

`plushie::test::TestSession` operates on the resolved accessibility
tree, so tests catch missing labels, wrong roles, and missing state
annotations before they ship. The tree is the same one AccessKit
would receive, with both widget-SDK inference and explicit
overrides applied.

```rust
use plushie::prelude::*;
use serde_json::json;

let mut session = TestSession::<MyApp>::start();

session.assert_role(Selector::id("save"), "button");
session.assert_a11y("email", &json!({"required": true, "invalid": false}));

let focused = session.find(Selector::focused());

let save = session.find(Selector::role("button"));
let email = session.find(Selector::label("Email address"));

// Full resolved record, as AccessKit will see it.
let resolved = session.resolved_a11y("email");
```

`assert_a11y` accepts any JSON object of expected fields; keys not
in `expected` are ignored. `session.assert_no_diagnostics()` fails
the test on any accumulated normalization warnings, including
`MissingAccessibleName` and `A11yRefUnresolved`.

## Platform notes

| Platform | AT service | Integration |
|---|---|---|
| macOS | VoiceOver | AccessKit to NSAccessibility |
| Linux | Orca (AT-SPI) | AccessKit to AT-SPI2 |
| Windows | NVDA / JAWS | AccessKit to UI Automation |

NVDA and JAWS operate in browse mode and focus mode, auto-switching
to focus mode when Tab reaches an interactive control. VoiceOver
uses a rotor for category-based navigation; correct roles ensure
widgets appear in the right rotor categories. Orca provides
structural navigation similar to browse mode. Wayland keyboard
input is currently broken for Linux screen readers, so Linux screen
reader users need X11.

## See also

- [Built-in widgets](built-in-widgets.md) for the widget list and
  the `a11y` setter available on every builder.
- [Events](events.md) for `Focused`, `Blurred`, and the
  `Diagnostic` variants emitted by normalization.
- [Commands](commands.md) for focus commands and
  `Command::announce`.
- [AccessKit](https://github.com/AccessKit/accesskit) for the
  cross-platform accessibility library Plushie drives.
