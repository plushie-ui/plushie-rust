//! Iced-dependent extension methods for OutgoingEvent.
//!
//! These constructors reference iced types (KeyEventData) and cannot
//! live in plushie-core.

use plushie_core::protocol::OutgoingEvent;

/// Extension trait for OutgoingEvent keyboard constructors.
pub trait OutgoingEventKeyExt {
    fn key_press(tag: String, data: &crate::message::KeyEventData) -> OutgoingEvent;
    fn key_release(tag: String, data: &crate::message::KeyEventData) -> OutgoingEvent;
}

impl OutgoingEventKeyExt for OutgoingEvent {
    fn key_press(tag: String, data: &crate::message::KeyEventData) -> OutgoingEvent {
        let mut event = OutgoingEvent::tagged("key_press", tag);
        event.modifiers = Some(crate::message::serialize_modifiers(data.modifiers));
        event.value = Some(serde_json::json!({
            "key": crate::message::serialize_key(&data.key),
            "modified_key": crate::message::serialize_key(&data.modified_key),
            "physical_key": crate::message::serialize_physical_key(&data.physical_key),
            "location": crate::message::serialize_location(&data.location),
            "text": data.text.as_deref(),
            "repeat": data.repeat,
        }));
        event
    }

    fn key_release(tag: String, data: &crate::message::KeyEventData) -> OutgoingEvent {
        let mut event = OutgoingEvent::tagged("key_release", tag);
        event.modifiers = Some(crate::message::serialize_modifiers(data.modifiers));
        event.value = Some(serde_json::json!({
            "key": crate::message::serialize_key(&data.key),
            "modified_key": crate::message::serialize_key(&data.modified_key),
            "physical_key": crate::message::serialize_physical_key(&data.physical_key),
            "location": crate::message::serialize_location(&data.location),
        }));
        event
    }
}
