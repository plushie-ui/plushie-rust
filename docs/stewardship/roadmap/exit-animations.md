# Exit animation ghost lifecycle

Exit animation ghost retention is currently disabled. The renderer can
notice a removed node with an `exit` prop and route it to the
`GhostManager`, but the manager intentionally stores nothing until the
full lifecycle is implemented. Removed nodes disappear immediately.

That disabled state is the correct temporary behavior. Retaining a
removed node before it can be rendered, advanced, made inert, and
pruned through the normal renderer lifecycle is worse than not
supporting exit animations.

This roadmap item captures the direction for re-enabling exit
animations without letting renderer-local ghosts corrupt the protocol
tree, patch indices, event surface, or cleanup paths.

## Recommendation

Exit ghosts should be renderer-local visual overlays derived from a
successful removal. They should not be inserted into `Core.tree`, should
not participate in protocol indexing, and should not be observable to
the host as live widgets.

`Core.tree` remains the source of truth for the host-owned UI. A ghost is
temporary renderer state attached to a live parent for paint only. It is
advanced by the same `TransitionManager` that drives normal renderer-side
animations and is removed when its finite exit animations complete.

## Lifecycle

The intended lifecycle:

- The host renders a live node with an `exit` prop before removal.
  The prop is inert while the node remains live. Normal animatable props
  continue to behave as they do today.
- A later patch removes that child. Only a successful `remove_child`
  may promote the removed node to a ghost. A stale or invalid remove must
  not create a duplicate visual node.
- Promotion clones the removed subtree into renderer-local ghost state,
  records the live parent id, records the removal index for visual
  placement, strips the `exit` prop, and overlays the exit descriptors
  onto the cloned node's normal props.
- Exit descriptors start from the current rendered value where possible.
  If a prop was already animating, the current interpolated value is the
  start. If it was static, the removed node's static prop value is the
  start. An explicit `from` on the descriptor still wins.
- The live tree is prepared and scanned as usual. Ghosts are prepared and
  scanned through a separate renderer-local pass, or through a composed
  render tree that is never written back to `Core.tree`.
- Windowed mode keeps frame subscription active while either live
  animations or ghosts are active. Headless and mock mode advance ghosts
  from `AdvanceFrame` using the same deterministic timestamp path as
  normal animations.
- When all finite exit animations for a ghost finish, the ghost is
  pruned. Pruning also removes its animation state, interpolated props,
  widget factory state, cached render state, subscriptions collected
  from the ghost, and any other renderer-local state keyed by the ghost's
  node ids.
- Snapshot, reset, parent replacement, parent removal, renderer restart,
  and reduced-motion mode all clear ghosts immediately.

## Invariants

- A ghost is not a model node. Queries, tree hashes, patch targets,
  widget commands, focus search, and host-visible tree inspection see
  only `Core.tree`.
- A ghost is visual only. It must not emit widget events, effect
  requests, subscription events, focus changes, accessibility events, or
  command responses.
- A renderable node id cannot be live and ghosted at the same time. If a
  new live node appears with the same id as an active ghost, the ghost is
  dropped before rendering.
- A ghost must have a live parent. If the parent is gone, replaced, or
  moved outside the current render tree, the ghost is dropped.
- A ghost lifetime is bounded. Exit descriptors that repeat forever or
  otherwise cannot reach completion are not valid exit descriptors.
  Invalid exit descriptors degrade to immediate removal with a diagnostic,
  not a retained ghost.
- Reduced motion prefers immediate removal. The renderer should not keep
  a ghost alive just to satisfy a decorative transition.
- Ghost cleanup is part of the animation lifecycle, not a best-effort
  side path. Once a ghost is pruned, no caches or animation state for its
  ids should survive.
- Renderer crash recovery does not restore ghosts. The host recovery path
  re-syncs the live tree, and ghosts are deliberately outside that tree.

## Patch index implications

Protocol patch indices are SDK indices over live children. Ghosts must
not consume those indices.

That means:

- Patch application in `Core.tree` should not call ghost index
  adjustment. The existing patch order contract remains about live
  children only.
- The ghost removal index is for visual placement only. It may influence
  where the composed render tree paints the ghost, but it must never
  rewrite incoming patch paths or child indices.
