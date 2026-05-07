//! Iced-dependent extension methods for OutgoingEvent.
//!
//! These constructors reference iced types (KeyEventData) and cannot
//! live in plushie-core.

use plushie_core::protocol::OutgoingEvent;

/// Extension trait for OutgoingEvent keyboard constructors.
pub trait OutgoingEventKeyExt {
    /// Build a key press event from iced key data.
    fn key_press(tag: impl Into<String>, data: &crate::runtime::KeyEventData) -> OutgoingEvent;

    /// Build a key release event from iced key data.
    fn key_release(tag: impl Into<String>, data: &crate::runtime::KeyEventData) -> OutgoingEvent;
}

impl OutgoingEventKeyExt for OutgoingEvent {
    fn key_press(tag: impl Into<String>, data: &crate::runtime::KeyEventData) -> OutgoingEvent {
        let mut event = OutgoingEvent::tagged("key_press", tag);
        event.modifiers = Some(crate::runtime::serialize_modifiers(data.modifiers));
        event.value = Some(serde_json::json!({
            "key": crate::runtime::serialize_key(&data.key),
            "modified_key": crate::runtime::serialize_key(&data.modified_key),
            "physical_key": crate::runtime::serialize_physical_key(&data.physical_key),
            "location": crate::runtime::serialize_location(&data.location),
            "text": data.text.as_deref(),
            "repeat": data.repeat,
        }));
        event
    }

    fn key_release(tag: impl Into<String>, data: &crate::runtime::KeyEventData) -> OutgoingEvent {
        let mut event = OutgoingEvent::tagged("key_release", tag);
        event.modifiers = Some(crate::runtime::serialize_modifiers(data.modifiers));
        event.value = Some(serde_json::json!({
            "key": crate::runtime::serialize_key(&data.key),
            "modified_key": crate::runtime::serialize_key(&data.modified_key),
            "physical_key": crate::runtime::serialize_physical_key(&data.physical_key),
            "location": crate::runtime::serialize_location(&data.location),
        }));
        event
    }
}
