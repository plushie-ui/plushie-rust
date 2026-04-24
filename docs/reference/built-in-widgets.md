# Built-in widgets

The built-in widget catalog lives under `plushie::ui`. Every
widget has a constructor function and a matching builder struct.
Constructors return the builder; chained setters return the
builder by value; conversion into a `View` happens through an
`Into<View>` impl that container builders call when the builder
is passed to `.child()` or `.children([..])`.

## The builder pattern

A widget call is a short pipeline: constructor, zero or more
setters, then a final implicit conversion when the value reaches
a `child` / `children` slot or the top-level `ViewList`. Every
setter takes `self` by value and returns `Self`, so a builder
can flow through chained calls without intermediate bindings.

```rust
use plushie::prelude::*;

let save = button("save", "Save")
    .style(Style::primary())
    .width(Length::Fixed(120.0))
    .padding(Padding::all(8));

let screen = column()
    .spacing(8)
    .padding(16)
    .children([text("Hello, world!").into(), save.into()]);
```

Constructors split into two groups. Interactive and stateful
widgets take the ID as the first argument, which is the stable
key used for event routing. Layout containers and display leaves
use `#[track_caller]` to derive an auto-ID from the call site,
and expose `.id("explicit-id")` when a scope prefix is needed.

## Layout

Layout widgets control spatial arrangement. They live in
`plushie::ui::layout` and are re-exported from `plushie::ui`.

### window

```rust
pub fn window(id: &str) -> WindowBuilder
```

Top-level window. ID is required. A view function returns a
`ViewList` of one or more window builders.

| Method | Signature | Description |
|---|---|---|
| `title` | `(title: &str) -> Self` | Title bar text |
| `size` | `(w: f32, h: f32) -> Self` | Initial size in logical pixels |
| `position` | `(x: f32, y: f32) -> Self` | Initial screen position |
| `min_size` | `(w: f32, h: f32) -> Self` | Minimum size |
| `max_size` | `(w: f32, h: f32) -> Self` | Maximum size |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height |
| `theme` | `(theme: impl Into<Theme>) -> Self` | Window theme |
| `scale_factor` | `(factor: f64) -> Self` | DPI scale override |
| `maximized` | `(v: bool) -> Self` | Start maximized |
| `fullscreen` | `(v: bool) -> Self` | Start fullscreen |
| `visible` | `(v: bool) -> Self` | Visible at start |
| `resizable` | `(v: bool) -> Self` | User can resize |
| `decorations` | `(v: bool) -> Self` | Show native decorations |
| `transparent` | `(v: bool) -> Self` | Transparent background |
| `closeable` | `(v: bool) -> Self` | Show close button |
| `minimizable` | `(v: bool) -> Self` | Allow minimize |
| `blur` | `(v: bool) -> Self` | Platform-dependent blur |
| `level` | `(level: WindowLevel) -> Self` | Stacking level |
| `exit_on_close_request` | `(v: bool) -> Self` | Close request exits the app |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec; 0 = unbounded |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` | `(c: impl Into<View>) -> Self` | Append a child |
| `children` | `(items: I) -> Self` | Replace child list |

### column

```rust
pub fn column() -> ColumnBuilder
```

Arranges children vertically. Auto-ID.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `spacing` | `(v: impl Into<Animatable<f32>>) -> Self` | Space between children |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height |
| `max_width` | `(w: impl Into<Animatable<f32>>) -> Self` | Maximum width |
| `align_x` | `(a: Align) -> Self` | Horizontal alignment |
| `clip` | `(v: bool) -> Self` | Clip overflowing content |
| `wrap` | `(v: bool) -> Self` | Wrap to new line when out of room |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` | `(c: impl Into<View>) -> Self` | Append a child |
| `children` | `(items: I) -> Self` | Replace child list |

### row

```rust
pub fn row() -> RowBuilder
```

Arranges children horizontally. Auto-ID. Same setters as
`column` plus `align_y(a: Align)`.

### container

```rust
pub fn container() -> ContainerBuilder
```

