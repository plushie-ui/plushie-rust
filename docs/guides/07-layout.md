# Layout

The todo list from [chapter 6](06-lists-and-inputs.md) renders its items in
a single `column`, and that gets the mechanics right. Real UIs have more
structure: a sidebar next to the main view, a toolbar above a list, a
status bar underneath, a scrollable log pane. This chapter covers the
vocabulary that turns flat trees into laid-out interfaces: rows and
columns, `Length`, `Padding`, `Align`, and the handful of containers that
compose them.

For the reference-style tour of every container, see
[windows and layout](../reference/windows-and-layout.md). Here we focus on
the patterns you reach for daily.

## Rows and columns

`column` stacks its children vertically, top to bottom. `row` stacks them
horizontally, left to right. Both are zero-argument builders and both
accept the same core setters: `spacing`, `padding`, `width`, `height`,
and a per-axis alignment setter.

```rust
use plushie::prelude::*;

column()
    .spacing(12.0)
    .padding(Padding::all(16.0))
    .children([
        text("First").into(),
        text("Second").into(),
        text("Third").into(),
    ])
```

`spacing` is the gap between siblings. `padding` is the gap between the
container's edges and its children. The two compose independently: a
`row` with `.spacing(8.0).padding(Padding::all(16.0))` has 16 pixels of
inner padding on every side and 8 pixels between adjacent children.

A `row` can flow onto additional lines with `.wrap(true)`. This is what
turns a horizontal toolbar into a tag cloud when the window narrows.
`column` has the same setter, flowing into additional columns once the
first fills.

## Length

Every sizeable widget accepts a `Length` for width and height. The enum
lives in `plushie::types`:

```rust
pub enum Length {
    Fill,
    Shrink,
    FillPortion(u32),
    Fixed(f32),
}
```

The variants describe how a child negotiates for space:

| Variant | Behaviour |
|---|---|
| `Fill` | Take everything left over |
| `Shrink` | Take only what the content needs (the default) |
| `FillPortion(n)` | Take a proportional share of leftover space |
| `Fixed(px)` | Take an exact pixel count |

A `row` with one `Fixed` child and one `Fill` child gives the fixed child
its pixels, then hands the remainder to the filler:

```rust
row()
    .width(Length::Fill)
    .children([
        container().width(Length::Fixed(200.0)).child(sidebar(model)).into(),
        container().width(Length::Fill).child(main(model)).into(),
    ])
```

When two siblings both want fill, they share. `FillPortion(n)` makes the
split proportional. A `FillPortion(1)` sibling next to a `FillPortion(3)`
sibling takes one quarter of the remaining width; the other takes three
quarters. `Length::Fill` is shorthand for `FillPortion(1)`.

```rust
row()
    .width(Length::Fill)
    .children([
        container().width(Length::FillPortion(1)).child(sidebar(model)).into(),
        container().width(Length::FillPortion(3)).child(main(model)).into(),
    ])
```

Width and height setters accept `impl Into<Length>`, so plain numeric
literals coerce to `Length::Fixed` through the `From` impls. These are
equivalent:

```rust
container().width(Length::Fixed(48.0))
container().width(48.0_f32)
container().width(48)
```

Stick to the explicit form in mixed code where a reader has to tell
`Fixed` apart from a percentage or a factor. Use the coerced form when
the intent is obviously pixels.

The resolution order inside a `row` or `column` is: `Fixed` first,
`Shrink` next, then `Fill` and `FillPortion` divide whatever remains.

### Capping a filler

`max_width` and `max_height` bound a `Fill` child. This is useful for
reading columns that should stretch with the window up to a limit, then
stop:

```rust
container()
    .width(Length::Fill)
    .max_width(720.0)
    .center(true)
    .child(article(model))
```

`column`, `row`, and `container` all accept `max_width`. `container`
additionally accepts `max_height`.

## Padding

`Padding` has per-side fields for `top`, `right`, `bottom`, and `left`.
The struct lives in `plushie::types` and has constructors for every
shape you normally want:

