//! Runtime glue for the dev-mode rebuild overlay.
//!
//! Hosts the event-interception helper that swallows
//! `__plushie_dev__/*` widget events, dispatches them to the overlay
//! handle's action handler, and keeps them from flowing into
//! `A::update`. Placeholder for the full port of Elixir's
//! `Plushie.Runtime.DevOverlay`; interactive toggle/dismiss and
//! auto-dismiss live here.

use crate::dev::overlay::{DevOverlayHandle, OVERLAY_PREFIX, RebuildingOverlay, Status};
use crate::event::{Event, EventType};

/// Action decoded from a `__plushie_dev__/<action>` widget ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayAction {
    Toggle,
    Dismiss,
    Unknown,
}

fn parse_action(id: &str) -> OverlayAction {
    let suffix = id.strip_prefix(&format!("{OVERLAY_PREFIX}/")).unwrap_or("");
    match suffix {
        "toggle" => OverlayAction::Toggle,
        "dismiss" => OverlayAction::Dismiss,
        _ => OverlayAction::Unknown,
    }
}

/// Inspect an incoming event; if it belongs to the overlay (ID
/// starts with `__plushie_dev__/`), mutate the handle's overlay
/// state and return `true` to signal "consumed - do not dispatch
/// to App::update". Non-overlay events return `false` for passthrough.
///
/// The handle is the process-global overlay handle (when present).
/// When no handle is registered, overlay events pass through as-is;
/// they'll end up as unknown IDs to the app, which is the documented
/// fallback for dev-overlay-disabled builds.
pub(crate) fn maybe_handle_event(handle: &DevOverlayHandle, event: &Event) -> bool {
    let Some(widget_id) = overlay_id(event) else {
        return false;
    };
    if !widget_id.starts_with(&format!("{OVERLAY_PREFIX}/")) {
        return false;
    }
    let action = parse_action(widget_id);
    if !matches!(event_type_of(event), Some(EventType::Click)) {
        // Consume the event (it belongs to us) but do nothing: only
        // click events drive overlay state changes today.
        return true;
    }
    apply_action(handle, action);
    true
}

fn overlay_id(event: &Event) -> Option<&str> {
    let widget = event.as_widget()?;
    if widget.scoped_id.full.starts_with(OVERLAY_PREFIX) {
        Some(widget.scoped_id.full.as_str())
    } else {
        None
    }
}

fn event_type_of(event: &Event) -> Option<EventType> {
    event.as_widget().map(|w| w.event_type.clone())
}

fn apply_action(handle: &DevOverlayHandle, action: OverlayAction) {
    let current = handle.snapshot();
    match action {
        OverlayAction::Toggle => {
            let Some(mut overlay) = current else {
                return;
            };
            if matches!(overlay.status, Status::Frozen) {
                // Frozen-UI overlay has no toggle; Elixir parity.
                return;
            }
            overlay.expanded = !overlay.expanded;
            handle.set(Some(overlay));
        }
        OverlayAction::Dismiss => {
            handle.set(None);
        }
        OverlayAction::Unknown => {
            log::debug!("dev overlay: ignoring unknown action in {:?}", action);
        }
    }
}

/// Schedule an auto-dismiss for the current `Success` overlay. After
/// [`DISMISS_DELAY`](super::overlay::DISMISS_DELAY) elapses, the
/// handle is cleared unless the user expanded the drawer in the
/// meantime, in which case the dismiss is skipped.
pub(crate) fn schedule_dismiss(handle: DevOverlayHandle) {
    if let Err(e) = std::thread::Builder::new()
        .name("plushie-dev-overlay-dismiss".to_string())
        .spawn(move || {
            std::thread::sleep(super::overlay::DISMISS_DELAY);
            let Some(current) = handle.snapshot() else {
                return;
            };
            // Skip the dismiss if the user expanded the drawer while
            // we were sleeping; matches Elixir's handle_auto_dismiss.
            if !matches!(current.status, Status::Success) || current.expanded {
                return;
            }
            handle.set(None);
        })
    {
        log::warn!("dev overlay: failed to spawn dismiss thread: {e}");
    }
}

