//! Composable tree walker.
//!
//! A [`TreeTransform`] encapsulates one pass over a [`TreeNode`] tree.
//! [`walk`] drives one or more transforms through a single depth-first
//! traversal, invoking `enter` on descent and `exit` on ascent. New
//! passes land as additional transforms instead of additional walks.
//!
//! # Why a single walker?
//!
//! The Rust SDK and the renderer each used to walk the retained tree
//! multiple times per frame (widget expansion, ID normalization, widget
//! state preparation, animation scanning). Every pass paid a fresh
//! traversal cost and every new pass added another walk. The walker
//! consolidates all mutation and observation into one traversal while
//! keeping individual concerns isolated behind the `TreeTransform`
//! trait.
//!
//! # Responsibilities
//!
//! The walker itself is intentionally narrow:
//!
//! - Depth-first recursion over `node.children`.
//! - `enter` before descending, `exit` after (in reverse order, so
//!   transforms can reliably pair setup and teardown like stack frames).
//! - `skip_children` to short-circuit descent.
//!
//! The walker does not manage scope strings, diagnostic buckets, or
//! transform-specific state. Each [`TreeTransform`] owns whatever
//! state it needs; shared fields (currently `scope`, `window_id`,
//! `warnings`) live on [`WalkCtx`] and are explicitly typed rather
//! than stashed behind `Any`. Keep [`WalkCtx`] small - add fields
//! only when a second transform actually needs to read the same
//! value the first transform writes.
//!
//! # Composition example
//!
//! ```ignore
//! use plushie_core::tree_walk::{TreeTransform, WalkCtx, walk};
//! use plushie_core::protocol::TreeNode;
//!
//! struct Normalize { /* ... */ }
//! struct Prepare<'a> { widget_state: &'a mut WidgetStateStore }
//!
//! impl TreeTransform for Normalize { /* ... */ }
//! impl TreeTransform for Prepare<'_> { /* ... */ }
//!
//! fn run(tree: &mut TreeNode, store: &mut WidgetStateStore) -> Vec<String> {
//!     let mut normalize = Normalize { /* ... */ };
//!     let mut prepare = Prepare { widget_state: store };
//!     let mut ctx = WalkCtx::default();
//!     walk(
//!         tree,
//!         &mut [&mut normalize, &mut prepare],
//!         &mut ctx,
//!     );
//!     ctx.warnings
//! }
//! ```
//!
//! Transforms are given a `&mut [&mut dyn TreeTransform]` slice so
//! they can mutate their own state while the walker iterates. Each
//! transform typically borrows whatever mutable state it needs (a
//! registry, a state store, a manager) with a lifetime that outlives
//! the walk.

use crate::protocol::TreeNode;

/// Shared context threaded through a tree walk.
///
/// Fields here must be read or written by more than one transform. If
/// only one transform needs the data, keep it inside that transform
/// instead.
#[derive(Debug, Default, Clone)]
pub struct WalkCtx {
    /// Current scope chain for ID and a11y-ref rewriting. Normalize
    /// pushes its scope segment in `enter` and pops it in `exit`;
    /// downstream transforms read this to compute scoped IDs.
    pub scope: String,

    /// ID of the window subtree currently being walked. Window nodes
    /// set this in `enter` and clear it in `exit`.
    pub window_id: String,

    /// Diagnostic messages accumulated during the walk.
    pub warnings: Vec<String>,
}

/// A single pass over a tree node.
///
/// Implementors observe or mutate each node during traversal. Use
/// `enter` for pre-order work (e.g. scope push, node rewrite),
/// `exit` for post-order work (e.g. scope pop, cleanup tracking),
/// and `skip_children` to prune the traversal.
///
/// Invocation order for `exit` is reversed relative to `enter` so
/// paired push/pop semantics nest correctly when multiple transforms
/// share [`WalkCtx`] state.
pub trait TreeTransform {
    /// Called before descending into `node.children`.
    fn enter(&mut self, node: &mut TreeNode, ctx: &mut WalkCtx);

    /// Called after returning from `node.children`. Default is a no-op.
    fn exit(&mut self, _node: &mut TreeNode, _ctx: &mut WalkCtx) {}

    /// If any transform returns true, the walker skips this node's
    /// children. `exit` still runs for all transforms. Default `false`.
    fn skip_children(&self, _node: &TreeNode, _ctx: &WalkCtx) -> bool {
        false
    }
}

