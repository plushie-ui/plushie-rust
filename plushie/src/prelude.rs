//! Common re-exports for app developers.
//!
//! `use plushie::prelude::*` imports everything needed to write
//! a plushie app: the `App` trait, event/command types, UI builders,
//! and common property types.

// Core trait
pub use crate::App;

// Widget registrar (for App::view)
pub use crate::widget::WidgetRegistrar;

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
    A11y, Align, Animatable, Anchor, ArrowMode, Background, Border, Color,
    ContentFit, CursorStyle, Direction, DragAxis, Ellipsis, ErrorCorrection,
    FillRule, FilterMethod, Font, FontStretch, Gradient, HorizontalAlignment,
    InputPurpose, KeyModifiers, Length, Length::*, LineCap, LineJoin,
    LineHeight, Padding, PlushieType, Position, Radius, Shadow, Shaping,
    SortOrder, Style, StyleMap, Theme, WindowLevel, Wrapping,
};

// A11y sub-types for typed accessibility builders
pub use plushie_core::types::a11y::{Live, Role};

// Animation
pub use crate::animation::{
    AnimationStep, Easing, Repeat, Sequence, Spring, Transition,
};
