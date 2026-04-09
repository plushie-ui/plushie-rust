//! Undo/redo stack with bounded size.
//!
//! Stores snapshots of state. Push a snapshot before making changes,
//! then undo/redo navigates the history. When the stack exceeds
//! `max_size`, the oldest entry is dropped.

/// A bounded undo/redo stack storing state snapshots.
///
/// # Usage
///
/// ```ignore
/// let mut stack = UndoStack::new("initial".to_string());
/// stack.push("after edit".to_string());
/// assert_eq!(stack.current(), "after edit");
/// stack.undo();
/// assert_eq!(stack.current(), "initial");
/// stack.redo();
/// assert_eq!(stack.current(), "after edit");
/// ```
#[derive(Debug, Clone)]
pub struct UndoStack<T: Clone> {
    current: T,
    max_size: usize,
    undo_stack: Vec<T>,
    redo_stack: Vec<T>,
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

    /// Save the current state to the undo stack and set a new state.
    /// Clears the redo stack (new edits fork the timeline).
    /// Drops the oldest entry if the stack exceeds max size.
    pub fn push(&mut self, state: T) {
        self.undo_stack.push(self.current.clone());
        self.current = state;
        self.redo_stack.clear();

        if self.undo_stack.len() > self.max_size {
            self.undo_stack.remove(0);
        }
    }

    /// Restore the previous state. Returns `false` if at the bottom.
    pub fn undo(&mut self) -> bool {
        match self.undo_stack.pop() {
            Some(prev) => {
                self.redo_stack.push(self.current.clone());
                self.current = prev;
                true
            }
            None => false,
        }
    }

    /// Re-apply a previously undone state. Returns `false` if at the top.
    pub fn redo(&mut self) -> bool {
        match self.redo_stack.pop() {
            Some(next) => {
                self.undo_stack.push(self.current.clone());
                self.current = next;
                true
            }
            None => false,
        }
    }

    /// Reference to the current state.
    pub fn current(&self) -> &T {
        &self.current
    }

    /// Mutable reference to the current state, for in-place edits.
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
}
