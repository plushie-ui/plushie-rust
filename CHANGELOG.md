# Changelog

All notable changes to plushie will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [0.7.1] - 2026-05-09

### Fixed

- **Canvas structural groups no longer collected as interactive elements.**
  Groups used only for transforms or clipping were incorrectly treated as
  interactive when they carried an auto-assigned id. The canvas engine then
  emitted spurious a11y warnings for any widget that uses a structural group
  for positioning (e.g. star rating readonly branch, theme toggle face group).
  `parse_interactive_element` now returns `None` when none of `on_click`,
  `on_hover`, `draggable`, or `focusable` are set.

## [0.7.0] - 2026-05-08

This release introduces the Rust application SDK (`plushie` crate),
the widget SDK (`plushie-widget-sdk`), the `cargo-plushie` tool, and
moves the workspace to a crate-split layout. Everything below is
relative to 0.6.1.

### Breaking changes

- **`close_window` moved from `widget_op` to `window_op`.** Hosts that
  sent `widget_op` with `op: "close_window"` must switch to
  `window_op` with `op: "close"` and a `window_id` field. The renderer
  no longer handles the old widget_op form.
- **Workspace restructured into per-role crates.** The former single
  renderer crate is split across `plushie-core`, `plushie-renderer-engine`,
  `plushie-renderer-lib`, `plushie-renderer` (native binary),
  `plushie-renderer-wasm`, `plushie-widget-sdk`, and `plushie` (app SDK).
  Workspace crates now live under `crates/`. The pure renderer state
  engine (`Core`), retained tree, and wire codec live in
  `plushie-renderer-engine`; widget authors do not depend on it.
- **`Props` representation unified.** The `Props` enum (Typed vs Wire
  variants) collapses to a single `PropMap`. SDK builders, prop
  helpers, and widget renderers now operate on one representation.
  Callers that matched on `Props::Typed` or `Props::Wire` need to
  migrate to the unified API.
- **`plushie-widget-sdk` is optional.** It is gated by the `direct`
  feature on the `plushie` crate. Wire-only consumers get a slimmer
  build. App authors using built-in widgets see no change; widget
  authors must depend on `plushie-widget-sdk` explicitly.
- **`PlushieRenderer` trait is sealed.** External implementations are
  no longer permitted. All supported backends ship with the crate.
- **Scripting `interact` payload format.** Typed `Key`, `KeyPress`,
  `MouseButton`, `InteractAction`, and `EffectKind` replace the
  stringly-typed forms. The `cmd`/`command` modifier is now resolved
  to the platform-appropriate physical modifier by the renderer.
- **Canonical renderer env whitelist.** The renderer subprocess inherits
  only an explicit list of environment variables (names and prefixes).
  Custom vars must match a documented prefix or be passed via a
  supported hook. See `docs/reference/wire-protocol.md`.
- **Diagnostic wire format.** Diagnostics emitted by the renderer use
  the typed `Diagnostic` enum with structured variants instead of
  free-form strings. Host SDKs that pattern-match diagnostic text
  must migrate to the variant form.
- **Widget-command wire format unified.** Per-widget command variants
  (`PaneGridOp`, etc.) are replaced by a single `widget_command`
  envelope with typed payloads. Extension authors that hand-built
  command messages must move to the `#[derive(WidgetCommand)]` path.
- **`WidgetExtension` trait removed.** Replaced by `PlushieWidget`,
  `WidgetRegistry`, and factory-based dispatch. The term "extension"
  is retired throughout the API in favour of "widget".
- **`[patch.crates-io]` replaced by `.cargo/config.toml` path
  overrides** for the `plushie-iced` fork during development. Consumer
  projects vendoring the fork need to update their override mechanism.

### Deprecated

- **`plushie-widget-sdk::JsonProps`.** The type alias remains available
  for compatibility, but widget authors should use `&Props` directly.

### Added

