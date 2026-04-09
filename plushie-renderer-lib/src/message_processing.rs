//! Shared widget message processing for daemon and headless modes.
//!
//! Both the iced daemon's `update()` and the headless
//! `process_captured_messages()` need to convert iced [`Message`]s into
//! [`OutgoingEvent`]s. The conversion involves stateful operations:
//!
//! - **Slider value tracking:** `Slide` stores the latest value so
//!   `SlideRelease` can include it (iced only reports the final pane,
//!   not the value on release).
//! - **Text editor mutation:** `TextEditorAction` must be applied to
//!   the cached `Content` and the resulting text emitted.
//! - **Extension event routing:** `Message::Event` is routed through the
//!   `WidgetRegistry` which may consume, observe, or pass through.
//! - **Pane grid state:** resize, drag, and click events need the pane
//!   state map to resolve internal pane handles to plushie IDs.
//!
//! [`process_widget_message`] centralises all of this so the two modes
//! share one implementation.

use plushie_ext::PlushieRenderer;
use plushie_ext::message::Message;
use plushie_ext::protocol::OutgoingEvent;

use crate::emitters::message_to_event;

/// Convert an iced [`Message`] into outgoing protocol events.
///
/// Returns a (possibly empty) list of [`OutgoingEvent`]s. Messages that
/// don't produce outgoing events (subscription events, `NoOp`,
/// `MarkdownUrl`, etc.) return an empty vec.
///
/// Both the daemon and headless modes call this with references to their
/// respective state. The caller is responsible for emitting the returned
/// events (stdout, WireWriter, etc.).
pub fn process_widget_message<R: PlushieRenderer>(
    msg: Message,
    registry: &mut plushie_ext::registry::WidgetRegistry<R>,
) -> Vec<OutgoingEvent> {
    // Try registry dispatch first. If the factory handles the message
    // (returns Some), use that result. Otherwise fall through to the
    // match dispatch below.
    if let Some(node_id) = msg.node_id()
        && let Some((idx, _matched_id)) = registry.get_for_node_id(node_id)
        && let Some(factory) = registry.get_mut(idx)
        && let Some(events) = factory.handle_message(&msg)
    {
        return events;
    }

    match msg {
        // Simple widget events -- stateless conversion.
        ref m @ (Message::Click(..)
        | Message::Input(..)
        | Message::Submit(..)
        | Message::Toggle(..)
        | Message::Select(..)
        | Message::Paste(..)
        | Message::OptionHovered(..)
        | Message::SensorResize(..)
        | Message::ScrollEvent(..)
        | Message::MouseAreaEvent(..)
        | Message::MouseAreaMove(..)
        | Message::MouseAreaScroll(..)
        | Message::CanvasEvent { .. }
        | Message::CanvasScroll { .. }
        | Message::CanvasElementEnter { .. }
        | Message::CanvasElementLeave { .. }
        | Message::CanvasElementClick { .. }
        | Message::CanvasElementKeyPress { .. }
        | Message::CanvasElementKeyRelease { .. }
        | Message::CanvasElementDrag { .. }
        | Message::CanvasElementDragEnd { .. }
        | Message::CanvasElementFocused { .. }
        | Message::CanvasElementBlurred { .. }
        | Message::CanvasFocused { .. }
        | Message::CanvasBlurred { .. }
        | Message::CanvasGroupFocused { .. }
        | Message::CanvasGroupBlurred { .. }
        | Message::Diagnostic { .. }) => message_to_event(m).into_iter().collect(),

        // CanvasElementFocusChanged is handled by CanvasWidget::handle_message
        // (splits into blur + focus events). Fallback returns empty.
        Message::CanvasElementFocusChanged { .. } => vec![],

        // Slider Slide/SlideRelease and TextEditorAction are handled
        // by their PlushieWidget factories via registry dispatch.
        // These arms are fallback for edge cases where the registry
        // has no mapping.
        Message::Slide(..) | Message::SlideRelease(..) | Message::TextEditorAction(..) => vec![],

        // Extension events: if the registry's handle_message (above) didn't
        // match, pass through as a generic outgoing event.
        Message::Event {
            ref window_id,
            ref id,
            ref data,
            ref family,
        } => {
            let data_opt = if data.is_null() {
                None
            } else {
                Some(data.clone())
            };
            vec![
                OutgoingEvent::generic(family.clone(), id.clone(), data_opt)
                    .with_window_id(window_id.clone()),
            ]
        }

        // Pane grid events are handled by PaneGridWidget via registry
        // dispatch. Fallback returns empty.
        Message::PaneFocusCycle(..)
        | Message::PaneResized(..)
        | Message::PaneDragged(..)
        | Message::PaneClicked(..) => vec![],

        // Everything else (subscription events, NoOp, MarkdownUrl, StatusChanged, etc.)
        // produces no outgoing events.
        _ => vec![],
    }
}
