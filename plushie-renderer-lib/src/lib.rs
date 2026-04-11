//! Shared renderer logic for plushie.
//!
//! This crate contains the platform-independent rendering engine that
//! processes incoming messages, dispatches iced updates, and emits
//! outgoing events. It compiles to both native and wasm32 targets.
//!
//! Platform-specific behavior (I/O, effects, sleep) is injected via
//! traits and cfg-gated dependencies. The `plushie` binary crate and
//! `plushie-web` WASM crate each provide their own implementations.

pub mod app;
pub mod apply;
pub mod execute;
pub mod constants;
pub mod emitter;
pub mod emitters;
pub mod events;
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