- **`plushie` Rust application SDK.** Elm-style `App` trait with
  `init`/`update`/`view`/`subscriptions`, direct and wire runners,
  typed commands, subscription lifecycle diffing, effect dispatch,
  and composite-widget support.
- **`plushie-widget-sdk` for custom widget authors.** `PlushieWidget`
  trait, `WidgetRegistry`, `CanvasEngine`, `#[derive(PlushieWidget)]`,
  `#[derive(WidgetCommand)]`, `#[derive(WidgetEvent)]`,
  `#[derive(WidgetProps)]`, `widget!` function-like macro,
  `BUILTIN_TYPE_NAMES`, widget-scoped subscriptions, typed config
  helpers, panic-isolated entry points, and test helpers.
- **`cargo-plushie` build tool.** `build` (including `--wasm` via
  wasm-pack), `download` (renderer binaries), `run` (with `--watch`
  hot-reload via cargo-watch), `new-widget`, `init`, `doctor`, and
  `wasm` subcommands. Reads `[package.metadata.plushie]` and
  `[package.metadata.plushie.widget]` for project and widget
  configuration.
- **Dev-mode hot-reload.** `dev::watch_renderer` plus the `--watch`
  orchestrator in `cargo-plushie run`. The wire runner swaps the
  renderer subprocess on rebuild without losing session state. A
  `RebuildingOverlay` is injected into the view tree with interactive
  dismiss, auto-dismiss, and event interception.
- **`plushie::cli::run::<A>()` easy-path entrypoint** with the
  `--plushie-*` reserved flag prefix for framework-owned options.
- **`plushie::automation` module.** `TestSession` (direct) and
  `WidgetTestSession` (widget-scoped) for integration-style testing,
  `.plushie` script format (header + instructions), `Selector` with
  tree search, typed assertions (`AssertModel`, `run_with_model_debug`,
  `resolved_a11y` surfacing of inferred a11y). `automation::cli`
  primitives `script`, `replay`, and `inspect`. The `Backend` enum
  (`Mock`, `Headless`, `Windowed`) routes scripts through the
  appropriate runner, including a real renderer subprocess for
  `Windowed` replay.
- **`run_connect` + renderer-spawned-us mode.** The SDK can attach to
  a renderer launched externally via the `PLUSHIE_SOCKET` env var,
  using a new `SocketAdapter` and split `Bridge` transport
  (`Subprocess` + `Socket`).
- **Four-step renderer binary discovery.** `cargo-plushie` path,
  `PLUSHIE_RENDERER` env, workspace `target/` probe, and `PATH`
  lookup, with an advisory architecture check.
- **Multi-window lifecycle sync** in the wire runtime, at parity with
  the Elixir SDK.
- **Wire bridge auto-restart with heartbeat watchdog.** Transient
  renderer failures no longer terminate the app.
- **Protocol version handshake.** The SDK validates the renderer's
  `hello` message protocol version and honours the negotiated codec.
- **Typed `Diagnostic` enum in `plushie-core`** with structured
  variants. Widget SDK and renderer-lib emit typed diagnostics at all
  former log-warn sites.
- **`RENDERER_VERSION` const** exported from `plushie-renderer-lib`,
  deduplicated with `PROTOCOL_VERSION`.
- **Typed `Error` enum** on the `plushie` crate and a wire-renderer
  exit hook so app code sees a typed failure surface.
- **`plushie-core-macros` proc-macro crate.** `PlushieEnum` derive
  (used for FontWeight, FontStyle, FontStretch, and many other
  enums), wire-encode/decode generation, and the `widget!`
  function-like macro for built-in widget descriptions.
- **Typed core domain types.** `Color` (hex parsing with short-form
  expansion), `Angle` (dual-storage, degrees on the wire), `Padding`
  (per-side and axis constructors), `Length`, `Theme`/`CustomTheme`
  with shade overrides, `A11y` with merge semantics, `PathCommand`,
  `PointerKind`, `MouseButton` (with Back/Forward), `ArrowMode`,
  `SortOrder`, `ErrorCorrection`, `ValueRange` (renamed from `Range`),
  `EventType`, `OutgoingMessage`, and `ScopedId`.
