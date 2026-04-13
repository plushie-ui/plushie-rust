# Examples

Run any example with:

```
cargo run -p plushie --example <name>
```

## Apps

| Name | Description |
|------|-------------|
| `counter` | Minimal counter with increment/decrement buttons |
| `todo` | Todo list with add, toggle, delete, filter, and scoped events |
| `clock` | Timer subscriptions updating a clock display every second |
| `async_fetch` | Async data fetching with loading/error/success states |
| `shortcuts` | Global keyboard event logging with modifier display |
| `gallery` | Widget showcase: buttons, inputs, checkboxes, sliders, etc. |
| `notes` | Multi-page notes app using Route, UndoStack, and Selection helpers |
| `color_picker` | HSV color picker with sliders and live color preview |
| `rate_plushie` | App rating page with star rating, form validation, and reviews |

The `color_picker` and `rate_plushie` examples include inline
composite widget definitions (using the `Widget` trait) that
demonstrate reusable stateful components with event interception.

## Getting Started

Start with `counter` for the basics, then `todo` for dynamic lists
and scoped events. `gallery` shows all available widget types.
`notes` demonstrates advanced state management patterns.
