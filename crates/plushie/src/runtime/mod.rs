//! Shared runtime internals for both direct and wire mode runners.
//!
//! The runtime manages the event loop lifecycle that both runners
//! share: calling the user's App callbacks, managing subscriptions,
//! tracking window state, and executing commands.

use plushie_core::protocol::TreeNode;
use plushie_core::tree_walk::{WalkCtx, walk};

use crate::App;
use crate::widget::{ExpandWidgetsTransform, WidgetStateStore, register_expanders};

pub mod normalize;
pub mod subscriptions;
pub mod tree_diff;
pub mod view_errors;

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
) -> (TreeNode, Vec<String>) {
    let mut registrar = crate::widget::WidgetRegistrar::new();
    let mut tree = A::view(model, &mut registrar);

    // Merge newly-registered widget expanders into the store before
    // walking so the expand transform has up-to-date state.
    register_expanders(widget_store, registrar);

    let mut expand = ExpandWidgetsTransform::new(widget_store);
    let mut normalize_pass = normalize::NormalizeTransform::new();
    let mut ctx = WalkCtx::default();
    walk(&mut tree, &mut [&mut expand, &mut normalize_pass], &mut ctx);

    // Post-expansion a11y rewrite + missing-name checks. These stay
    // in a separate traversal because they need the full set of
    // declared IDs before they can resolve cross-widget references.
    let (warnings, _ctx) = normalize::finalize_a11y(&mut tree, ctx);
    (tree, warnings)
}