- **Composable `TreeTransform` walker** in core. Normalize, widget
  expansion, and animation scan now share a single walk.
- **Generic `Animatable<T>` types** and type-safe animation on
  widget-builder setters. Angle-bearing APIs and Image/SVG rotation
  flow through `Angle` with animation support.
- **Widget-scoped subscriptions.** `SubscribeCtx`, dispatch helpers,
  and native renderer wiring let widget authors manage their own
  subscription lifecycles.
- **Accessibility inference.** Widget-level `infer_a11y` merges with
  explicit host-supplied `A11y`. SDK builders set a11y defaults on
  the tree so tests can assert. Scoped refs rewrite automatically,
  implicit radio groups are populated, and pick_list gains an
  `infer_a11y` fallback. A `missing_accessible_name` diagnostic
  flags unlabelled widgets. `Command::focus_next_within` and
  `focus_previous_within` scope keyboard navigation.
- **Automation backend dispatch.** `.plushie` scripts with
  `backend: windowed` spawn a real `plushie-renderer` subprocess so
  the run can be watched.
- **`[profile.dist]`** for shipping artifacts and Sigstore signing of
  release binaries.
- **CI matrix expanded** to macOS (including darwin-x86_64) and
  Windows, with nightly `cargo audit` and `cargo deny` workflows.
- **Workspace-level `[workspace.dependencies]`** pins shared crates
  in one place.
- **Comprehensive rustdoc pass** with `#![deny(missing_docs)]` on
  `plushie`, `plushie-core`, and `plushie-widget-sdk`, plus
  Panics/Errors sections and workspace clippy lints.
- **docs.rs metadata** on every crate; README badges, versioning
  policy, and a pull-request template.

### Changed

- `plushie::run` unified across direct and wire modes; same API, mode
  selected by features and environment.
- `EffectHandler` returns `Future` instead of an iced `Task`.
- Codec state is per-App (`EventSink`, `WriterSink`) rather than
  global; `Codec::get_global()` and the last of the global singletons
  are removed.
- `TestSession` ergonomics pass: richer event-bridge polish,
  subscription grouping, assertion helpers, multi-finger touch,
  multi-window coverage, async delivery contract for `Err`/`Cancel`/
  panic, and end-to-end `run_wire` integration.
- `parking_lot::Mutex` on hot paths (direct-mode event queue,
  renderer sink).
- Nextest CI profile tuned; `just test-examples` runs inline tests in
  simple examples, added to preflight.
- Canvas shape hashing is direct instead of going through `Debug`
  strings; per-event allocations trimmed on hot paths;
  `scope`-string buffer threaded through normalize with a fast path
  when widget expansion is a no-op.
- `memo()` actually memoizes at normalize time.
- Msgpack depth pre-check and invalid UTF-8 rejection share a single
  codec-boundary guard.
- Protocol envelope unified: `_op` messages nested under a payload
  envelope; effect-stub acks renamed to `*_register_ack`.

### Fixed

- Widget-state invariant violations surface as panics (were silent).
- `Bridge::receive` reuses a single `BufReader` across calls.
- `Props` equality treats null-valued keys as absent; tree-diff
  round-trips null props correctly.
- Cooperative `SendAfter` delay and async panic guard in the wire
  runner.
- `Length::wire_decode` rejects off-canonical shapes.
- Coalesce compatibility check includes the `Accumulate` field list.
- Scripting cursor and scroll events normalize to `f32`.
- Pointer-area registration and derived `type_names` corrected in
  the widget SDK.
- Toggle inverts current state; `Display` shows the window; interact
  processes all pending events.
- `cargo-plushie` output polish and rustdoc link hygiene.

### Security