/// Drive `transforms` over the subtree rooted at `node`.
///
/// Order of operations per node:
///
/// 1. Each transform's `enter` is called in slice order.
/// 2. If any transform reports `skip_children`, child recursion is
///    skipped; otherwise the walker recurses into `node.children`.
/// 3. Each transform's `exit` is called in reverse slice order.
pub fn walk(node: &mut TreeNode, transforms: &mut [&mut dyn TreeTransform], ctx: &mut WalkCtx) {
    for t in transforms.iter_mut() {
        t.enter(node, ctx);
    }

    let skip = transforms.iter().any(|t| t.skip_children(node, ctx));

    if !skip {
        // Children are walked in place. The walker does not manage
        // `ctx.scope` or `ctx.window_id`; transforms that care about
        // scope state push in `enter` and pop in `exit`.
        let child_count = node.children.len();
        for i in 0..child_count {
            walk(&mut node.children[i], transforms, ctx);
        }
    }

    for t in transforms.iter_mut().rev() {
        t.exit(node, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::TreeNode;

    // -- test helpers ------------------------------------------------------

    fn node(id: &str, children: Vec<TreeNode>) -> TreeNode {
        TreeNode {
            id: id.to_string(),
            type_name: "test".to_string(),
            props: crate::protocol::Props::Wire(serde_json::Value::Object(Default::default())),
            children,
        }
    }

    /// Records each `enter`/`exit` call on a shared trace.
    struct Recorder {
        name: &'static str,
        trace: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl TreeTransform for Recorder {
        fn enter(&mut self, node: &mut TreeNode, _ctx: &mut WalkCtx) {
            self.trace
                .borrow_mut()
                .push(format!("{}:enter:{}", self.name, node.id));
        }
        fn exit(&mut self, node: &mut TreeNode, _ctx: &mut WalkCtx) {
            self.trace
                .borrow_mut()
                .push(format!("{}:exit:{}", self.name, node.id));
        }
    }

    /// Short-circuits descent when a node's id matches `skip_id`.
    struct Pruner {
        skip_id: &'static str,
    }

    impl TreeTransform for Pruner {
        fn enter(&mut self, _node: &mut TreeNode, _ctx: &mut WalkCtx) {}
        fn skip_children(&self, node: &TreeNode, _ctx: &WalkCtx) -> bool {
            node.id == self.skip_id
        }
    }

    #[test]
    fn enter_is_called_in_slice_order() {
        let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut a = Recorder {
            name: "A",
            trace: trace.clone(),
        };
        let mut b = Recorder {
            name: "B",
            trace: trace.clone(),
        };
        let mut tree = node("root", vec![]);
        let mut ctx = WalkCtx::default();
        walk(&mut tree, &mut [&mut a, &mut b], &mut ctx);

        let t = trace.borrow();
        assert_eq!(t[0], "A:enter:root");
        assert_eq!(t[1], "B:enter:root");
    }

    #[test]
    fn exit_is_called_in_reverse_slice_order() {
        let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut a = Recorder {
            name: "A",
            trace: trace.clone(),
        };
        let mut b = Recorder {
            name: "B",
            trace: trace.clone(),
        };
        let mut tree = node("root", vec![]);
        let mut ctx = WalkCtx::default();
        walk(&mut tree, &mut [&mut a, &mut b], &mut ctx);

        let t = trace.borrow();
        // The last two entries should be B's exit then A's exit
        // (reverse of enter order).
        assert_eq!(t[t.len() - 2], "B:exit:root");
        assert_eq!(t[t.len() - 1], "A:exit:root");
    }

    #[test]
    fn depth_first_traversal_enters_before_descending() {
        let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut rec = Recorder {
            name: "R",
            trace: trace.clone(),
        };
        let mut tree = node(
            "root",
            vec![node("a", vec![node("a1", vec![])]), node("b", vec![])],
        );
        let mut ctx = WalkCtx::default();
        walk(&mut tree, &mut [&mut rec], &mut ctx);

        let t = trace.borrow();
        let expected = vec![
            "R:enter:root",
            "R:enter:a",
            "R:enter:a1",
            "R:exit:a1",
            "R:exit:a",
            "R:enter:b",
            "R:exit:b",
            "R:exit:root",
        ];
        assert_eq!(*t, expected);
    }

    #[test]
    fn skip_children_prunes_the_subtree_but_still_runs_exit() {
        let trace = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut rec = Recorder {
            name: "R",
            trace: trace.clone(),
        };
        let mut pruner = Pruner { skip_id: "a" };
        let mut tree = node(
            "root",
            vec![node("a", vec![node("a1", vec![])]), node("b", vec![])],
        );
        let mut ctx = WalkCtx::default();
        walk(&mut tree, &mut [&mut rec, &mut pruner], &mut ctx);

        let t = trace.borrow();
        // `a1` is never entered because pruner short-circuits at `a`.
        // `a`'s enter and exit both fire; subtree is skipped.
        assert!(!t.iter().any(|line| line.contains("a1")));
        assert!(t.contains(&"R:enter:a".to_string()));
        assert!(t.contains(&"R:exit:a".to_string()));
        assert!(t.contains(&"R:enter:b".to_string()));
    }

    #[test]
    fn transforms_can_mutate_nodes_in_enter() {
        struct Renamer;
        impl TreeTransform for Renamer {
            fn enter(&mut self, node: &mut TreeNode, _ctx: &mut WalkCtx) {
                node.id = format!("x:{}", node.id);
            }
        }

        let mut tree = node("root", vec![node("child", vec![])]);
        let mut r = Renamer;
        let mut ctx = WalkCtx::default();
        walk(&mut tree, &mut [&mut r], &mut ctx);

        assert_eq!(tree.id, "x:root");
        assert_eq!(tree.children[0].id, "x:child");
    }

    #[test]
    fn warnings_accumulate_across_transforms() {
        struct Warner(&'static str);
        impl TreeTransform for Warner {
            fn enter(&mut self, node: &mut TreeNode, ctx: &mut WalkCtx) {
                ctx.warnings.push(format!("{}@{}", self.0, node.id));
            }
        }

        let mut tree = node("root", vec![node("child", vec![])]);
        let mut a = Warner("A");
        let mut b = Warner("B");
        let mut ctx = WalkCtx::default();
        walk(&mut tree, &mut [&mut a, &mut b], &mut ctx);

        // Two warnings per node (one per transform), two nodes.
        assert_eq!(ctx.warnings.len(), 4);
        assert!(ctx.warnings.contains(&"A@root".to_string()));
        assert!(ctx.warnings.contains(&"B@root".to_string()));
        assert!(ctx.warnings.contains(&"A@child".to_string()));
        assert!(ctx.warnings.contains(&"B@child".to_string()));
    }
}
