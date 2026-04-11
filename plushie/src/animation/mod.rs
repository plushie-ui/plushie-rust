//! Animation descriptors and SDK-side interpolation.
//!
//! Descriptor types ([`Transition`], [`Spring`], [`Sequence`], [`Easing`])
//! are defined in [`plushie_core::animation`] and re-exported here.
//! The [`Tween`] type is SDK-only (client-side interpolation).
//!
//! ```ignore
//! text("value")
//!     .transition("size", Transition::new(300, 24.0).easing(Easing::EaseOut))
//! ```

mod tween;

// Re-export all animation descriptor types from plushie-core.
pub use plushie_core::animation::*;

// SDK-only: client-side interpolation.
pub use tween::Tween;
