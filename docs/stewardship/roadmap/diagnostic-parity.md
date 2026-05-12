# Diagnostic Parity

Typed diagnostics should be a shared host-SDK contract, not a Rust-only
convenience. The renderer already has the canonical shape:
`plushie_core::Diagnostic` serialized under
`DiagnosticMessage { type: "diagnostic", session, level, diagnostic }`.
The host SDKs should make that payload visible to tests and apps without
requiring log scraping or string parsing.

This roadmap is about exposure parity. It does not add new diagnostic
kinds, change renderer fatality semantics, or turn diagnostics into a
trust boundary. The taxonomy and payload rules live in
`../diagnostics.md`.

## Current Contract

- The wire carrier is a top-level `diagnostic` message with `session`,
  `level`, and a typed `diagnostic` payload. The payload uses the
  `kind` discriminator from `plushie_core::Diagnostic`.
- Unknown diagnostic kinds should be loud. A newer renderer emitting a
  kind an SDK does not know about is version skew, not ignorable noise.
- Tests should collect typed diagnostics by default and provide an
  explicit escape hatch for tests that exercise diagnostic paths.
- Apps should be able to branch on diagnostic kind and structured
  fields. `DiagnosticLevel` remains an attention signal, not fatality.

## Source Survey

Rust is the protocol authority. `crates/plushie-core/src/diagnostic.rs`
defines `Diagnostic` and `DiagnosticKind`; `protocol/outgoing.rs`
defines `DiagnosticLevel` and `DiagnosticMessage`. The Rust testing
guide describes strict diagnostics, `allow_diagnostics`,
`has_diagnostic(DiagnosticKind::...)`, and `assert_no_diagnostics`.

Elixir has typed diagnostic structs and a carrier:
`lib/plushie/event/diagnostic.ex`,
`lib/plushie/event/diagnostic/variants.ex`, and
`lib/plushie/event/diagnostic_message.ex`. The protocol decoder turns
top-level diagnostics into `%Plushie.Event.DiagnosticMessage{}`. Runtime
command depth also emits a typed `DispatchLoopExceeded` locally. The
test case helper still collects `[:plushie, :diagnostic]` telemetry
metadata and its failure copy says "Prop validation diagnostics", which
keeps the test surface narrower than the typed runtime surface.

Gleam has a typed `event.Diagnostic` union and `event.DiagnosticLevel`.
Its wire docs say top-level diagnostics decode into typed
`Error(Diagnostic(..))`, and unknown kinds fail decode. The main testing
module still exposes `diagnostics(ctx) -> List(String)` as a placeholder
that always returns an empty list, so typed diagnostics are documented
and decoded but not yet useful from the host test harness.

Python has `src/plushie/diagnostics.py` with dataclass variants and a
`DiagnosticMessage` carrier. `src/plushie/protocol.py` decodes top-level
diagnostics, mirrors them to logging, and raises on unknown kinds.
Runtime and testing fixtures collect both legacy `Diagnostic` events
and typed `DiagnosticMessage` values. The remaining gap is mostly
surface naming and docs: several helpers still describe the buffer as
prop validation diagnostics even though it now contains broader typed
diagnostics.

Ruby has `lib/plushie/event/diagnostic.rb` with typed `Data` variants,
an unknown-kind decoder failure, and `Event::DiagnosticMessage`.
Runtime and test sessions intercept typed diagnostic messages and
buffer them. Like Python, helper comments and docs still describe the
surface as prop-validation-oriented in places.

TypeScript has a typed `Diagnostic` discriminated union,
`DiagnosticKind`, `DiagnosticLevel`, and `DiagnosticMessage` in
`src/client/protocol.ts`; unknown kinds throw during decode. Runtime
currently logs top-level diagnostic messages via
`handleDiagnosticMessage`, while `getDiagnostics()` returns only
widget-owned `PropValidationDiagnostic` events captured from the normal
event stream. This is the largest app and test exposure gap.

## Parity Gaps

