//! Shared renderer logic for plushie.
//!
//! This crate contains the platform-independent rendering engine that
//! processes incoming messages, dispatches iced updates, and emits
//! outgoing events. It compiles to both native and wasm32 targets.
//!
//! Platform-specific behavior (I/O, effects, sleep) is injected via
//! traits and cfg-gated dependencies. The `plushie` binary crate and
//! `plushie-web` WASM crate each provide their own implementations.

/// Renderer crate version string.
///
/// Emitted by the renderer in the `hello` handshake message and
/// cross-referenced by host SDKs (and `plushie::run_with_renderer`) to
/// detect version skew between the SDK and an installed renderer
/// binary. A mismatch is not fatal by itself (wire protocol
/// compatibility is governed by `plushie_core::protocol::PROTOCOL_VERSION`),
/// but host SDKs use this to surface a clear upgrade hint.
pub const RENDERER_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod app;
pub mod apply;
pub mod constants;
pub mod emitter;
pub mod emitters;
pub mod events;
pub mod execute;
pub mod scripting;
pub mod settings;
pub mod subscriptions;
pub mod update;
pub mod view;
pub mod widget_ops;
pub mod window_map;
pub mod window_ops;

pub mod effects;

/// The iced daemon application. See [`app::App`] for details.
pub use app::App;

/// Clamp or reject invalid scale factors. See [`app::validate_scale_factor`].
pub use app::validate_scale_factor;

/// Trait for platform-specific side effects. See [`effects::EffectHandler`].
pub use effects::EffectHandler;

/// Pluggable output for renderer events. See [`emitters::EventSink`].
pub use emitters::EventSink;
/// An EventSink that encodes via a codec and writes to a writer.
pub use emitters::WriterSink;
