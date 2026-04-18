//! Shared renderer state and cross-cutting cache management.
//!
//! [`SharedState`] holds cross-cutting state shared by all widget types:
//! style overrides and animation interpolated props.
//! It also holds pane_grid layout state and canvas pending focus, which
//! are shared with widget_ops.rs for programmatic operations.
//!
//! Widget-specific state is owned by PlushieWidget factories
//! (see `widgets/builtins.rs`).

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::protocol::TreeNode;

/// Maximum recursion depth for tree walks (render, prepare_walk,
/// and the SDK's normalize pass). Prevents stack overflow from
/// pathologically nested trees (hostile hosts, runaway builder
/// code). Normal UI trees rarely exceed 20-30 levels; 256 is
/// generous.
///
/// All three passes cap at the same depth so defence-in-depth
/// holds: normalize truncates the subtree; prepare_walk stops
/// recursing but preserves the current node's live-key entry;
/// render returns a placeholder. Each pass emits a `tree_too_deep`
/// error-level diagnostic at the overflow boundary.
pub const MAX_TREE_DEPTH: usize = 256;

/// Maximum recursion depth for [`hash_json_value`]. JSON values within
/// props (e.g. canvas shapes) can be arbitrarily nested. Bounded to
/// match [`MAX_TREE_DEPTH`] for consistency.
const MAX_HASH_DEPTH: usize = 256;

// ---------------------------------------------------------------------------
// Widget caches
// ---------------------------------------------------------------------------

/// Per-widget mutable state that persists across renders.
///
/// Fields are `pub(crate)` to avoid leaking internal HashMap
/// structure to widget authors. The renderer binary accesses
/// specific entries through the accessor methods below.
pub struct SharedState {
    // -- Cross-cutting shared state (used by all widget types) --
    /// Parsed style overrides with content hash for invalidation.
    /// Populated in `prepare_walk` for any node with a `style`
    /// object prop; read during render to avoid re-parsing every frame.
    pub(crate) style_overrides: HashMap<String, (u64, crate::widget::helpers::StyleOverrides)>,
    /// Interpolated prop values from active renderer-side animations.
    /// Keyed by widget ID -> prop name -> current value.
    /// Populated by the TransitionManager on each frame tick.
    /// Widget render functions check this before falling back to tree props.
    pub interpolated_props: HashMap<String, serde_json::Map<String, serde_json::Value>>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            style_overrides: HashMap::new(),
            interpolated_props: HashMap::new(),
        }
    }

    /// Clear all shared state.
    pub fn clear(&mut self) {
        self.style_overrides.clear();
        self.interpolated_props.clear();
    }

    /// Remove stale cross-cutting entries. Called by registry.prepare_walk().
    pub fn prune_shared(&mut self, live_ids: &HashSet<String>) {
        self.style_overrides.retain(|id, _| live_ids.contains(id));
        self.interpolated_props
            .retain(|id, _| live_ids.contains(id));
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cache pre-population
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Cache helpers (used by ensure_* functions in widget modules)
// ---------------------------------------------------------------------------

/// Cache parsed `StyleOverrides` for a node's `style` prop. Only
/// re-parses when the content hash of the JSON value changes.
pub(crate) fn ensure_style_overrides_cache(node: &TreeNode, caches: &mut SharedState) {
    let style_val = match node.props.get("style").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return,
    };

    // Hash the map directly rather than wrapping it in a fresh
    // `Value::Object` and cloning the whole map. Mirrors the Object
    // branch of `hash_json_value` so the resulting hash is stable
    // across refactors of that helper.
    let mut hasher = DefaultHasher::new();
    hash_json_map(style_val, &mut hasher);
    let hash = hasher.finish();

    if let Some((cached_hash, _)) = caches.style_overrides.get(&node.id)
        && *cached_hash == hash
    {
        return;
    }

    let overrides = crate::widget::helpers::parse_style_overrides(style_val);
    caches
        .style_overrides
        .insert(node.id.clone(), (hash, overrides));
}

/// Hash a `serde_json::Map` with the same byte sequence the
/// `Value::Object` branch of [`hash_json_value`] would produce.
fn hash_json_map(map: &serde_json::Map<String, serde_json::Value>, h: &mut impl std::hash::Hasher) {
    5u8.hash(h);
    map.len().hash(h);
    for (k, v) in map {
        k.hash(h);
        hash_json_value(v, h);
    }
}

/// Look up cached `StyleOverrides` for a node. Returns `None` if the
/// node has no `style` prop or if prepare_walk hasn't run yet.
/// Used by widget render functions to avoid re-parsing the style JSON
/// on every frame.
pub(crate) fn cached_style_overrides<'a>(
    caches: &'a SharedState,
    node_id: &str,
) -> Option<&'a crate::widget::helpers::StyleOverrides> {
    caches.style_overrides.get(node_id).map(|(_, ov)| ov)
}

