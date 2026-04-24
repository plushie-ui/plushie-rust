# Windows and layout

Every plushie app starts with a window. Inside it, layout containers
arrange children along axes, in grids, in z-order, or at absolute
coordinates. This reference covers the window surface and the shared
vocabulary (`Length`, `Padding`, `Align`, `Direction`, `Anchor`,
`Position`) that every widget builder speaks. The shared types live
in `plushie_core::types` and are re-exported as `plushie::types`; the
container builders live in `plushie::ui::layout`.

For the full list of container methods, see
[built-in widgets](built-in-widgets.md). This page focuses on the
types and patterns that compose across containers.

## Length

```rust
pub enum Length {
    Fill,
    Shrink,
    FillPortion(u32),
    Fixed(f32),
}
```

`Shrink` is the default. `Fixed(px)` takes a non-negative pixel
value. `FillPortion(n)` must be at least `1` (the encoder panics on
`0`). Any width or height setter takes `impl Into<Length>`, so plain
numeric literals coerce through the `From` impls:

| Setter input | Resolved `Length` |
|---|---|
| `Length::Fill` | `Fill` |
| `Length::Shrink` | `Shrink` |
| `Length::FillPortion(3)` | `FillPortion(3)` |
| `Length::Fixed(200.0)` | `Fixed(200.0)` |
| `200.0_f32` | `Fixed(200.0)` (via `From<f32>`) |
| `200_i32` | `Fixed(200.0)` (via `From<i32>`) |
| `200_u32` | `Fixed(200.0)` (via `From<u32>`) |

```rust
use plushie::prelude::*;

column()
    .width(Length::Fill)
    .height(Length::Fixed(400.0))
    .child(text("Hello"))
```

### How FillPortion works

Siblings that both request fill divide the remaining space after
fixed-size and `Shrink` siblings are measured. The numbers are
relative ratios, not percentages.

```rust
row()
    .width(Length::Fill)
    .children([
        container().width(Length::FillPortion(1)).child(sidebar(model)).into(),
        container().width(Length::FillPortion(3)).child(main(model)).into(),
    ])
```

The sidebar takes one quarter of the width; the main column takes
three quarters. `Length::Fill` behaves identically to
`Length::FillPortion(1)`.

### Sizing resolution order

The layout engine processes siblings in this order inside a `row` or
`column`:

1. `Length::Fixed(px)` children take their pixels.
2. `Length::Shrink` children take their intrinsic content size.
3. `Length::Fill` and `Length::FillPortion(n)` children divide the
   remaining space.

### Constraints

`max_width` and `max_height` cap a `Fill` child at a ceiling. They
are available on `column`, `row`, and `container`, and accept
`impl Into<Animatable<f32>>`, so both static `f32` and
`Animatable<f32>` values work.

## Padding

```rust
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
```

Construct uniformly, by axis, or per-side:

| Constructor | Result |
|---|---|
| `Padding::all(v)` | All four sides set to `v` |
| `Padding::axes(vertical, horizontal)` | Top/bottom and left/right pairs |
| `Padding::vertical(v)` | Top and bottom, left/right zero |
| `Padding::horizontal(v)` | Left and right, top/bottom zero |
| `Padding::top(v)` / `::right` / `::bottom` / `::left` | One side, others zero |
| `Padding::new(top, right, bottom, left)` | Fully explicit |

`From` impls accept plain numbers and tuples, so setter call sites
stay terse:

| Expression | Equivalent |
|---|---|
| `16.0_f32.into()` | `Padding::all(16.0)` |
| `16_i32.into()` | `Padding::all(16.0)` |
| `(16.0, 8.0).into()` | `Padding::axes(16.0, 8.0)` |
| `(16.0, 8.0, 16.0, 8.0).into()` | `Padding::new(16.0, 8.0, 16.0, 8.0)` |

```rust
container()
    .padding(Padding::axes(16.0, 8.0))
    .child(text("Padded"))
```

Padding is the gap between a container's edge and its child. It
reduces the space available to children: a 200px wide container
with `Padding::all(16.0)` has 168px of content width. Negative
values panic on encode.

