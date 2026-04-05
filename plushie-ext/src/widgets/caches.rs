//! Widget cache management.
//!
//! Several iced widgets (`text_editor`, `markdown`, `combo_box`, `canvas`,
//! `pane_grid`) require mutable state that must persist across renders, but
//! iced's `view()` only has `&self`. The solution: [`ensure_caches`] runs
//! during `apply()` (mutable context) to populate [`WidgetCaches`], and
//! `render()` in `view()` reads them immutably. No `RefCell` needed.
//!
//! Caches are keyed by node ID and automatically pruned when nodes leave
//! the tree. Content-addressed hashing detects prop changes without
//! clobbering user edits (e.g. a text_editor's cursor position).

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use iced::widget::canvas as iced_canvas;
use iced::widget::{combo_box, markdown, pane_grid, text_editor};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::protocol::{OutgoingEvent, TreeNode};

/// Maximum recursion depth for tree walks (render, ensure_caches, prepare).
/// Prevents stack overflow from pathologically nested trees. Normal UI trees
/// rarely exceed 20-30 levels; 256 is generous.
pub(crate) const MAX_TREE_DEPTH: usize = 256;

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
/// structure to extension authors. The renderer binary accesses
/// specific entries through the accessor methods below.
///
/// The `R` parameter selects the renderer backend. Some caches
/// (text_editor Content, canvas Cache) are parameterized on the
/// renderer type because iced's widget state depends on it.
pub struct WidgetCaches<R: PlushieRenderer = iced::Renderer> {
    /// text_editor Content state (preserves cursor, undo history).
    pub(crate) editor_contents: HashMap<String, text_editor::Content<R>>,
    /// Hash of last-synced "content" prop per text_editor. Detects
    /// host-side prop changes without clobbering user edits.
    pub(crate) editor_content_hashes: HashMap<String, u64>,
    /// Parsed markdown items with content hash for invalidation.
    pub(crate) markdown_items: HashMap<String, (u64, Vec<markdown::Item>)>,
    /// combo_box filter/selection state.
    pub(crate) combo_states: HashMap<String, combo_box::State<String>>,
    /// combo_box option lists for change detection.
    pub(crate) combo_options: HashMap<String, Vec<String>>,
    /// pane_grid layout state.
    pub(crate) pane_grid_states: HashMap<String, pane_grid::State<String>>,
    /// Per-canvas, per-layer geometry caches. Inner key is layer name,
    /// u64 is content hash for invalidation.
    pub(crate) canvas_caches: HashMap<String, HashMap<String, (u64, iced_canvas::Cache<R>)>>,
    /// Per-canvas interactive element data parsed from group JSON. Used for
    /// hit testing in `Program::update()` without re-parsing every frame.
    pub(crate) canvas_interactions: HashMap<String, Vec<super::canvas::InteractiveElement>>,
    /// Pending programmatic focus for a canvas element, set by the
    /// `focus_element` widget_op. Read and drained by `render_canvas`,
    /// which passes the value to `CanvasProgram`. The Program consumes
    /// it at the top of `update()` to set `focused_id`.
    pub(crate) canvas_pending_focus: HashMap<String, String>,
    /// Per-qr_code caches (content hash, canvas Cache).
    pub(crate) qr_code_caches: HashMap<String, (u64, iced_canvas::Cache<R>)>,
    /// Resolved themes for Themer widget nodes.
    pub(crate) themer_themes: HashMap<String, iced::Theme>,
    /// Parsed style overrides with content hash for invalidation.
    /// Populated in `ensure_caches_walk` for any node with a `style`
    /// object prop; read during render to avoid re-parsing every frame.
    pub(crate) style_overrides: HashMap<String, (u64, super::helpers::StyleOverrides)>,
    /// Extension-owned caches. Public so extension authors can
    /// access their own cached state during render/prepare/cleanup.
    pub extension: crate::extensions::ExtensionCaches,
    /// Interpolated prop values from active renderer-side animations.
    /// Keyed by widget ID -> prop name -> current value.
    /// Populated by the TransitionManager on each frame tick.
    /// Widget render functions check this before falling back to tree props.
    pub interpolated_props: HashMap<String, serde_json::Map<String, serde_json::Value>>,
}