```rust
use plushie::types::Padding;

Padding::all(16.0)              // all sides 16
Padding::axes(16.0, 8.0)        // top/bottom 16, left/right 8
Padding::vertical(16.0)         // top and bottom only
Padding::horizontal(8.0)        // left and right only
Padding::new(16.0, 8.0, 16.0, 8.0)  // top, right, bottom, left
```

Setter call sites accept anything that implements `Into<Padding>`, so
plain numbers and tuples work:

```rust
container().padding(16.0)           // Padding::all(16.0)
container().padding((16.0, 8.0))    // Padding::axes(16.0, 8.0)
container().padding(Padding::top(12.0))
```

Padding reduces the space available to children. A 200 pixel wide
container with `Padding::all(16.0)` has 168 pixels of content width.
Negative values panic at encode time.

## Alignment

`Align` controls how children sit inside a container's available space:

```rust
pub enum Align { Start, Center, End }
```

A `column` stacks children down, so `align_x` positions them horizontally
(`Start` is left, `End` is right). A `row` flows children across, so
`align_y` positions them vertically (`Start` is top, `End` is bottom).
`container` wraps a single child and accepts both axes.

```rust
container()
    .width(Length::Fill)
    .height(Length::Fixed(200.0))
    .align_x(Align::Center)
    .align_y(Align::Center)
    .child(text("Centred"))
```

The `center(true)` shorthand on `container` sets both axes to
`Align::Center`:

```rust
container()
    .width(Length::Fill)
    .height(Length::Fill)
    .center(true)
    .child(text("Centred both ways"))
```

For text-specific alignment, `text`'s `align_x` takes `impl Into<TextAlignment>`,
which accepts `Align` as well as `TextAlignment::Justify`. Prefer `Align`
unless you specifically need justified text.

## Containers

`container` is a single-child wrapper. It carries background and
border styling, an optional scope ID for its subtree, and sizing and
alignment of its child. Use it whenever you want
to draw a box around something.

```rust
container()
    .padding(Padding::all(16.0))
    .background(Color::rgb(0.95, 0.95, 0.97))
    .border(Border::new().width(1.0).radius(8.0))
    .child(
        column()
            .spacing(8.0)
            .child(text("Profile").size(18.0))
            .child(text(&model.name))
            .child(text(&model.email)),
    )
```

`background`, `border`, and `shadow` are the styling hooks; chapter 8
covers them in detail. For now, note that a bare `container()` with
nothing but a child is still useful: an explicit `container` gives a
subtree an ID scope, which keeps its descendant events addressable as
`card/save` rather than the anonymous auto-IDs.

## Nested layouts

Real screens compose the containers above into familiar shapes. The
sidebar-plus-content layout is a `row` with one fixed-width column and
one `Fill` main area:

```rust
use plushie::prelude::*;

fn view(model: &Model, _widgets: &mut WidgetRegistrar) -> ViewList {
    window("main")
        .title("Inbox")
        .size(960.0, 640.0)
        .child(
            row()
                .width(Length::Fill)
                .height(Length::Fill)
                .children([
                    sidebar(model).into(),
                    content(model).into(),
                ]),
        )
        .into()
}

fn sidebar(model: &Model) -> impl Into<View> {
    container()
        .id("sidebar")
        .width(Length::Fixed(240.0))
        .height(Length::Fill)
        .padding(Padding::all(12.0))
        .background(Color::rgb(0.95, 0.95, 0.97))
        .child(
            column()
                .spacing(4.0)
                .children(model.folders.iter().map(folder_entry).collect::<Vec<_>>()),
        )
}

fn content(model: &Model) -> impl Into<View> {
    column()
        .width(Length::Fill)
        .height(Length::Fill)
        .child(toolbar(model))
        .child(
            container()
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(Padding::all(16.0))
                .child(message_list(model)),
        )
}
```

The outer `row` gives the two children the full window. The sidebar
takes its 240 pixels first, the content area takes whatever is left.
Inside the content area, a `column` stacks the toolbar above a `Fill`
container wrapping the message list, which is the standard header over
body pattern.

