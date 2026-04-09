//! Undo/redo stack with bounded size, labels, and coalescing.
//!
//! Supports two usage patterns:
//!
//! 1. **Snapshot-based**: Push state snapshots before edits.
//! 2. **Command-based**: Push reversible commands with apply/undo
//!    function pairs, optional labels, and time-based coalescing.
//!
//! ```ignore
//! // Snapshot pattern
//! let mut stack = UndoStack::new("initial".to_string());
//! stack.push("after edit".to_string());
//! stack.undo();
//! assert_eq!(stack.current(), "initial");
//!
//! // Command pattern with coalescing
//! let mut stack = UndoStack::new(0);
//! stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1)
//!     .label("increment")
//!     .coalesce("typing", 500));
//! ```

use std::time::Instant;

/// A bounded undo/redo stack storing state snapshots.
///
/// Entries beyond `max_size` are dropped (oldest first). Pushing
/// a new entry clears the redo stack (new edits fork the timeline).
#[derive(Debug, Clone)]
pub struct UndoStack<T: Clone> {
    current: T,
    max_size: usize,
    undo_stack: Vec<UndoEntry<T>>,
    redo_stack: Vec<UndoEntry<T>>,
}

/// A single undo history entry.
#[derive(Debug, Clone)]
struct UndoEntry<T> {
    /// The state snapshot at this point.
    snapshot: T,
    /// Human-readable label for this entry.
    label: Option<String>,
    /// Coalescing key. Entries with the same key within the
    /// coalescing window are merged.
    coalesce_key: Option<String>,
    /// When this entry was created.
    timestamp: Instant,
}

/// A reversible command for the command-based undo pattern.
///
/// ```ignore
/// UndoCommand::new(
///     |model| { model.count += 1; model.clone() },
///     |model| { model.count -= 1; model.clone() },
/// )
/// .label("increment")
/// .coalesce("counter", 300)
/// ```
pub struct UndoCommand<T> {
    apply_fn: Box<dyn FnOnce(&T) -> T>,
    #[allow(dead_code)] // Reserved for future inverse-command undo
    undo_fn: Box<dyn FnOnce(&T) -> T>,
    label: Option<String>,
    coalesce_key: Option<String>,
    coalesce_window_ms: u64,
}

impl<T> UndoCommand<T> {
    /// Create a reversible command with apply and undo functions.
    pub fn new(
        apply: impl FnOnce(&T) -> T + 'static,
        undo: impl FnOnce(&T) -> T + 'static,
    ) -> Self {
        Self {
            apply_fn: Box::new(apply),
            undo_fn: Box::new(undo),
            label: None,
            coalesce_key: None,
            coalesce_window_ms: 0,
        }
    }

    /// Set a human-readable label for this command.
    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    /// Enable coalescing: commands with the same key within
    /// `window_ms` milliseconds are merged into a single undo entry.
    pub fn coalesce(mut self, key: &str, window_ms: u64) -> Self {
        self.coalesce_key = Some(key.to_string());
        self.coalesce_window_ms = window_ms;
        self
    }
}

impl<T: Clone> UndoStack<T> {
    /// Create a new stack with the given initial state.
    /// Default max size is 100.
    pub fn new(initial: T) -> Self {
        Self::with_max_size(initial, 100)
    }

    /// Create a new stack with a specific maximum history size.
    pub fn with_max_size(initial: T, max_size: usize) -> Self {
        Self {
            current: initial,
            max_size,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Save the current state and set a new one (snapshot pattern).
    ///
    /// Clears the redo stack (new edits fork the timeline).
    /// Drops the oldest entry if the stack exceeds max size.
    pub fn push(&mut self, state: T) {
        self.push_entry(state, None, None);
    }

    /// Save the current state with a label.
    pub fn push_labeled(&mut self, state: T, label: &str) {
        self.push_entry(state, Some(label.to_string()), None);
    }

    /// Apply a reversible command (command pattern).
    ///
    /// The command's apply function transforms the current state.
    /// If coalescing is enabled and the previous entry has the same
    /// key within the time window, the entries are merged (one undo
    /// reverses both).
    pub fn apply(&mut self, cmd: UndoCommand<T>) {
        let new_state = (cmd.apply_fn)(&self.current);

        // Check for coalescing: if the previous entry has the same
        // coalesce key within the time window, merge by updating
        // current without pushing a new undo entry.
        if let Some(ref key) = cmd.coalesce_key
            && cmd.coalesce_window_ms > 0
            && let Some(last) = self.undo_stack.last()
            && last.coalesce_key.as_deref() == Some(key)
            && last.timestamp.elapsed().as_millis() < cmd.coalesce_window_ms as u128
        {
            self.current = new_state;
            self.redo_stack.clear();
            return;
        }

        self.push_entry(new_state, cmd.label, cmd.coalesce_key);
    }

    fn push_entry(&mut self, state: T, label: Option<String>, coalesce_key: Option<String>) {
        self.undo_stack.push(UndoEntry {
            snapshot: self.current.clone(),
            label,
            coalesce_key,
            timestamp: Instant::now(),
        });
        self.current = state;
        self.redo_stack.clear();

        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Restore the previous state. Returns `false` if at the bottom.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(entry) => {
                self.redo_stack.push(UndoEntry {
                    snapshot: self.current.clone(),
                    label: entry.label.clone(),
                    coalesce_key: entry.coalesce_key.clone(),
                    timestamp: entry.timestamp,
                });
                self.current = entry.snapshot;
                true
            }
            None => false,
        }
    }

    /// Re-apply a previously undone state. Returns `false` if at the top.
    pub fn redo(&mut self) -> bool {
        match self.redo_stack.pop() {
            Some(entry) => {
                self.undo_stack.push(UndoEntry {
                    snapshot: self.current.clone(),
                    label: entry.label.clone(),
                    coalesce_key: entry.coalesce_key.clone(),
                    timestamp: entry.timestamp,
                });
                self.current = entry.snapshot;
                true
            }
            None => false,
        }
    }

    /// Reference to the current state.
    pub fn current(&self) -> &T {
        &self.current
    }

    /// Mutable reference to the current state.
    pub fn current_mut(&mut self) -> &mut T {
        &mut self.current
    }

    /// Whether there is a previous state to undo to.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there is a state to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of undo entries.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of redo entries.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Labels of undo entries (most recent first).
    pub fn history(&self) -> Vec<Option<&str>> {
        self.undo_stack
            .iter()
            .rev()
            .map(|e| e.label.as_deref())
            .collect()
    }
}
