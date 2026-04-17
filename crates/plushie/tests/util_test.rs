//! Tests for utility data structures: Selection, UndoStack, Route, Query.

use std::collections::HashMap;

use plushie::util::{Query, Route, Selection, SelectionMode, UndoCommand, UndoStack};

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

fn item_ids() -> Vec<String> {
    vec!["a", "b", "c", "d", "e"]
        .into_iter()
        .map(String::from)
        .collect()
}

#[test]
fn single_select_replaces_previous() {
    let mut sel = Selection::new(SelectionMode::Single, item_ids());
    sel.select("a");
    sel.select("c");
    assert!(!sel.is_selected("a"));
    assert!(sel.is_selected("c"));
    assert_eq!(sel.count(), 1);
}

#[test]
fn multi_select_extends() {
    let mut sel = Selection::new(SelectionMode::Multi, item_ids());
    sel.select_extend("a");
    sel.select_extend("c");
    assert!(sel.is_selected("a"));
    assert!(sel.is_selected("c"));
    assert_eq!(sel.count(), 2);
}

#[test]
fn toggle_adds_and_removes_in_multi_mode() {
    let mut sel = Selection::new(SelectionMode::Multi, item_ids());
    sel.toggle("b");
    assert!(sel.is_selected("b"));
    sel.toggle("b");
    assert!(!sel.is_selected("b"));
}

#[test]
fn toggle_in_single_mode_replaces_selection() {
    let mut sel = Selection::new(SelectionMode::Single, item_ids());
    sel.toggle("a");
    assert!(sel.is_selected("a"));
    assert_eq!(sel.count(), 1);

    // toggling a different item replaces, not adds
    sel.toggle("b");
    assert!(!sel.is_selected("a"));
    assert!(sel.is_selected("b"));
    assert_eq!(sel.count(), 1);

    // toggling the same item off clears everything
    sel.toggle("b");
    assert!(!sel.is_selected("b"));
    assert_eq!(sel.count(), 0);
}

#[test]
fn select_all_selects_everything() {
    let mut sel = Selection::new(SelectionMode::Multi, item_ids());
    sel.select_all();
    assert_eq!(sel.count(), 5);
    for id in &["a", "b", "c", "d", "e"] {
        assert!(sel.is_selected(id));
    }
}

#[test]
fn clear_empties_selection() {
    let mut sel = Selection::new(SelectionMode::Multi, item_ids());
    sel.select_extend("a");
    sel.select_extend("b");
    sel.clear();
    assert_eq!(sel.count(), 0);
    assert!(!sel.is_selected("a"));
}

#[test]
fn range_select_between_anchor_and_target() {
    let mut sel = Selection::new(SelectionMode::Range, item_ids());
    sel.select("b");
    sel.range_select("d");
    assert!(!sel.is_selected("a"));
    assert!(sel.is_selected("b"));
    assert!(sel.is_selected("c"));
    assert!(sel.is_selected("d"));
    assert!(!sel.is_selected("e"));
    assert_eq!(sel.count(), 3);
}

#[test]
fn range_select_reverse_direction() {
    let mut sel = Selection::new(SelectionMode::Range, item_ids());
    sel.select("d");
    sel.range_select("b");
    assert!(sel.is_selected("b"));
    assert!(sel.is_selected("c"));
    assert!(sel.is_selected("d"));
    assert_eq!(sel.count(), 3);
}

#[test]
fn range_select_without_anchor_behaves_as_select() {
    let mut sel = Selection::new(SelectionMode::Range, item_ids());
    sel.range_select("c");
    assert!(sel.is_selected("c"));
    assert_eq!(sel.count(), 1);
}

#[test]
fn is_selected_returns_correct_state() {
    let mut sel = Selection::new(SelectionMode::Single, item_ids());
    assert!(!sel.is_selected("a"));
    sel.select("a");
    assert!(sel.is_selected("a"));
    assert!(!sel.is_selected("b"));
}

#[test]
fn deselect_removes_specific_item() {
    let mut sel = Selection::new(SelectionMode::Multi, item_ids());
    sel.select_extend("a");
    sel.select_extend("b");
    sel.deselect("a");
    assert!(!sel.is_selected("a"));
    assert!(sel.is_selected("b"));
}

#[test]
fn toggle_on_in_multi_mode_sets_anchor() {
    let mut sel = Selection::new(SelectionMode::Multi, item_ids());
    sel.toggle("b");
    assert!(sel.is_selected("b"));
    // Anchor should be set so range_select works from here
    sel.range_select("d");
    assert!(sel.is_selected("b"));
    assert!(sel.is_selected("c"));
    assert!(sel.is_selected("d"));
    assert_eq!(sel.count(), 3);
}