Single-child wrapper for styling, alignment, and sizing.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height |
| `max_width` | `(v: impl Into<Animatable<f32>>) -> Self` | Maximum width |
| `max_height` | `(v: impl Into<Animatable<f32>>) -> Self` | Maximum height |
| `align_x` | `(a: Align) -> Self` | Horizontal alignment |
| `align_y` | `(a: Align) -> Self` | Vertical alignment |
| `clip` | `(v: bool) -> Self` | Clip overflowing content |
| `background` | `(bg: impl Into<Animatable<Background>>) -> Self` | Background color or gradient |
| `color` | `(c: impl Into<Animatable<Color>>) -> Self` | Foreground color |
| `border` | `(b: Border) -> Self` | Border |
| `shadow` | `(s: Shadow) -> Self` | Drop shadow |
| `center` | `(v: bool) -> Self` | Center the child |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` | `(c: impl Into<View>) -> Self` | Single child |

### scrollable

```rust
pub fn scrollable() -> ScrollableBuilder
```

Scrollable viewport around a single child. Holds
renderer-side scroll position keyed by ID.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height |
| `spacing` | `(v: impl Into<Animatable<f32>>) -> Self` | Spacing |
| `direction` | `(dir: Direction) -> Self` | Scroll axis |
| `anchor` | `(a: Anchor) -> Self` | Anchor at start or end |
| `on_scroll` | `(v: bool) -> Self` | Emit scroll viewport events |
| `auto_scroll` | `(v: bool) -> Self` | Follow new content at the anchor end |
| `scrollbar_width` | `(v: impl Into<Animatable<f32>>) -> Self` | Track width |
| `scrollbar_margin` | `(v: impl Into<Animatable<f32>>) -> Self` | Track margin |
| `scroller_width` | `(v: impl Into<Animatable<f32>>) -> Self` | Handle width |
| `scrollbar_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Track color |
| `scroller_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Handle color |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` | `(c: impl Into<View>) -> Self` | Single child |

### stack

```rust
pub fn stack() -> StackBuilder
```

Layers children on top of each other along the z-axis.
Later children render above earlier ones.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height |
| `clip` | `(v: bool) -> Self` | Clip overflow |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` / `children` |  | Append / replace children |

### grid

```rust
pub fn grid() -> GridBuilder
```

Fixed-column or fluid grid. Use `num_columns` for a
fixed-column grid, or `fluid(max_cell_width)` for
auto-wrapping cells.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `num_columns` | `(n: u32) -> Self` | Fixed column count |
| `fluid` | `(max_cell_width: f32) -> Self` | Enable fluid mode |
| `column_width` | `(w: impl Into<Length>) -> Self` | Column width |
| `row_height` | `(h: impl Into<Length>) -> Self` | Row height |
| `spacing` | `(v: impl Into<Animatable<f32>>) -> Self` | Cell spacing |
| `width` | `(w: f32) -> Self` | Grid width in pixels |
| `height` | `(h: f32) -> Self` | Grid height in pixels |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` / `children` |  | Append / replace children |

### keyed_column

```rust
pub fn keyed_column() -> KeyedColumnBuilder
```

Same layout as `column`, but children are diffed by key
rather than position. Use for dynamic lists where items
are added, removed, or reordered. Same setters as `column`
minus `align_y`, `clip`, and `wrap`.

### pin

```rust
pub fn pin() -> PinBuilder
```

Positions a single child at absolute `(x, y)` pixel
coordinates within the parent.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `x` | `(v: impl Into<Animatable<f32>>) -> Self` | X offset in pixels |
| `y` | `(v: impl Into<Animatable<f32>>) -> Self` | Y offset in pixels |
| `width` / `height` |  | Preferred size |
| `event_rate`, `a11y`, `child` |  | Standard |

### floating

```rust
pub fn floating() -> FloatingBuilder
```

Applies translate and scale transforms to a single child.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit scope ID |
| `translate_x` | `(v: impl Into<Animatable<f32>>) -> Self` | X translation |
| `translate_y` | `(v: impl Into<Animatable<f32>>) -> Self` | Y translation |
| `scale` | `(v: impl Into<Animatable<f32>>) -> Self` | Scale factor |
| `width` / `height` |  | Preferred size |
| `event_rate`, `a11y`, `child` |  | Standard |

### responsive

```rust
pub fn responsive() -> ResponsiveBuilder
```

Emits resize events as the available size changes. Single
child, no intrinsic geometry overrides beyond `width`,
`height`, `event_rate`, `a11y`, `child`.

### pane_grid

```rust
pub fn pane_grid(id: &str) -> PaneGridBuilder
```

Resizable tiled panes. ID is required because panes hold
renderer-side state (sizes, arrangement).

| Method | Signature | Description |
|---|---|---|
| `panes` | `(pane_ids: &[&str]) -> Self` | Pane identifiers |
| `spacing` | `(v: impl Into<Animatable<f32>>) -> Self` | Space between panes |
| `width` / `height` |  | Preferred size |
| `split_axis` | `(axis: &str) -> Self` | Initial split direction (`"horizontal"` or `"vertical"`) |
| `min_size` | `(v: impl Into<Animatable<f32>>) -> Self` | Minimum pane size |
| `divider_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Divider colour |
| `divider_width` | `(v: impl Into<Animatable<f32>>) -> Self` | Divider thickness |
| `leeway` | `(v: impl Into<Animatable<f32>>) -> Self` | Grabbable margin around dividers |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` / `children` |  | Append / replace children |

