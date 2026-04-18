//! Undo/redo stack with bounded size, labels, and coalescing.
//!
//! Uses **function-based** undo: each command provides an `apply`
//! function to move forward and an `undo` function to reverse the
//! change. Undo and redo invoke these functions on the current state
//! rather than restoring snapshots.
//!
//! Two usage patterns:
//!
//! 1. **Command-based** (primary): push reversible commands with
//!    apply/undo function pairs, optional labels, and time-based
//!    coalescing.
//! 2. **Snapshot convenience**: `push(state)` captures the current
//!    and new states as closures (sugar over the command pattern).
//!
//! ```ignore
//! // Command pattern
//! let mut stack = UndoStack::new(0);
//! stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1)
//!     .label("increment")
//!     .coalesce("typing", 500));
//! stack.undo();
//! assert_eq!(*stack.current(), 0);
//!
//! // Snapshot convenience
//! let mut stack = UndoStack::new("initial".to_string());
//! stack.push("after edit".to_string());
//! stack.undo();
//! assert_eq!(stack.current(), "initial");
//! ```

use std::fmt;
use std::time::Instant;

/// A bounded undo/redo stack using function-based reversal.
///
/// Each entry stores apply and undo functions. Undo calls the stored
/// undo function on the current state; redo re-applies the stored
/// apply function. This matches the Elixir implementation.
///
/// Entries beyond `max_size` are dropped (oldest first). Pushing
/// a new entry clears the redo stack (new edits fork the timeline).
pub struct UndoStack<T: Clone + Send + 'static> {
    current: T,
    max_size: usize,
    undo_stack: Vec<UndoEntry<T>>,
    redo_stack: Vec<UndoEntry<T>>,
}

/// A single undo history entry (internal).
struct UndoEntry<T> {
    /// Function to re-apply this change (for redo).
    apply_fn: Box<dyn Fn(&T) -> T + Send>,
    /// Function to reverse this change (for undo).
    undo_fn: Box<dyn Fn(&T) -> T + Send>,
    /// Human-readable label for this entry.
    label: Option<String>,
    /// Coalescing key. Entries with the same key within the
    /// coalescing window are merged by composing their functions.
    coalesce_key: Option<String>,
    /// When this entry was created (or last coalesced).
    timestamp: Instant,
}

/// A reversible command for the undo stack.
///
/// Both functions must be callable multiple times (redo re-invokes
/// `apply`, repeated undo/redo cycles call both repeatedly).
///
/// ```ignore
/// UndoCommand::new(
///     |model| { let mut m = model.clone(); m.count += 1; m },
///     |model| { let mut m = model.clone(); m.count -= 1; m },
/// )
/// .label("increment")
/// .coalesce("counter", 300)
/// ```
pub struct UndoCommand<T> {
    apply_fn: Box<dyn Fn(&T) -> T + Send>,
    undo_fn: Box<dyn Fn(&T) -> T + Send>,
    label: Option<String>,
    coalesce_key: Option<String>,
    coalesce_window_ms: u64,
}

impl<T> UndoCommand<T> {
    /// Create a reversible command with apply and undo functions.
    ///
    /// `apply` transforms the current state forward. `undo` reverses
    /// it. Both must be pure functions of the state they receive.
    pub fn new(
        apply: impl Fn(&T) -> T + Send + 'static,
        undo: impl Fn(&T) -> T + Send + 'static,
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
    /// `window_ms` milliseconds are merged into a single undo entry
    /// by composing their apply/undo functions.
    pub fn coalesce(mut self, key: &str, window_ms: u64) -> Self {
        self.coalesce_key = Some(key.to_string());
        self.coalesce_window_ms = window_ms;
        self
    }
}

impl<T: Clone + Send + 'static> UndoStack<T> {
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

    /// Snapshot convenience: save the current state and set a new one.
    ///
    /// Internally creates closures that capture the old and new
    /// states, so undo/redo restore exact values regardless of
    /// intermediate mutations.
    ///
    /// Clears the redo stack. Drops the oldest entry if the stack
    /// exceeds max size.
    pub fn push(&mut self, state: T) {
        let old = self.current.clone();
        let new = state;
        let new_for_apply = new.clone();
        let old_for_undo = old;
        self.push_entry(
            new,
            Box::new(move |_| new_for_apply.clone()),
            Box::new(move |_| old_for_undo.clone()),
            None,
            None,
        );
    }

