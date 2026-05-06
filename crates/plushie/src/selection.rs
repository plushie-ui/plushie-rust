//! Single/multi/range selection state for lists and tables.

use std::collections::HashSet;

/// How selection behaves when items are clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionMode {
    /// Only one item selected at a time. `select` replaces.
    Single,
    /// Multiple items. `select_extend` adds, `toggle` flips.
    Multi,
    /// Contiguous range. `range_select` selects from anchor to target.
    Range,
}

/// Tracks which items are selected in a list or table.
///
/// Items are identified by string IDs. The `order` vector defines
/// the display order, used by `range_select` to determine which
/// items fall between the anchor and target.
///
/// When the underlying list of items changes (rows added, removed,
/// reordered) call [`Selection::set_order`] with the new ID list.
/// That replaces the order vector, drops any selected IDs that no
/// longer exist, and clears the anchor if it is no longer present
/// so a later [`Selection::range_select`] doesn't span from a stale,
/// removed id.
#[derive(Debug, Clone)]
pub struct Selection {
    mode: SelectionMode,
    selected: HashSet<String>,
    anchor: Option<String>,
    order: Vec<String>,
}

impl Selection {
    /// Create a new selection with the given mode and item order.
    pub fn new(mode: SelectionMode, order: Vec<String>) -> Self {
        Self {
            mode,
            selected: HashSet::new(),
            anchor: None,
            order,
        }
    }

    /// Select a single item, replacing any previous selection.
    /// Sets the anchor for subsequent range operations.
    pub fn select(&mut self, id: &str) {
        self.selected.clear();
        self.selected.insert(id.to_string());
        self.anchor = Some(id.to_string());
    }

    /// Add an item to the selection without clearing others.
    /// Only meaningful in `Multi` mode.
    pub fn select_extend(&mut self, id: &str) {
        self.selected.insert(id.to_string());
        self.anchor = Some(id.to_string());
    }

    /// Toggle an item's selection state.
    ///
    /// In `Single` mode: replaces the selection if toggling on,
    /// clears entirely if toggling off.
    /// In `Multi`/`Range` mode: adds if absent, removes if present.
    pub fn toggle(&mut self, id: &str) {
        if self.selected.contains(id) {
            self.selected.remove(id);
            if self.mode == SelectionMode::Single {
                self.anchor = None;
            }
        } else if self.mode == SelectionMode::Single {
            self.select(id);
        } else {
            self.selected.insert(id.to_string());
            self.anchor = Some(id.to_string());
        }
    }

    /// Remove a specific item from the selection. If the deselected
    /// item was the range anchor, the anchor is cleared so a later
    /// `range_select` doesn't span from a stale, no-longer-selected
    /// id.
    pub fn deselect(&mut self, id: &str) {
        self.selected.remove(id);
        if self.anchor.as_deref() == Some(id) {
            self.anchor = None;
        }
    }

    /// Select all items in the order list.
    pub fn select_all(&mut self) {
        self.selected = self.order.iter().cloned().collect();
    }

    /// Replace the underlying order with a new list of IDs.
    ///
    /// Drops any currently-selected IDs that aren't in `new_order`
    /// and clears the anchor if it's no longer present. Use this when
    /// the data behind the list changes (rows added, removed, or
    /// reordered) so subsequent `range_select` calls don't span from
    /// a stale anchor and selection state stays consistent with the
    /// visible items.
    pub fn set_order(&mut self, new_order: Vec<String>) {
        let valid: HashSet<&str> = new_order.iter().map(String::as_str).collect();
        self.selected.retain(|id| valid.contains(id.as_str()));
        if let Some(anchor) = &self.anchor
            && !valid.contains(anchor.as_str())
        {
            self.anchor = None;
        }
        self.order = new_order;
    }

    /// Remove all items from the selection.
    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// Select all items from the current anchor to `id` (inclusive),
    /// based on the order vector. Replaces the current selection
    /// with the range. If no anchor is set, behaves like `select`.
    pub fn range_select(&mut self, id: &str) {
        let anchor = match &self.anchor {
            Some(a) => a.clone(),
            None => {
                self.select(id);
                return;
            }
        };

        let anchor_pos = self.order.iter().position(|x| x == &anchor);
        let target_pos = self.order.iter().position(|x| x == id);

        match (anchor_pos, target_pos) {
            (Some(a), Some(t)) => {
                let (start, end) = if a <= t { (a, t) } else { (t, a) };
                self.selected.clear();
                for item in &self.order[start..=end] {
                    self.selected.insert(item.clone());
                }
            }
            _ => {
                self.select(id);
            }
        }
    }

    /// The current selection mode.
    pub fn mode(&self) -> &SelectionMode {
        &self.mode
    }

    /// The set of currently selected item IDs.
    pub fn selected(&self) -> &HashSet<String> {
        &self.selected
    }

    /// The selected item ID when exactly one item is selected.
    ///
    /// Returns `None` for empty and multi-item selections so callers do
    /// not accidentally choose an arbitrary item.
    pub fn selected_value(&self) -> Option<&str> {
        if self.selected.len() == 1 {
            self.selected.iter().next().map(String::as_str)
        } else {
            None
        }
    }

    /// Whether a specific item is selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// How many items are selected.
    pub fn count(&self) -> usize {
        self.selected.len()
    }
}