Each child's ID must match an entry in `panes`.

### space

```rust
pub fn space() -> SpaceBuilder
```

Invisible spacer. Setters: `id`, `width`, `height`,
`event_rate`, `a11y`.

## Display

Display widgets render content without accepting user
input. They live in `plushie::ui::display`.

### text

```rust
pub fn text(content: &str) -> TextBuilder
```

Static text. Auto-ID. Pass an explicit scope ID via
`.id("greeting")`.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Font size |
| `color` | `(c: impl Into<Animatable<Color>>) -> Self` | Text color |
| `font` | `(f: Font) -> Self` | Family and weight |
| `width` / `height` |  | Preferred size |
| `align_x` | `(a: impl Into<TextAlignment>) -> Self` | Horizontal alignment |
| `align_y` | `(a: Align) -> Self` | Vertical alignment |
| `text_direction` | `(d: TextDirection) -> Self` | Logical text direction |
| `wrapping` | `(w: Wrapping) -> Self` | Line wrap strategy |
| `shaping` | `(s: Shaping) -> Self` | Shaping engine |
| `line_height` | `(lh: impl Into<Animatable<LineHeight>>) -> Self` | Line height |
| `ellipsis` | `(e: Ellipsis) -> Self` | Trailing ellipsis |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |

```rust
text("Hello, world!")
    .size(16.0)
    .color(Color::rgb(0.2, 0.4, 0.9))
```

### rich_text

```rust
pub fn rich_text() -> RichTextBuilder
pub fn rich_text_id(id: &str) -> RichTextBuilder
```

Styled text composed of individually formatted spans. The
spans themselves are `plushie::ui::Span` builders.

| Method | Signature | Description |
|---|---|---|
| `spans` | `(spans: Vec<Span>) -> Self` | Ordered typed spans |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Default font size |
| `font` | `(f: Font) -> Self` | Default font |
| `color` | `(c: impl Into<Animatable<Color>>) -> Self` | Default color |
| `width` / `height` |  | Preferred size |
| `line_height` | `(lh: impl Into<Animatable<LineHeight>>) -> Self` | Line height |
| `wrapping` | `(w: Wrapping) -> Self` | Line wrap strategy |
| `ellipsis` | `(e: Ellipsis) -> Self` | Trailing ellipsis |
| `event_rate`, `a11y`, `id` |  | Standard |

`Span` carries its own builder: `Span::new(text)` plus
chainable `.size(f32)`, `.font(Font)`, `.color(impl Into<Color>)`,
`.line_height(..)`, `.link(url)`, `.underline(bool)`,
`.strikethrough(bool)`, `.padding(..)`, and `.highlight(SpanHighlight)`.
`SpanHighlight::new()` takes `.background(color)` and
`.border(Border)`.

```rust
use plushie::ui::{rich_text, Span};

rich_text()
    .id("greeting")
    .spans(vec![
        Span::new("Hello, ").size(16.0),
        Span::new("world").size(16.0).color(Color::rgb(0.23, 0.51, 0.96)).underline(true),
        Span::new("!").size(16.0),
    ])
```

### rule

```rust
pub fn rule() -> RuleBuilder
```

Horizontal or vertical divider line.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `direction` | `(d: Direction) -> Self` | Horizontal or vertical |
| `width` | `(w: impl Into<Animatable<f32>>) -> Self` | Width when vertical |
| `height` | `(h: impl Into<Animatable<f32>>) -> Self` | Height when horizontal |
| `thickness` | `(t: f32) -> Self` | Direction-agnostic thickness |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate`, `a11y` |  | Standard |

### progress_bar

```rust
pub fn progress_bar(range: (f32, f32), value: f32) -> ProgressBarBuilder
```

Progress indicator. Range is a `(min, max)` tuple; `value`
is the current progress.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `width` / `height` |  | Preferred size |
| `vertical` | `(v: bool) -> Self` | Render as a vertical bar |
| `label` | `(l: &str) -> Self` | Accessible label |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate`, `a11y` |  | Standard |

### image

```rust
pub fn image(source: &str) -> ImageBuilder
```

