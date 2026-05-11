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