impl<R: PlushieRenderer> WidgetCaches<R> {
    pub fn new() -> Self {
        Self {
            editor_contents: HashMap::new(),
            editor_content_hashes: HashMap::new(),
            markdown_items: HashMap::new(),
            combo_states: HashMap::new(),
            combo_options: HashMap::new(),
            pane_grid_states: HashMap::new(),
            canvas_caches: HashMap::new(),
            canvas_interactions: HashMap::new(),
            canvas_pending_focus: HashMap::new(),
            qr_code_caches: HashMap::new(),
            themer_themes: HashMap::new(),
            style_overrides: HashMap::new(),
            extension: crate::extensions::ExtensionCaches::new(),
            interpolated_props: HashMap::new(),
        }
    }

    /// Clear per-node widget caches without touching extension caches.
    ///
    /// Used by the Snapshot handler so that extension cleanup callbacks
    /// (via `ExtensionDispatcher::prepare_all`) can run before the
    /// extension cache entries are removed.
    pub fn clear_builtin(&mut self) {
        self.editor_contents.clear();
        self.editor_content_hashes.clear();
        self.markdown_items.clear();
        self.combo_states.clear();
        self.combo_options.clear();
        self.pane_grid_states.clear();
        self.canvas_caches.clear();
        self.canvas_interactions.clear();
        self.canvas_pending_focus.clear();
        self.qr_code_caches.clear();
        self.themer_themes.clear();
        self.style_overrides.clear();
        self.interpolated_props.clear();
    }

    /// Remove entries whose node IDs are no longer in the live set.
    fn prune_stale(&mut self, live_ids: &HashSet<String>) {
        self.editor_contents.retain(|id, _| live_ids.contains(id));
        self.editor_content_hashes
            .retain(|id, _| live_ids.contains(id));
        self.markdown_items.retain(|id, _| live_ids.contains(id));
        self.combo_states.retain(|id, _| live_ids.contains(id));
        self.combo_options.retain(|id, _| live_ids.contains(id));
        self.pane_grid_states.retain(|id, _| live_ids.contains(id));
        self.canvas_caches.retain(|id, _| live_ids.contains(id));
        self.canvas_interactions
            .retain(|id, _| live_ids.contains(id));
        self.canvas_pending_focus
            .retain(|id, _| live_ids.contains(id));
        self.qr_code_caches.retain(|id, _| live_ids.contains(id));
        self.themer_themes.retain(|id, _| live_ids.contains(id));
        self.style_overrides.retain(|id, _| live_ids.contains(id));
        self.interpolated_props
            .retain(|id, _| live_ids.contains(id));
    }
}

impl<R: PlushieRenderer> Default for WidgetCaches<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: PlushieRenderer> WidgetCaches<R> {
    pub fn clear(&mut self) {
        self.clear_builtin();
        self.extension.clear();
    }

    // -- Accessor methods for renderer crate --
    // Fields are pub(crate) to avoid leaking internal HashMap structure to
    // extension authors, but the renderer binary needs access to a few.

    /// Get a mutable reference to a text_editor Content by node ID.
    pub fn editor_content_mut(&mut self, id: &str) -> Option<&mut text_editor::Content<R>> {
        self.editor_contents.get_mut(id)
    }

    /// Get a mutable reference to a pane_grid State by node ID.
    pub fn pane_grid_state_mut(&mut self, id: &str) -> Option<&mut pane_grid::State<String>> {
        self.pane_grid_states.get_mut(id)
    }

    /// Get an immutable reference to a pane_grid State by node ID.
    pub fn pane_grid_state(&self, id: &str) -> Option<&pane_grid::State<String>> {
        self.pane_grid_states.get(id)
    }

    /// Set a pending programmatic focus for a canvas element.
    /// Called by the `focus_element` widget_op. The canvas Program
    /// consumes this on the next update cycle.
    pub fn set_canvas_pending_focus(&mut self, canvas_id: String, element_id: String) {
        self.canvas_pending_focus.insert(canvas_id, element_id);
    }