Raster image from a file path.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `width` / `height` |  | Preferred size |
| `content_fit` | `(fit: ContentFit) -> Self` | Scale-to-fit strategy |
| `filter_method` | `(method: FilterMethod) -> Self` | Pixel interpolation |
| `rotation` | `(angle: impl Into<Animatable<Angle>>) -> Self` | Rotation angle |
| `opacity` | `(o: impl Into<Animatable<f32>>) -> Self` | Alpha in 0..=1 |
| `border_radius` | `(r: impl Into<Animatable<f32>>) -> Self` | Rounded corners |
| `expand` | `(v: bool) -> Self` | Fill available space |
| `scale` | `(s: impl Into<Animatable<f32>>) -> Self` | Uniform scale |
| `crop` | `(x, y, w, h: f32) -> Self` | Source crop rectangle |
| `alt` | `(alt: &str) -> Self` | Short alt text |
| `description` | `(desc: &str) -> Self` | Long description |
| `decorative` | `(v: bool) -> Self` | Hide from assistive tech |
| `event_rate`, `a11y` |  | Standard |

### svg

```rust
pub fn svg(source: &str) -> SvgBuilder
```

Vector image from an SVG file.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `width` / `height` |  | Preferred size |
| `color` | `(c: impl Into<Animatable<Color>>) -> Self` | Tint color |
| `content_fit` | `(fit: ContentFit) -> Self` | Scale-to-fit strategy |
| `rotation` | `(angle: impl Into<Animatable<Angle>>) -> Self` | Rotation angle |
| `opacity` | `(o: impl Into<Animatable<f32>>) -> Self` | Alpha in 0..=1 |
| `alt`, `description`, `decorative` |  | Accessibility metadata |
| `event_rate`, `a11y` |  | Standard |

### markdown

```rust
pub fn markdown(content: &str) -> MarkdownBuilder
```

Rendered markdown block.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `text_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Body font size |
| `h1_size`, `h2_size`, `h3_size` |  | Heading font sizes |
| `code_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Code font size |
| `spacing` | `(s: impl Into<Animatable<f32>>) -> Self` | Space between blocks |
| `link_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Link color |
| `code_theme` | `(theme: &str) -> Self` | Syntax theme name |
| `event_rate`, `a11y` |  | Standard |

### qr_code

```rust
pub fn qr_code(data: &str) -> QrCodeBuilder
```

QR code encoding the given data string.

| Method | Signature | Description |
|---|---|---|
| `id` | `(id: &str) -> Self` | Explicit ID |
| `cell_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Module side length |
| `total_size` | `(s: f32) -> Self` | Total rendered size |
| `error_correction` | `(level: ErrorCorrection) -> Self` | Error-correction level |
| `cell_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Filled-cell color |
| `background` | `(c: impl Into<Animatable<Color>>) -> Self` | Background color |
| `alt` / `description` |  | Accessibility text |
| `event_rate`, `a11y` |  | Standard |

## Input

Input widgets accept user input. They live in
`plushie::ui::input`. ID is always the first argument
because interactive widgets need stable, explicit IDs.

### text_input

```rust
pub fn text_input(id: &str, value: &str) -> TextInputBuilder
```

Single-line editable field. Holds renderer-side cursor
and selection state keyed by ID.

| Method | Signature | Description |
|---|---|---|
| `placeholder` | `(p: &str) -> Self` | Empty-state text |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Font size |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `font` | `(f: Font) -> Self` | Font |
| `line_height` | `(lh: impl Into<Animatable<LineHeight>>) -> Self` | Line height |
| `on_submit` | `(enabled: bool) -> Self` | Emit on Enter |
| `on_paste` | `(enabled: bool) -> Self` | Emit on paste |
| `secure` | `(enabled: bool) -> Self` | Password masking |
| `align_x` | `(a: Align) -> Self` | Horizontal alignment |
| `icon` | `(icon: PropValue) -> Self` | Leading icon |
| `input_purpose` | `(purpose: InputPurpose) -> Self` | Keyboard hint |
| `placeholder_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Placeholder color |
| `selection_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Selection highlight |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `required` | `(v: bool) -> Self` | Flows into `a11y.required` |
| `validation` | `(v: Validation) -> Self` | Form-validation state |
| `event_rate`, `a11y` |  | Standard |

### text_editor

```rust
pub fn text_editor(id: &str, content: &str) -> TextEditorBuilder
```

Multi-line editor. Holds renderer-side cursor, selection,
scroll, and undo state keyed by ID.

| Method | Signature | Description |
|---|---|---|
| `placeholder` | `(p: &str) -> Self` | Empty-state text |
| `width` / `height` |  | Preferred size |
| `min_height` | `(h: impl Into<Animatable<f32>>) -> Self` | Minimum height |
| `max_height` | `(h: impl Into<Animatable<f32>>) -> Self` | Maximum height |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `font` | `(f: Font) -> Self` | Font |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Font size |
| `line_height` | `(lh: impl Into<Animatable<LineHeight>>) -> Self` | Line height |
| `wrapping` | `(w: Wrapping) -> Self` | Line wrap |
| `text_direction` | `(d: TextDirection) -> Self` | Logical direction |
| `input_purpose` | `(purpose: InputPurpose) -> Self` | Keyboard hint |
| `highlight_syntax` | `(lang: &str) -> Self` | Syntax highlight language |
| `highlight_theme` | `(theme: &str) -> Self` | Highlight theme |
| `key_bindings` | `(bindings: PropValue) -> Self` | Declarative key rules |
| `placeholder_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Placeholder color |
| `selection_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Selection highlight |
| `on_paste` | `(enabled: bool) -> Self` | Emit on paste |
| `style`, `required`, `validation` |  | As on `text_input` |
| `event_rate`, `a11y` |  | Standard |

### checkbox

```rust
pub fn checkbox(id: &str, checked: bool) -> CheckboxBuilder
```

Boolean toggle rendered as a box.

| Method | Signature | Description |
|---|---|---|
| `label` | `(l: &str) -> Self` | Label text |
| `spacing` | `(s: impl Into<Animatable<f32>>) -> Self` | Space between box and label |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Box size |
| `text_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Label font size |
| `font` | `(f: Font) -> Self` | Label font |
| `icon` | `(icon: PropValue) -> Self` | Check icon override |
| `line_height` | `(lh: impl Into<Animatable<LineHeight>>) -> Self` | Label line height |
| `shaping` | `(s: Shaping) -> Self` | Label shaping |
| `wrapping` | `(w: Wrapping) -> Self` | Label wrap |
| `disabled` | `(d: bool) -> Self` | Disable interaction |
| `style`, `required`, `validation` |  | Form prop set |
| `event_rate`, `a11y` |  | Standard |