- Test collection is inconsistent. Rust, Python, and Ruby can assert on
  buffered typed diagnostics. Elixir has typed runtime diagnostics but
  its automatic test collector is still telemetry-metadata based. Gleam
  has a placeholder that returns no diagnostics. TypeScript buffers only
  legacy widget diagnostic events.
- App delivery differs. Elixir, Gleam, Python, and Ruby expose typed
  carrier objects or union variants that app code can match. TypeScript
  decodes the carrier but logs it instead of delivering or buffering it
  as a first-class runtime diagnostic.
- Naming is stale in several SDKs. Public helpers and docs often say
  "prop validation diagnostics" even when the implementation handles
  runtime, transport, settings, accessibility, and command-loop
  diagnostics.
- Strict-mode policy is not aligned. Rust documents strict diagnostics
  with an opt-out for diagnostic-path tests. Ruby and Elixir have
  teardown checks in their test helpers. Python exposes assertion and
  drain helpers but the default strictness depends on fixture use.
  Gleam and TypeScript need the typed buffer before a meaningful strict
  policy can be shared.
- Diagnostic-path assertions are not equally ergonomic. Rust uses
  `DiagnosticKind`. Python and Ruby use typed classes. Gleam can match
  union variants once the harness buffers them. TypeScript can narrow on
  `kind` once top-level diagnostics enter the buffer.
- Session attribution is unevenly surfaced in tests. The wire carrier
  always has `session`, but some test helpers only present message text
  or legacy event fields.

## Rollout Direction

Start from the wire carrier and preserve language idiom:

- Every SDK should decode top-level `diagnostic` into a typed carrier
  that includes `session`, `level`, and the typed payload.
- Every SDK should fail loudly on unknown `kind`.
- Every SDK test harness should buffer the typed carrier. A diagnostic
  buffer should not discard session or level.
- Every SDK should keep a simple `assert_no_diagnostics` check and a
  drain helper. Names should not imply prop validation only.
- Every SDK should provide an idiomatic assertion for a specific kind:
  enum discriminant in Rust, struct class in Elixir and Ruby, dataclass
  class in Python, union constructor in Gleam, and discriminated-union
  `kind` in TypeScript.
- Diagnostic-path tests should opt out of automatic strict failure, or
  drain and assert the expected diagnostic before teardown.

Suggested order:

- Align terminology first in SDK docs and helper comments where the
  implementation already buffers typed diagnostics. Python and Ruby are
  close to the desired shape.
- Change TypeScript runtime state so top-level `DiagnosticMessage`
  values are buffered by `getDiagnostics()` in addition to console
  mirroring. Keep legacy widget diagnostics only if they still arrive
  through the event stream, but prefer the typed carrier for new tests.
- Change Gleam testing to collect typed diagnostics from the runtime or
  session backend instead of returning an empty placeholder list.
- Change Elixir test collection to buffer `%DiagnosticMessage{}` where
  possible, while preserving existing telemetry collection only for
  legacy emit sites that have not moved to the carrier.
- Add a small diagnostic contract test in each SDK that drives a stable
  renderer-emitted diagnostic and asserts kind plus one structured
  field. `required_widgets_missing`, `font_family_not_found`, or
  `missing_accessible_name` are good candidates because they do not
  require malformed wire bytes.

## Open Questions

- Should app runtimes deliver top-level diagnostics through `update`, or
  should they always buffer and log unless the user subscribes to them?
  Existing SDKs differ, and the answer affects whether diagnostics are
  control-flow events or observability data.
- Should strict test mode fail on `info` diagnostics, or only `warn` and
  `error`? Rust's guide says every diagnostic accumulates; shared SDK
  policy should make the severity rule explicit.
- Should legacy widget-owned `family: "diagnostic"` events remain part
  of the public contract, or should they become compatibility shims once
  all emit sites use `DiagnosticMessage`?

## Done Shape

Diagnostic parity is in place when a maintainer can write the same
intent in every SDK test:

- enable normal test diagnostics collection,
- exercise a renderer-backed diagnostic path,
- assert no unexpected diagnostics,
- assert an expected diagnostic by kind and structured fields without
  parsing display strings,
- see unknown diagnostic kinds fail as host and renderer version skew.
