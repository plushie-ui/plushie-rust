//! Path-based state container with revision tracking.
//!
//! Provides a nested key-value store with a monotonically increasing
//! revision counter for change detection. Supports transactions that
//! can be committed or rolled back atomically.
//!
//! ```
//! use plushie::util::State;
//! use serde_json::json;
//!
//! let mut state = State::new(json!({"count": 0, "items": []}));
//! state.put(&["count"], json!(5));
//! assert_eq!(state.get(&["count"]), Some(&json!(5)));
//! assert_eq!(state.revision(), 1);
//!
//! // Transactions
//! state.begin_transaction().unwrap();
//! state.put(&["count"], json!(10));
//! state.rollback_transaction().unwrap();
//! assert_eq!(state.get(&["count"]), Some(&json!(5)));
//! ```

use serde_json::Value;

/// Path-based state container with revision tracking and transactions.
#[derive(Debug, Clone)]
pub struct State {
    data: Value,
    revision: u64,
    transaction: Option<Transaction>,
}

#[derive(Debug, Clone)]
struct Transaction {
    data: Value,
    revision: u64,
}

impl State {
    /// Create a new state from initial data. The initial revision is 0.
    pub fn new(data: Value) -> Self {
        Self {
            data,
            revision: 0,
            transaction: None,
        }
    }

    /// Read a value at the given path.
    /// Empty path returns the entire data.
    pub fn get(&self, path: &[&str]) -> Option<&Value> {
        let mut current = &self.data;
        for key in path {
            current = current.get(*key)?;
        }
        Some(current)
    }

    /// Set a value at the given path, incrementing the revision.
    /// Intermediate objects are created if they don't exist.
    pub fn put(&mut self, path: &[&str], value: Value) {
        if path.is_empty() {
            self.data = value;
        } else {
            set_nested(&mut self.data, path, value);
        }
        self.revision += 1;
    }

    /// Update a value at the given path using a function.
    /// If the path doesn't exist, the function receives `Value::Null`.
    pub fn update(&mut self, path: &[&str], f: impl FnOnce(&Value) -> Value) {
        let current = self.get(path).cloned().unwrap_or(Value::Null);
        let new_value = f(&current);
        self.put(path, new_value);
    }

    /// Delete a value at the given path, incrementing the revision.
    /// No-op on the data if the path doesn't exist, but the revision
    /// still increments for consistency with the Elixir implementation.
    pub fn delete(&mut self, path: &[&str]) {
        if path.is_empty() {
            self.data = Value::Object(serde_json::Map::new());
        } else if let Some((key, parent_path)) = path.split_last() {
            let parent = if parent_path.is_empty() {
                Some(&mut self.data)
            } else {
                navigate_mut(&mut self.data, parent_path)
            };
            if let Some(obj) = parent.and_then(|v| v.as_object_mut()) {
                obj.remove(*key);
            }
        }
        self.revision += 1;
    }

    /// Current revision number.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Begin a transaction. Returns Err if one is already active.
    pub fn begin_transaction(&mut self) -> Result<(), &'static str> {
        if self.transaction.is_some() {
            return Err("transaction already active");
        }
        self.transaction = Some(Transaction {
            data: self.data.clone(),
            revision: self.revision,
        });
        Ok(())
    }

    /// Commit the active transaction. Revision becomes
    /// pre-transaction revision + 1.
    pub fn commit_transaction(&mut self) -> Result<(), &'static str> {
        let tx = self.transaction.take().ok_or("no active transaction")?;
        self.revision = tx.revision + 1;
        Ok(())
    }

    /// Roll back the active transaction, restoring data and revision.
    pub fn rollback_transaction(&mut self) -> Result<(), &'static str> {
        let tx = self.transaction.take().ok_or("no active transaction")?;
        self.data = tx.data;
        self.revision = tx.revision;
        Ok(())
    }

    /// Whether a transaction is currently active.
    pub fn in_transaction(&self) -> bool {
        self.transaction.is_some()
    }
}