### toggler

```rust
pub fn toggler(id: &str, is_toggled: bool) -> TogglerBuilder
```

Boolean toggle rendered as a switch. Same setters as
`checkbox` minus `icon`, `required`, and `validation`,
plus `text_alignment(HorizontalAlignment)`.

### radio

```rust
pub fn radio(id: &str, value: &str, selected: Option<&str>) -> RadioBuilder
```

One-of-many selection. The radio is checked when
`value == selected`.

| Method | Signature | Description |
|---|---|---|
| `label` | `(l: &str) -> Self` | Label text |
| `group` | `(g: &str) -> Self` | Group name (accessible grouping) |
| `spacing` | `(s: impl Into<Animatable<f32>>) -> Self` | Space between indicator and label |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Indicator size |
| `text_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Label font size |
| `font`, `line_height`, `shaping`, `wrapping` |  | Text rendering |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate`, `a11y` |  | Standard |

### slider, vertical_slider

```rust
pub fn slider(id: &str, range: (f32, f32), value: f32) -> SliderBuilder
pub fn vertical_slider(id: &str, range: (f32, f32), value: f32)
    -> VerticalSliderBuilder
```

Horizontal or vertical range input. Slide events are
coalescable; use `event_rate` to cap frequency.

| Method | Signature | Description |
|---|---|---|
| `step` | `(s: f32) -> Self` | Step size |
| `shift_step` | `(s: f32) -> Self` | Step size when Shift is held |
| `default` | `(d: f32) -> Self` | Snap-back value |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width (slider) |
| `height` | `(h: impl Into<Length>) -> Self` | Preferred height (vertical_slider) |
| `circular_handle` | `(enabled: bool) -> Self` | Slider only: round handle |
| `handle_radius` | `(r: impl Into<Animatable<f32>>) -> Self` | Slider only: handle radius |
| `rail_color` | `(c: impl Into<Animatable<Color>>) -> Self` | Rail color |
| `rail_width` | `(w: impl Into<Animatable<f32>>) -> Self` | Rail thickness |
| `label` | `(l: &str) -> Self` | Accessible label |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate`, `a11y` |  | Standard |

The orthogonal axis uses a bare `f32`: `slider.height(f32)`
and `vertical_slider.width(f32)`.

### pick_list

```rust
pub fn pick_list(id: &str, options: &[&str], selected: Option<&str>)
    -> PickListBuilder
```

Dropdown selection.

| Method | Signature | Description |
|---|---|---|
| `placeholder` | `(p: &str) -> Self` | Empty-state text |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `text_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Text size |
| `font`, `line_height` |  | Text rendering |
| `menu_height` | `(h: impl Into<Animatable<f32>>) -> Self` | Dropdown height |
| `menu_style` | `(s: impl Into<Style>) -> Self` | Dropdown style |
| `shaping` | `(s: Shaping) -> Self` | Text shaping |
| `handle` | `(h: PropValue) -> Self` | Handle glyph override |
| `ellipsis` | `(e: Ellipsis) -> Self` | Trailing ellipsis |
| `on_open` | `(enabled: bool) -> Self` | Emit on open |
| `on_close` | `(enabled: bool) -> Self` | Emit on close |
| `style`, `required`, `validation` |  | Form prop set |
| `event_rate`, `a11y` |  | Standard |

