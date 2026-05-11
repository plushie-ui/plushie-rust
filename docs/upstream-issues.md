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
