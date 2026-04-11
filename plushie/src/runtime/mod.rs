//! Shared runtime internals for both direct and wire mode runners.
//!
//! The runtime manages the event loop lifecycle that both runners
//! share: calling the user's App callbacks, managing subscriptions,
//! tracking window state, and executing commands.

use plushie_core::protocol::TreeNode;

use crate::App;
use crate::widget::WidgetStateStore;

pub mod normalize;
pub mod subscriptions;
pub mod tree_diff;

/// Build, expand, and normalize a view tree from the current model.
///
/// Pipeline: `App::view()` -> widget expansion -> ID normalization.
///
/// Returns the normalized tree and any validation warnings (duplicate
/// IDs, reserved characters). Used by both the direct runner and
/// test session to produce the canonical tree representation.
///
/// The wire runner calls `normalize` directly (no widget expansion)
/// because composite widgets are handled by the Elixir SDK, not the
/// Rust SDK's view layer.
pub fn prepare_tree<A: App>(
    model: &A::Model,
    widget_store: &mut WidgetStateStore,
) -> (TreeNode, Vec<String>) {
    let view = A::view(model);
    let expanded = widget_store.expand_tree(&view);
    normalize::normalize(&expanded)
}