- **Renderer subprocess env whitelist.** Only an explicit set of
  variables and prefixes cross the process boundary.
- **Canonical widget-id rules enforced** in the Rust SDK.
- **Tree-depth cap enforced centrally** in the walker; duplicate-id
  collection short-circuits past a sane cap.
- **Msgpack depth pre-check and UTF-8 validation** at the codec
  boundary.
- **SVG decode bounded** by a pinned `usvg` version and a wall-clock
  pre-parse guard.
- **Inline startup fonts counted** against the process-wide font load
  cap.
- **Unix socket creation hardened** (`bind_unix`), with a safer auto
  socket directory.
- **WASM settings validated** before wiring up the event sink.
- **Pathological viewport dimensions** rejected in `.plushie`
  automation files.
- **Oversized text prop content truncated** at the renderer boundary.
- **Numeric prop ranges** emit a warning when out of sane bounds.
- **Native writer channel** has a backpressure timeout and
  diagnostic.
- **TOCTOU and pass-through trust boundaries** documented on platform
  effects.

## [0.6.1] - 2026-04-02

### Fixed

- Semantic actions (click, toggle, select) now use the synthetic event
  path in all modes (mock and headless). The iced event injection path
  was unreliable for toggle and select in headless mode because cursor
  positioning didn't reliably hit the target widget.

- Mock mode enabled in release builds (requires plushie-iced 0.8.3).

- `iced::time::Instant` used for animation timestamps instead of
  `std::time::Instant` to avoid type mismatch with crates.io builds.

- Cargo.lock pinned to resolve `gpu-allocator` against `windows` 0.62
  to avoid version conflict with `wgpu-hal` on Windows builds.

- Unused variable warning on Windows for Unix socket path.

- CI: added release build check and Windows cross-compilation check.

## [0.6.0] - 2026-04-02

### Breaking changes

- **Unified pointer events.** Canvas-specific (`canvas_press`,
  `canvas_release`, `canvas_move`, `canvas_scroll`) and mouse area
  specific (`mouse_right_press`, `mouse_middle_press`, `mouse_move`,
  `mouse_scroll`, `mouse_enter`, `mouse_exit`, `mouse_double_click`)
  event families replaced with unified device-agnostic families:
  `press`, `release`, `move`, `scroll`, `enter`, `exit`,
  `double_click`. All carry `pointer` type, `modifiers` state, and
  optional `finger` ID for touch.

- **Canvas element events unified.** `canvas_element_enter`/`leave`/
  `focused`/`blurred`/`drag`/`drag_end`/`key_press`/`key_release`
  replaced with standard families (`enter`, `exit`, `focused`,
  `blurred`, `drag`, `drag_end`, `key_press`, `key_release`) using
  scoped IDs (`"{canvas_id}/{element_id}"`).

- **`mouse_area` widget renamed to `pointer_area`** on the wire.

- **Scrollable viewport event** renamed from `scroll` to `scrolled`
  on the wire. `scroll` is now the pointer wheel event.

- **`sensor_resize` event** renamed to `resize`.

- **`:start`/`:end` alignment aliases removed.** Use `:left`/`:right`/
  `:top`/`:bottom`/`:center`. Unknown alignment values log a warning.

- **Subscription wire types renamed.** `on_mouse_move`/`button`/
  `scroll` and `on_touch` renamed to `on_pointer_move`/`button`/
  `scroll`/`touch`.

### Added

- **Device awareness on pointer events.** Every pointer event includes
  `pointer` field (`"mouse"`, `"touch"`, `"pen"`), keyboard `modifiers`
  state (`{shift, ctrl, alt, logo, command}`), and `finger` ID for
  touch events.

- **Canvas touch support.** Canvas now handles `FingerPressed`,
  `FingerMoved`, and `FingerLifted` events with full hit testing, drag,
  and click detection. Touch events are emitted with `pointer: "touch"`
  and the finger ID.

