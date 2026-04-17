//! Common re-exports for app developers.
//!
//! `use plushie::prelude::*` imports everything needed to write
//! a plushie app: the `App` trait, event/command types, UI builders,
//! and common property types.

// Core trait
pub use crate::App;

// Automation
pub use crate::automation::{Element, Selector};
pub use plushie_core::key::{EffectKind, Key, KeyPress, MouseButton, PointerKind};

// Derive macros for widget authoring
pub use crate::{WidgetCommand, WidgetEvent, WidgetProps};

// Widget registrar (for App::view)
pub use crate::widget::WidgetRegistrar;

// View
pub use crate::View;

// Events
pub use crate::event::WidgetMatch::*;
pub use crate::event::{Event, EventType, WidgetMatch};

// Scoped IDs
pub use plushie_core::ScopedId;

// Commands and operation types
pub use crate::command::{
    Command, FileDialogOpts, NotificationOpts, NotificationUrgency, WindowMode,
};

// Subscriptions
pub use crate::subscription::Subscription;

// UI builder functions
pub use crate::ui::*;

// Property types
pub use crate::types::{
    A11y, Align, Anchor, Angle, Animatable, ArrowMode, Background, Border, Color, ContentFit,
    CursorStyle, CustomTheme, Direction, DragAxis, Ellipsis, ErrorCorrection, FillRule,
    FilterMethod, Font, FontStretch, FromNode, Gradient, HorizontalAlignment, InputPurpose,
    KeyModifiers, Length, Length::*, LineCap, LineHeight, LineJoin, Padding, PlushieType, Position,
    Radius, Shadow, Shaping, SortOrder, Style, StyleMap, Theme, UntypedProps, WidgetCommandEncode,
    WidgetEventEncode, WindowLevel, Wrapping,
};

// A11y sub-types for typed accessibility builders
pub use plushie_core::types::a11y::{HasPopup, Live, Orientation, Role};

// Animation
pub use crate::animation::{
    AnimationStep, Easing, Repeat, Sequence, Spring, SpringConfig, Transition, Tween,
};
