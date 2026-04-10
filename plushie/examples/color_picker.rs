//! HSV color picker using a composite widget.
//!
//! Demonstrates the Widget trait: the HSV slider controls live inside
//! a `ColorPickerWidget` that owns its own state and emits a single
//! "change" event with the current HSV values. The app only sees the
//! high-level event; it never touches the sliders directly.
//!
//! Run with: `cargo run -p plushie --example color_picker`

use serde_json::Value;

use plushie::prelude::*;
use plushie::widget::{Widget, WidgetView, EventResult};

// ---------------------------------------------------------------------------
// ColorPickerWidget (composite widget)
// ---------------------------------------------------------------------------

struct ColorPickerWidget;

#[derive(Default)]
struct PickerState {
    hue: f64,
    saturation: f64,
    value: f64,
}

impl Widget for ColorPickerWidget {
    type State = PickerState;

    fn view(id: &str, _props: &Value, state: &Self::State) -> View {
        column().id(id).spacing(8.0)
            .child(
                slider("hue", (0.0, 360.0), state.hue as f32)
                    .step(1.0)
                    .label("Hue"),
            )
            .child(
                slider("sat", (0.0, 100.0), (state.saturation * 100.0) as f32)
                    .step(1.0)
                    .label("Saturation"),
            )
            .child(
                slider("val", (0.0, 100.0), (state.value * 100.0) as f32)
                    .step(1.0)
                    .label("Value"),
            )
            .into()
    }

    fn handle_event(event: &Event, state: &mut Self::State) -> EventResult {
        match event.widget_match() {
            Some(Slide("hue", h)) => {
                state.hue = h;
                emit_change(state)
            }
            Some(Slide("sat", s)) => {
                state.saturation = s / 100.0;
                emit_change(state)
            }
            Some(Slide("val", v)) => {
                state.value = v / 100.0;
                emit_change(state)
            }
            _ => EventResult::Ignored,
        }
    }
}

fn emit_change(state: &PickerState) -> EventResult {
    EventResult::emit(
        "change",
        serde_json::json!({
            "hue": state.hue,
            "saturation": state.saturation,
            "value": state.value,
        }),
    )
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

struct ColorPickerApp {
    hue: f64,
    saturation: f64,
    value: f64,
}

impl App for ColorPickerApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (
            ColorPickerApp {
                hue: 0.0,
                saturation: 1.0,
                value: 1.0,
            },
            Command::none(),
        )
    }

    fn update(model: &mut Self, event: Event) -> Command {
        // The widget emits "change" which maps to EventType::Other(0)
        // since "change" isn't a built-in family. Use as_widget() to
        // read the structured value.
        if let Some(w) = event.as_widget() {
            if w.id == "picker" {
                if let Some(obj) = w.value.as_object() {
                    model.hue = obj.get("hue").and_then(|v| v.as_f64()).unwrap_or(model.hue);
                    model.saturation = obj.get("saturation").and_then(|v| v.as_f64()).unwrap_or(model.saturation);
                    model.value = obj.get("value").and_then(|v| v.as_f64()).unwrap_or(model.value);
                }
            }
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        let hex = hsv_to_hex(model.hue, model.saturation, model.value);
        let is_initial =
            model.hue == 0.0 && model.saturation == 1.0 && model.value == 1.0;

        window("color_picker").title("Color Picker").child(
            column().spacing(16.0).padding(20).align_x(Align::Center)
                .child(WidgetView::<ColorPickerWidget>::new("picker"))
                .child(
                    row().spacing(16.0).align_y(Align::Center)
                        .child(
                            container()
                                .id("swatch")
                                .width(48.0)
                                .height(48.0)
                                .background(Color::hex(&hex))
                                .border(
                                    Border::new()
                                        .color("#cccccc")
                                        .width(1.0)
                                        .radius(4.0),
                                )
                                .a11y(&serde_json::json!({
                                    "role": "image",
                                    "label": format!("Selected color: {hex}")
                                })),
                        )
                        .child(
                            column().spacing(4.0)
                                .child(
                                    text(&hex)
                                        .id("hex_display")
                                        .size(18.0)
                                        .a11y(&serde_json::json!({
                                            "live": "polite",
                                            "busy": is_initial
                                        })),
                                )
                                .child(
                                    text(&hsv_label(model))
                                        .id("hsv_display")
                                        .a11y(&serde_json::json!({
                                            "live": "polite"
                                        })),
                                ),
                        ),
                ),
        )
        .into()
    }
}

fn hsv_label(model: &ColorPickerApp) -> String {
    let h_int = model.hue.round() as i64;
    let s_pct = (model.saturation * 100.0).round() as i64;
    let v_pct = (model.value * 100.0).round() as i64;
    format!("H: {h_int}  S: {s_pct}%  V: {v_pct}%")
}

fn hsv_to_hex(h: f64, s: f64, v: f64) -> String {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let c = v * s;
    let h_sector = h / 60.0;
    let x = c * (1.0 - ((h_sector % 2.0) - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h_sector < 1.0 {
        (c, x, 0.0)
    } else if h_sector < 2.0 {
        (x, c, 0.0)
    } else if h_sector < 3.0 {
        (0.0, c, x)
    } else if h_sector < 4.0 {
        (0.0, x, c)
    } else if h_sector < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let r = ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn main() -> plushie::Result {
    plushie::run::<ColorPickerApp>()
}