/// Navigate to a mutable reference at the given path.
fn navigate_mut<'a>(data: &'a mut Value, path: &[&str]) -> Option<&'a mut Value> {
    let mut current = data;
    for key in path {
        current = current.get_mut(*key)?;
    }
    Some(current)
}

/// Set a nested value, creating intermediate objects as needed.
fn set_nested(data: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() {
        return;
    }
    if path.len() == 1 {
        if let Some(obj) = data.as_object_mut() {
            obj.insert(path[0].to_string(), value);
        }
        return;
    }
    // Ensure intermediate object exists.
    let key = path[0];
    let obj = match data.as_object_mut() {
        Some(obj) => obj,
        None => return,
    };
    if !obj.contains_key(key) {
        obj.insert(key.to_string(), Value::Object(serde_json::Map::new()));
    }
    let child = obj.get_mut(key).unwrap();
    set_nested(child, &path[1..], value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- get/put basics --

    #[test]
    fn new_state_starts_at_revision_zero() {
        let state = State::new(json!({"x": 1}));
        assert_eq!(state.revision(), 0);
    }

    #[test]
    fn get_empty_path_returns_entire_data() {
        let state = State::new(json!({"a": 1, "b": 2}));
        assert_eq!(state.get(&[]), Some(&json!({"a": 1, "b": 2})));
    }

    #[test]
    fn get_single_key() {
        let state = State::new(json!({"count": 42}));
        assert_eq!(state.get(&["count"]), Some(&json!(42)));
    }

    #[test]
    fn get_nested_key() {
        let state = State::new(json!({"user": {"name": "Lister"}}));
        assert_eq!(state.get(&["user", "name"]), Some(&json!("Lister")));
    }

    #[test]
    fn get_missing_key_returns_none() {
        let state = State::new(json!({"a": 1}));
        assert_eq!(state.get(&["nope"]), None);
    }

    #[test]
    fn get_missing_nested_key_returns_none() {
        let state = State::new(json!({"a": 1}));
        assert_eq!(state.get(&["a", "b"]), None);
    }

    #[test]
    fn put_sets_value_and_increments_revision() {
        let mut state = State::new(json!({"count": 0}));
        state.put(&["count"], json!(5));
        assert_eq!(state.get(&["count"]), Some(&json!(5)));
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn put_empty_path_replaces_entire_data() {
        let mut state = State::new(json!({"old": true}));
        state.put(&[], json!({"new": true}));
        assert_eq!(state.get(&[]), Some(&json!({"new": true})));
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn put_creates_intermediate_objects() {
        let mut state = State::new(json!({}));
        state.put(&["a", "b", "c"], json!(99));
        assert_eq!(state.get(&["a", "b", "c"]), Some(&json!(99)));
    }

    #[test]
    fn put_overwrites_existing_nested_value() {
        let mut state = State::new(json!({"user": {"name": "Cat"}}));
        state.put(&["user", "name"], json!("Rimmer"));
        assert_eq!(state.get(&["user", "name"]), Some(&json!("Rimmer")));
    }

    #[test]
    fn multiple_puts_increment_revision_each_time() {
        let mut state = State::new(json!({}));
        state.put(&["a"], json!(1));
        state.put(&["b"], json!(2));
        state.put(&["c"], json!(3));
        assert_eq!(state.revision(), 3);
    }

    // -- update --

    #[test]
    fn update_transforms_existing_value() {
        let mut state = State::new(json!({"count": 3}));
        state.update(&["count"], |v| json!(v.as_i64().unwrap() + 10));
        assert_eq!(state.get(&["count"]), Some(&json!(13)));
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn update_missing_key_receives_null() {
        let mut state = State::new(json!({}));
        state.update(&["missing"], |v| {
            assert!(v.is_null());
            json!("created")
        });
        assert_eq!(state.get(&["missing"]), Some(&json!("created")));
    }

    // -- delete --

    #[test]
    fn delete_removes_key_and_increments_revision() {
        let mut state = State::new(json!({"a": 1, "b": 2}));
        state.delete(&["a"]);
        assert_eq!(state.get(&["a"]), None);
        assert_eq!(state.get(&["b"]), Some(&json!(2)));
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn delete_nested_key() {
        let mut state = State::new(json!({"user": {"name": "Holly", "iq": 6000}}));
        state.delete(&["user", "iq"]);
        assert_eq!(state.get(&["user", "iq"]), None);
        assert_eq!(state.get(&["user", "name"]), Some(&json!("Holly")));
    }

    #[test]
    fn delete_missing_key_still_increments_revision() {
        let mut state = State::new(json!({"a": 1}));
        state.delete(&["nonexistent"]);
        assert_eq!(state.revision(), 1);
    }

    #[test]
    fn delete_empty_path_resets_to_empty_object() {
        let mut state = State::new(json!({"a": 1, "b": 2}));
        state.delete(&[]);
        assert_eq!(state.get(&[]), Some(&json!({})));
        assert_eq!(state.revision(), 1);
    }

    // -- transactions --

    #[test]
    fn begin_transaction_captures_snapshot() {
        let mut state = State::new(json!({"x": 1}));
        state.put(&["x"], json!(2));
        assert!(state.begin_transaction().is_ok());
        assert!(state.in_transaction());
    }

    #[test]
    fn begin_transaction_twice_returns_error() {
        let mut state = State::new(json!({}));
        assert!(state.begin_transaction().is_ok());
        assert!(state.begin_transaction().is_err());
    }

    #[test]
    fn rollback_restores_data_and_revision() {
        let mut state = State::new(json!({"count": 0}));
        state.put(&["count"], json!(5));
        assert_eq!(state.revision(), 1);

        state.begin_transaction().unwrap();
        state.put(&["count"], json!(99));
        state.put(&["extra"], json!("oops"));
        assert_eq!(state.revision(), 3);

        state.rollback_transaction().unwrap();
        assert_eq!(state.get(&["count"]), Some(&json!(5)));
        assert_eq!(state.get(&["extra"]), None);
        assert_eq!(state.revision(), 1);
        assert!(!state.in_transaction());
    }

    #[test]
    fn commit_sets_revision_to_pre_transaction_plus_one() {
        let mut state = State::new(json!({"count": 0}));
        state.put(&["count"], json!(1)); // rev 1
        state.put(&["count"], json!(2)); // rev 2

        state.begin_transaction().unwrap();
        state.put(&["count"], json!(10)); // rev 3
        state.put(&["count"], json!(20)); // rev 4
        state.put(&["count"], json!(30)); // rev 5

        state.commit_transaction().unwrap();
        assert_eq!(state.get(&["count"]), Some(&json!(30)));
        // Pre-transaction revision was 2, so commit sets it to 3.
        assert_eq!(state.revision(), 3);
        assert!(!state.in_transaction());
    }

    #[test]
    fn commit_without_transaction_returns_error() {
        let mut state = State::new(json!({}));
        assert!(state.commit_transaction().is_err());
    }

    #[test]
    fn rollback_without_transaction_returns_error() {
        let mut state = State::new(json!({}));
        assert!(state.rollback_transaction().is_err());
    }

    #[test]
    fn in_transaction_reflects_state() {
        let mut state = State::new(json!({}));
        assert!(!state.in_transaction());
        state.begin_transaction().unwrap();
        assert!(state.in_transaction());
        state.commit_transaction().unwrap();
        assert!(!state.in_transaction());
    }

    #[test]
    fn transaction_after_rollback_works() {
        let mut state = State::new(json!({"v": 1}));
        state.begin_transaction().unwrap();
        state.put(&["v"], json!(2));
        state.rollback_transaction().unwrap();

        // Should be able to start a new transaction.
        state.begin_transaction().unwrap();
        state.put(&["v"], json!(3));
        state.commit_transaction().unwrap();
        assert_eq!(state.get(&["v"]), Some(&json!(3)));
    }

    #[test]
    fn clone_produces_independent_copy() {
        let mut state = State::new(json!({"x": 1}));
        state.put(&["x"], json!(2));
        let mut clone = state.clone();
        clone.put(&["x"], json!(99));
        assert_eq!(state.get(&["x"]), Some(&json!(2)));
        assert_eq!(clone.get(&["x"]), Some(&json!(99)));
    }
}
