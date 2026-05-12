# Roadmap

Direction items that are stated goals or considered directions
but not currently scheduled work. Each file captures the rough
shape, the threat model or use case it could address, open
design questions, and observations from the current codebase
that may become relevant when the work is taken up.

Observations that connect to a roadmap item, from any source
(design discussions, refactors, exploration, review passes), get
appended to that item's "Observations" section rather than
tracked as standalone work.

## Items

- `capability-manifest.md` - Capability-based bounds on the
  host-to-renderer surface. Pairs with the structural
  renderer-to-host protection already in place; see
  `../trust-model.md`.
- `diagnostic-parity.md` - Typed diagnostic exposure and test-helper
  parity across host SDKs.
- `exit-animations.md` - Renderer-local ghost lifecycle for
  re-enabling exit animations without changing protocol tree
  semantics.
- `rust-sdk-panic-semantics.md` - Rust SDK app-callback panic
  rollback and typed diagnostic direction.
- `sensor-resize-parity.md` - Canonical sensor resize prop and
  event shape across SDKs.

(More items appear here as directions get articulated.)
