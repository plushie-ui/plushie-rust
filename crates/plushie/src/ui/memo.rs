//! View memoization.
//!
//! Wraps a view subtree in a `__memo__` marker node so the runtime
//! can reuse the previously-normalized subtree when the author-
//! supplied deps are unchanged.
//!
//! The view function still runs - `view(&model)` is pure, so
//! short-circuiting it would require thread-local trickery. What the
//! cache avoids is re-walking the subtree through normalization
//! (scope prefixing, a11y rewrites) and re-emitting patches for
//! unchanged subtrees downstream.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::PropMap;

use crate::View;

/// Cache a view subtree by a stable key plus a set of hashable deps.
///
/// The `key` identifies the memo call site (one stable id per
/// distinct memo in the view tree). The `deps` are hashed on every
/// render; when the hash matches the previous render's, normalization
/// reuses the cached subtree instead of re-walking it.
///
/// ```ignore
/// column().children([
///     memo("header", (model.user_id, model.revision), || {
///         expensive_header_view(&model)
///     }),
///     text(&model.dynamic_text),
/// ])
/// ```
///
/// Any `Hash` deps work: a tuple, a single `&str`, a `u64`, a custom
/// type that derives `Hash`. Implementations should avoid hashing
/// floats unless they have deliberate bit-level semantics.
///
/// The `view_fn` always runs (the view function is pure; the SDK
/// can't know deps haven't changed without hashing them, which
/// requires calling the closure once and comparing after the fact).
/// The saving is downstream: normalization, tree diff, and renderer
/// apply all reuse the cached subtree on a hit.
pub fn memo<D: Hash>(key: impl Into<String>, deps: D, view_fn: impl FnOnce() -> View) -> View {
    let key_str = key.into();

    let mut hasher = DefaultHasher::new();
    deps.hash(&mut hasher);
    let deps_hash = hasher.finish();

    let inner = view_fn();

    let mut props = PropMap::new();
    // Store the deps hash so `NormalizeTransform` can compare it
    // against its cache without re-hashing the original deps.
    props.insert("__memo_deps__", deps_hash);

    super::view_node(format!("memo:{key_str}"), "__memo__", props, vec![inner])
}
