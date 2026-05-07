//! Renderer-internal state engine and wire codec.
//!
//! This crate holds the pieces of the Plushie renderer that widget
//! authors do not need at compile time: the pure UI tree state
//! machine (`Core`), the retained node tree (`Tree`), and the wire
//! codec (`Codec`).
//!
//! The split keeps `plushie-widget-sdk` focused on the public widget-
//! author surface while consolidating the renderer-internal modules
//! used by `plushie-renderer-lib`, `plushie-renderer`, the WASM
//! entry point, and the Rust SDK direct runner here.
//!
//! # Dependency direction
//!
//! ```text
//! plushie-core
//!       |
//!       v
//! plushie-widget-sdk
//!       |
//!       v
//! plushie-renderer-engine    (this crate)
//!       |
//!       v
//! plushie-renderer-lib
//! ```
