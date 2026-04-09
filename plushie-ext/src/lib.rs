//! # plushie-core
//!
//! The public SDK for plushie. Widget authors depend on this crate to
//! implement the [`PlushieWidget`](registry::PlushieWidget) trait and
//! build custom native widgets. The [`prelude`] module re-exports
//! everything a widget author needs; [`iced`] is re-exported so widgets
//! don't need a direct iced dependency.
//!
//! This crate also provides the rendering engine, wire protocol, and
//! widget infrastructure used internally by the `plushie` binary.
//!
//! ## Module guide
//!
//! **Widget SDK (stable API):**
//! - [`prelude`] -- common re-exports for widget authors
//! - [`registry`] -- `PlushieWidget` trait, `WidgetRegistry`, `WidgetSet`
//! - [`app`] -- `PlushieAppBuilder` for registering widgets
//! - [`prop_helpers`] -- public prop extraction helpers
//! - [`extensions`] -- legacy `WidgetExtension` trait (being replaced by `PlushieWidget`)
//! - [`testing`] -- test factory helpers
//!
//! **Internal modules** (used by the plushie binary, not part of the SDK):
//! `engine`, `tree`, `message`, `widgets`, `protocol`, `codec`,
//! `theming`, `image_registry`

// Ensure catch_unwind works: extension panic isolation requires unwinding.
// If this fails, remove `panic = "abort"` from your Cargo profile.
// On WASM, catch_unwind is a no-op (panics always abort), so skip this check.
#[cfg(all(not(test), not(target_arch = "wasm32"), panic = "abort"))]
compile_error!(
    "plushie-core requires panic=\"unwind\" (the default). \
     Extension panic isolation via catch_unwind is a no-op with panic=\"abort\"."
);

// -- Public SDK modules (stable API for extension authors) --
pub mod app;
pub mod canvas_engine;
pub mod extensions;
pub mod prelude;
pub mod prop_helpers;
pub mod registry;
pub mod testing;

pub mod animation;

// -- Internal modules used by the plushie binary --
//
// These are public so the binary crate can access them, but they are
// NOT part of the stable extension SDK. Extension authors should use
// the prelude and `plushie_ext::iced::*` instead.
#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod engine;
#[doc(hidden)]
pub mod image_registry;
#[doc(hidden)]
pub mod message;
#[doc(hidden)]
pub mod protocol;
#[doc(hidden)]
pub mod theming;
#[doc(hidden)]
pub mod tree;
#[doc(hidden)]
pub mod widgets;

// Re-export iced so extension crates can use `plushie_ext::iced::*` without
// adding a direct iced dependency. This avoids version conflicts when
// plushie-core bumps its iced version -- extensions that use only
// `plushie_ext::prelude::*` and `plushie_ext::iced::*` get the upgrade
// automatically.
pub use iced;

/// Trait alias for renderer types that can be used with the plushie widget pipeline.
///
/// Both `iced::Renderer` (tiny-skia, used by headless and windowed modes) and
/// `()` (null renderer, used by mock mode) satisfy these bounds.
pub trait PlushieRenderer:
    iced::advanced::Renderer
    + iced::advanced::text::Renderer<Font = iced::Font>
    + iced::advanced::image::Renderer<Handle = iced::advanced::image::Handle>
    + iced::advanced::svg::Renderer
    + iced::advanced::renderer::Headless
    + iced::advanced::graphics::geometry::Renderer
    + 'static
{
}

impl PlushieRenderer for () {}
impl PlushieRenderer for iced::Renderer {}