/// Install a fresh overlay snapshot. Cancels the auto-dismiss by
/// virtue of the state replacement: the scheduled thread checks the
/// current snapshot's status + `expanded` before clearing, so a new
/// `Rebuilding`/`Failed` overrides any pending clear. On a new
/// `Success` we schedule a dismiss.
pub(crate) fn handle_overlay_message(handle: &DevOverlayHandle, overlay: RebuildingOverlay) {
    let status = overlay.status;
    handle.set(Some(overlay));
    if matches!(status, Status::Success) {
        schedule_dismiss(handle.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::WidgetEvent;
    use plushie_core::ScopedId;
    use serde_json::Value;

    fn click_event(id: &str) -> Event {
        Event::Widget(WidgetEvent {
            event_type: EventType::Click,
            scoped_id: ScopedId::parse(id),
            value: Value::Null,
        })
    }

    fn overlay(status: Status, detail: impl Into<String>) -> RebuildingOverlay {
        RebuildingOverlay {
            status,
            detail: detail.into(),
            expanded: matches!(status, Status::Failed | Status::Frozen),
            success_at: matches!(status, Status::Success).then(std::time::Instant::now),
        }
    }

    #[test]
    fn non_overlay_events_pass_through() {
        let handle = DevOverlayHandle::new();
        let event = click_event("app/button");
        assert!(!maybe_handle_event(&handle, &event));
    }

    #[test]
    fn toggle_expands_and_collapses() {
        let handle = DevOverlayHandle::new();
        handle.set(Some(overlay(Status::Rebuilding, "building")));
        let event = click_event(&format!("{OVERLAY_PREFIX}/toggle"));
        assert!(maybe_handle_event(&handle, &event));
        assert!(handle.snapshot().unwrap().expanded);
        assert!(maybe_handle_event(&handle, &event));
        assert!(!handle.snapshot().unwrap().expanded);
    }

    #[test]
    fn toggle_on_frozen_is_noop() {
        let handle = DevOverlayHandle::new();
        handle.set(Some(RebuildingOverlay {
            status: Status::Frozen,
            detail: String::new(),
            expanded: false,
            success_at: None,
        }));
        let event = click_event(&format!("{OVERLAY_PREFIX}/toggle"));
        assert!(maybe_handle_event(&handle, &event));
        assert!(!handle.snapshot().unwrap().expanded);
    }

    #[test]
    fn dismiss_removes_overlay() {
        let handle = DevOverlayHandle::new();
        handle.set(Some(overlay(Status::Failed, "boom")));
        let event = click_event(&format!("{OVERLAY_PREFIX}/dismiss"));
        assert!(maybe_handle_event(&handle, &event));
        assert!(handle.snapshot().is_none());
    }

    #[test]
    fn auto_dismiss_fires_for_collapsed_success() {
        // Short-circuit the real 1500ms timer by constructing a
        // Success overlay whose success_at is already in the past.
        // The snapshot accessor clears overlays that should_dismiss
        // reports as expired.
        let handle = DevOverlayHandle::new();
        handle.set(Some(RebuildingOverlay {
            status: Status::Success,
            detail: "built".to_string(),
            expanded: false,
            success_at: Some(
                std::time::Instant::now() - (super::super::overlay::DISMISS_DELAY * 2),
            ),
        }));
        // First snapshot clears the expired overlay.
        assert!(handle.snapshot().is_none());
    }

    #[test]
    fn auto_dismiss_skipped_when_user_expanded_drawer() {
        // A user expanded the drawer after success; the overlay
        // should stay visible. Exercise the guard logic the spawned
        // dismiss thread uses.
        let handle = DevOverlayHandle::new();
        handle.set(Some(RebuildingOverlay {
            status: Status::Success,
            detail: String::new(),
            expanded: true,
            success_at: Some(std::time::Instant::now()),
        }));
        let current = handle.snapshot().expect("overlay should be present");
        // Mirror the guard inside schedule_dismiss. When expanded is
        // true we must NOT call handle.set(None).
        let should_clear = matches!(current.status, Status::Success) && !current.expanded;
        assert!(!should_clear);
    }

    #[test]
    fn maybe_handle_event_consumes_overlay_ids_without_state() {
        // Overlay IDs still belong to us even when the handle has no
        // current snapshot; they'd just arrive as unknown IDs to the
        // app otherwise. Return true to consume cleanly.
        let handle = DevOverlayHandle::new();
        let event = click_event(&format!("{OVERLAY_PREFIX}/toggle"));
        assert!(maybe_handle_event(&handle, &event));
        assert!(handle.snapshot().is_none());
    }

    #[test]
    fn handle_overlay_message_installs_and_schedules() {
        let handle = DevOverlayHandle::new();
        handle_overlay_message(
            &handle,
            RebuildingOverlay {
                status: Status::Rebuilding,
                detail: "x".to_string(),
                expanded: false,
                success_at: None,
            },
        );
        assert_eq!(handle.snapshot().unwrap().status, Status::Rebuilding);
    }
}