## Align

```rust
pub enum Align { Start, Center, End }
```

`Align` is the SDK-level ergonomic enum used by layout containers.
It maps to different wire strings depending on context:

| Context | Start | Center | End |
|---|---|---|---|
| `align_x` (horizontal) | `"left"` | `"center"` | `"right"` |
| `align_y` (vertical) | `"top"` | `"center"` | `"bottom"` |
| `overlay::align` (cross-axis) | `"start"` | `"center"` | `"end"` |

| Setter | Container | Axis |
|---|---|---|
| `align_x(Align)` | `column`, `row`, `container`, `text` | Horizontal |
| `align_y(Align)` | `row`, `container`, `text` | Vertical |

A `column` stacks children vertically, so `align_x` positions them
left, center, or right. A `row` flows children horizontally, so
`align_y` positions them at the top, centre, or bottom. A
`container` wraps one child and supports both axes; the
`center(true)` shorthand sets both to `Align::Center`.

```rust
container()
    .width(Length::Fill)
    .height(Length::Fill)
    .center(true)
    .child(text("Centred in both axes"))
```

For text-specific alignment, the text builders additionally accept
`plushie::types::TextAlignment` (`Left`, `Center`, `Right`,
`Justify`) via `Align: Into<TextAlignment>`. Core exposes
`HorizontalAlignment` and `VerticalAlignment` variants used by the
wire codec; apps stick to `Align`.

## Direction

```rust
pub enum Direction { Horizontal, Vertical, Both }
```

Layout axis. Used by:

- `scrollable().direction(Direction)` - sets the scroll axis; `Both`
  enables scrolling on both axes.
- `rule().direction(Direction)` - horizontal or vertical divider.

## Anchor

```rust
pub enum Anchor { Start, End }
```

Used by `scrollable().anchor(Anchor)` to pin the scroll position to
the top/left (`Start`) or bottom/right (`End`). Pair
`Anchor::End` with `auto_scroll(true)` for chat-style interfaces
that follow new content as it arrives.

## Position

```rust
pub enum Position { Below, Above, Left, Right }
```

Used by `tooltip().position(Position)` and
`overlay().position(Position)` to place a floating node relative to
its anchor.

## Window

```rust
pub fn window(id: &str) -> WindowBuilder
```

The top-level container. `App::view` returns an `impl Into<ViewList>`,
and a window builder is the root of each list entry. The ID is
required: it is the root scope for every widget inside the window and
the stable key the runtime uses to reconcile windows across renders.

```rust
use plushie::prelude::*;

fn view(model: &Model, _: &mut WidgetRegistrar) -> ViewList {
    window("main")
        .title("Counter")
        .size(640.0, 480.0)
        .theme(Theme::Dark)
        .child(
            column()
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(16)
                .spacing(8)
                .child(text(&format!("Count: {}", model.count)))
                .child(button("inc", "Increment")),
        )
        .into()
}
```