- **Modifier state tracking.** Renderer tracks current keyboard
  modifier state and includes it on all outgoing pointer events.

- **Mock mode canvas element click.** `click("#canvas-id/element-id")`
  works in mock mode by detecting scoped IDs, finding the canvas,
  verifying the element exists, and emitting a click event.

- **Renderer-side animation system.** Transitions, springs, and
  sequences with animatable props across display, layout, and input
  widgets.

- **Per-window scale_factor support.**

- **Window-scoped subscriptions.** Subscription events include
  `window_id` for multi-window disambiguation.

- **Widget-targeted scroll commands** for specific scrollable widgets.

- **Effect stubs** for testing (register/unregister via wire protocol).

- **Canvas element key events.** `key_press`/`key_release` on focused
  elements when `arrow_mode` is `"none"`.

- **Canvas scoped IDs** for all element events.

- **Radio group accessibility role** for canvas elements.

### Fixed

- **Mock mode modifier keys.** Click/toggle actions in mock mode now
  extract modifiers from the interact payload instead of hardcoding
  empty modifiers.

- **Mock mode sequential clicks.** Replaced the fragile focus+space
  approach with direct synthetic event emission. Sequential clicks on
  different widgets now work reliably.

- **Broken pipe handling.** Ignore broken pipes during hello handshake.

### Changed

- Upgraded to plushie-iced 0.8.1 (mouse_area cursor position callbacks).
- Renamed `plushie_id` to `window_id` throughout renderer codebase.
- Generic renderer pipeline with null renderer for mock mode.
- Extracted shared startup sequence into startup module.

## [0.5.1] - 2026-03-23

### Fixed

- WASM release builds now disable `wasm-bindgen` `externref`, avoiding
  browser startup failures such as `RangeError: failed to grow table`.
- Inline fonts supplied in settings are now loaded into the font system
  before the first render. This fixes missing default text rendering on
  WASM when no system fonts are available.

## [0.5.0] - 2026-03-23

### Breaking changes

- **Canvas group redesign.** Interactive elements are now groups with
  top-level fields (`id`, `on_click`, `a11y`, etc.) instead of a nested
  `"interactive"` sub-object. Only groups can be interactive; leaf
  shapes (rect, circle, etc.) are no longer interactive on their own.
- **Standalone transform/clip commands removed.** `push_transform`,
  `pop_transform`, `translate`, `rotate`, `scale`, `push_clip`,
  `pop_clip` are no longer supported as standalone shape types. Use the
  `transforms` array and `clip` field on groups instead.
- **Group `x`/`y` fields removed.** Use `transforms: [{"type":
  "translate", "x": ..., "y": ...}]` instead.
- **Event families renamed.** `canvas_shape_*` -> `canvas_element_*`,
  `shape_id` -> `element_id` in event data.

### Added

- **Transforms on groups.** Groups carry an ordered `transforms` array
  (translate, rotate, scale) and an optional `clip` field, replacing
  standalone push/pop commands.
- **Transform-aware hit testing.** Full 2D affine matrix tracks
  translate, rotate, and scale through nested groups. Cursor positions
  are mapped to local space via the inverse matrix. Clip regions from
  ancestor groups are intersected and tested.
- **Focus lifecycle events.** New event families: `canvas_element_blurred`,
  `canvas_focused`, `canvas_blurred`, `canvas_group_focused`,
  `canvas_group_blurred`.
- **Click-to-focus.** Clicking an interactive element grants the canvas
  keyboard focus and sets internal focus to the clicked element.
- **ID-based focus tracking.** Focus survives element reordering between
  renders. Stale elements are detected and blurred automatically.
- **Focus style.** New `focus_style` field on groups for visual feedback
  when keyboard-focused. Priority: pressed > hover > focus.
- **Suppressible focus ring.** `show_focus_ring: false` disables the
  default ring (use `focus_style` for custom indicators).