    /// Snapshot convenience with a label.
    pub fn push_labeled(&mut self, state: T, label: &str) {
        let old = self.current.clone();
        let new = state;
        let new_for_apply = new.clone();
        let old_for_undo = old;
        self.push_entry(
            new,
            Box::new(move |_| new_for_apply.clone()),
            Box::new(move |_| old_for_undo.clone()),
            Some(label.to_string()),
            None,
        );
    }

    /// Apply a reversible command.
    ///
    /// The command's apply function is called immediately to compute
    /// the new state. Both functions are stored for future undo/redo.
    ///
    /// If coalescing is enabled and the previous entry has the same
    /// key within the time window, the functions are composed: a
    /// single undo reverses all coalesced changes.
    ///
    /// # Panics
    ///
    /// Never in practice: the internal `unwrap` in the coalesce path
    /// is guarded by the preceding `undo_stack.last().is_some()`
    /// check, so the stack is guaranteed non-empty at the pop site.
    pub fn apply(&mut self, cmd: UndoCommand<T>) {
        let new_state = (cmd.apply_fn)(&self.current);

        // Try coalescing with the top entry.
        if let Some(ref key) = cmd.coalesce_key
            && cmd.coalesce_window_ms > 0
            && let Some(top) = self.undo_stack.last()
            && top.coalesce_key.as_deref() == Some(key)
            && top.timestamp.elapsed().as_millis() < cmd.coalesce_window_ms as u128
        {
            let top = self.undo_stack.pop().unwrap();

            // Compose: composed_apply(m) = cmd.apply(top.apply(m))
            let top_apply = top.apply_fn;
            let cmd_apply = cmd.apply_fn;
            let composed_apply: Box<dyn Fn(&T) -> T + Send> = Box::new(move |model| {
                let intermediate = top_apply(model);
                cmd_apply(&intermediate)
            });

            // Compose: composed_undo(m) = top.undo(cmd.undo(m))
            let top_undo = top.undo_fn;
            let cmd_undo = cmd.undo_fn;
            let composed_undo: Box<dyn Fn(&T) -> T + Send> = Box::new(move |model| {
                let intermediate = cmd_undo(model);
                top_undo(&intermediate)
            });

            self.undo_stack.push(UndoEntry {
                apply_fn: composed_apply,
                undo_fn: composed_undo,
                label: top.label,
                coalesce_key: top.coalesce_key,
                timestamp: Instant::now(),
            });

            self.current = new_state;
            self.redo_stack.clear();
            return;
        }

        self.push_entry(
            new_state,
            cmd.apply_fn,
            cmd.undo_fn,
            cmd.label,
            cmd.coalesce_key,
        );
    }

    fn push_entry(
        &mut self,
        new_state: T,
        apply_fn: Box<dyn Fn(&T) -> T + Send>,
        undo_fn: Box<dyn Fn(&T) -> T + Send>,
        label: Option<String>,
        coalesce_key: Option<String>,
    ) {
        self.undo_stack.push(UndoEntry {
            apply_fn,
            undo_fn,
            label,
            coalesce_key,
            timestamp: Instant::now(),
        });
        self.current = new_state;
        self.redo_stack.clear();

        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Reverse the last change by calling its undo function.
    /// Returns `false` if at the bottom of the history.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(entry) => {
                let old_state = (entry.undo_fn)(&self.current);
                self.redo_stack.push(entry);
                self.current = old_state;
                true
            }
            None => false,
        }
    }

    /// Re-apply a previously undone change by calling its apply function.
    /// Returns `false` if at the top of the history.
    pub fn redo(&mut self) -> bool {
        match self.redo_stack.pop() {
            Some(entry) => {
                let new_state = (entry.apply_fn)(&self.current);
                self.undo_stack.push(entry);
                self.current = new_state;
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
    ///
    /// Mutations through this reference are invisible to the undo
    /// system. Use `apply` or `push` to track changes.
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

impl<T: Clone + Send + fmt::Debug + 'static> fmt::Debug for UndoStack<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UndoStack")
            .field("current", &self.current)
            .field("undo_count", &self.undo_stack.len())
            .field("redo_count", &self.redo_stack.len())
            .field("max_size", &self.max_size)
            .finish()
    }
}

impl<T> fmt::Debug for UndoCommand<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UndoCommand")
            .field("label", &self.label)
            .field("coalesce_key", &self.coalesce_key)
            .field("coalesce_window_ms", &self.coalesce_window_ms)
            .finish()
    }
}
