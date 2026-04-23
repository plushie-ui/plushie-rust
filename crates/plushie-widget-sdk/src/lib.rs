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
//! ## Dependencies
//!
//! Widget crates need both `plushie-widget-sdk` and `plushie-core`
//! as direct dependencies. The widget derive macros
//! (`#[derive(WidgetEvent)]`, `#[derive(WidgetCommand)]`,
//! `#[derive(WidgetProps)]`, `#[derive(PlushieWidget)]`) emit code
//! that references `::plushie_core::*` paths.
//!
//! ## iced stability
//!
//! iced is re-exported as a transitive dependency. iced surfaces may
//! change on any plushie minor release. For stable semantics, prefer
//! prelude names and [`iced_convert`] conversions; reach into
//! `plushie_widget_sdk::iced::*` only for iced-specific constructs
//! that are not in the prelude.
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

/// Re-export of [`plushie_core::diagnostics`] for convenience so widget
/// crates that already depend on `plushie-widget-sdk` don't need a
/// second direct dep on `plushie-core` just for diagnostic emission.
pub use plushie_core::diagnostics;

pub mod fonts;

pub mod iced_convert;

pub(crate) mod a11y;
pub mod shared_state;
pub(crate) mod svg_guard;
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

// Re-export the widget derive macros for widget authors.
//
// The generated code references `::plushie_core::*` paths so widget
// crates must still add `plushie-core` as a direct dependency
// alongside `plushie-widget-sdk`; see the crate-level "Dependencies"
// section.
//
// `PlushieWidget` from plushie_core_macros is the derive macro; it
// shares a name with the `registry::PlushieWidget` trait. Rust
// permits the coexistence (derives and traits inhabit different
// namespaces) but importers must take care not to glob-import only
// the trait and then use `#[derive(PlushieWidget)]`.
pub use plushie_core_macros::{PlushieWidget, WidgetCommand, WidgetEvent, WidgetProps};

/// Sorted list of every built-in widget type name registered by the
/// stock renderer's iced widget set.
///
/// The build tool reads this list to detect native widgets that would
/// shadow a built-in name. It is the single source of truth for the
/// reserved-name list alongside [`widget::widget_set::IcedWidgetSet`];
/// a drift-detection test asserts the two stay in sync.
pub const BUILTIN_TYPE_NAMES: &[&str] = &[
    "button",
    "canvas",
    "checkbox",
    "column",
    "combo_box",
    "container",
    "float",
    "grid",
    "image",
    "keyed_column",
    "markdown",
    "overlay",
    "pane_grid",
    "pick_list",
    "pin",
    "pointer_area",
    "progress_bar",
    "qr_code",
    "radio",
    "responsive",
    "rich",
    "rich_text",
    "row",
    "rule",
    "scrollable",
    "sensor",
    "slider",
    "space",
    "stack",
    "svg",
    "table",
    "text",
    "text_editor",
    "text_input",
    "themer",
    "toggler",
    "tooltip",
    "vertical_slider",
    "window",
];

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
///
/// This trait is sealed: only `iced::Renderer` and `()` implement it,
/// and new super-trait bounds can be added without breaking external
/// code (external crates cannot implement `PlushieRenderer`).
pub trait PlushieRenderer:
    sealed::Sealed
    + iced::advanced::Renderer
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

mod sealed {
    /// Sealing trait for [`super::PlushieRenderer`]. Not part of the
    /// public API; keeps `PlushieRenderer` closed to external impls.
    pub trait Sealed {}
    impl Sealed for () {}
    impl Sealed for super::iced::Renderer {}
}

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
