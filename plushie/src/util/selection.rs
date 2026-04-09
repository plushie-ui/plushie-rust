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

    /// Toggle an item: add if absent, remove if present.
    pub fn toggle(&mut self, id: &str) {
        if self.selected.contains(id) {
            self.selected.remove(id);
        } else {
            self.selected.insert(id.to_string());
        }
    }

    /// Remove a specific item from the selection.
    pub fn deselect(&mut self, id: &str) {
        self.selected.remove(id);
    }

    /// Select all items in the order list.
    pub fn select_all(&mut self) {
        self.selected = self.order.iter().cloned().collect();
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

    /// Whether a specific item is selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.contains(id)
    }

    /// How many items are selected.
    pub fn count(&self) -> usize {
        self.selected.len()
    }
}
