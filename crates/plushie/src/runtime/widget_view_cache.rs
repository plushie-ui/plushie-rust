//! Widget view cache for composite widgets that opt in via
//! [`Widget::cache_key`][crate::widget::Widget::cache_key].
//!
//! Mirrors the widget view cache pattern already present in the
//! Elixir, Gleam, TypeScript, and Ruby SDKs: when a widget's
//! `cache_key(props, state)` returns the same hash between renders,
//! the previously-expanded view tree is reused and `view()` is not
//! re-invoked. Widgets that don't override `cache_key` (the default
//! returns `None`) bypass the cache entirely, keeping the default
//! path identical to the pre-cache behaviour.
//!
//! Owned by the app runner (direct mode, test session) alongside
//! [`WidgetStateStore`][crate::widget::WidgetStateStore] and
//! [`MemoCache`][super::memo_cache::MemoCache]. Keyed by the widget's
//! scoped ID so two widget placements that happen to hash to the
//! same cache key don't collide.
//!
//! Stale entries are pruned at the end of each render via
//! [`WidgetViewCache::finish_cycle`]: [`WidgetViewCache::begin_cycle`]
//! resets the live-set at the start, [`WidgetViewCache::mark_live`]
//! records every widget touched during the render, and `finish_cycle`
//! evicts anything not in that set. Mirrors the live-IDs pattern used
//! by `MemoCache`.

use std::collections::{HashMap, HashSet};

use plushie_core::protocol::TreeNode;

/// Per-render widget-view cache.
///
/// Entry is `(cache_key_hash, expanded_view)`. A hit only needs to
/// compare the stored hash against the incoming widget's cache-key
/// hash; on match, the cached expanded view is dropped in and
/// `W::view()` is skipped.
#[derive(Default)]
pub(crate) struct WidgetViewCache {
    entries: HashMap<String, (u64, TreeNode)>,
    /// IDs touched during the current render cycle. Anything not in
    /// this set at the end of `finish_cycle` is evicted so stale
    /// widgets (unmounted since the previous render) don't leak
    /// memory.
    live_this_cycle: HashSet<String>,
}

impl WidgetViewCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset the live-set at the start of a new render cycle.
    pub fn begin_cycle(&mut self) {
        self.live_this_cycle.clear();
    }

    /// Mark a widget ID as live this cycle.
    pub fn mark_live(&mut self, widget_id: &str) {
        self.live_this_cycle.insert(widget_id.to_string());
    }

    /// Evict entries that weren't touched this cycle.
    pub fn finish_cycle(&mut self) {
        self.entries
            .retain(|id, _| self.live_this_cycle.contains(id));
    }

    /// Retrieve a cached expanded view if the stored cache-key hash
    /// matches.
    pub fn get(&self, widget_id: &str, key_hash: u64) -> Option<&TreeNode> {
        let (stored_hash, cached) = self.entries.get(widget_id)?;
        if *stored_hash == key_hash {
            Some(cached)
        } else {
            None
        }
    }

    /// Store an expanded view for reuse next render.
    pub fn insert(&mut self, widget_id: String, key_hash: u64, view: TreeNode) {
        self.entries.insert(widget_id, (key_hash, view));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plushie_core::protocol::{PropMap, Props};

    fn node(id: &str) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "container".to_string(),
            props: Props::from(PropMap::new()),
            children: vec![],
        }
    }

    #[test]
    fn get_requires_matching_cache_key_hash() {
        let mut cache = WidgetViewCache::new();
        cache.insert("widget".to_string(), 7, node("expanded"));

        assert!(cache.get("widget", 8).is_none());
        assert_eq!(cache.get("widget", 7).unwrap().id, "expanded");
    }

    #[test]
    fn finish_cycle_evicts_entries_not_marked_live() {
        let mut cache = WidgetViewCache::new();
        cache.insert("keep".to_string(), 1, node("a"));
        cache.insert("drop".to_string(), 1, node("b"));

        cache.begin_cycle();
        cache.mark_live("keep");
        cache.finish_cycle();

        assert!(cache.get("keep", 1).is_some());
        assert!(cache.get("drop", 1).is_none());
    }

    #[test]
    fn removed_then_readded_id_uses_new_entry() {
        let mut cache = WidgetViewCache::new();
        cache.insert("widget".to_string(), 1, node("old"));

        cache.begin_cycle();
        cache.finish_cycle();
        assert!(cache.get("widget", 1).is_none());

        cache.begin_cycle();
        cache.mark_live("widget");
        cache.insert("widget".to_string(), 1, node("new"));
        cache.finish_cycle();

        assert_eq!(cache.get("widget", 1).unwrap().id, "new");
    }
}
