//! Core types and protocol for Plushie.
//!
//! This crate contains the shared data types used by both the Plushie
//! SDK and the renderer. It has no iced dependency, making it suitable
//! for wire-mode apps, FFI bindings, and tooling that doesn't need
//! the full rendering stack.

extern crate self as plushie_core;

pub use plushie_core_macros::{PlushieEnum, WidgetCommand, WidgetEvent};

pub mod animation;
pub mod codec_safety;
pub mod event_type;
pub mod key;
pub mod ops;
pub mod outgoing_message;
pub mod pointer;
pub mod protocol;
pub mod scoped_id;
pub mod selector;
pub mod settings;
pub mod spec;
pub mod tree_walk;
pub mod types;
pub mod widget_builder;

pub use event_type::EventType;
pub use key::{EffectKind, InteractAction, Key, KeyPress, MouseButton, PointerKind};
pub use scoped_id::ScopedId;
pub use selector::Selector;
pub use spec::{CommandSpec, EventSpec, PayloadSpec, ValueType, WidgetCommandEncode};
pub use widget_builder::WidgetBuilder;
