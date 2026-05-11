# Upstream Issues

This file records issues where the right fix is in a dependency or
the current behavior is blocked by dependency behavior. These notes are
not a commitment to carry local patches in plushie-rust.

## Windows headless synthetic input hangs

Several renderer integration tests skip specific headless-mode arms on
Windows because synthetic click or text-input commit paths can hang in
the iced tiny-skia event loop. The affected tests keep the windowed or
non-Windows coverage active, and the Windows skip avoids turning an
upstream event-injection hang into a permanent local CI failure.

Track this as an iced or plushie-iced headless event-loop issue before
removing the skips or adding more local workaround code.

## Grid column width only accepts pixels

`plushie-widget-sdk` exposes `grid.column_width` as a `Length` for API
shape consistency with row height and other layout sizing props. The
underlying iced grid builder currently accepts column width as pixels,
not a general `Length`, so only fixed lengths can be applied locally.

Track this as an iced or plushie-iced grid API limitation before
trying to support `Fill`, `Shrink`, or `FillPortion` column widths in
Plushie.

## Progress bar styles cannot apply shadows

`plushie-widget-sdk` style maps carry a generic `shadow` field, but
iced's `progress_bar::Style` currently exposes only `background`,
`bar`, and `border`. The renderer can map background, fill, and border
locally, but a progress bar shadow cannot be expressed without an iced
API change or wrapping the widget in an extra decorated container.

Track this as an iced or plushie-iced progress bar style limitation
before adding local wrapper behavior.
