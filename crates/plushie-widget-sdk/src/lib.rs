//! # plushie-widget-sdk
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
//! - [`prelude`] - common re-exports for widget authors
//! - [`registry`] - `PlushieWidget` trait, `WidgetRegistry`, `WidgetSet`,
//!   `InitCtx`, `GenerationCounter`
//! - [`app`] - `PlushieAppBuilder` for registering widgets
//! - [`prop_helpers`] - public prop extraction helpers
//! - [`render_ctx`] - `RenderCtx`, the core rendering context for all widgets
//! - [`testing`] - test factory helpers
//!
//! **Internal modules** (used by the plushie binary, not part of the SDK):
//! `engine`, `tree`, `message`, `widget`, `protocol`, `codec`,
//! `theming`, `image_registry`

#![deny(missing_docs)]

// Ensure catch_unwind works: widget panic isolation requires unwinding.
// If this fails, remove `panic = "abort"` from your Cargo profile.
// On WASM, catch_unwind is a no-op (panics always abort), so skip this check.
#[cfg(all(not(test), not(target_arch = "wasm32"), panic = "abort"))]
compile_error!(
    "plushie-core requires panic=\"unwind\" (the default). \
     Widget panic isolation via catch_unwind is a no-op with panic=\"abort\"."
);

// -- Public SDK modules (stable API for widget authors) --
pub mod app;
pub mod canvas_engine;
pub mod prelude;
pub mod prop_helpers;
pub mod registry;
pub mod render_ctx;
pub mod testing;

pub mod animation;

pub mod iced_convert;

pub(crate) mod a11y;
pub mod shared_state;
pub mod svg_guard;
pub(crate) mod validate;

// -- Internal modules used by the plushie binary --
//
// These are public so the binary crate can access them, but they are
// NOT part of the stable widget SDK. Widget authors should use
// the prelude and `plushie_widget_sdk::iced::*` instead.
#[doc(hidden)]
pub mod codec;
#[doc(hidden)]
pub mod engine;
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
pub mod widget;

// Re-export the widget derive macros for widget authors. Keeping the
// re-exports here (and mirrored in the prelude) means a widget crate
// depends on `plushie-widget-sdk` alone; there is no reason to pull
// in `plushie-core` directly.
//
// `PlushieWidget` from plushie_core_macros is the derive macro; it
// shares a name with the `registry::PlushieWidget` trait. Rust
// permits the coexistence (derives and traits inhabit different
// namespaces) but importers must take care not to glob-import only
// the trait and then use `#[derive(PlushieWidget)]`.
pub use plushie_core_macros::{PlushieWidget, WidgetCommand, WidgetEvent, WidgetProps};

// Re-export iced so widget crates can use `plushie_widget_sdk::iced::*` without
// adding a direct iced dependency. This avoids version conflicts when
// plushie-core bumps its iced version. Widgets that use only
// `plushie_widget_sdk::prelude::*` and `plushie_widget_sdk::iced::*` get the upgrade
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

/// Convenience alias for the `Element` type returned by
/// [`PlushieWidget::render`](registry::PlushieWidget::render).
///
/// Equivalent to `iced::Element<'a, Message, iced::Theme, R>`. Widget
/// impls that are generic over the renderer should write
/// `PlushieElement<'a, R>`; widgets that only ever render under the
/// real iced renderer can omit the parameter (`PlushieElement<'a>`)
/// and get `iced::Renderer` as the default.
pub type PlushieElement<'a, R = iced::Renderer> =
    iced::Element<'a, crate::message::Message, iced::Theme, R>;
