//! Dev-mode tooling: file watcher, renderer live-reload, in-tree
//! rebuild overlay.
//!
//! Everything in this module is gated behind the `dev` Cargo feature
//! so production builds carry none of the extra dependencies
//! (`notify`, `cargo_metadata`) or code paths.
//!
//! Enable it in your app's `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! plushie = { version = "0", features = ["dev"] }
//! ```
//!
//! ...and wire the watcher into your `main`:
//!
//! ```ignore
//! fn main() -> plushie::Result {
//!     plushie::dev::watch_renderer::<MyApp>()
//! }
//! ```
//!
//! # What's dev-mode?
//!
//! - **Widget-crate watcher**: reads `[package.metadata.plushie]`
//!   from the app's cargo metadata, watches each widget crate's
//!   `src/` directory and `Cargo.toml`, and rebuilds the custom
//!   renderer (via `cargo plushie build`) when sources change.
//! - **Rebuilding overlay**: a slim in-tree status bar injected at
//!   the top of every window so the app can see build status
//!   without hunting through terminal logs. See [`overlay`].
//!
//! # App-source watching
//!
//! This module does **not** watch the app's own source. The running
//! binary would need to be replaced for those changes to take effect,
//! which the SDK can't do from inside. Use `cargo-watch` outside the
//! process, or the `cargo plushie run --watch` convenience wrapper,
//! for app-src live reload.

pub mod overlay;
mod watch;

pub use overlay::{DevOverlayHandle, RebuildingOverlay, Status};
pub use watch::{WatchOpts, watch_renderer, watch_renderer_with_opts};

use std::sync::OnceLock;

/// Process-global dev-overlay handle. Registered once (ideally before
/// `plushie::run` starts) so the runtime's tree walker can read the
/// current overlay snapshot on each frame without passing the handle
/// through every layer. `None` when no handle is registered, which is
/// the production default; the runtime treats the absence as "no
/// overlay" and skips the injection pass entirely.
static GLOBAL_OVERLAY: OnceLock<DevOverlayHandle> = OnceLock::new();

/// Register a dev-overlay handle with the runtime.
///
/// Once registered, the handle cannot be swapped out (OnceLock
/// semantics); a second call is a no-op. Typically called by the
/// watcher before handing off to [`crate::run`], but library code
/// that wants to build its own watcher can register a handle here
/// and push status to it directly.
pub fn register_overlay(handle: DevOverlayHandle) {
    let _ = GLOBAL_OVERLAY.set(handle);
}

/// Best-effort read of the current overlay snapshot, dismissing
/// expired `Success` states so the auto-dismiss timer fires the
/// next time the tree gets rebuilt.
///
/// `crate::runtime::prepare_tree` calls this once per view cycle;
/// production builds don't compile this path at all.
pub(crate) fn current_overlay_snapshot() -> Option<RebuildingOverlay> {
    GLOBAL_OVERLAY.get().and_then(|h| h.snapshot())
}
