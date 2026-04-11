//! Execution backends for plushie apps.
//!
//! Two modes are available:
//!
//! - **Direct** (`plushie::run`): Renders in-process using iced.
//!   No subprocess, no serialization. Default.
//! - **Wire** (`plushie::run_wire`): Spawns a renderer binary
//!   and communicates over stdin/stdout.

#[cfg(feature = "direct")]
pub mod direct;

#[cfg(feature = "direct")]
mod effects;

pub(crate) mod event_bridge;

#[cfg(feature = "direct")]
mod queue_sink;

#[cfg(feature = "wire")]
pub mod bridge;

#[cfg(feature = "wire")]
pub mod wire;
