//! Renderer and direct-mode support API.
//!
//! This module exposes the pieces needed by the Plushie renderer crates
//! and the Rust SDK direct runner without making the widget SDK's
//! implementation modules part of the public API.

pub use crate::codec::{Codec, MAX_MESSAGE_SIZE};
pub use crate::engine::{Core, CoreEffect, Dispatch, Emit, StateChange, SubscriptionEntry};
pub use crate::image_registry::ImageRegistry;
pub use crate::message::{
    KeyEventData, Message, StdinEvent, serialize_key, serialize_location, serialize_modifiers,
    serialize_mouse_button, serialize_physical_key, serialize_scroll_delta,
};
pub use crate::shared_state::SharedState;
pub use crate::theming::{
    ThemeChrome, resolve_theme, resolve_theme_and_chrome_only, resolve_theme_only,
    resolve_theme_with_chrome,
};
pub use crate::validate::{is_validate_props_enabled, set_validate_props};
pub use crate::widget::render::render;
pub use crate::widget::widget_set::{IcedWidgetSet, iced_widget_set};

/// Canvas runtime query helpers.
pub mod canvas {
    pub use crate::widget::canvas::{
        canvas_find_element_by_id, canvas_has_on_press, canvas_hit_test,
    };
}
