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
pub use watch::{WatchOpts, watch_renderer};