- **Geometry-aware focus rings.** Ring shape adapts to hit region: rounded
  rectangle for rect, circle for circle, capsule for line. Full transform
  support via matrix decomposition.
- **Arrow mode.** New `arrow_mode` canvas prop: `wrap` (default), `clamp`,
  `linear`, `none`.
- **Focusable groups.** Groups with `focusable: true` become Tab stops
  for two-level navigation. Tab moves between top-level entries, arrows
  navigate within the focused group.
- **Canvas accessible role.** New `role` canvas prop (defaults to `group`
  when interactive, `image` otherwise). `active_descendant` set
  dynamically from focused element.
- **Accessibility tree structure.** Focusable groups create parent-child
  relationships via `traverse()`. Child accessible nodes have widget IDs
  for `active_descendant` resolution.
- **Validation diagnostics.** Warnings for: interactive elements without
  `a11y`, stateful roles missing state props (switch/toggled,
  radio/selected), elements without set position.
- **`focus_element` widget op.** Programmatically focus a canvas and set
  internal focus to a specific element.
- **`click_element` / `focus_element` test interact actions.**
- **`CanvasElementFocusChanged` message.** Single message for blur+focus
  transitions, split into separate outgoing events by the emitter.
- **Theme-aware canvas colors.** Color strings in fill, stroke, and text
  that match iced palette names (`"primary"`, `"text"`, `"background"`,
  `"success"`, `"danger"`, `"warning"`) are resolved against the current
  theme at draw time. Canvas shapes now participate in the theme system.
- **Focus-visible pattern.** Focus ring and focus_style only show for
  keyboard navigation (Tab), not mouse clicks. Matches iced's built-in
  button behavior.
- **Custom focus ring radius.** New `focus_ring_radius` field on groups
  for shape-matched focus rings (e.g. pill-shaped toggles).

### Fixed

- WASM build: replaced `wasm-opt --all-features` with explicit feature
  flags for compatibility with older wasm-opt versions.
- Canvas style overrides (hover_style, pressed_style, focus_style) now
  correctly read from the group, not from children.
- Canvas keyboard events now work when the mouse cursor is outside the
  canvas bounds.
- Canvas focus visuals (focus ring, focus_style) clear when the canvas
  loses iced-level focus.
- `text_editor` cursor no longer resets on every keystroke. Content hash
  is updated after TextEditorAction to prevent stale prop sync.
- `text_editor` cursor movement, selection, and click-to-position now
  work. All actions (not just edits) are performed on the Content.

### Changed

- Release binary assets renamed from `plushie-{os}-{arch}` to
  `plushie-renderer-{os}-{arch}`. WASM archive renamed from
  `plushie-wasm.tar.gz` to `plushie-renderer-wasm.tar.gz`.
- Updated to plushie-iced 0.8.0.

## [0.4.1] - 2026-03-22

### Fixed

- Windows release build failure: `extern` block updated to `unsafe extern`
  as required by Rust 2024 edition.

## [0.4.0] - 2026-03-21

### Breaking changes

- **Project renamed from toddy to plushie.** All crate names, binary name,
  module paths, and import paths have changed: `toddy` -> `plushie`,
  `toddy-core` -> `plushie-core`, `toddy_core` -> `plushie_core`. The
  binary is now `plushie` (was `toddy`). The iced fork is now
  `plushie-iced` (was `toddy-iced`).
- **Crate split.** The single `toddy` crate is now three:
  `plushie-core` (SDK library), `plushie-renderer` (shared renderer
  logic, compilable to native and wasm32), and `plushie` (native binary).
  A fourth crate `plushie-wasm` provides the WASM entry point.
  Extension authors now depend on `plushie-core` instead of `toddy-core`.
- **Wire protocol field renames.** `canvas_scroll` position fields
  changed from `cursor_x`/`cursor_y` to `x`/`y`. `canvas_shape_drag`
  delta fields changed from `dx`/`dy` to `delta_x`/`delta_y`.
