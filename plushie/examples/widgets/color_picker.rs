//! HSV color picker widget.
//!
//! A reusable canvas-based color picker with hue, saturation, and
//! value controls. Emits "change" events with the current HSV values.
//!
//! This is a widget example demonstrating the composite Widget
//! trait pattern for building reusable interactive components.

use plushie::prelude::*;
use plushie::widget::{Widget, EventResult};
use serde_json::Value;

pub struct ColorPickerWidget;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct PickerState {
    pub hue: f64,
    pub saturation: f64,
    pub value: f64,
}

impl Widget for ColorPickerWidget {
    type State = PickerState;

    fn view(id: &str, _props: &Value, state: &Self::State) -> View {
        column().id(id).spacing(8.0)
            .child(slider("hue", (0.0, 360.0), state.hue as f32).label("Hue"))
            .child(slider("sat", (0.0, 100.0), (state.saturation * 100.0) as f32).label("Saturation"))
            .child(slider("val", (0.0, 100.0), (state.value * 100.0) as f32).label("Value"))
            .into()
    }

    fn handle_event(event: &Event, state: &mut PickerState) -> EventResult {
        match event.widget_match() {
            Some(Slide("hue", h)) => {
                state.hue = h;
                EventResult::emit("change", serde_json::json!({
                    "hue": state.hue,
                    "saturation": state.saturation,
                    "value": state.value,
                }))
            }
            Some(Slide("sat", s)) => {
                state.saturation = s / 100.0;
                EventResult::emit("change", serde_json::json!({
                    "hue": state.hue,
                    "saturation": state.saturation,
                    "value": state.value,
                }))
            }
            Some(Slide("val", v)) => {
                state.value = v / 100.0;
                EventResult::emit("change", serde_json::json!({
                    "hue": state.hue,
                    "saturation": state.saturation,
                    "value": state.value,
                }))
            }
            _ => EventResult::Ignored,
        }
    }
}