    /// Update the cached prop hash for a text_editor after the Content
    /// was mutated by a `TextEditorAction`. This prevents
    /// `ensure_text_editor_cache` from resetting the Content (and losing
    /// cursor position) when the host's prop lags one frame behind.
    pub fn update_editor_content_hash(&mut self, id: &str, text: &str) {
        self.editor_content_hashes
            .insert(id.to_string(), hash_str(text));
    }
}

// ---------------------------------------------------------------------------
// Cache pre-population
// ---------------------------------------------------------------------------

/// Walk the tree and ensure that every `text_editor`, `markdown`,
/// `combo_box`, `pane_grid`, `canvas`, `qr_code`, and `themer` node has
/// an entry in the corresponding cache. This must be called *before*
/// `render` so that `render` can work with shared (`&`) references to
/// the caches.
///
/// After populating caches, prunes stale entries for nodes no longer in the
/// tree across all cache types.
///
/// The full-tree walk is intentional: it collects all live node IDs for
/// the pruning step. The expensive work (parsing styles, hashing canvas
/// layers, etc.) is guarded by per-node content hashes, so unchanged
/// nodes are O(1). A dirty-flag optimization would only skip those hash
/// lookups, which are already cheap relative to the tree walk itself.
pub fn ensure_caches<R: PlushieRenderer>(node: &TreeNode, caches: &mut WidgetCaches<R>) -> Vec<OutgoingEvent> {
    let mut live_ids = HashSet::new();
    let mut diagnostics = Vec::new();
    ensure_caches_walk(node, caches, &mut live_ids, &mut diagnostics, 0);
    caches.prune_stale(&live_ids);
    diagnostics
}

/// Inner recursive walk: populate caches and collect live node IDs.
fn ensure_caches_walk<R: PlushieRenderer>(
    node: &TreeNode,
    caches: &mut WidgetCaches<R>,
    live_ids: &mut HashSet<String>,
    diagnostics: &mut Vec<OutgoingEvent>,
    depth: usize,
) {
    if depth > MAX_TREE_DEPTH {
        log::warn!(
            "[id={}] ensure_caches depth exceeds {MAX_TREE_DEPTH}, skipping subtree",
            node.id
        );
        return;
    }
    live_ids.insert(node.id.clone());

    match node.type_name.as_str() {
        "text_editor" => super::input::ensure_text_editor_cache(node, caches),
        "markdown" => super::display::ensure_markdown_cache(node, caches),
        "combo_box" => super::input::ensure_combo_box_cache(node, caches),
        "pane_grid" => super::layout::ensure_pane_grid_cache(node, caches),
        "canvas" => diagnostics.extend(super::canvas::ensure_canvas_cache(node, caches)),
        "themer" => super::interactive::ensure_themer_cache(node, caches),
        "qr_code" => super::display::ensure_qr_code_cache(node, caches),
        _ => {}
    }

    // Cache parsed StyleOverrides for any node with a `style` object prop.
    // Uses the same content-hash pattern as canvas/markdown: only re-parse
    // when the JSON value changes.
    ensure_style_overrides_cache(node, caches);

    for child in &node.children {
        ensure_caches_walk(child, caches, live_ids, diagnostics, depth + 1);
    }
}

// ---------------------------------------------------------------------------
// Cache helpers (used by ensure_* functions in widget modules)
// ---------------------------------------------------------------------------

/// Reconstruct a shape JSON Value from a tree node.
///
/// Shape nodes have `{id, type, props, children}`. This converts back to
/// the `{type: type, id: id, ...props, children: [...]}` format that the
/// canvas rendering and hit-testing code expects.
fn tree_node_to_shape_value(node: &TreeNode) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String(node.type_name.clone()));

    // Copy all props into the shape map
    if let Some(obj) = node.props.as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), v.clone());
        }
    }

    // Recursively convert children (for group shapes)
    if !node.children.is_empty() {
        let child_shapes: Vec<Value> = node.children.iter()
            .map(tree_node_to_shape_value)
            .collect();
        map.insert("children".to_string(), Value::Array(child_shapes));
    }

    Value::Object(map)
}