- **`scroll_to` field change.** The legacy `offset` key is removed;
  use `offset_y` only.
- **Scripting scroll event family renamed** from `scroll` to
  `wheel_scrolled` (the old name collided with the scrollable widget
  family).
- **Accessibility role name aliases removed.** Concatenated forms like
  `columnheader` are gone; use underscore form only (`column_header`).
- **Color format restricted.** Only hex notation (`#RRGGBB` /
  `#RRGGBBAA`) is accepted; other color notations are rejected.
- **Shaping prop renamed.** `text_shaping` is now `shaping`.
- **`OutgoingEvent` constructor signatures changed.** Parameter types
  standardized across the SDK; callers constructing events manually
  will need updating.
- **`CoalesceHint` added to `OutgoingEvent`.** The hardcoded coalescing
  table is removed. Extensions and host code that relied on implicit
  coalescing behavior must set `CoalesceHint` explicitly.
- **Core is zero-I/O.** Platform effects (file dialogs, clipboard,
  notifications) moved out of `plushie-core` into the binary crate.
  `Core` now returns `CoreEffect` variants instead of performing I/O.
  Extension authors using core directly will see a different effect API.
- **IME event family names changed** to avoid collisions with other
  event families.
- **Key event shapes unified.** Scripting and real key events now use
  the same field layout.

### Added

- Event throttling and coalescing system: `EventEmitter` with per-event
  `max_rate` and session-wide `default_event_rate` for rate-limited
  delivery. `CoalesceHint` on `OutgoingEvent` replaces hardcoded
  coalescing tables so extensions get equal footing with built-in events.
- Transport abstraction for renderer-owned host startup, SSH, and remote
  rendering scenarios. A background writer thread handles non-blocking
  I/O in windowed mode.
- Canvas interactive shapes: hit testing, hover/pressed styles, drag
  events, tooltips, and semantic click/press/release events on
  individual shapes.
- Canvas shape groups for composing multi-shape elements into a single
  interactive unit.
- Canvas keyboard navigation: Tab/Shift-Tab between shapes, arrow keys,
  Home/End, PgUp/PgDown, Enter/Space activation, Escape to exit.
- Canvas shape accessibility via `A11yOverride` wrappers, using the same
  system as all other widgets. Focused event emitted on keyboard focus
  transitions.
- Canvas interactive field validation with warnings for unknown keys.
- Overlay `flip` prop for auto-flipping when popup content overflows the
  viewport edge.
- Overlay `align` prop for cross-axis alignment (start, center, end).
- Accessibility overrides: `disabled`, `position_in_set`, `size_of_set`,
  `has_popup` exposed to host SDKs.
- Table semantic roles (Table, Row, Cell, ColumnHeader) for screen
  reader navigation.
- Widget `label`, `alt`, `description`, and `decorative` props passed
  through to iced's accessibility layer.
- Headless mode: announce events and `find_focused` query responses.
- Session lifecycle events (`session_error`, `session_closed`) and
  error response when `max_sessions` is exceeded.
- Duplicate node ID detection and error reporting on snapshot.
- `Debug` impls on all public SDK types.
- Extension `InitCtx` and enriched `RenderCtx` with `window_id` and
  `scale_factor`.
- `TreeNode` convenience methods (`prop_str`, `prop_f32`, `prop_bool`,
  etc.) and `testing` module helpers for extension authors.
- Property-based tests (proptest) for codec and prop helpers.
- Headless mode: custom font loading from Settings and `load_font` ops.

### Changed

- `CoalesceHint` on `OutgoingEvent` drives coalescing decisions; the
  hardcoded coalescing table in the emitter is removed.
- Accessibility role names standardized to underscore form only
  (concatenated aliases like `columnheader` removed).
- Color values standardized to hex-only format (`#RRGGBB` /
  `#RRGGBBAA`); other notations are rejected.
