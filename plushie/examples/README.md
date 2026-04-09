# Examples

Run any example with:

```
cargo run -p plushie --example <name>
```

## Examples

| Name | Description |
|------|-------------|
| counter | Minimal counter with increment/decrement buttons |
| todo | Todo list with add, toggle, delete, and scoped events |
| clock | Timer subscription updating every second |
| shortcuts | Global keyboard event logging with modifier detection |
| gallery | Interactive showcase of common widget types |
| notes | Multi-page notes app with undo, selection, and routing |
| star_rating | App rating page with reusable star rating component |
| async_fetch | Async background work with Command and AsyncEvent |
| multi_window | Multiple windows from a single view function |
| custom_theme | Custom StyleMaps with status overrides |
| canvas_drawing | Canvas shapes: rect, circle, line, path, text |

## Beginner path

Start with **counter** to learn the Elm architecture (init/update/view),
then **todo** for dynamic lists and scoped events. **clock** and
**shortcuts** introduce subscriptions. **gallery** is a reference
for available widget types and their events. **notes** shows how
the utility helpers (Route, Selection, UndoStack) compose in a
real app.

## Running examples

All examples use direct mode (in-process renderer). No subprocess
or binary path needed.

```
cargo run -p plushie --example counter
cargo run -p plushie --example todo
cargo run -p plushie --example clock
cargo run -p plushie --example shortcuts
cargo run -p plushie --example gallery
cargo run -p plushie --example notes
cargo run -p plushie --example star_rating
cargo run -p plushie --example async_fetch
cargo run -p plushie --example multi_window
cargo run -p plushie --example custom_theme
cargo run -p plushie --example canvas_drawing
```
