//! Exit animation ghost management.
//!
//! When a widget with an `exit` prop is removed, it becomes a "ghost"
//! that stays in its parent's layout flow during the exit animation.
//! The renderer adjusts patch indices to account for ghosts.

use crate::protocol::TreeNode;
use std::collections::HashMap;

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
pub struct GhostManager {
    /// Parent widget ID -> list of ghost children.
    ghosts: HashMap<String, Vec<GhostNode>>,
}

impl Default for GhostManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GhostManager {
    pub fn new() -> Self {
        Self {
            ghosts: HashMap::new(),
        }
    }

    /// Returns true if any ghosts exist.
    pub fn has_active(&self) -> bool {
        !self.ghosts.is_empty()
    }

    /// Adds a ghost for an exiting child.
    pub fn add_ghost(&mut self, parent_id: &str, node: TreeNode, index: usize) {
        self.ghosts
            .entry(parent_id.to_string())
            .or_default()
            .push(GhostNode {
                node,
                insert_index: index,
                finished: false,
            });
    }

    /// Returns the ghost nodes for a parent, if any.
    pub fn ghosts_for(&self, parent_id: &str) -> Option<&[GhostNode]> {
        self.ghosts.get(parent_id).map(|v| v.as_slice())
    }

    /// Returns the number of ghosts before a given SDK index for a parent.
    ///
    /// Used to adjust patch indices: the SDK doesn't know about ghosts,
    /// so we need to offset its indices by the ghost count before each position.
    pub fn ghost_count_before(&self, parent_id: &str, sdk_index: usize) -> usize {
        self.ghosts
            .get(parent_id)
            .map(|ghosts| {
                ghosts
                    .iter()
                    .filter(|g| g.insert_index <= sdk_index)
                    .count()
            })
            .unwrap_or(0)
    }

    /// Adjusts an SDK-provided child index to account for ghost nodes.
    pub fn adjust_index(&self, parent_id: &str, sdk_index: usize) -> usize {
        sdk_index + self.ghost_count_before(parent_id, sdk_index)
    }

    /// Marks a ghost as finished and removes it if all ghosts for the parent are done.
    pub fn mark_finished(&mut self, parent_id: &str, ghost_index: usize) {
        if let Some(ghosts) = self.ghosts.get_mut(parent_id)
            && let Some(ghost) = ghosts.get_mut(ghost_index)
        {
            ghost.finished = true;
        }
    }

    /// Removes all finished ghosts and returns a list of removed ghost node IDs.
    pub fn prune_finished(&mut self) -> Vec<String> {
        let mut removed_ids = Vec::new();
        self.ghosts.retain(|_parent_id, ghosts| {
            ghosts.retain(|g| {
                if g.finished {
                    removed_ids.push(g.node.id.clone());
                    false
                } else {
                    true
                }
            });
            !ghosts.is_empty()
        });
        removed_ids
    }

    /// Clears all ghosts (used on snapshot/reset).
    pub fn clear(&mut self) {
        self.ghosts.clear();
    }
}
