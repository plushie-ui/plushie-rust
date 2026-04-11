//! Shared runtime internals for both direct and wire mode runners.
//!
//! The runtime manages the event loop lifecycle that both runners
//! share: calling the user's App callbacks, managing subscriptions,
//! tracking window state, and executing commands.

use plushie_core::protocol::TreeNode;

use crate::App;
use crate::widget::WidgetStateStore;

pub mod normalize;
pub mod tree_diff;

/// Build, expand, and normalize a view tree from the current model.
///
/// Calls `App::view()`, expands composite widgets, and normalizes
/// IDs. Used by both the wire and direct runners to produce the
/// canonical tree representation.
pub fn prepare_tree<A: App>(
    model: &A::Model,
    widget_store: &mut WidgetStateStore,
) -> (TreeNode, Vec<String>) {
    let view = A::view(model);
    let expanded = widget_store.expand_tree(&view);
    normalize::normalize(&expanded)
}