// ---------------------------------------------------------------------------
// UndoStack
// ---------------------------------------------------------------------------

#[test]
fn new_stack_has_initial_state() {
    let stack = UndoStack::new(42);
    assert_eq!(*stack.current(), 42);
    assert!(!stack.can_undo());
    assert!(!stack.can_redo());
}

#[test]
fn push_saves_and_updates_current() {
    let mut stack = UndoStack::new("first".to_string());
    stack.push("second".to_string());
    assert_eq!(stack.current(), "second");
    assert!(stack.can_undo());
}

#[test]
fn undo_restores_previous() {
    let mut stack = UndoStack::new(1);
    stack.push(2);
    stack.push(3);
    assert!(stack.undo());
    assert_eq!(*stack.current(), 2);
    assert!(stack.undo());
    assert_eq!(*stack.current(), 1);
}

#[test]
fn redo_restores_undone() {
    let mut stack = UndoStack::new(1);
    stack.push(2);
    stack.undo();
    assert!(stack.redo());
    assert_eq!(*stack.current(), 2);
}

#[test]
fn undo_at_bottom_returns_false() {
    let mut stack = UndoStack::new(1);
    assert!(!stack.undo());
    assert_eq!(*stack.current(), 1);
}

#[test]
fn redo_at_top_returns_false() {
    let mut stack = UndoStack::new(1);
    assert!(!stack.redo());
}

#[test]
fn push_clears_redo_stack() {
    let mut stack = UndoStack::new(1);
    stack.push(2);
    stack.push(3);
    stack.undo();
    assert!(stack.can_redo());
    stack.push(4);
    assert!(!stack.can_redo());
    assert_eq!(*stack.current(), 4);
}

#[test]
fn max_size_drops_oldest() {
    let mut stack = UndoStack::with_max_size(0, 3);
    stack.push(1);
    stack.push(2);
    stack.push(3);
    stack.push(4);
    // Undo stack should have at most 3 entries
    assert!(stack.undo()); // 3
    assert!(stack.undo()); // 2
    assert!(stack.undo()); // 1
    assert!(!stack.undo()); // 0 was dropped
    assert_eq!(*stack.current(), 1);
}

#[test]
fn current_mut_allows_in_place_edit() {
    let mut stack = UndoStack::new(vec![1, 2, 3]);
    stack.current_mut().push(4);
    assert_eq!(stack.current(), &vec![1, 2, 3, 4]);
}

#[test]
fn apply_calls_apply_fn() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 10, |n| n - 10));
    assert_eq!(*stack.current(), 10);
    assert!(stack.can_undo());
}

#[test]
fn undo_calls_undo_fn() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 10, |n| n - 10));
    stack.apply(UndoCommand::new(|n| n + 5, |n| n - 5));
    assert_eq!(*stack.current(), 15);

    assert!(stack.undo());
    assert_eq!(*stack.current(), 10);
    assert!(stack.undo());
    assert_eq!(*stack.current(), 0);
}

#[test]
fn redo_calls_apply_fn() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 10, |n| n - 10));
    stack.undo();
    assert_eq!(*stack.current(), 0);

    assert!(stack.redo());
    assert_eq!(*stack.current(), 10);
}

#[test]
fn apply_with_label_appears_in_history() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).label("increment"));
    stack.apply(UndoCommand::new(|n| n * 2, |n| n / 2).label("double"));
    let history = stack.history();
    assert_eq!(history, vec![Some("double"), Some("increment")]);
}

#[test]
fn coalesce_merges_entries() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).coalesce("typing", 500));
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).coalesce("typing", 500));
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).coalesce("typing", 500));
    assert_eq!(*stack.current(), 3);
    assert_eq!(stack.undo_count(), 1);

    // One undo reverses all three coalesced changes
    assert!(stack.undo());
    assert_eq!(*stack.current(), 0);
}

#[test]
fn coalesce_redo_reapplies_all() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).coalesce("typing", 500));
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).coalesce("typing", 500));
    assert_eq!(*stack.current(), 2);

    stack.undo();
    assert_eq!(*stack.current(), 0);

    stack.redo();
    assert_eq!(*stack.current(), 2);
}

#[test]
fn apply_clears_redo_stack() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1));
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1));
    stack.undo();
    assert!(stack.can_redo());

    stack.apply(UndoCommand::new(|n| n + 100, |n| n - 100));
    assert!(!stack.can_redo());
    assert_eq!(*stack.current(), 101);
}

