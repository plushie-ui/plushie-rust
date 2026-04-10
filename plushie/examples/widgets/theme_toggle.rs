//! Animated theme toggle widget.
//!
//! A toggle switch that emits "toggled" events. Uses internal state
//! for smooth animation of the thumb position.
//!
//! This is a widget example, not a standalone app. It's used by
//! other examples (like rate_plushie) as a reusable component.

// This module demonstrates the Widget trait pattern. In a real app,
// this would be in a separate crate or module, used via
// `ThemeToggle::widget("id").prop(...)`.
//
// Since Rust examples are single files, this serves as reference
// documentation for the composite widget pattern.

use plushie::prelude::*;
use plushie::widget::{Widget, EventResult};
use serde_json::Value;

pub struct ThemeToggle;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToggleState {
    pub progress: f64,
    pub target: f64,
}

impl Widget for ThemeToggle {
    type State = ToggleState;

    fn view(id: &str, props: &Value, state: &Self::State) -> View {
        let is_dark = state.target >= 0.5;
        let label = if is_dark { "Dark" } else { "Light" };

        row().id(id).spacing(8.0)
            .child(button("switch", label)
                .style(if is_dark { Style::primary() } else { Style::secondary() }))
            .into()
    }

    fn handle_event(event: &Event, state: &mut ToggleState) -> EventResult {
        match event.widget_match() {
            Some(Click("switch")) => {
                state.target = if state.target >= 0.5 { 0.0 } else { 1.0 };
                EventResult::emit("toggled", state.target >= 0.5)
            }
            _ => EventResult::Consumed,
        }
    }
}
