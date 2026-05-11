# Documentation

## Guides

Sequential chapters that build on each other. Start here if you're
new to Plushie.

1. [Introduction](guides/01-introduction.md) - what Plushie is and how it works
2. [Getting Started](guides/02-getting-started.md) - installation, binary setup, first run
3. [Your First App](guides/03-your-first-app.md) - building a counter with the Elm architecture
4. [The Development Loop](guides/04-the-development-loop.md) - hot reload, cargo-plushie, debugging
5. [Events](guides/05-events.md) - widget events, keyboard, pointer, pattern matching
6. [Lists and Inputs](guides/06-lists-and-inputs.md) - dynamic lists, text inputs, forms
7. [Layout](guides/07-layout.md) - rows, columns, containers, responsive sizing
8. [Styling](guides/08-styling.md) - themes, colors, fonts, per-widget style overrides
9. [Animation and Transitions](guides/09-animation.md) - transitions, springs, tweens, easing
10. [Subscriptions](guides/10-subscriptions.md) - timers, global key/pointer events, window events
11. [Async and Effects](guides/11-async-and-effects.md) - async tasks, streams, platform effects
12. [Canvas](guides/12-canvas.md) - shapes, layers, transforms, interactive elements
13. [Custom Widgets](guides/13-custom-widgets.md) - composing widgets, canvas widgets, native Rust widgets
14. [State Management](guides/14-state-management.md) - routing, undo/redo, selection, data pipelines
15. [Testing](guides/15-testing.md) - test harness, selectors, async, strict mode
16. [Shared State](guides/16-shared-state.md) - multi-session apps over SSH
17. [WASM Deployment](guides/17-wasm-deployment.md) - browser-hosted renderer, packaging native binaries

## Reference

Lookup material organized by topic. Each page is self-contained.

- [Accessibility](reference/accessibility.md) - AccessKit integration, roles, labels, keyboard navigation
- [Animation](reference/animation.md) - transitions, springs, sequences, easing curves, animatable props
- [App Lifecycle](reference/app-lifecycle.md) - the `App` trait, init/update/view, supervision, startup sequence
- [Built-in Widgets](reference/built-in-widgets.md) - every widget with props, events, and examples
- [Canvas](reference/canvas.md) - shapes, layers, groups, transforms, interactive regions
- [CLI Flags](reference/cli.md) - reserved `--plushie-*` flags layered on `plushie::run`
- [CLI Commands](reference/cli-commands.md) - `cargo plushie` subcommands: build, download, run, init, new-widget, doctor
- [Commands and Effects](reference/commands.md) - async, focus, scroll, window ops, platform effects
- [Composition Patterns](reference/composition-patterns.md) - helper components, memoisation, multi-window
- [Configuration](reference/configuration.md) - `Settings`, `WindowConfig`, environment variables, feature flags
- [Custom Widgets](reference/custom-widgets.md) - the `PlushieWidget` trait, derive macros, native crates
- [Dev Mode](reference/dev.md) - `watch_renderer`, restart policy, in-tree rebuild overlay
- [Direct vs Wire](reference/direct-vs-wire.md) - in-process iced vs subprocess renderer, feature flags, WASM renderer
- [Events](reference/events.md) - event types, `widget_match`, scoped ID routing
- [Scoped IDs](reference/scoped-ids.md) - `#[track_caller]` auto-IDs, explicit IDs, scope paths
- [Subscriptions](reference/subscriptions.md) - timer, keyboard, pointer, window, theme, animation frame
- [Testing](reference/testing.md) - `TestSession`, selectors, interactions, assertions, async
- [Themes and Styling](reference/themes-and-styling.md) - built-in themes, custom palettes, style maps
- [Versioning](reference/versioning.md) - workspace version, wire protocol version, pinning, host-SDK coordination
- [WASM Transport](reference/wasm-transport.md) - browser callback transport, JSON-only codec, web-specific behavior
- [Windows and Layout](reference/windows-and-layout.md) - `Length`, `Padding`, `Align`, window config, layout containers
- [Wire Protocol](reference/wire-protocol.md) - MessagePack/JSON framing, message types, transport modes

## By topic

When you know what you want to do but not which page to start on:

**Building UI**: [Layout](reference/windows-and-layout.md),
[Built-in Widgets](reference/built-in-widgets.md),
[Themes and Styling](reference/themes-and-styling.md),
[Canvas](reference/canvas.md),
[Composition Patterns](reference/composition-patterns.md).

**Reacting to input**: [Events](reference/events.md),
[Subscriptions](reference/subscriptions.md),
[Scoped IDs](reference/scoped-ids.md),
[Accessibility](reference/accessibility.md).

**Side effects and async**: [Commands and Effects](reference/commands.md),
[App Lifecycle](reference/app-lifecycle.md).

**Animation**: [Animation](reference/animation.md).

**Testing and automation**: [Testing](reference/testing.md),
[CLI Flags](reference/cli.md).

**Custom widgets**: [Custom Widgets](reference/custom-widgets.md),
[CLI Commands](reference/cli-commands.md).

**Deployment**: [Direct vs Wire](reference/direct-vs-wire.md),
[Configuration](reference/configuration.md),
[WASM Transport](reference/wasm-transport.md),
[Versioning](reference/versioning.md),
[Wire Protocol](reference/wire-protocol.md).

**Development workflow**: [Dev Mode](reference/dev.md),
[CLI Commands](reference/cli-commands.md).

## Other resources

- [Examples](https://github.com/plushie-ui/plushie-rust/tree/main/crates/plushie/examples) - example apps included in the repo
- [Changelog](../CHANGELOG.md) - version history and migration notes
- [Demo apps](https://github.com/plushie-ui/plushie-demos/tree/main/rust) - multi-file projects with custom widgets and real scaffolding
