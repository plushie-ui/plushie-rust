//! Memoization cache for `__memo__` subtrees.
//!
//! Owned by the app runner (direct mode, test session) alongside
//! [`WidgetStateStore`][crate::widget::WidgetStateStore]. Keyed by
//! the memo node's scoped ID so two memo call sites that happen to
//! hash the same deps don't collide.
//!
//! The cache stores the fully-normalized subtree produced by the
//! prior render so a hit can drop the cached subtree straight in
//! without re-walking it. See [`NormalizeTransform`] in
//! `runtime/normalize.rs` for how hits are detected and installed.
//!
//! Stale entries are pruned at the start of each render via
//! [`MemoCache::begin_cycle`] and [`MemoCache::mark_live`], mirroring
//! the live-IDs pattern used by shared state.

use std::collections::{HashMap, HashSet};

use plushie_core::protocol::TreeNode;

/// Per-render memoization cache.
///
/// Entry is `(deps_hash, cached_children)` so a hit only needs to
/// compare the stored hash against the incoming memo's deps prop.
#[derive(Default)]
pub(crate) struct MemoCache {
    entries: HashMap<String, (u64, Vec<TreeNode>)>,
    /// IDs touched during the current render cycle. Anything not in
    /// this set at the end of `finish_cycle` is evicted.
    live_this_cycle: HashSet<String>,
}

impl MemoCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the live-set at the start of a new render cycle.
    pub fn begin_cycle(&mut self) {
        self.live_this_cycle.clear();
    }

    /// Mark a memo node as live this cycle.
    pub fn mark_live(&mut self, scoped_id: &str) {
        self.live_this_cycle.insert(scoped_id.to_string());
    }

    /// Evict entries that weren't touched this cycle.
    pub fn finish_cycle(&mut self) {
        self.entries
            .retain(|id, _| self.live_this_cycle.contains(id));
    }

    /// Retrieve a cached subtree if the stored deps hash matches.
    pub fn get(&self, scoped_id: &str, deps_hash: u64) -> Option<&[TreeNode]> {
        let (stored_hash, cached) = self.entries.get(scoped_id)?;
        if *stored_hash == deps_hash {
            Some(cached.as_slice())
        } else {
            None
        }
    }

    /// Store the normalized children of a memo subtree for reuse next
    /// render.
    pub fn insert(&mut self, scoped_id: String, deps_hash: u64, children: Vec<TreeNode>) {
        self.entries.insert(scoped_id, (deps_hash, children));
    }
}
