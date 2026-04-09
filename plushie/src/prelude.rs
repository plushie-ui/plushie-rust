//! Common re-exports for app developers.
//!
//! `use plushie::prelude::*` imports everything needed to write
//! a plushie app: the `App` trait, event/command types, UI builders,
//! and common property types.

// Core trait
pub use crate::App;

// View
pub use crate::View;

// Events
pub use crate::event::{Event, EventType, WidgetMatch};
pub use crate::event::WidgetMatch::*;

// Commands
pub use crate::command::Command;

// Subscriptions
pub use crate::subscription::Subscription;

// UI builder functions
pub use crate::ui::*;

// Property types
pub use crate::types::{
    Align, Border, Color, Font, Gradient, KeyModifiers,
    Length, Length::*, Padding, Shadow, Style, StyleMap,
};