### combo_box

```rust
pub fn combo_box(id: &str, options: &[&str], value: &str) -> ComboBoxBuilder
```

Searchable dropdown (text input + filtered list). Holds
renderer-side search text and open state.

| Method | Signature | Description |
|---|---|---|
| `placeholder` | `(p: &str) -> Self` | Empty-state text |
| `width` | `(w: impl Into<Length>) -> Self` | Preferred width |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `size` | `(s: impl Into<Animatable<f32>>) -> Self` | Text size |
| `font`, `line_height`, `shaping`, `ellipsis`, `menu_height` |  | Text and menu layout |
| `icon` | `(icon: PropValue) -> Self` | Leading icon |
| `menu_style` | `(s: impl Into<Style>) -> Self` | Dropdown style |
| `on_option_hovered` | `(enabled: bool) -> Self` | Emit on option hover |
| `on_open` | `(enabled: bool) -> Self` | Emit on open |
| `on_close` | `(enabled: bool) -> Self` | Emit on close |
| `style`, `required`, `validation` |  | Form prop set |
| `event_rate`, `a11y` |  | Standard |

## Interactive

Interactive wrappers live in `plushie::ui::interactive`.
All require an explicit ID.

### button

```rust
pub fn button(id: &str, label: &str) -> ButtonBuilder
```

| Method | Signature | Description |
|---|---|---|
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `disabled` | `(v: bool) -> Self` | Disable interaction |
| `width` / `height` |  | Preferred size |
| `padding` | `(p: impl Into<Padding>) -> Self` | Inner padding |
| `clip` | `(v: bool) -> Self` | Clip label overflow |
| `event_rate`, `a11y` |  | Standard |

### pointer_area

```rust
pub fn pointer_area(id: &str) -> PointerAreaBuilder
```

Captures pointer events on a single child: press, release,
enter, exit, move, scroll, plus right and middle button
variants and double-click. The emitted events carry a
`pointer` field (mouse, touch, pen) and `modifiers` state.

