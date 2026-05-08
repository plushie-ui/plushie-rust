# Project posture

What plushie is, who it is for, and the disciplines that keep
it that way.

## What plushie is

A native desktop GUI renderer driven by a typed wire protocol.
The renderer (Rust binary, native windows via iced) is shared
across every host SDK; each SDK implements an Elm-architecture
app runtime against it. Six host SDKs exist: Elixir (canonical
reference), Rust, Gleam, Python, TypeScript, Ruby. The Rust SDK
can additionally run the renderer in-process; that is an
optimization for the same-language case, not a different
architecture.

## Audience

- App developers writing Plushie apps via a host SDK. They see
  the host SDK and the renderer binary; they do not typically
  see the wire protocol or any Rust internals.
- Widget authors implementing custom native widgets in Rust via
  `plushie-widget-sdk`. They see the widget trait and the canvas
  engine.
- SDK authors implementing or maintaining a host SDK. They see
  the wire protocol, the parity surface across SDKs, and the
  renderer's startup contract.

The two public crates are `plushie` and `plushie-widget-sdk`.
Everything else (`plushie-core`, `plushie-core-macros`,
`plushie-renderer-engine`, `plushie-renderer-lib`,
`plushie-renderer`, `plushie-renderer-wasm`) is internal.
These crates are published on crates.io as a distribution
requirement (the SDK and build tooling depend on them at compile
time), but they carry no stable API obligations and are not part
of any audience's stable surface.

## Cross-SDK relationship

Six host SDKs is a load-bearing constraint, not an accident.

- Elixir is the canonical reference for API shape. When a
  concept's name, structure, or behavior is contested across
  SDKs, Elixir is the tiebreaker.
- plushie-rust is the protocol authority. Wire protocol changes
  happen here first; the spec lives at `docs/protocol.md`. A
  wire change is a six-SDK change.
- Cross-SDK parity is audited through the `plushie-sdk-parity`
  repo. Findings about cross-SDK drift route through that
  workflow rather than as standalone plushie-rust tickets.
- Within-language idiom prevails on syntax (snake_case vs
  camelCase, methods vs functions, keyword vs positional args).
  Concepts, names, parameter ordering, and behavior converge
  across SDKs so that porting an app from one SDK to another is
  straightforward.

A plushie-rust API rename that does not propagate to the other
SDKs is drift, not refactoring. "More idiomatic in Rust" alone
is not sufficient justification for breaking parity.

## Stage

Pre-1.0. There is no backwards-compatibility obligation today.
When the best design requires renaming a field across every
SDK, that is the right call. Each release notes breaking
changes explicitly in the changelog.

The 1.0 boundary is when stability obligations begin. Until
then, the priority is getting the shape right, not preserving
the current shape. Once 1.0 lands, that flips: every observable
behavior starts to harden into a contract whether documented or
not, so the pre-1.0 window is the time to settle questions about
shape, naming, and structure that will be expensive to revisit.

## Disciplines

Recurring decision rules. Not negotiable on a per-ticket basis.

- **Tests run through the real renderer.** Default test
  backends in every SDK exercise the actual binary; mocking is
  reserved for failure modes the binary cannot exhibit cleanly.
  See `test-discipline.md` for the full posture.
- **Cross-SDK claims are verified, not assumed.** Findings about
  parity drift are confirmed by reading source on each side.
  "It looks like" is not a verification.
- **Design before code at boundaries.** Wire protocol, public
  SDK API, widget trait surface. Internal refactors can iterate
  fast; boundary changes pay the design tax up front.
- **Clarity is the bar.** Code reads clearly to someone new to
  the file; abstractions earn their place by use, not by
  hypothesis; complexity is treated as a cost. See
  `simplicity.md`.
- **No half-built features.** A feature lands fully or not at
  all. Half-built features create drift in the parity surface
  and accumulate into "the docs say it does X but three SDKs
  do not actually."
- **Local cleanup, not scope creep.** Small, low-risk
  improvements to code under active modification are welcome.
  Larger or risky adjacent improvements get noted and advocated
  for as follow-on work, not silently rolled into the current
  change.
- **No legacy or compatibility shims.** Pre-1.0; remove dead
  paths cleanly rather than preserving old behavior.