/// Extract canvas layer data from a node's children. Returns owned Values
/// suitable for hashing and rendering.
///
/// Canvas nodes carry shapes as tree children:
/// - `__layer__` children with a `name` prop and shape children (layered)
/// - Direct shape children without a layer wrapper (flat, treated as "default" layer)
///
/// Returns a BTreeMap so layer order is deterministic (alphabetical by name).
pub(crate) fn canvas_layers_from_node(
    node: &TreeNode,
) -> std::collections::BTreeMap<String, Value> {
    let mut map = std::collections::BTreeMap::new();

    // Check if children are __layer__ containers
    let has_layers = node.children.iter().any(|c| c.type_name == "__layer__");

    if has_layers {
        for child in &node.children {
            if child.type_name == "__layer__" {
                let layer_name = child.props.as_object()
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                let shapes: Vec<Value> = child.children.iter()
                    .map(tree_node_to_shape_value)
                    .collect();
                map.insert(layer_name, Value::Array(shapes));
            }
        }
    } else if !node.children.is_empty() {
        // Direct shape children (flat canvas)
        let shapes: Vec<Value> = node.children.iter()
            .map(tree_node_to_shape_value)
            .collect();
        map.insert("default".to_string(), Value::Array(shapes));
    }

    map
}

/// Cache parsed `StyleOverrides` for a node's `style` prop. Only
/// re-parses when the content hash of the JSON value changes.
fn ensure_style_overrides_cache<R: PlushieRenderer>(node: &TreeNode, caches: &mut WidgetCaches<R>) {
    let style_val = match node.props.get("style").and_then(|v| v.as_object()) {
        Some(obj) => obj,
        None => return,
    };

    let mut hasher = DefaultHasher::new();
    hash_json_value(&serde_json::Value::Object(style_val.clone()), &mut hasher);
    let hash = hasher.finish();

    if let Some((cached_hash, _)) = caches.style_overrides.get(&node.id)
        && *cached_hash == hash
    {
        return;
    }

    let overrides = super::helpers::parse_style_overrides(style_val);
    caches
        .style_overrides
        .insert(node.id.clone(), (hash, overrides));
}

/// Look up cached `StyleOverrides` for a node. Returns `None` if the
/// node has no `style` prop or if `ensure_caches` hasn't run yet.
/// Used by widget render functions to avoid re-parsing the style JSON
/// on every frame.
pub(crate) fn cached_style_overrides<'a, R: PlushieRenderer>(
    caches: &'a WidgetCaches<R>,
    node_id: &str,
) -> Option<&'a super::helpers::StyleOverrides> {
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
        // hash (vs. recursing further) but is safe -- worst case is an
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

    // -- WidgetCaches --

    #[test]
    fn widget_caches_new_is_empty() {
        let c: WidgetCaches = WidgetCaches::new();
        assert!(c.editor_contents.is_empty());
        assert!(c.markdown_items.is_empty());
        assert!(c.combo_states.is_empty());
        assert!(c.combo_options.is_empty());
        assert!(c.pane_grid_states.is_empty());
    }

    #[test]
    fn widget_caches_clear_empties_maps() {
        let mut c: WidgetCaches = WidgetCaches::new();
        c.combo_options.insert("x".into(), vec!["a".into()]);
        c.clear();
        assert!(c.combo_options.is_empty());
    }

    // -- clear_builtin vs clear --

    #[test]
    fn clear_builtin_preserves_extension_caches() {
        let mut caches: WidgetCaches = WidgetCaches::new();

        // Add a built-in cache entry and an extension cache entry.
        caches
            .editor_contents
            .insert("ed1".to_string(), iced::widget::text_editor::Content::new());
        caches.extension.insert("ext", "key", 42u32);

        caches.clear_builtin();

        // Built-in caches should be empty.
        assert!(caches.editor_contents.is_empty());
        // Extension caches should survive.
        assert_eq!(caches.extension.get::<u32>("ext", "key"), Some(&42));
    }

    #[test]
    fn clear_wipes_both_builtin_and_extension() {
        let mut caches: WidgetCaches = WidgetCaches::new();

        caches
            .editor_contents
            .insert("ed1".to_string(), iced::widget::text_editor::Content::new());
        caches.extension.insert("ext", "key", 42u32);

        caches.clear();

        assert!(caches.editor_contents.is_empty());
        assert!(!caches.extension.contains("ext", "key"));
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