| Method | Signature | Description |
|---|---|---|
| `on_press` | `(tag: &str) -> Self` | Emit press with tag |
| `on_release` | `(tag: &str) -> Self` | Emit release with tag |
| `on_enter` | `(v: bool) -> Self` | Emit enter |
| `on_exit` | `(v: bool) -> Self` | Emit exit |
| `on_move` | `(v: bool) -> Self` | Emit move (coalescable) |
| `on_scroll` | `(v: bool) -> Self` | Emit scroll (coalescable) |
| `on_right_press` | `(v: bool) -> Self` | Emit right-press |
| `on_right_release` | `(v: bool) -> Self` | Emit right-release |
| `on_middle_press` | `(v: bool) -> Self` | Emit middle-press |
| `on_middle_release` | `(v: bool) -> Self` | Emit middle-release |
| `on_double_click` | `(v: bool) -> Self` | Emit double-click |
| `cursor` | `(cursor: CursorStyle) -> Self` | Hover cursor |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` | `(c: impl Into<View>) -> Self` | Single child |

### sensor

```rust
pub fn sensor(id: &str) -> SensorBuilder
```

Emits layout-change events for its single child. Use for
responsive layouts, lazy loading, and intersection
observation.

| Method | Signature | Description |
|---|---|---|
| `delay` | `(ms: u32) -> Self` | Delay before emitting |
| `anticipate` | `(pixels: f32) -> Self` | Visibility anticipation distance |
| `on_resize` | `(tag: &str) -> Self` | Emit resize with tag |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |
| `child` | `(c: impl Into<View>) -> Self` | Single child |

### tooltip

```rust
pub fn tooltip(id: &str, tip: &str) -> TooltipBuilder
```

Popup tip on hover. The child is the anchor.

| Method | Signature | Description |
|---|---|---|
| `position` | `(pos: Position) -> Self` | Tip location relative to child |
| `gap` | `(v: impl Into<Animatable<f32>>) -> Self` | Gap between anchor and tip |
| `padding` | `(v: impl Into<Animatable<f32>>) -> Self` | Tip padding |
| `snap_within_viewport` | `(v: bool) -> Self` | Keep tip inside viewport |
| `delay` | `(ms: u32) -> Self` | Delay before showing |
| `style` | `(s: impl Into<Style>) -> Self` | Named or custom style |
| `event_rate`, `a11y`, `child` |  | Standard |

### overlay

```rust
pub fn overlay(id: &str) -> OverlayBuilder
```

Positions floating children relative to an anchor. Renders
above other content.

| Method | Signature | Description |
|---|---|---|
| `position` | `(pos: Position) -> Self` | Overlay position |
| `align` | `(a: Align) -> Self` | Cross-axis alignment |
| `flip` | `(v: bool) -> Self` | Auto-flip on viewport overflow |
| `gap` | `(v: impl Into<Animatable<f32>>) -> Self` | Gap between anchor and overlay |
| `offset_x` | `(v: impl Into<Animatable<f32>>) -> Self` | X offset |
| `offset_y` | `(v: impl Into<Animatable<f32>>) -> Self` | Y offset |
| `width` | `(w: impl Into<Length>) -> Self` | Overlay container width |
| `event_rate`, `a11y`, `child` / `children` |  | Standard |

### themer

```rust
pub fn themer(id: &str) -> ThemerBuilder
```

Applies a different theme to its child subtree.

| Method | Signature | Description |
|---|---|---|
| `theme` | `(theme: impl Into<Theme>) -> Self` | Theme to apply |
| `event_rate`, `a11y`, `child` |  | Standard |

## Canvas primitives

The canvas drawing surface is in `plushie::ui::canvas`.
This section lists only the top-level constructors;
full drawing, transforms, and hit regions are covered
in [canvas.md](canvas.md).

```rust
pub fn canvas(id: &str) -> CanvasBuilder
pub fn layer(name: &str) -> LayerBuilder
pub fn group(id: &str) -> GroupBuilder
pub fn rect(x: f32, y: f32, w: f32, h: f32) -> RectBuilder
pub fn circle(x: f32, y: f32, r: f32) -> CircleBuilder
pub fn line(x1: f32, y1: f32, x2: f32, y2: f32) -> LineBuilder
pub fn path(commands: impl IntoIterator<Item = PathCommand>) -> PathBuilder
pub fn canvas_text(x: f32, y: f32, content: &str) -> CanvasTextBuilder
pub fn canvas_image(x: f32, y: f32, source: &str) -> CanvasImageBuilder
pub fn canvas_svg(x: f32, y: f32, source: &str) -> CanvasSvgBuilder
pub fn interactive(id: &str) -> GroupBuilder
```

`PathCommand` constructors: `move_to`, `line_to`,
`bezier_to`, `quadratic_to`, `arc`, `arc_to`, `ellipse`,
`rounded_rect`, `close`. `linear_gradient(start, end, stops)`
builds a reusable `Gradient` value.

## Memo

View memoisation lives in `plushie::ui::memo`.

```rust
pub fn memo<D: Hash>(
    key: impl Into<String>,
    deps: D,
    view_fn: impl FnOnce() -> View,
) -> View
```

Wraps a view subtree in a memo marker. The `view_fn`
always runs (view functions are pure), but when the
hash of `deps` matches the previous render, normalisation
reuses the cached subtree downstream instead of re-walking
and re-diffing it.

```rust
column().children([
    memo("header", (model.user_id, model.revision), || {
        expensive_header(&model)
    }),
    text(&model.dynamic_text).into(),
])
```

Use any `Hash` type for deps: a tuple, a `&str`, a
`u64`, a custom type that derives `Hash`. Avoid hashing
floats unless the bit-level identity is intentional.

## Table

The table widget lives in `plushie::ui::table`.

```rust
pub fn table(id: &str) -> TableBuilder
```

Columns are metadata; rows are real `table_row` children,
so add, remove, and reorder operations produce minimal
wire patches via LIS-based diffing.

### Column spec

`TableBuilder::column(key, f)` takes a closure that
configures a `TableColumnSpec`:

| Method | Signature | Description |
|---|---|---|
| `label` | `(label: &str) -> Self` | Header text, defaults to key |
| `width` | `(w: impl Into<Length>) -> Self` | Column width |
| `min_width` | `(px: f32) -> Self` | Minimum column width |
| `sortable` | `(v: bool) -> Self` | Header emits sort events |
| `align` | `(a: HorizontalAlignment) -> Self` | Cell alignment |

The shorthand `.columns(&[(key, label), ...])` adds plain
columns without the closure form.

### Rows

`TableBuilder::row(id, f)` takes a closure that configures
a `TableRowBuilder`. Each cell maps a column key to widget
content:

```rust
table("users")
    .column("name", |c| c.label("Name").sortable(true).width(Length::Fill))
    .column("email", |c| c.label("Email"))
    .column("actions", |c| c.label(""))
    .sort_by("name")
    .sort_order(SortOrder::Asc)
    .row("u1", |r| r
        .cell("name", text("Alice"))
        .cell("email", text("alice@example.com"))
        .cell("actions", button("del-u1", "Delete")))
