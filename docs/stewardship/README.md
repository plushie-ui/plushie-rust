# Stewardship

Working notes on the project's direction, trust model, goals, and
deliberate non-goals. The intent is to make the project's posture
explicit so that any proposed work, whether a feature, a refactor,
a library upgrade, an API change, a breaking-change call, or an
observation surfaced during a review, can be evaluated against a
stable reference instead of restated intuition.

These docs are about long-lived guiding principles, not about any
particular review or report. They name the axes the project takes
seriously, the disciplines that hold them up, and the directions
that are explicitly out of scope. They should outlive any
particular finding, proposal, or working pass.

A dense summary of every doc in this directory lives in the
project's top-level `CLAUDE.md` "Stewardship" section. The files
below are the authority on each axis when an axis is in play.

## Layout

- `posture.md` describes what plushie is, who it is for, the
  cross-SDK relationship, the project's stage, and the recurring
  disciplines that hold it together.
- `goals-and-non-goals.md` lists the testable shipping criteria
  the project optimizes for, and the explicit non-objectives it
  declines work against.
- `trust-model.md` describes what plushie protects, what it does
  not protect against today, and how those choices shape design
  and triage decisions.
- `resilience.md` describes how plushie behaves when things go
  wrong (panic isolation, fault tolerance, parser timeouts,
  hard caps as correctness bounds), and where fail-fast is the
  right answer instead of graceful degradation.
- `diagnostics.md` describes the taxonomy for structured
  diagnostics: fatality, scope, delivery surface, payload shape,
  and how those choices differ from log severity.
- `performance-bar.md` lists the working principle for keeping
  plushie lightweight, the readability-as-bound rule for
  optimizations, and numeric direction for the realistic
  application profile.
- `test-discipline.md` describes the integration-spine rule
  (tests exercise the real renderer), the three test modes
  shared across SDKs, when mocking is acceptable, and the
  related working principles (tests as documentation, slow
  tests as slow code, failing test before fix).
- `simplicity.md` describes the clarity bar code in plushie
  has to clear, the discipline around when an abstraction
  earns its place, the preference for local complexity over
  global complexity, and the codebase's functional flavor.
- `triage.md` consolidates the routing logic from the other
  docs into a single first-match-wins flow for evaluating
  proposed work. The underlying docs remain the authority on
  each axis.
- `roadmap/` holds direction items that are stated goals or
  considered directions but not currently scheduled work.
  Observations that relate to a future direction get captured
  against the relevant roadmap item rather than as standalone
  work, regardless of source. See `roadmap/README.md` for the
  current items.

More files appear here as topics get carved out (named
invariants, breaking-change posture, iced-fork stewardship,
etc.).

## How to use these docs

When a piece of proposed work shows up (a feature idea, a
refactor proposal, a library upgrade, an observation from a
review pass, a "this code feels off" instinct), the question is
not "is the proposer wrong" but "does this map to something the
project has committed to or explicitly declined." `triage.md`
is the routing tool; the other docs are the authority on each
axis.

Common shapes:

- Work aligned with a stated goal: just do it.
- Work that lands on a stated non-goal: decline; the close note
  references the relevant doc so the next person sees why.
- Work tied to a roadmap item: append to that roadmap file's
  "Observations" section as context for when the direction is
  taken up. Do not open it as standalone work on the strength
  of a roadmap-only relevance.
- Work that does not map to anything here: either it is plain
  maintenance (just do it) or it is a stewardship-level
  question the docs have not yet taken a position on. In the
  latter case, the conversation happens here, before the work,
  not in a ticket queue or a refactor PR.

## What this directory is not

Not a marketing surface. Not a public security policy. Not an
exhaustive design document. Audience is maintainers and agents
working on the codebase. If a doc grows past one screen, it
probably wants to be split.
