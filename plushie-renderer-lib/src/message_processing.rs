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
//! - **Extension event routing:** `Message::Event` is forwarded to the
//!   `ExtensionDispatcher` which may consume, observe, or pass through.
//! - **Pane grid state:** resize, drag, and click events need the pane
//!   state map to resolve internal pane handles to plushie IDs.
//!
//! [`process_widget_message`] centralises all of this so the two modes
//! share one implementation.

use std::collections::HashMap;

use iced::widget::pane_grid;

use plushie_ext::PlushieRenderer;
use plushie_ext::extensions::{EventResult, ExtensionDispatcher};
use plushie_ext::message::Message;
use plushie_ext::protocol::OutgoingEvent;
use plushie_ext::widgets::WidgetCaches;

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
    caches: &mut WidgetCaches<R>,
    dispatcher: &mut ExtensionDispatcher<R>,
    registry: &mut plushie_ext::registry::WidgetRegistry<R>,
    last_slide_values: &mut HashMap<String, f64>,
) -> Vec<OutgoingEvent> {
    // Try registry dispatch first. If the factory handles the message
    // (returns Some), use that result. Otherwise fall through to the
    // legacy dispatch below.
    if let Some(node_id) = msg.node_id() {
        if let Some((idx, _matched_id)) = registry.get_for_node_id(node_id) {
            if let Some(factory) = registry.get_mut(idx) {
                if let Some(events) = factory.handle_message(&msg) {
                    return events;
                }
            }
        }
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

        // Focus transition produces up to 2 events (blur old + focus new).
        Message::CanvasElementFocusChanged {
            window_id,
            canvas_id,
            old_element_id,
            new_element_id,
        } => {
            let mut events = Vec::with_capacity(2);
            if let Some(old_id) = old_element_id {
                events.push(
                    OutgoingEvent::canvas_element_blurred(canvas_id.clone(), old_id.clone())
                        .with_window_id(window_id.clone()),
                );
            }
            if let Some(new_id) = new_element_id {
                events.push(
                    OutgoingEvent::canvas_element_focused(canvas_id.clone(), new_id.clone())
                        .with_window_id(window_id.clone()),
                );
            }
            events
        }

        // Slider -- needs value tracking for SlideRelease.
        Message::Slide(window_id, id, value) => {
            last_slide_values.insert(id.clone(), value);
            vec![OutgoingEvent::slide(id, value).with_window_id(window_id)]
        }
        Message::SlideRelease(window_id, id) => {
            let value = last_slide_values.remove(&id).unwrap_or(0.0);
            vec![OutgoingEvent::slide_release(id, value).with_window_id(window_id)]
        }

        // Text editor -- apply action to content, emit new text.
        Message::TextEditorAction(window_id, id, action) => {
            if let Some(content) = caches.editor_content_mut(&id) {
                let is_edit = action.is_edit();
                content.perform(action);

                if is_edit {
                    let new_text = content.text();
                    // Update the prop hash to match the mutated Content so
                    // ensure_text_editor_cache doesn't reset on the host's
                    // lagging prop.
                    caches.update_editor_content_hash(&id, &new_text);
                    return vec![OutgoingEvent::input(id, new_text).with_window_id(window_id)];
                }
            }
            vec![]
        }

        // Extension events -- route through dispatcher.
        Message::Event {
            ref window_id,
            ref id,
            ref data,
            ref family,
        } => {
            let result = dispatcher.handle_event(id, family, data, &mut caches.extension);
            let data_opt = if data.is_null() {
                None
            } else {
                Some(data.clone())
            };
            match result {
                EventResult::PassThrough => vec![
                    OutgoingEvent::generic(family.clone(), id.clone(), data_opt)
                        .with_window_id(window_id.clone()),
                ],
                EventResult::Consumed(ext_events) => ext_events
                    .into_iter()
                    .map(|event| event.with_window_id(window_id.clone()))
                    .collect(),
                EventResult::Observed(ext_events) => {
                    let mut events = vec![
                        OutgoingEvent::generic(family.clone(), id.clone(), data_opt)
                            .with_window_id(window_id.clone()),
                    ];
                    events.extend(
                        ext_events
                            .into_iter()
                            .map(|event| event.with_window_id(window_id.clone())),
                    );
                    events
                }
            }
        }

        // Pane grid events -- need pane state lookup.
        Message::PaneFocusCycle(ref window_id, ref grid_id, pane) => {
            if let Some(state) = caches.pane_grid_state(grid_id) {
                let pane_id = state.get(pane).cloned().unwrap_or_default();
                vec![
                    OutgoingEvent::pane_focus_cycle(grid_id.clone(), pane_id)
                        .with_window_id(window_id.clone()),
                ]
            } else {
                vec![]
            }
        }
        Message::PaneResized(ref window_id, ref grid_id, ref evt) => {
            if let Some(state) = caches.pane_grid_state_mut(grid_id) {
                state.resize(evt.split, evt.ratio);
            }
            vec![
                OutgoingEvent::pane_resized(grid_id.clone(), format!("{:?}", evt.split), evt.ratio)
                    .with_window_id(window_id.clone()),
            ]
        }
        Message::PaneDragged(ref window_id, ref grid_id, ref evt) => {
            process_pane_drag(window_id, grid_id, evt, caches)
        }
        Message::PaneClicked(ref window_id, ref grid_id, pane) => {
            if let Some(state) = caches.pane_grid_state(grid_id) {
                let pane_id = state.get(pane).cloned().unwrap_or_default();
                vec![
                    OutgoingEvent::pane_clicked(grid_id.clone(), pane_id)
                        .with_window_id(window_id.clone()),
                ]
            } else {
                vec![]
            }
        }

        // Everything else (subscription events, NoOp, MarkdownUrl, StatusChanged, etc.)
        // produces no outgoing events.
        _ => vec![],
    }
}