#[test]
fn undo_count_and_redo_count() {
    let mut stack = UndoStack::new(0);
    assert_eq!(stack.undo_count(), 0);
    assert_eq!(stack.redo_count(), 0);

    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1));
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1));
    assert_eq!(stack.undo_count(), 2);

    stack.undo();
    assert_eq!(stack.undo_count(), 1);
    assert_eq!(stack.redo_count(), 1);
}

#[test]
fn coalesce_preserves_original_label() {
    let mut stack = UndoStack::new(0);
    stack.apply(
        UndoCommand::new(|n| n + 1, |n| n - 1)
            .label("first edit")
            .coalesce("typing", 500),
    );
    stack.apply(
        UndoCommand::new(|n| n + 1, |n| n - 1)
            .label("second edit")
            .coalesce("typing", 500),
    );
    let history = stack.history();
    assert_eq!(history, vec![Some("first edit")]);
}

#[test]
fn coalesce_different_keys_do_not_merge() {
    let mut stack = UndoStack::new(0);
    stack.apply(UndoCommand::new(|n| n + 1, |n| n - 1).coalesce("a", 500));
    stack.apply(UndoCommand::new(|n| n + 10, |n| n - 10).coalesce("b", 500));
    assert_eq!(*stack.current(), 11);
    assert_eq!(stack.undo_count(), 2);
}

#[test]
fn push_labeled_appears_in_history() {
    let mut stack = UndoStack::new("a".to_string());
    stack.push_labeled("b".to_string(), "change to b");
    let history = stack.history();
    assert_eq!(history, vec![Some("change to b")]);
}

// ---------------------------------------------------------------------------
// Route
// ---------------------------------------------------------------------------

#[test]
fn new_route_starts_at_path() {
    let route = Route::new("/home");
    assert_eq!(route.current(), "/home");
    assert_eq!(route.depth(), 1);
    assert!(!route.can_go_back());
}

#[test]
fn new_with_params_sets_initial_params() {
    let mut params = std::collections::HashMap::new();
    params.insert("tab".to_string(), serde_json::json!("general"));
    let route = Route::new_with_params("/settings", params);
    assert_eq!(route.current(), "/settings");
    assert_eq!(
        route.params().get("tab").and_then(|v| v.as_str()),
        Some("general")
    );
    assert_eq!(route.depth(), 1);
}

#[test]
fn push_adds_to_stack() {
    let mut route = Route::new("/home");
    route.push("/settings");
    assert_eq!(route.current(), "/settings");
    assert_eq!(route.depth(), 2);
    assert!(route.can_go_back());
}

#[test]
fn pop_returns_to_previous() {
    let mut route = Route::new("/home");
    route.push("/settings");
    assert!(route.pop());
    assert_eq!(route.current(), "/home");
}

#[test]
fn pop_at_root_returns_false() {
    let mut route = Route::new("/home");
    assert!(!route.pop());
    assert_eq!(route.current(), "/home");
}

#[test]
fn replace_top_changes_current() {
    let mut route = Route::new("/home");
    route.push("/settings");
    route.replace_top("/profile");
    assert_eq!(route.current(), "/profile");
    assert_eq!(route.depth(), 2);
}

#[test]
fn can_go_back_reflects_depth() {
    let mut route = Route::new("/");
    assert!(!route.can_go_back());
    route.push("/a");
    assert!(route.can_go_back());
    route.push("/b");
    assert!(route.can_go_back());
    route.pop();
    assert!(route.can_go_back());
    route.pop();
    assert!(!route.can_go_back());
}

#[test]
fn push_with_params_stores_params() {
    let mut route = Route::new("/home");
    let mut params = HashMap::new();
    params.insert("id".to_string(), serde_json::json!(42));
    route.push_with_params("/item", params);
    assert_eq!(route.params()["id"], serde_json::json!(42));
}

#[test]
fn replace_top_clears_params() {
    let mut route = Route::new("/home");
    let mut params = HashMap::new();
    params.insert("key".to_string(), serde_json::json!("val"));
    route.push_with_params("/item", params);
    route.replace_top("/other");
    assert!(route.params().is_empty());
}

#[test]
fn replace_top_with_params_preserves_new_params() {
    let mut route = Route::new("/home");
    route.push("/item");
    let mut params = HashMap::new();
    params.insert("id".to_string(), serde_json::json!(42));
    route.replace_top_with_params("/detail", params);
    assert_eq!(route.current(), "/detail");
    assert_eq!(route.params()["id"], 42);
}