Break a view into small functions that return `impl Into<View>`. The
`into()` calls at the splice point keep the parent tree readable and
let the inner function swap its root container type later without
touching the caller.

## Responsive sizing

Layout-aware widgets emit size events the app can branch on.
`responsive` fires a resize event when the available size inside it
changes. Drop a child in, branch on the measured width inside `update`,
and re-render. `sensor` is similar but general: attach it to any
subtree and opt into resize events with `.on_resize("tag")`, and the
subtree reports its own box rather than the available space.

For window-level geometry, the `Subscription::on_window_resize()`
subscription is usually what you want. Store the width in the model and
pick a layout in `view`:

```rust
use plushie::prelude::*;

fn subscribe(_model: &Model) -> Vec<Subscription> {
    vec![Subscription::on_window_resize()]
}

fn view(model: &Model, _: &mut WidgetRegistrar) -> ViewList {
    let wide = model.window_width >= 720.0;
    let body: View = if wide {
        row().children([sidebar(model).into(), main(model).into()]).into()
    } else {
        column().children([main(model).into(), sidebar(model).into()]).into()
    };

    window("main").size(960.0, 640.0).child(body).into()
}
```

The [events chapter](05-events.md#window-events) covers the window event
shape. Subscriptions proper are [chapter 10](10-subscriptions.md).

## Grid

`grid` is for uniform cells. Either fix the column count explicitly with
`num_columns`, or let the cells auto-wrap with `fluid(max_cell_width)`.
Use `spacing` for inter-cell gaps.

```rust
grid()
    .num_columns(3)
    .spacing(8.0)
    .children(model.photos.iter().map(photo_cell).collect::<Vec<_>>())
```

The grid shines for aligned form layouts, where labels and inputs sit in
opposing columns and widgets of different intrinsic sizes line up
anyway. A `pick_list` next to a `slider` in a `row` will cling to their
shrink sizes and drift out of alignment as values change; drop them into
a two-column grid and the left edges stay put:

```rust
grid()
    .num_columns(2)
    .spacing(8.0)
    .children([
        text("Quality").into(),
        pick_list("quality", &["Low", "Medium", "High"], Some(&model.quality)).into(),
        text("Volume").into(),
        slider("volume", (0.0, 100.0), model.volume).into(),
    ])
```

The `fluid` variant is how you build a photo wall or a card grid that
reflows as the window resizes, without computing column counts in the
app.

## Scrollable

Wrap a tall subtree in `scrollable` to get scroll bars when content
overflows. The scroll bar's axis is `direction`; pin the initial
position with `anchor`; follow new content as it arrives with
`auto_scroll(true)` (chat rooms and log viewers want this paired with
`Anchor::End`).

```rust
use plushie::types::{Anchor, Direction, Length};

scrollable()
    .id("messages")
    .height(Length::Fixed(400.0))
    .direction(Direction::Vertical)
    .anchor(Anchor::End)
    .auto_scroll(true)
    .child(message_list(model))
```

Give `scrollable` an explicit `id` whenever identity matters across
renders. The renderer stores its scroll position keyed by that ID.
Without a stable ID, an auto-generated one drifts across edits and the
viewport jumps back to the top. This mirrors the advice from
[chapter 6](06-lists-and-inputs.md) about stable IDs on list containers.

A scrollable area needs a bounded axis. Wrapping an unbounded `column`
in a `scrollable` without a `height` gives the scrollable nothing to
measure against, and nothing will scroll. Either set a `Fixed` height,
or let it sit inside a parent that already has a bounded height (for
instance, the body slot of a header-body-footer column where the outer
column is `Length::Fill`).

## What's next

With rows, columns, containers, and the sizing vocabulary, you can lay
out most screens. Padding and alignment polish the geometry; grids and
scrollable areas cover the cases that plain stacking cannot. What is
missing is colour, typography, and the visual language that makes a
layout look finished. That is [chapter 8](08-styling.md).

---

Next: [Styling](08-styling.md)
