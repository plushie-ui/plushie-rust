# Goals and non-goals

The objectives the project optimizes for, and the explicit
non-objectives it declines work against. The lists are
deliberately short; they earn their place by being the recurring
decision criteria, not by enumerating every aspiration.

## Goals

Testable shipping criteria. Findings that improve any of these
are real work.

- **Wire protocol correctness.** Messages encode and decode
  identically across every SDK and the renderer; serializer and
  parser stay in lockstep; values round-trip without coercion
  drift.
- **Cross-SDK concept parity.** Concepts (event shapes, widget
  props, command structures) converge across SDKs at the
  semantic level. Audited via the `plushie-sdk-parity` repo.
- **Lightweight by default.** Plushie is meant to feel light in
  use and in the process listing. The project actively maintains
  that, not as an optimization target chased after the fact but
  as a baseline. Algorithmic consolidation, removing redundant
  work, and choosing data structures appropriate to the workload
  are real work and welcome. See `performance-bar.md` for the
  working principle and numeric direction.
- **Panic isolation between widgets.** A widget bug crashes the
  widget, not the renderer or the app. See `resilience.md`.
- **Fault tolerance across the wire.** Renderer crash auto-
  recovers with state re-sync; app exception reverts to the
  last good state. Neither side takes the other down. See
  `resilience.md`.
- **Structural host protection on the renderer-to-host channel.**
  See `trust-model.md`. Wire changes that loosen this are
  serious decisions, not routine refactors.

## Non-goals

Explicit non-objectives. Findings or proposals that push the
project toward them get declined; they are not candidates that
lost a priority contest.

- **Browser-grade isolation of arbitrary remote hosts.** The
  trust model targets bounded boundaries. See `trust-model.md`
  and `roadmap/capability-manifest.md`.
- **DoS-proofness at any cost.** Configurable knobs over
  aggressive defaults. Caps that break legitimate-but-edge use
  cases are worse than no cap.
- **Backwards compatibility before 1.0.** The right design wins;
  the rename happens.
- **Coverage targets as a metric.** Test discipline is "exercise
  real surfaces," not "hit a percentage."
- **Micro-optimization at the cost of readability.** Clever
  encoding, layout, or lookup schemes that obscure intent need
  to earn the obscurity with measurement. Optimizations that
  look clean and do not damage readability are different and
  welcome; see `performance-bar.md` for the bound.
- **Refactoring without a forcing function.** Module size or
  file length alone is not a reason to refactor. The trigger is
  a real change that the existing structure cannot accommodate
  cleanly.
- **Per-Rust-SDK API ergonomics that diverge from cross-SDK
  shape.** See `posture.md`.
- **API stability hardening before 1.0.** `#[non_exhaustive]`,
  `#[must_use]`, sealed traits, and similar happen in a single
  planned sweep at the 1.0 cut, not piecemeal during normal
  development.
- **Defending against speculative deployment shapes.**
  Server-side WASM, multi-tenant renderer hosting,
  browser-as-arbitrary-host, and similar are not currently
  goals. Defenses against them are out of scope unless and
  until the shape is taken up.
