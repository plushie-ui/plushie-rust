//! Property-based round-trip test for the tree diff.
//!
//! Generates pairs of arbitrary `TreeNode`s and checks that applying
//! `diff_tree(a, b)` to `a` produces `b`. The diff is the algorithm
//! the wire runner relies on, and the LIS path in particular has
//! enough edge cases that shaped examples can miss a failure mode.

use plushie::runtime_internals::{apply_patch, diff_tree};
use plushie_core::protocol::{Props, TreeNode};
use proptest::prelude::*;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn arb_leaf_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i32>().prop_map(|n| json!(n)),
        "[a-z0-9_]{0,8}".prop_map(Value::String),
    ]
}

fn arb_props() -> impl Strategy<Value = Props> {
    prop::collection::vec(("[a-z]{1,6}", arb_leaf_value()), 0..4).prop_map(|pairs| {
        let map: serde_json::Map<String, Value> = pairs.into_iter().collect();
        Props::from_json(Value::Object(map))
    })
}

fn arb_tree() -> impl Strategy<Value = TreeNode> {
    let leaf = (
        "[a-z][a-z0-9_]{0,4}",
        prop::sample::select(vec!["text", "button", "spacer"]),
        arb_props(),
    )
        .prop_map(|(id, type_name, props)| TreeNode {
            id,
            type_name: type_name.to_string(),
            props,
            children: vec![],
        });

    leaf.prop_recursive(
        3,  // depth
        16, // max nodes
        4,  // children per node
        |inner| {
            (
                "[a-z][a-z0-9_]{0,4}",
                prop::sample::select(vec!["column", "row", "container"]),
                arb_props(),
                prop::collection::vec(inner, 0..4),
            )
                .prop_map(|(id, type_name, props, children)| TreeNode {
                    id,
                    type_name: type_name.to_string(),
                    props,
                    children,
                })
        },
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn apply_diff_round_trips_on_arbitrary_tree_pairs(
        a in arb_tree(),
        b in arb_tree(),
    ) {
        let ops = diff_tree(&a, &b);
        let mut a_copy = a.clone();
        apply_patch(&mut a_copy, &ops);
        prop_assert_eq!(&a_copy, &b);
    }

    #[test]
    fn apply_diff_identity_when_trees_are_equal(t in arb_tree()) {
        let ops = diff_tree(&t, &t);
        prop_assert!(
            ops.is_empty(),
            "diff of a tree against itself must be empty; got {} ops",
            ops.len()
        );
        let mut copy = t.clone();
        apply_patch(&mut copy, &ops);
        prop_assert_eq!(copy, t);
    }
}
