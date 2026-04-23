//! Core types and protocol for Plushie.
//!
//! This crate contains the shared data types used by both the Plushie
//! SDK and the renderer. It has no iced dependency, making it suitable
//! for wire-mode apps, FFI bindings, and tooling that doesn't need
//! the full rendering stack.

extern crate self as plushie_core;

pub use plushie_core_macros::{PlushieEnum, WidgetCommand, WidgetEvent, widget};

pub mod animation;
pub mod codec_safety;
pub mod diagnostic;
pub mod diagnostics;
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

pub use diagnostic::{Diagnostic, DiagnosticKind};
pub use event_type::EventType;
pub use key::{EffectKind, InteractAction, Key, KeyPress, MouseButton, PointerKind};
pub use scoped_id::ScopedId;
pub use selector::Selector;
pub use spec::{CommandSpec, EventSpec, PayloadSpec, ValueType, WidgetCommandEncode};
pub use widget_builder::WidgetBuilder;

/// Sorted list of every built-in widget type name reserved by the
/// stock renderer's iced widget set.
///
/// Tooling uses this list to detect native widgets that would shadow a
/// built-in name without depending on `plushie-widget-sdk` or iced. The
/// widget SDK has a drift-detection test that compares this list with
/// the renderer's registered built-in widget set.
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
