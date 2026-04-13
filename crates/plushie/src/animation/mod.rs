//! Animation descriptors and SDK-side interpolation.
//!
//! Descriptor types ([`Transition`], [`Spring`], [`Sequence`], [`Easing`])
//! are defined in [`plushie_core::animation`] and re-exported here.
//! The [`Tween`] type is SDK-only (client-side interpolation).
//!
//! Animation descriptors are generic over the value type. Use them
//! directly in prop setters via [`Animatable`](crate::types::Animatable):
//!
//! ```ignore
//! // Static value (no animation, same as before)
//! text("value").size(24.0)
//!
//! // Animate size with a transition
//! text("value").size(Transition::new(300, 24.0_f32).easing(Easing::EaseOut))
//!
//! // Animate color with a spring
//! text("value").color(Spring::bouncy(Color::red()))
//! ```

mod tween;

// Re-export all animation descriptor types from plushie-core.
pub use plushie_core::animation::*;

// SDK-only: client-side interpolation.
pub use tween::{RedirectOpts, SpringConfig, Tween};