The full window setter table lives in
[built-in widgets](built-in-widgets.md#window). Common setters:

| Setter | Purpose |
|---|---|
| `title(&str)` | Title bar text |
| `size(w, h)` | Initial size in logical pixels |
| `position(x, y)` | Initial screen position |
| `min_size(w, h)` / `max_size(w, h)` | Size bounds |
| `width(impl Into<Length>)` / `height(impl Into<Length>)` | Preferred size |
| `theme(impl Into<Theme>)` | Per-window theme |
| `maximized(bool)`, `fullscreen(bool)`, `visible(bool)` | Initial state |
| `resizable(bool)`, `decorations(bool)`, `transparent(bool)`, `blur(bool)` | Chrome |
| `closeable(bool)`, `minimizable(bool)` | Title-bar controls |
| `level(WindowLevel)` | `Normal`, `AlwaysOnTop`, `AlwaysOnBottom` |
| `exit_on_close_request(bool)` | Close button exits the app |
| `scale_factor(f64)` | DPI scale override |
| `event_rate(u32)` | Max coalescable events per second |

### Multi-window apps

A view returns a `ViewList`, so returning multiple windows is just
returning multiple builders. `ViewList: From<Vec<View>>` and
`ViewList: From<[View; N]>` handle the common shapes.

```rust
fn view(model: &Model, _: &mut WidgetRegistrar) -> ViewList {
    let mut windows: Vec<View> = vec![
        window("main").title("App").child(main_ui(model)).into(),
    ];

    if model.show_settings {
        windows.push(
            window("settings")
                .title("Settings")
                .exit_on_close_request(false)
                .child(settings_ui(model))
                .into(),
        );
    }

    windows.into()
}
```

`exit_on_close_request(false)` on secondary windows means closing
them removes the window without exiting the app. Window IDs must
be stable across renders; a changing ID causes a close and
re-open. Returning `()` (an empty list) renders an empty tree,
useful for a transient loading or error state.

### WindowConfig

Per-window defaults come from `App::window_config`, not from a
config file. The struct lives in `plushie::settings`
(re-exported from `plushie_core::settings`). Every field is
`Option<T>`; unset fields fall back to renderer defaults.

```rust
use plushie::settings::WindowConfig;
use plushie::types::{Theme, WindowLevel};

impl App for MyApp {
    // ...
    fn window_config() -> WindowConfig {
        WindowConfig {
            title: Some("My App".into()),
            width: Some(800.0),
            height: Some(600.0),
            min_size: Some((320.0, 240.0)),
            theme: Some(Theme::Nord),
            level: Some(WindowLevel::Normal),
            ..Default::default()
        }
    }
}
```

The settings in `window_config` apply at startup. Setters on a
`window(id)` builder inside `view` override them per render and
per window.

## Layout containers

The container builders in `plushie::ui::layout` all follow the
same shape: a zero-arg or ID-keyed constructor, chainable setters
returning `Self`, and an implicit conversion to `View` at the
container boundary. See
[built-in widgets](built-in-widgets.md#layout) for the full setter
tables. The summary below gives each container's signature, role,
and a concise example.

### column

```rust
pub fn column() -> ColumnBuilder
```

Arranges children vertically, top to bottom. Auto-ID, override
with `.id("name")` when a scope prefix is useful. Setters include
`spacing`, `padding`, `width`, `height`, `max_width`, `align_x`,
`clip`, `wrap`, `child`, `children`.

```rust
column()
    .spacing(12)
    .padding(Padding::all(16.0))
    .align_x(Align::Center)
    .children([
        text("First").into(),
        text("Second").into(),
        text("Third").into(),
    ])
```

`wrap(true)` flows children into additional columns once the
current one fills.

### row

```rust
pub fn row() -> RowBuilder
```

Arranges children horizontally, left to right. Same setter set as
`column` plus `align_y`. `wrap(true)` flows into additional rows
(useful for tag clouds and reflowing toolbars).

### container

```rust
pub fn container() -> ContainerBuilder
```

Single-child wrapper. Carries three responsibilities:

- Styling: `background`, `color`, `border`, `shadow`, `style`.
- Scoping: `.id(prefix)` creates a scope for descendants (see
  [scoped IDs](scoped-ids.md)).
- Alignment: `align_x`, `align_y`, `center`.

```rust
container()
    .padding(Padding::all(16.0))
    .background(Color::rgb(0.95, 0.95, 0.97))
    .border(Border::new().width(1.0).radius(8.0))
    .child(text("Boxed"))
```

### scrollable

```rust
pub fn scrollable() -> ScrollableBuilder
```

Single-child viewport with scrollbars. Holds renderer-side scroll
state, so it benefits from an explicit `.id("name")` when the
subtree identity matters. Axis via `.direction(Direction)`,
starting position via `.anchor(Anchor)`, follow-new-content via
`.auto_scroll(true)`.

```rust
scrollable()
    .id("messages")
    .direction(Direction::Vertical)
    .anchor(Anchor::End)
    .auto_scroll(true)
    .height(Length::Fixed(400.0))
    .child(message_list(model))
```

### keyed_column

```rust
pub fn keyed_column() -> KeyedColumnBuilder
```

A `column` that diffs children by ID rather than by position. Use
for dynamic lists where items are added, removed, or reordered.
A positional `column` shifts widget state when the list changes;
`keyed_column` preserves focus, scroll, and cursor state by
matching IDs.

### stack

```rust
pub fn stack() -> StackBuilder
```

Layers children along the z-axis. Later children render above
earlier ones. Useful for overlays, badges, and loading spinners.

### grid

```rust
pub fn grid() -> GridBuilder
```

Fixed-column or fluid grid. `.num_columns(n)` sets a fixed column
count; `.fluid(max_cell_width)` enables auto-wrap where the column
count adjusts to the available width.

```rust
grid()
    .num_columns(3)
    .spacing(8.0)
    .children(model.photos.iter().map(photo_cell).collect::<Vec<_>>())
```

### pin

```rust
pub fn pin() -> PinBuilder
```

Positions a single child at absolute `(x, y)` pixel coordinates
within the parent. The child is removed from flow layout.

```rust
pin()
    .x(10.0)
    .y(10.0)
    .child(text("Badge"))
```

### floating

```rust
pub fn floating() -> FloatingBuilder
```

Applies translate and scale transforms to a single child without
removing it from flow. The child still occupies its original
space; the transform is visual only.

### responsive

```rust
pub fn responsive() -> ResponsiveBuilder
```

Emits resize events when the available size changes. Store the
measured size in the model and branch `view` on it (sidebar
above a width, stacked below it).

### pane_grid

```rust
pub fn pane_grid(id: &str) -> PaneGridBuilder
```

Resizable tiled panes with split, close, swap, and drag. ID is
required because the renderer tracks pane layout as internal
state. Pane management happens through commands (see
[commands](commands.md)).

### space

```rust
pub fn space() -> SpaceBuilder
```

Invisible spacer. Use for explicit gaps and for pushing siblings
apart inside a `row` or `column`.

## Spacing and padding idioms

`spacing` inserts a gap between sibling children inside a `column`,
`row`, `grid`, or `keyed_column`. It does not apply before the
first or after the last child. `padding` inserts a gap between a
container's edges and its single or multiple children. The two
settings compose independently.

```rust
column()
    .padding(Padding::axes(16.0, 24.0))
    .spacing(12.0)
    .children([
        button("save", "Save").into(),
        button("cancel", "Cancel").into(),
    ])
```

A `row` with `.spacing(8.0).padding(16.0)` draws 16px of inner
padding on all sides and 8px between adjacent children.

## Width and height patterns

Every layout container accepts the same family of size setters.
Common patterns:

- Fixed sidebar, fluid main area:
  ```rust
  row()
      .width(Length::Fill)
      .height(Length::Fill)
      .children([
          column().width(Length::Fixed(200.0)).child(sidebar(model)).into(),
          container().width(Length::Fill).child(main(model)).into(),
      ])
  ```

- Header / body / footer (header and footer shrink, body fills):
  ```rust
  column()
      .width(Length::Fill)
      .height(Length::Fill)
      .child(header(model))
      .child(container().width(Length::Fill).height(Length::Fill).child(body(model)))
      .child(footer(model))
  ```

- Scrollable list capped at a height:
  ```rust
  scrollable()
      .id("items")
      .height(Length::Fixed(400.0))
      .child(
          keyed_column()
              .spacing(4.0)
              .children(model.items.iter().map(item_row).collect::<Vec<_>>()),
      )
  ```

- Overlay badge on top of content:
  ```rust
  stack()
      .width(Length::Fill)
      .height(Length::Fill)
      .child(main_content(model))
      .child(pin().x(10.0).y(10.0).child(text("NEW").size(10.0)))
  ```

## See also

- [Built-in widgets](built-in-widgets.md) for the full setter
  tables on every container.
- [Themes and styling](themes-and-styling.md) for `Color`,
  `Theme`, `Border`, `Shadow`, and `Background`.
- [Events](events.md) for the events `responsive`, `pane_grid`,
  and `scrollable` emit.
- [Scoped IDs](scoped-ids.md) for how container IDs prefix their
  descendants.
- [Canvas](canvas.md) for absolute-positioned drawing inside a
  layout tree.
