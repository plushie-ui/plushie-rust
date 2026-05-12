//! Shared runtime internals for both direct and wire mode runners.
//!
//! The runtime manages the event loop lifecycle that both runners
//! share: calling the user's App callbacks, managing subscriptions,
//! tracking window state, and executing commands.

use plushie_core::protocol::TreeNode;
use plushie_core::tree_walk::{WalkCtx, walk};

use crate::App;
use crate::widget::{ExpandWidgetsTransform, WidgetStateStore, register_expanders};

pub(crate) mod memo_cache;
pub mod normalize;
pub mod subscriptions;
pub mod tree_diff;
pub mod view_errors;
pub(crate) mod widget_view_cache;
#[cfg(any(feature = "direct", feature = "wire"))]
pub mod windows;

pub(crate) use memo_cache::MemoCache;
pub(crate) use widget_view_cache::WidgetViewCache;

/// Maximum depth of a synchronous command dispatch chain.
///
/// `Command::dispatch` and `Command::SendAfter(Duration::ZERO)` let an
/// `update` callback kick another event into the MVU loop. That is
/// convenient for chaining derived state transitions but creates a risk
/// of unbounded recursion when one update keeps producing another
/// dispatch. The runtime tracks the chain length and drops a command
/// once it reaches this cap, emitting
/// [`plushie_core::Diagnostic::DispatchLoopExceeded`] so the loop is
/// visible instead of stalling silently.
pub const DISPATCH_DEPTH_LIMIT: usize = 100;

/// Build, expand, and normalize a view tree from the current model.
///
/// Pipeline: `App::view()` produces the placeholder tree, then a
/// single walk drives `ExpandWidgetsTransform` and `NormalizeTransform`
/// together so widget expansion and scope rewriting happen in one
/// traversal instead of two.
///
/// Returns the normalized tree and any validation warnings (duplicate
/// IDs, reserved characters, unresolved a11y refs). Used by both the
/// direct runner and test session to produce the canonical tree
/// representation.
///
/// The wire runner calls `normalize` directly (no widget expansion)
/// because composite widgets are handled by the Elixir SDK, not the
/// Rust SDK's view layer.
pub fn prepare_tree<A: App>(
    model: &A::Model,
    widget_store: &mut WidgetStateStore,
    memo_cache: &mut MemoCache,
    widget_view_cache: &mut WidgetViewCache,
) -> (TreeNode, Vec<plushie_core::Diagnostic>) {
    let mut registrar = crate::widget::WidgetRegistrar::new();
    // view() returns a ViewList of top-level windows. An empty list is
    // a valid "no UI" signal (loading, transition, error state);
    // multi-window lists wrap under a transparent synthetic container
    // so the diff pipeline sees a uniform tree shape.
    let mut tree: TreeNode = A::view(model, &mut registrar).into_tree_node();

    // Merge newly-registered widget expanders into the store before
    // walking so the expand transform has up-to-date state.
    register_expanders(widget_store, registrar);

    memo_cache.begin_cycle();
    widget_view_cache.begin_cycle();
    // Scope both transforms so their &mut borrows on the caches are
    // released before we prune stale entries below.
    let mut ctx = WalkCtx::default();
    {
        let mut expand = ExpandWidgetsTransform::with_cache(widget_store, Some(widget_view_cache));
        let mut normalize_pass = normalize::NormalizeTransform::with_memo_cache(Some(memo_cache));
        walk(&mut tree, &mut [&mut expand, &mut normalize_pass], &mut ctx);
    }
    memo_cache.finish_cycle();
    widget_view_cache.finish_cycle();

    // Post-expansion a11y rewrite + missing-name checks. These stay
    // in a separate traversal because they need the full set of
    // declared IDs before they can resolve cross-widget references.
    let (warnings, _ctx) = normalize::finalize_a11y(&mut tree, ctx);

    // Dev-mode overlay injection. Gated behind the `dev` feature so
    // production builds don't carry the handle lookup at all; a
    // registered overlay handle (see `plushie::dev::register_overlay`)
    // wraps every window's content with a rebuild-status stack.
    #[cfg(feature = "dev")]
    {
        if let Some(snapshot) = crate::dev::current_overlay_snapshot() {
            tree = crate::dev::overlay::inject(tree, Some(&snapshot));
        }
    }

    (tree, warnings)
}