- `parse_shaping` reads the `shaping` prop (was `text_shaping`).
- Core is zero-I/O: platform effects moved out of `plushie-core` into the
  binary crate. Core now returns `CoreEffect` variants instead of
  performing I/O directly.
- Shared message processing logic between daemon and headless modes
  (extracted into reusable helpers).
- Extension caches unified: `core.caches.extension` used everywhere
  instead of separate per-mode storage.
- `canvas_scroll` position fields renamed from `cursor_x`/`cursor_y` to
  `x`/`y`.
- `canvas_shape_drag` delta fields use `delta_x`/`delta_y` (not
  `dx`/`dy`).
- Scripting scroll uses `wheel_scrolled` event family (was `scroll`,
  which collided with the scrollable widget family).
- `scroll_to` uses `offset_y` only (removed legacy `offset` key).
- Workspace-level lints replace per-crate `#![deny(warnings)]`.
- `OutgoingEvent` constructor parameter types standardized across the
  SDK.
- Scripting and real key event shapes unified.
- IME events use distinct family names to avoid collisions.
- Event field names aligned with protocol spec.

### Fixed

- Overlay `operate()` forwards to both anchor and content children,
  fixing accessibility and focus traversal for overlaid widgets.
- Subscription rate not cleared when re-subscribing with `max_rate`
  removed; coalesce key collision between similarly-named events.
- `prop_f32`/`prop_f64` reject NaN and Infinity from string parsing.
- Input clamping across widget props (padding, color channels, range
  bounds, spacing, opacity, etc.).
- Content size limits: markdown capped at 1 MB, text_editor at 10 MB.
- Resource limits: images capped at 4096 handles / 1 GiB total, fonts
  at 16 MiB per file / 256 runtime loads, font family name length
  bounded, dash segment intern cache bounded.
- Tree depth limit (256) on recursive functions (`find_window`,
  `collect_window_ids`).
- Window size and position clamped to reasonable bounds.
- Animation epoch resets on `Reset` message for clean hot-reload.
- Bounded channels for multiplexed headless sessions, preventing
  unbounded memory growth.
- Session thread `catch_unwind` with error events; extension
  `catch_unwind` on `fresh_for_session` and `handle_event`.
- Validate schemas added for checkbox, toggler, and radio (`line_height`,
  `wrapping`, `shaping`) and pane_grid (`split_axis`).
- Image `border_radius` validate type corrected (Number, was Any).
- `f64`-to-`f32` conversions clamped via `f64_to_f32` helper to avoid
  silent overflow.
- `tree_hash` returns sentinel on serialization failure instead of
  panicking.
- Headless mode: canvas interact actions (`canvas_press`, `canvas_release`,
  `canvas_move`) now inject real iced mouse events, producing shape-level
  events (enter/leave/click/drag) just like windowed mode. Previously
  they were synthetic-only and could not trigger canvas shape interaction.
- Headless mode: break event injection loop on EOF mid-interact; use
  `cancelled` status for unavailable async effects; emit
  `theme_changed` subscription events.
- Decode errors include debug context; `set_global` panic documented.
- Binary mode set on stdin/stdout for Windows compatibility.
- `list_images` query returns correct response kind.
- Wayland: no-op warnings for unsupported window position ops;
  fullscreen behavior documented.
- `last_slide_values` cleared on snapshot to avoid stale slider state.
- `ExtensionCaches` logs warning on type mismatch in `get`/`get_mut`
  instead of silently returning `None`.
- Font log accuracy and panic logging improvements.

## [0.3.1] - 2026-03-19

### Fixed

- Preserve iced widget defaults when props are unset. Padding,
  spacing, text size, and other optional props now use `Option`
  return types from parsers. When absent from the wire message,
  the widget setter is skipped and iced uses its built-in default.
  Affected widgets: button, container, window, column, row, grid,
  keyed_column, text_input, pick_list, combo_box, table.

## [0.3.0] - 2026-03-19

Initial public release.
