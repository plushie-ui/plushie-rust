# Performance bar

Plushie is meant to feel lightweight in use and lightweight in the
process listing. That is a baseline expectation, not an optimization
target chased after the fact.

A native UI framework that pegs CPU on every interaction is worse
than the browser-based alternative; the whole point of going
native is to feel lighter than that. A host SDK that eats a
server's headroom is a non-starter for the deployment shapes the
project wants to support, where plushie shares the host with other
work. Drained laptop batteries are a quieter version of the same
problem.

## Working principle

Lightweight is achieved by not doing unnecessary work in the first
place. Optimizing a hot path after the fact is sometimes
necessary; far more of the win comes from never letting the work
appear.

Each piece of work in the system has a cost. Individually most of
them are cheap; the cost compounds across a frame, an interaction,
an app's lifetime, the user's battery. A tree walk that runs in
0.3ms looks fine in isolation; six of them per update on a
complex tree is visible latency. Watch the compounding, not just
the individual microbenchmark.

The canonical example to keep in mind: the tree-walking
consolidation that brought six redundant traversals down to one.
None of the six would have flagged a hotspot in a profile of a
small app. The consolidation was correct work because the
redundant work was unnecessary, the change made the code clearer
rather than worse, and the aggregate cost mattered for larger
apps and edge cases. That is the shape of performance work that
earns its place without a benchmark.

## Readability is the bound

Optimizations that obscure intent trade a forever cost (every
future reader) against a one-time benefit. Decline that trade by
default.

Worth doing without a benchmark because the win is obvious in
shape and readability is preserved or improved:

- Consolidating redundant traversals, dispatches, or
  serialization passes.
- Picking the right data structure for a known access pattern.
- Avoiding a clearly unnecessary allocation, clone, or
  conversion.
- Localized refactors where the optimized form is also the
  cleaner form.

Need a benchmark, profile, or repro before they land, because the
readability cost is real:

- Clever encoding, lookup, or layout schemes that change how the
  code reads.
- Big-O claims of the form "this is O(n) on a hot path" without
  realistic N. Many such claims have N in the dozens, where
  the constant factor of a HashMap or index lookup is worse
  than the linear scan.
- Optimizations on idle or rarely-hit paths (startup, settings
  parsing, error paths).
- Anything that asks the reader to look up a comment to
  understand what the code is doing.

Measurement is a tiebreaker for the second list, not a gate on
the first.

## What lightweight looks like

Numeric direction for the realistic application profile (a few
hundred to about a thousand active tree nodes, dozens of images,
one to five fonts):

- **Frame budget.** 16.67ms (60fps) for a single update cycle
  including iced's render step, the SDK update + diff + emit,
  and widget prepare work.
- **Event-to-update.** Visible by the next frame.
  Sub-millisecond wire round-trip on a local pipe.
- **Startup.** Renderer ready to accept Settings within 1s;
  first frame visible within 2s on a typical desktop.
- **Resident memory.** Under 200 MiB for the renderer at this
  profile, excluding image and font budgets.
- **Idle CPU.** When nothing is happening, neither side does
  measurable work. No periodic polling, no animation tick when
  no animation is active, no spinning subscription threads.

These are direction. There is no benchmark infrastructure in the
repo today; numbers should be tightened or relaxed when
measurement disagrees.

## Hard caps on the input side

Existing process-wide caps that bound runaway inputs:

- `MAX_TREE_DEPTH = 256`
- `MAX_LOADED_FONTS = 256` with `MAX_FONT_BYTES = 16 MiB` per
  font
- `MAX_IMAGES = 4096`
- 64 MiB wire message cap

These are correctness bounds, not performance targets. Workloads
pressing against them usually indicate something upstream went
wrong.
