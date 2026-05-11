# Diagnostics

Diagnostics are structured observations about things going wrong or
needing attention inside Plushie. They are not all fatal, not all
app-facing events, and not all delivered over the same surface. This
document starts the taxonomy so future emit sites choose the right
shape deliberately instead of treating every problem as a log line or
every log line as a protocol event.

The current canonical payload is `plushie_core::Diagnostic`. It is a
tagged enum serialized with a `kind` discriminator and variant-specific
fields. `DiagnosticLevel` (`info`, `warn`, `error`) is an attention
level only. It must not be used as the sole source of truth for
fatality or recovery behavior.

## Relationship To Resilience And Trust

Diagnostics sit on the resilience axis. They help the host, app, tests,
and maintainers understand malformed input, recoverable degradation,
caps, widget panics, session failure, and version skew.

They are not a new trust boundary. Renderer-to-host remains closed and
typed: diagnostic payloads are typed data, not instructions. Host-to-
renderer bounding remains the capability-manifest direction, not the
diagnostic channel's job.

Panic locations, parser details, version-skew hints, and similar debug
payloads are acceptable under the current trust model when they are
useful for recovery or diagnosis. Revisit that only if the renderer-
protection direction becomes active work.

## Fatality

Fatality describes what stops, if anything. It is separate from
`DiagnosticLevel`.

- None: observation only. The result is unchanged or already expected.
  Current examples: `FontCacheCapExceeded`, `DashCacheCapExceeded`,
  and `DashSegmentsCapExceeded` currently log as info and continue
  with fallback cache behavior.
- Degraded: work continues, but the output changes or fallback behavior
  is used. Current examples: `PropRangeExceeded` clamps a prop,
  `ContentLengthExceeded` truncates text, `SvgParseError` and
  `SvgDecodeTimeout` fall back from SVG decode, and
  `AnimationDescriptorInvalid` ignores an animation descriptor.
- Dropped operation: one unit is rejected or ignored while surrounding
  work continues. Current examples: `UnknownPatchOp` ignores one patch
  op, `DispatchLoopExceeded` drops the offending command, and
  `RequiredWidgetsMissing` lets the renderer continue so the host SDK
  decides policy.
- Callback recovered: user callback or widget code failed and was
  isolated. Current examples: `WidgetPanic` ignores the widget
  contribution or renders a placeholder, `ViewPanicked` keeps drawing
  the last-good tree, and `UpdatePanicked` catches the panic and
  returns `Command::None`.
- Session fatal: one renderer session is considered failed or closed,
  but the process may survive. Current examples: renderer panic
  handling emits `session_error` with `code: "renderer_panic"` and then
  `session_closed`; headless multiplexing relies on session isolation.
- Transport fatal: the peer can no longer trust the stream. The
  connection exits or is restarted. Current examples: broken sink
  writes return `iced::exit()`. `BufferOverflow` rejects the frame and
  closes the transport on the SDK read path.
- Process fatal: continuing would violate a framework invariant or
  lacks a usable renderer runtime. Current examples: event sink double
  initialization panics, native `panic = "abort"` is rejected at
  compile time for paths that rely on unwinding, and unrecoverable
  renderer runtime errors belong here.

Guideline: decide fatality from the recovery contract, not from how
scary the message sounds. An error-level diagnostic can be callback-
recovered (`WidgetPanic`) while a warning can still indicate degraded
output (`SvgDecodeTimeout`).

## Scope

Scope answers what the diagnostic is about and what identity fields it
should carry.

- Process: use global process context, usually with no widget ID.
  Current examples: `FontCapExceeded`, `FontCacheCapExceeded`, and
  `RendererRuntimeError`.
- Transport: use frame, codec, or protocol stream context. Current
  examples: `BufferOverflow { size, limit }` and
  `WireInputError { detail }`.
- Session: use `session` on `DiagnosticMessage`; use `session_error`
  and `session_closed` event payloads for session termination. Current
  examples: the renderer panic hook emits session-scoped events, and
  `DiagnosticMessage::with_session` is the structured diagnostic hook
  for session-aware emitters.
- App callback: use callback name and panic detail. Current examples:
  `ViewPanicked` and `UpdatePanicked`.
- Tree or window: use scoped widget ID or window ID. Current examples:
  `DuplicateId`, `EmptyId`, `TreeDepthExceeded`, `UnknownWindow`, and
  `MultipleTopLevelWindows`.
