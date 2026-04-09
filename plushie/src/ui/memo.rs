//! View memoization.
//!
//! Wraps a view subtree in a `__memo__` marker node so the runtime
//! can skip re-rendering when the cache key has not changed.

use serde_json::{Map, json};

use crate::View;

/// Cache a view subtree by key. If the key has not changed since
/// the last render, the cached tree is reused without calling
/// the view function.
///
/// Useful for expensive subtrees that rarely change:
///
/// ```ignore
/// column().children([
///     memo("header", || expensive_header_view(&model.header)),
///     text(&model.dynamic_text),
/// ])
/// ```
///
/// The SDK produces a `__memo__` marker node. Caching happens at
/// the normalization/renderer level; this function always calls
/// `view_fn` to produce the inner tree.
pub fn memo(key: impl Into<String>, view_fn: impl FnOnce() -> View) -> View {
    let key_str = key.into();
    let inner = view_fn();

    let mut props = Map::new();
    props.insert("__memo_key__".into(), json!(key_str));

    View::node(
        format!("memo:{key_str}"),
        "__memo__",
        props,
        vec![inner],
    )
}