- If a parent has live children plus ghosts, the render path may merge
  them for display. That merged child list is an ephemeral render input,
  not retained protocol state.
- A later insert at the same SDK index is applied relative to live
  children. Visually, the inserted live child and the exiting ghost may
  overlap or sit near each other until the ghost finishes. That is a
  layout consequence of exit animation, not a protocol index change.
- Removes in the same patch batch are captured from the live tree before
  each successful removal. The capture order is renderer-local metadata;
  the host's later ops still address the live tree after prior ops have
  been applied.

The existing `GhostManager::adjust_index` shape should not become part
of core patch application. If it remains useful, it should be limited to
building a renderer-local merged child list.

## Staged implementation

Each stage should leave the renderer functional with exit ghosts either
still disabled or complete enough to satisfy the invariants for that
stage. Do not enable public exit retention until the render, advance,
inertness, and prune path all exist.

### Capture and storage contract

Make `GhostManager` real storage, but keep it renderer-local and
non-rendered until later stages connect it to painting.

Work:

- Replace the unit `GhostManager` with parent-keyed storage containing
  `GhostNode` values ordered by `insert_index`.
- Keep `add_ghost` callable only from `StateChange::ExitNodes`, after a
  successful `remove_child` capture.
- Store the parent id, cloned node, visual insert index, and finished
  state. Do not write the clone into `Core.tree`.
- Keep `ghost_count_before` and `adjust_index` out of core patch
  application. If those helpers survive, document and test them as
  render-composition helpers only.
- Add cleanup helpers for parent disappearance, live-id collision, and
  reset.

Invariants:

- `TransitionManager::has_active` reflects stored ghosts.
- `Core.tree` contents, id index, patch paths, tree hashes, and query
  results are unchanged by ghost storage.
- Invalid or stale remove operations do not call `add_ghost`, because
  capture remains tied to successful live-tree removal.

Verification gate:

- Unit tests around `GhostManager` cover insertion order, clear, prune,
  parent removal, live-id collision, and no index adjustment in patch
  application.
- Existing patch tests keep proving that removed live nodes disappear
  from `Core.tree` immediately.

### Exit descriptor promotion

Convert an `exit` prop into normal animated props on the cloned ghost
subtree.

Work:

- Strip `exit` from the ghost clone before render scanning.
- Overlay exit descriptors onto the cloned node's normal props.
- Resolve each descriptor start value from explicit `from`, current
  interpolated value, then static removed-node prop value.
- Reject looping or otherwise unbounded exit descriptors with a
  diagnostic and immediate removal.
- Respect reduced motion by declining promotion and removing
  immediately.

Invariants:

- Exit animation state is keyed to ghost node ids only while the ghost
  exists.
- Live animation state for the remaining tree is not cancelled or
  retargeted by ghost promotion.
- Exit descriptors cannot create an unbounded retained node.

Verification gate:

- Animation unit tests cover explicit `from`, current animated value,
  static fallback, invalid looping descriptors, and reduced motion.
- A removal with no valid finite exit descriptors leaves no active
  ghost state.

### Renderer-local prepare and scan

Prepare and scan ghosts without writing a composed tree back into
`Core.tree`.

Work:

- Add a renderer-local ghost prepare pass, or a composed render input
  owned outside `Core.tree`.
- Feed ghost nodes through widget `prepare` and animation scanning with
  the same validation settings as live nodes.
- Keep ghost widget factory state and cached render state separately
  identifiable so prune can remove it by ghost id.
- Drop ghosts whose parent is missing or whose id collides with a live
  node before prepare or render.

Invariants:

- Ghosts do not enter host-visible query, command, focus, subscription,
  effect, accessibility, or event-dispatch paths.
- A live node id and a ghost id cannot both render in the same frame.
- Parent replacement, parent removal, snapshot, and reset clear the
  ghost before it can be prepared again.

Verification gate:

- Renderer-lib tests prove widget commands, focus search, query,
  tree_hash, subscriptions, effects, and event dispatch ignore ghosts.
- Cache tests prove prune removes interpolated props and widget state
  for ghost ids while preserving live ids.

### Visual composition

Make ghosts visible as paint-only children of their live parent.

Work:

- Build an ephemeral child list that merges live children with ghosts by
  visual `insert_index`.