/// Process a pane grid drag event into outgoing events.
fn process_pane_drag<R: PlushieRenderer>(
    window_id: &str,
    grid_id: &str,
    evt: &pane_grid::DragEvent,
    caches: &mut WidgetCaches<R>,
) -> Vec<OutgoingEvent> {
    match evt {
        pane_grid::DragEvent::Picked { pane } => {
            if let Some(state) = caches.pane_grid_state(grid_id) {
                let pane_id = state.get(*pane).cloned().unwrap_or_default();
                vec![
                    OutgoingEvent::pane_dragged(
                        grid_id.to_string(),
                        "picked",
                        pane_id,
                        None,
                        None,
                        None,
                    )
                    .with_window_id(window_id.to_string()),
                ]
            } else {
                vec![]
            }
        }
        pane_grid::DragEvent::Dropped { pane, target } => {
            if let Some(state) = caches.pane_grid_state_mut(grid_id) {
                let pane_id = state.get(*pane).cloned().unwrap_or_default();
                let (target_pane, region, edge) = match target {
                    pane_grid::Target::Edge(e) => {
                        let edge_str = match e {
                            pane_grid::Edge::Top => "top",
                            pane_grid::Edge::Bottom => "bottom",
                            pane_grid::Edge::Left => "left",
                            pane_grid::Edge::Right => "right",
                        };
                        (None, None, Some(edge_str))
                    }
                    pane_grid::Target::Pane(p, region) => {
                        let target_id = state.get(*p).cloned().unwrap_or_default();
                        let region_str = match region {
                            pane_grid::Region::Center => "center",
                            pane_grid::Region::Edge(pane_grid::Edge::Top) => "top",
                            pane_grid::Region::Edge(pane_grid::Edge::Bottom) => "bottom",
                            pane_grid::Region::Edge(pane_grid::Edge::Left) => "left",
                            pane_grid::Region::Edge(pane_grid::Edge::Right) => "right",
                        };
                        (Some(target_id), Some(region_str), None)
                    }
                };
                state.drop(*pane, *target);
                vec![
                    OutgoingEvent::pane_dragged(
                        grid_id.to_string(),
                        "dropped",
                        pane_id,
                        target_pane,
                        region,
                        edge,
                    )
                    .with_window_id(window_id.to_string()),
                ]
            } else {
                vec![]
            }
        }
        pane_grid::DragEvent::Canceled { pane } => {
            if let Some(state) = caches.pane_grid_state(grid_id) {
                let pane_id = state.get(*pane).cloned().unwrap_or_default();
                vec![
                    OutgoingEvent::pane_dragged(
                        grid_id.to_string(),
                        "canceled",
                        pane_id,
                        None,
                        None,
                        None,
                    )
                    .with_window_id(window_id.to_string()),
                ]
            } else {
                vec![]
            }
        }
    }
}
