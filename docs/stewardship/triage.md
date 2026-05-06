# Triage

How proposed work gets evaluated against the stewardship docs.

Sources of proposed work are many: design proposals, refactor
ideas, library upgrades, feature requests, breaking-change
calls, "while I was in there" cleanups, cross-SDK divergence
flags, observations from review passes. The flow below applies
regardless of source. The underlying docs (`posture.md`,
`goals-and-non-goals.md`, `trust-model.md`, `resilience.md`,
`performance-bar.md`, and the `roadmap/` items) are the
authority on each axis; this file is a consolidated routing
tool.

## Outcomes

For any proposed work, one of:

- **Do.** Aligned with a stated goal, addresses a real bug, or
  is plain maintenance hygiene that does not warrant a
  stewardship-level question.
- **Defer to a roadmap item.** Real concern tied to a
  considered direction not currently scheduled. Append to the
  relevant `roadmap/<item>.md` "Observations" section as
  context for when the work is taken up.
- **Decline.** Misframed against the trust model, defends
  against speculative futures or impossible states, asks for
  work without the evidence the relevant doc requires, or
  otherwise lands on a stated non-goal.
- **Route to cross-SDK parity.** Concerns parity drift or an
  SDK API shape that affects parity. Goes through the
  `plushie-sdk-parity` workflow rather than being decided
  here.

## Routing flow

For a piece of proposed work, run these in order. First match
wins.

1. **Cross-SDK shape.** Does the work alter or surface drift
   in an API shape, behavior name, or wire form across
   multiple SDKs? Route to the parity workflow.
2. **Trust-model misframe.** Does the proposal assume a threat
   model or boundary the project does not currently make a
   claim against (wire as its own crypto, host-as-adversary
   under an unclaimed boundary, browser-grade isolation of
   arbitrary remote hosts)? Decline; reference
   `trust-model.md`.
3. **Renderer-to-host integrity.** Does the work touch the
   renderer-to-host channel in a way that loosens the closed
   protocol surface (host-side eval paths, untyped opaque
   blobs, spoofable response correlation, unsafe wire parser
   shapes)? Treat as a deliberate decision, not a routine
   refactor; default to no unless there is a strong reason
   and the decision is recorded.
4. **Resilience axis.** Does the work address a real
   things-go-wrong path that fails ungracefully, or
   inconsistency between resilience patterns the codebase
   already uses? Do; reference `resilience.md`. Conversely,
   does the proposal add defensive layers for conditions that
   cannot occur given the surrounding invariants? Decline.
5. **Memory corruption or RCE shape.** Anywhere in the
   codebase, on either side. Do; this is a stated direction
   today regardless of the broader capability-manifest
   roadmap.
6. **Wire protocol correctness.** Encode and decode symmetry,
   round-trip through the codecs, cross-codec consistency,
   field-name drift between sender and receiver. Do; stated
   goal.
7. **Lightweight by default.** Does the work consolidate
   redundant work, choose a data structure better suited to
   the realistic profile, or remove clearly unnecessary
   per-call cost, while preserving or improving readability?
   Do; reference `performance-bar.md`. Conversely, is the
   work clever-for-speed at the cost of intent, or a big-O
   claim without realistic N? Decline absent measurement.
8. **Host-to-renderer surface.** File path inputs, effect
   dispatch, transport spawn surface, capability scoping.
   Defer to `roadmap/capability-manifest.md` "Observations,"
   unless the issue is also memory-corruption or RCE shaped.
9. **Stated non-goal.** Backwards compatibility before 1.0,
   API stability hardening as standalone work, coverage
   milestones, refactoring without a forcing function,
   defending against a speculative deployment shape. Decline;
   reference `goals-and-non-goals.md`.

If nothing matches and the work is plain maintenance
(advisories, portability bugs, broken examples, dead code,
typo-class corrections, obvious self-consistency restorations),
the default is to do it without a stewardship category. The
flow earns its keep on the harder cases: declining speculative
defenses, deferring host-to-renderer concerns to the roadmap,
recognizing trust-model misframes, distinguishing real
algorithmic consolidation from speculative micro-optimization.

## When the docs need updating

If the proposed work feels stewardship-level (a real direction
question, a new threat model, a new constraint, a posture the
docs have not yet taken) but does not match any axis above,
that is a signal the docs are missing a category. Surface the
question to the maintainer rather than improvising a category,
and update the docs once the direction is settled.

The docs decay when every novel question gets shoehorned into
the closest existing axis. They stay useful by being explicit
about what they cover and acknowledging when they do not cover
something.