/// Hash a serde_json::Value recursively without allocating a serialized string.
/// Each variant is discriminated by a type tag byte to avoid collisions.
/// Recursion is bounded by [`MAX_HASH_DEPTH`].
///
/// NOTE: DefaultHasher output is not stable across Rust versions or builds.
/// These hashes must never be persisted or compared across process restarts.
pub(crate) fn hash_json_value(v: &serde_json::Value, h: &mut impl std::hash::Hasher) {
    hash_json_value_inner(v, h, 0);
}

fn hash_json_value_inner(v: &serde_json::Value, h: &mut impl std::hash::Hasher, depth: usize) {
    if depth > MAX_HASH_DEPTH {
        // Treat excessively nested values as opaque. This changes the
        // hash (vs. recursing further) but is safe; worst case is an
        // unnecessary cache invalidation.
        6u8.hash(h);
        return;
    }
    match v {
        serde_json::Value::Null => 0u8.hash(h),
        serde_json::Value::Bool(b) => {
            1u8.hash(h);
            b.hash(h);
        }
        serde_json::Value::Number(n) => {
            2u8.hash(h);
            if let Some(f) = n.as_f64() {
                f.to_bits().hash(h);
            } else if let Some(i) = n.as_i64() {
                i.hash(h);
            } else if let Some(u) = n.as_u64() {
                u.hash(h);
            }
        }
        serde_json::Value::String(s) => {
            3u8.hash(h);
            s.hash(h);
        }
        serde_json::Value::Array(arr) => {
            4u8.hash(h);
            arr.len().hash(h);
            for item in arr {
                hash_json_value_inner(item, h, depth + 1);
            }
        }
        serde_json::Value::Object(obj) => {
            5u8.hash(h);
            obj.len().hash(h);
            for (k, v) in obj {
                k.hash(h);
                hash_json_value_inner(v, h, depth + 1);
            }
        }
    }
}

/// Hash a string using DefaultHasher for same-process cache invalidation.
/// NOTE: DefaultHasher output is not stable across Rust versions or builds.
/// These hashes must never be persisted or compared across process restarts.
pub(crate) fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- SharedState --

    #[test]
    fn shared_state_new_is_empty() {
        let c: SharedState = SharedState::new();
        assert!(c.style_overrides.is_empty());
        assert!(c.interpolated_props.is_empty());
    }

    #[test]
    fn shared_state_clear_empties_maps() {
        let mut c: SharedState = SharedState::new();
        c.interpolated_props
            .insert("w1".into(), serde_json::Map::new());
        c.clear();
        assert!(c.interpolated_props.is_empty());
    }

    // -- hash_json_value --

    #[test]
    fn hash_json_value_same_input_same_hash() {
        use std::collections::hash_map::DefaultHasher;

        let val = serde_json::json!({"shapes": [{"type": "rect", "x": 0, "y": 0}]});
        let h1 = {
            let mut h = DefaultHasher::new();
            hash_json_value(&val, &mut h);
            h.finish()
        };
        let h2 = {
            let mut h = DefaultHasher::new();
            hash_json_value(&val, &mut h);
            h.finish()
        };
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_json_value_different_input_different_hash() {
        use std::collections::hash_map::DefaultHasher;

        let a = serde_json::json!({"type": "rect"});
        let b = serde_json::json!({"type": "circle"});
        let hash_a = {
            let mut h = DefaultHasher::new();
            hash_json_value(&a, &mut h);
            h.finish()
        };
        let hash_b = {
            let mut h = DefaultHasher::new();
            hash_json_value(&b, &mut h);
            h.finish()
        };
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn hash_json_value_type_discrimination() {
        use std::collections::hash_map::DefaultHasher;

        // null, false, and 0 should produce different hashes
        let vals = [
            serde_json::json!(null),
            serde_json::json!(false),
            serde_json::json!(0),
            serde_json::json!(""),
            serde_json::json!([]),
            serde_json::json!({}),
        ];
        let hashes: Vec<u64> = vals
            .iter()
            .map(|v| {
                let mut h = DefaultHasher::new();
                hash_json_value(v, &mut h);
                h.finish()
            })
            .collect();

        // All should be distinct
        for (i, h1) in hashes.iter().enumerate() {
            for (j, h2) in hashes.iter().enumerate() {
                if i != j {
                    assert_ne!(h1, h2, "type {i} and {j} should hash differently");
                }
            }
        }
    }
}
