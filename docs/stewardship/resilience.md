# Resilience

Plushie is meant to behave predictably when things go wrong: a
buggy widget, a malformed wire message, a panic in the user's
update function, a broken pipe to the renderer, a font that fails
to parse, a session that crashes mid-flight. Resilience here is
about graceful behavior under those conditions, not about
hardening against an attacker; that distinction is in
`trust-model.md`.

The user-facing promise comes from the host SDK side: a renderer
crash auto-recovers with state re-sync, an app exception reverts
to the last good state, neither side can take the other down.
plushie-rust holds up its half of that promise.

## What resilience means here

- **Panic isolation between widgets.** A widget that panics
  affects that widget, not the renderer and not its siblings.
  Widget calls run inside `catch_unwind`; caught panics surface
  as structured `session_error` events so the host can react.
- **Session isolation in multiplexed mode.** Concurrent sessions
  in headless and mock modes run in their own threads with
  isolated state. A panic or crash in one session does not
  cascade to others.
- **Fault tolerance across the wire.** A renderer crash is
  detected by the host bridge, which auto-restarts and re-syncs
  state. An app exception in `update` or `view` reverts to the
  last good state and surfaces the error.
- **Defensive parsing at boundaries.** Wire codec, font loading,
  SVG parsing, image decoding all assume their input could be
  wrong. Rejection with a structured error is the right outcome;
  crashing is not.
- **Hard caps as correctness bounds.** `MAX_TREE_DEPTH`,
  `MAX_LOADED_FONTS`, `MAX_FONT_BYTES`, `MAX_IMAGES`, the wire
  message cap. These are not security limits and not
  performance targets (see `performance-bar.md`); they exist so
  the renderer can stop cleanly when something upstream has
  gone wrong.
- **Clean exit on broken transport.** When the wire pipe breaks,
  the renderer exits cleanly via `iced::exit()`. Continuing to
  operate on a half-open transport is worse than shutting down,
  because the host's bridge restart is the recovery path and it
  cannot fire if the renderer is wedged instead of dead.

## What is appropriate to fail fast on

Some conditions are not recoverable at the framework level and
should fail fast rather than degrade:

- **Programming errors that violate invariants the surrounding
  code holds.** A widget type missing from the registry after
  startup is a programming error; the right behavior is panic
  with a clear message, not silent fallback.
- **Unrecoverable initialization failure.** GPU init failure on
  a headless machine should produce a structured error and
  exit; attempting to render without a renderer is not a
  degraded mode worth supporting.
- **Memory corruption indicators.** Atomic ordering violations,
  mutex poison from a thread that cannot have panicked, etc.
  are signals that state is gone and continuing is worse than
  aborting.

The line: degrade gracefully on user-facing conditions (parsing,
transport, widget bugs, session-level crashes). Fail fast on
framework-level invariant violations.

## Patterns in the codebase

Worth maintaining as the project evolves:

- `catch_unwind` around widget calls; structured error events
  on caught panics.
- Mutex poison recovery via `unwrap_or_else(|e| e.into_inner())`
  for state that survives another thread's panic. The codebase
  is currently inconsistent here; some sites recover, some
  propagate. Converging to recovery is a real bug fix when the
  lock guards survivable state, and worth doing whenever a
  related change touches the area.
- `parking_lot::Mutex` where poison is not meaningful or
  available.
- SVG parsing in worker threads with a deadline; parse-or-give-up
  semantics rather than unbounded waits.
- Hard caps with structured error reporting on violation, never
  silent truncation.
- Broken-pipe detection in the emit path returns `iced::exit()`.
  This is the load-bearing piece of the "neither side takes the
  other down" promise; emit paths that discard or coalesce
  away the exit signal are resilience bugs.

## What resilience is not

- **Not adversarial-input hardening.** The threat model is
  "things go wrong," not "attacker is trying to crash." Findings
  framed as the latter are usually misframed; see
  `trust-model.md`.
- **Not perfectionism.** The runtime does not try to fix the
  user's logic for them; it reverts and surfaces the error.
  The renderer does not invent placeholders for unknown widget
  types; it surfaces the error.
- **Not retry-at-any-cost.** A failed operation reports a
  structured error; the host or the user's `update` function
  decides whether to retry. The framework does not retry on
  its own.
- **Not defense against impossible states.** Adding a defensive
  branch for a condition that cannot occur in practice is
  accidental complexity, not resilience. The bar for "cannot
  occur" is reading the surrounding code and being confident
  in the invariant, not exhaustive proof.

## Implications

The resilience axis applies across working decisions in the
codebase:

- A real things-go-wrong path producing an ungraceful failure
  (panic propagating across the runtime, deadlock, hang,
  silently swallowed error on a path the host depends on,
  broken-pipe wedge) is in scope today and earns priority.
- Inconsistency between resilience patterns (one site recovers
  from poison, another propagates; one site logs and exits,
  another logs and continues) is itself a resilience bug
  because future maintainers cannot predict behavior.
  Convergence on the established pattern is real work.
- Defensive layers for conditions that cannot occur given the
  surrounding invariants are out of scope; they add accidental
  complexity without reducing real failure modes.
- Aborting on conditions where graceful degradation is the
  right answer ("this should panic on malformed input") is
  the wrong direction; the established pattern is
  reject-and-report.