- Widget: use scoped widget ID plus widget type. Current examples:
  `WidgetPanic`, `UnrecognizedWidgetPlaceholder`,
  `MissingAccessibleName`, and prop validation diagnostics.
- Prop or content field: use widget ID, widget type when known, field
  or prop name, raw value, and normalized value when applicable.
  Current examples: `PropTypeMismatch`, `PropRangeExceeded`,
  `PropUnknown`, `ContentLengthExceeded`, and
  `AnimationDescriptorInvalid`.
- Resource: use resource name, family, source, cap, or limit. Current
  examples: `FontFamilyNotFound`, `SvgParseError`, and
  `SvgDecodeTimeout`.
- SDK protocol skew: use message or operation name plus raw payload
  when useful and bounded. Current examples: `UnknownMessageType` and
  `UnknownPatchOp`.

Identity should be stable and machine-matchable. Prefer scoped IDs,
window IDs, prop names, family names, reason tags, and caps over prose
that a host SDK would have to parse.

## Delivery Surface

Diagnostics currently travel over more than one surface. New emit sites
should choose the narrowest surface that reaches the actor who can
respond.

- Log only: use `log::{info,warn,error}` with `Diagnostic::Display`
  where possible. Use this when there is no sink yet, the only audience
  is the developer log, or the path cannot safely emit to the wire.
  Current examples: `WidgetIdTypeCollision` logs then panics during SDK
  widget registration; `ViewPanicked` and `UpdatePanicked` currently
  log from the runtime guard.
- Typed diagnostic hook: use
  `plushie_core::diagnostics::emit(level, Diagnostic)`, routed through
  the renderer sink when installed. Use this when a structured
  diagnostic should reach hosts in wire mode and tests can still
  observe logs without a sink. Current examples: widget SDK
  `WidgetPanic`, content caps, SVG diagnostics, cache caps, and
  renderer settings diagnostics.
- Standalone wire diagnostic: use
  `DiagnosticMessage { type: "diagnostic", session, level, diagnostic }`.
  Use this for renderer-to-host diagnostics that are not user
  interactions and should bypass event coalescing. Current examples:
  `EventEmitter::emit_diagnostic`, `WriterSink::emit_diagnostic`, and
  renderer stdin warnings as `WireInputError`.
- SDK system event: use `Event::System(SystemEventType::Diagnostic)`
  with the raw diagnostic message as `value`. Use this when the Rust
  wire SDK receives a standalone renderer diagnostic and needs to
  deliver it into the app loop. Current example: `wire_to_sdk_events`
  logs the diagnostic and pushes `SystemEventType::Diagnostic`.
- Session lifecycle event: use `OutgoingEvent` families
  `session_error` and `session_closed`. Use this when a session has
  failed or closed and existing event handling must observe the state
  change. Current example: the renderer panic hook emits
  `session_error` carrying `code`, `error`, and `location`, followed by
  `session_closed`.
- Command error event: use `Event::CommandError(CommandError)`. Use
  this when a widget-targeted command failed and the app can branch on
  command-specific reason fields. Current example: `command_error` is
  converted by `event_bridge` into `Event::CommandError`.
- Effect or async result: use `EffectResult`, `AsyncEvent`, or
  `StreamEvent`. Use this when the user issued a command and the
  response belongs to that command's result channel, not the
  diagnostics channel. Current examples: effect timeout, unsupported
  backend, cancellation, platform error, renderer restart, and async
  task panic payloads.

Do not duplicate the same condition across unrelated app-facing
surfaces unless the recovery contract requires both. For example,
session failure uses lifecycle events because app control flow depends
on them; a normal prop warning should remain a diagnostic.

## Structured Payload Rules

- Add a dedicated `Diagnostic` variant for every new machine-branchable
  condition. Do not shoehorn a new condition into a string detail field
  on an unrelated variant.
- The `kind` discriminator is the stable branch key. Variant fields are
  the stable context. `Display` is for logs and assertions only.
- Use snake_case reason tags for subcategories when a variant covers a
  family of related failures, as `WidgetIdInvalid { reason }` does.
- Carry bounded raw payload only when it is necessary to diagnose
  protocol skew or malformed input. `UnknownPatchOp` carries the patch
  payload; most prop diagnostics carry selected field values instead of
  the whole node.
- Use strings for values that JSON cannot represent faithfully, such as
  non-finite floats in `PropRangeExceeded.raw`.
- Include both the configured bound and observed value for cap
  diagnostics. This makes host policy possible without parsing a
  message string.