- Pass the merged list only to the widget render path.
- Leave layout ownership with the live tree after removal. If the ghost
  overlaps an inserted replacement, accept that as the current visual
  contract.
- Ensure child ids from the merged list are not used as patch targets or
  host-visible traversal results.

Invariants:

- Protocol indices remain SDK indices over live children.
- The merged child list is discarded after render.
- Ghost rendering cannot emit interaction or accessibility output.

Verification gate:

- Unit or integration tests cover remove, remove followed by insert at
  the same index, and multiple removes under one parent.
- Screenshot tests in headless mode show the ghost during the exit
  interval and absence after prune.

### Advancement and pruning

Advance ghost animations through the existing `TransitionManager` timing
paths and prune them as part of the normal animation lifecycle.

Work:

- Extend `advance_all` and `advance_with_timestamp` so ghost animation
  state advances with live animation state.
- Keep windowed frame subscription active while ghosts exist.
- In headless and mock mode, advance ghosts from `AdvanceFrame` using
  the deterministic timestamp path.
- When all finite exit descriptors finish, mark the ghost finished and
  prune its node ids from animation state, interpolated props, widget
  caches, render caches, subscriptions collected during ghost prepare,
  and factory state.
- Do not emit `transition_complete` for ghost-only exit descriptors
  unless a later public event design explicitly adds that surface.

Invariants:

- Ghost lifetime is bounded by finite descriptor completion.
- Prune is atomic from the renderer's point of view: after prune, no
  renderer-local state for the ghost ids remains.
- Frame ticks stop once no live animations and no ghosts are active.

Verification gate:

- Windowed subscription tests prove animation frames continue while a
  ghost is active and stop after prune.
- Headless and mock tests prove deterministic advancement and final
  cleanup.
- Prune tests assert no stale cache, interpolated prop, subscription, or
  factory-state entries survive for removed ghost ids.

### Snapshot, reset, and recovery behavior

Close the lifecycle by making every whole-tree boundary drop ghosts.

Work:

- Clear ghosts and their associated state on snapshot and reset.
- Clear ghosts when reduced-motion state changes to active.
- Keep renderer restart behavior unchanged: recovery re-syncs only the
  live tree from the host.
- Add diagnostics for discarded ghosts where that helps local debugging,
  but do not surface ghosts as protocol state.

Invariants:

- Whole-tree replacement leaves no ghost state from the previous tree.
- Crash recovery and reconnect never attempt to restore ghosts.
- Diagnostics are informational and do not become a host contract.

Verification gate:

- Snapshot and reset integration tests assert that ghosts, animation
  state, caches, and render state are empty afterward.
- Recovery tests continue to assert that the restored state is exactly
  the host-owned live tree.

## Test plan

When this work is implemented, tests should cover:

- `GhostManager` stores ghosts by parent, preserves visual order, prunes
  finished ghosts, clears on reset, and drops ghosts whose parent or id
  collides with live state.
- Patch application promotes only successful removals with an `exit`
  prop and keeps all core patch indices live-tree based.
- Live animation pruning keeps normal removed-node cleanup intact while
  preserving promoted ghost animation state until prune.
- Windowed mode keeps frame ticks active while a ghost is animating and
  stops them after prune.
- Headless mode advances ghost animations through `AdvanceFrame` and
  produces deterministic screenshots before and after prune.
- Mock mode follows the same lifecycle without real screenshots.
- Snapshot and reset clear ghosts and associated caches.
- Reduced motion removes immediately and leaves no active ghost state.
- Pointer, keyboard, focus, widget command, query, tree hash,
  subscription, and effect paths cannot observe or interact with a ghost.
- A remove followed by insert or update under the same parent keeps SDK
  indices stable even while the exiting ghost is still visible.

## Deliberately unsupported until implemented

- Public exit animation behavior. Today `exit` does not retain or render
  ghosts.
- Rendering removed nodes in windowed, headless, or mock mode.
- Host-visible events, commands, effects, focus, accessibility, or
  subscriptions from ghosts.
- Infinite or looping exit animations.
- Ghost recovery across snapshots, resets, or renderer restarts.
- Using ghosts to preserve layout space. The live tree owns layout after
  removal; ghosts are visual retention only unless a later design adds a
  clear layout contract.
- Rewriting core patch indices to account for ghosts.