#[test]
fn history_returns_paths_most_recent_first() {
    let mut route = Route::new("/home");
    route.push("/about");
    route.push("/contact");
    let history = route.history();
    assert_eq!(history, vec!["/contact", "/about", "/home"]);
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

#[test]
fn query_without_filter_returns_all() {
    let items = vec![1, 2, 3, 4, 5];
    let result = Query::new(&items).page_size(100).run();
    assert_eq!(result.entries, vec![1, 2, 3, 4, 5]);
    assert_eq!(result.total, 5);
}

#[test]
fn filter_reduces_results() {
    let items = vec![1, 2, 3, 4, 5, 6];
    let result = Query::new(&items)
        .filter(|x| *x % 2 == 0)
        .page_size(100)
        .run();
    assert_eq!(result.entries, vec![2, 4, 6]);
    assert_eq!(result.total, 3);
}

#[test]
fn sort_orders_results() {
    let items = vec![3, 1, 4, 1, 5];
    let result = Query::new(&items)
        .sort(|a, b| a.cmp(b))
        .page_size(100)
        .run();
    assert_eq!(result.entries, vec![1, 1, 3, 4, 5]);
}

#[test]
fn pagination_slices_results() {
    let items: Vec<i32> = (1..=10).collect();
    let result = Query::new(&items).page(2).page_size(3).run();
    assert_eq!(result.entries, vec![4, 5, 6]);
    assert_eq!(result.page, 2);
    assert_eq!(result.page_size, 3);
}

#[test]
fn page_one_is_the_first_page() {
    let items: Vec<i32> = (1..=10).collect();
    let result = Query::new(&items).page(1).page_size(3).run();
    assert_eq!(result.entries, vec![1, 2, 3]);
    assert_eq!(result.page, 1);
}

#[test]
fn page_zero_clamps_to_page_one() {
    let items: Vec<i32> = (1..=10).collect();
    let result = Query::new(&items).page(0).page_size(3).run();
    assert_eq!(result.entries, vec![1, 2, 3]);
    assert_eq!(result.page, 1);
}

#[test]
fn total_reflects_pre_pagination_count() {
    let items: Vec<i32> = (1..=20).collect();
    let result = Query::new(&items)
        .filter(|x| *x > 10)
        .page(1)
        .page_size(5)
        .run();
    assert_eq!(result.entries, vec![11, 12, 13, 14, 15]);
    assert_eq!(result.total, 10);
}

#[test]
fn page_beyond_end_returns_empty() {
    let items = vec![1, 2, 3];
    let result = Query::new(&items).page(5).page_size(10).run();
    assert!(result.entries.is_empty());
    assert_eq!(result.total, 3);
}

#[test]
fn filter_and_sort_compose() {
    let items = vec![5, 3, 8, 1, 9, 2];
    let result = Query::new(&items)
        .filter(|x| *x > 3)
        .sort(|a, b| b.cmp(a))
        .page_size(100)
        .run();
    assert_eq!(result.entries, vec![9, 8, 5]);
}

#[test]
fn default_page_size_is_25() {
    let items: Vec<i32> = (1..=50).collect();
    let result = Query::new(&items).run();
    assert_eq!(result.entries.len(), 25);
    assert_eq!(result.page_size, 25);
}

#[test]
fn search_filters_by_substring() {
    let items = vec!["Alice Smith", "Bob Jones", "Alice Jones"];
    let result = Query::new(&items)
        .search("alice", |item| vec![item])
        .page_size(100)
        .run();
    assert_eq!(result.entries, vec!["Alice Smith", "Alice Jones"]);
}

#[test]
fn search_is_case_insensitive() {
    let items = vec!["HELLO", "world", "Hello World"];
    let result = Query::new(&items)
        .search("hello", |item| vec![item])
        .page_size(100)
        .run();
    assert_eq!(result.entries, vec!["HELLO", "Hello World"]);
}

#[test]
fn sort_by_multiple_fields() {
    use plushie::util::SortDir;
    let items = vec![(2, "b"), (1, "a"), (2, "a"), (1, "b")];
    let result = Query::new(&items)
        .sort_by(vec![
            (
                SortDir::Asc,
                Box::new(|a: &(i32, &str), b: &(i32, &str)| a.0.cmp(&b.0)),
            ),
            (
                SortDir::Desc,
                Box::new(|a: &(i32, &str), b: &(i32, &str)| a.1.cmp(b.1)),
            ),
        ])
        .page_size(100)
        .run();
    // Primary: asc by first element. Secondary: desc by second.
    assert_eq!(result.entries, vec![(1, "b"), (1, "a"), (2, "b"), (2, "a")]);
}

#[test]
fn group_partitions_results() {
    let items = vec!["apple", "avocado", "banana", "blueberry"];
    let result = Query::new(&items)
        .group(|item| item.chars().next().unwrap().to_string())
        .page_size(100)
        .run();
    let groups = result.groups.unwrap();
    assert_eq!(groups["a"].len(), 2);
    assert_eq!(groups["b"].len(), 2);
}
