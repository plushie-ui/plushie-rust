//! Exit animation ghost management.
//!
//! Exit ghost promotion is disabled until removed nodes are rendered,
//! advanced, and pruned through the normal renderer lifecycle.

use crate::protocol::TreeNode;

/// A ghost node: an exiting widget that stays visible during its exit animation.
pub struct GhostNode {
    /// The widget's tree node (with exit transition props applied).
    pub node: TreeNode,
    /// The ghost's original index in its parent's children list.
    pub insert_index: usize,
    /// Whether all exit transitions have completed.
    pub finished: bool,
}

/// Manages ghost nodes for exit animations.
pub struct GhostManager;

impl Default for GhostManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GhostManager {
    /// Create an empty ghost manager.
    pub fn new() -> Self {
        Self
    }

    /// Returns true if any ghosts exist.
    pub fn has_active(&self) -> bool {
        false
    }

    /// Adds a ghost for an exiting child.
    pub fn add_ghost(&mut self, _parent_id: &str, _node: TreeNode, _index: usize) {}

    /// Returns the ghost nodes for a parent, if any.
    pub fn ghosts_for(&self, parent_id: &str) -> Option<&[GhostNode]> {
        let _ = parent_id;
        None
    }

    /// Returns the number of ghosts before a given SDK index for a parent.
    ///
    /// Used to adjust patch indices: the SDK doesn't know about ghosts,
    /// so we need to offset its indices by the ghost count before each position.
    pub fn ghost_count_before(&self, parent_id: &str, sdk_index: usize) -> usize {
        let _ = (parent_id, sdk_index);
        0
    }

    /// Adjusts an SDK-provided child index to account for ghost nodes.
    pub fn adjust_index(&self, parent_id: &str, sdk_index: usize) -> usize {
        sdk_index + self.ghost_count_before(parent_id, sdk_index)
    }

    /// Marks a ghost as finished and removes it if all ghosts for the parent are done.
    pub fn mark_finished(&mut self, parent_id: &str, ghost_index: usize) {
        let _ = (parent_id, ghost_index);
    }

    /// Removes all finished ghosts and returns a list of removed ghost node IDs.
    pub fn prune_finished(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// Clears all ghosts (used on snapshot/reset).
    pub fn clear(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Props, TreeNode};

    fn node(id: &str) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "text".to_string(),
            props: Props::default(),
            children: vec![],
        }
    }

    #[test]
    fn add_ghost_is_disabled_until_lifecycle_is_complete() {
        let mut ghosts = GhostManager::new();

        ghosts.add_ghost("root", node("child"), 2);

        assert!(!ghosts.has_active());
        assert!(ghosts.ghosts_for("root").is_none());
    }

    #[test]
    fn adjust_index_is_identity_while_ghosts_are_disabled() {
        let mut ghosts = GhostManager::new();
        ghosts.add_ghost("root", node("a"), 0);
        ghosts.add_ghost("root", node("b"), 2);
        ghosts.add_ghost("root", node("c"), 4);

        assert_eq!(ghosts.ghost_count_before("root", 1), 0);
        assert_eq!(ghosts.adjust_index("root", 2), 2);
        assert_eq!(ghosts.adjust_index("root", 3), 3);
    }

    #[test]
    fn prune_finished_is_empty_while_ghosts_are_disabled() {
        let mut ghosts = GhostManager::new();
        ghosts.add_ghost("root", node("a"), 0);
        ghosts.add_ghost("root", node("b"), 1);

        ghosts.mark_finished("root", 0);
        let removed = ghosts.prune_finished();

        assert!(removed.is_empty());
        assert!(ghosts.ghosts_for("root").is_none());
    }

    #[test]
    fn clear_removes_all_ghosts() {
        let mut ghosts = GhostManager::new();
        ghosts.add_ghost("root", node("a"), 0);

        ghosts.clear();

        assert!(!ghosts.has_active());
        assert!(ghosts.ghosts_for("root").is_none());
    }
}