```

The shorthand `.data_row(id, &[(key, value), ...])` builds
text-only rows.

### Table props

| Method | Signature | Description |
|---|---|---|
| `width` | `(w: impl Into<Length>) -> Self` | Table width |
| `height` | `(h: impl Into<Length>) -> Self` | Height (scrolls when set) |
| `header` | `(v: bool) -> Self` | Show header row |
| `sort_by` | `(column: &str) -> Self` | Column key to sort by |
| `sort_order` | `(order: SortOrder) -> Self` | Ascending or descending |
| `separator` | `(thickness: f32) -> Self` | Divider thickness; 0.0 hides |
| `padding` | `(p: impl Into<Padding>) -> Self` | Cell internal padding |
| `header_text_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Header font size |
| `row_text_size` | `(s: impl Into<Animatable<f32>>) -> Self` | Data-shorthand font size |
| `event_rate` | `(rate: u32) -> Self` | Max events/sec |
| `a11y` | `(a11y: &A11y) -> Self` | Accessibility metadata |

## Common setters

These setters recur across widgets. Signatures vary per
builder (some accept `Animatable<T>`, some plain values)
but the semantics are constant.

- `id(&str)`: explicit node ID. Required on interactive
  and renderer-stateful widgets; override on auto-ID
  widgets when a scope prefix is needed.
- `width(impl Into<Length>)` / `height(impl Into<Length>)`:
  preferred size along an axis.
- `padding(impl Into<Padding>)`: inner padding.
- `style(impl Into<Style>)`: named preset (`Style::primary()`,
  `Style::secondary()`, `Style::success()`, `Style::danger()`)
  or a `StyleMap`.
- `a11y(&A11y)`: accessibility metadata (role, label, etc.).
- `event_rate(u32)`: max events per second for coalescable
  events; `0` means unbounded.

## Prop value types

Types and their constructors are defined in
`plushie_core::types` and re-exported as
`plushie::types`. The full styling and layout prop tables
live in the relevant reference pages; this section covers
the types used directly in widget signatures.

### Length

```rust
pub enum Length {
    Fill,
    Shrink,
    FillPortion(u32),
    Fixed(f32),
}
```

`Shrink` is the default. `Fixed(px)` takes a non-negative
pixel value.

### Padding

`Padding::all(px)` sets all sides. `Padding::top(px)`,
`Padding::right(px)`, `Padding::bottom(px)`, and
`Padding::left(px)` set a single side. The builder methods
compose (e.g. `Padding::all(8).top(16)`).

### Align

```rust
pub enum Align { Start, Center, End }
```

Mapped per context: `align_x` encodes as
`"left"` / `"center"` / `"right"`; `align_y` as
`"top"` / `"center"` / `"bottom"`; overlay `align` as
`"start"` / `"center"` / `"end"`.

### Wrapping

`Wrapping::None`, `Wrapping::Word`, `Wrapping::Glyph`,
`Wrapping::WordOrGlyph`. Used by `text`, `rich_text`,
`text_editor`, `checkbox`, `toggler`, `radio`.

### Shaping

`Shaping::Basic` (fastest, ASCII only), `Shaping::Advanced`
(HarfBuzz), `Shaping::Auto` (renderer picks).

### ContentFit

`ContentFit::Contain`, `Cover`, `Fill`, `ScaleDown`, `None`.
Used by `image` and `svg`.

### FilterMethod

`FilterMethod::Nearest` for pixel-perfect, `FilterMethod::Linear`
for smooth. Used by `image`.

### Direction

`Direction::Horizontal`, `Direction::Vertical`. Used by
`scrollable` and `rule`.

### Anchor

`Anchor::Start` or `Anchor::End`. Used by `scrollable` to
pin the scroll position to the top/left or bottom/right.

### Position

`Position::Below`, `Above`, `Left`, `Right`. Used by
`tooltip` and `overlay`.

## See also

- [Events](events.md) for the event types widgets emit.
- [Commands](commands.md) for commands dispatched from
  `update`.
- [Windows and layout](windows-and-layout.md) for sizing,
  alignment, and window configuration.
- [Themes and styling](themes-and-styling.md) for `Color`,
  `Theme`, `StyleMap`, `Border`, `Shadow`, and `Gradient`.
- [Scoped IDs](scoped-ids.md) for how auto-IDs and explicit
  IDs combine.