- Avoid derived counts in prose when the payload already carries the
  list or limit. If the host needs the value, make it a field.
- Prefer recoverable structured diagnostics at input boundaries:
  parsing, validation, caps, unknown widget or message names, broken
  but survivable resources.
- Prefer panic or process exit for framework invariant violations where
  continuing would hide a bug or wedge recovery.
- Keep payloads passive. A diagnostic must not ask the host to execute
  code, resolve a path, or perform an open-ended operation.

## Current Mapping

This is not exhaustive; it anchors the taxonomy to the code that exists
today.

- `DuplicateId`, `EmptyId`, `WidgetIdInvalid`, `A11yRefUnresolved`,
  `MissingAccessibleName`, and `TreeDepthExceeded`: degraded output or
  dropped tree subtree depending on variant. Scope is tree or widget.
  Delivery today is through runtime normalization warnings; callers log
  or expose them through test diagnostics.
- `PropTypeMismatch`, `PropRangeExceeded`, and `PropUnknown`:
  degraded validation warnings. Scope is prop. Delivery today is
  widget SDK validation returning display strings, not typed values.
- `ContentLengthExceeded`: degraded content field. Delivery today is
  the typed diagnostic hook through widget SDK diagnostics.
- `FontCacheCapExceeded`, `DashCacheCapExceeded`, and
  `DashSegmentsCapExceeded`: observation or degraded fallback. Scope is
  process or resource. Delivery today is the typed diagnostic hook at
  info level.
- `FontCapExceeded`, `RequiredWidgetsMissing`, and
  `FontFamilyNotFound`: dropped operation or degraded settings. Scope
  is process or resource. Delivery today is through renderer-lib
  settings and font paths emitting through widget SDK diagnostics.
- `WidgetPanic`: callback recovered at widget scope. Delivery today is
  widget SDK `catch_unwind` emitting error-level typed diagnostics;
  render falls back to a placeholder where needed.
- `UnrecognizedWidgetPlaceholder`: degraded widget output. Delivery
  today is the Rust SDK rewriting the placeholder to a visible
  container and emitting a typed warning.
- `SvgParseError` and `SvgDecodeTimeout`: degraded resource loading at
  widget scope. Delivery today is the SVG widget emitting typed
  warnings and falling back.
- `EmitterCoalesceCapExceeded`: degraded event delivery under pressure
  at process scope. Delivery today is the event emitter emitting a
  typed warning and flushing pending coalesced events.
- `ViewPanicked` and `UpdatePanicked`: callback recovered at app
  callback scope. Delivery today is the runtime guard logging typed
  display output; it is not currently routed through
  `DiagnosticMessage`.
- `DispatchLoopExceeded`: dropped command at app runtime scope.
  Delivery today is the test session recording the typed diagnostic;
  runtime docs define the cap.
- `UnknownMessageType`: dropped message at SDK protocol-skew scope.
  Delivery today is the Rust wire SDK emitting and logging a typed
  error locally. It does not deliver a diagnostic event unless the
  renderer sent one.
- `UnknownPatchOp`: dropped operation at renderer protocol-skew scope.
  A typed payload exists; renderer-side apply owns behavior.
- `BufferOverflow`: transport fatal. Delivery today is the SDK bridge
  emitting and logging a typed error, then returning an I/O error so
  the connection closes.
- `WireInputError`: dropped frame or transport warning. Delivery today
  is renderer stdin warning emitting a standalone `DiagnosticMessage`.
- `RendererRuntimeError`: process fatal. The typed variant exists for
  terminal runtime failure.
- `session_error` and `session_closed`: session fatal. Delivery today
  is renderer panic handling emitting typed event families rather than
  `DiagnosticMessage`.
- `command_error`: dropped command. Delivery today is `event_bridge`
  converting it to `Event::CommandError`.

## Direction

New diagnostic work should preserve `Diagnostic` as the canonical
structured payload and make every emit site choose fatality, scope, and
delivery surface explicitly.

The main gap is metadata, not payload shape. `DiagnosticLevel` is
already present, but fatality and scope are implicit in variant names
and comments. Before adding a parallel ad hoc error family, extend the
documentation or helper APIs so a new diagnostic variant has an
obvious home and delivery surface.

The other gap is consistency. Some paths already use the typed hook,
some return typed warnings to callers, some return display strings,
some are log-only by necessity, and session failure uses lifecycle
events. That is acceptable as a starting point, but future work should
avoid adding new surfaces unless an existing surface cannot carry the
recovery contract.
