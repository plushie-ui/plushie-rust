//! HSV color picker with sliders.
//!
//! Matches the Elixir ColorPicker example's view layout: a color
//! swatch beside hex/HSV labels, with hue/saturation/value sliders
//! below. The Elixir version delegates interaction to a canvas-based
//! ColorPickerWidget; here we use three sliders since the Rust SDK
//! does not expand composite widgets in standalone examples.
//!
//! Run with: `cargo run -p plushie --example color_picker`

use plushie::prelude::*;

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
        match event.widget_match() {
            Some(Slide("hue", h)) => model.hue = h,
            Some(Slide("sat", s)) => model.saturation = s / 100.0,
            Some(Slide("val", v)) => model.value = v / 100.0,
            _ => {}
        }
        Command::none()
    }

    fn view(model: &Self) -> View {
        let hex = hsv_to_hex(model.hue, model.saturation, model.value);
        let is_initial =
            model.hue == 0.0 && model.saturation == 1.0 && model.value == 1.0;

        window("color_picker").title("Color Picker").child(
            column().spacing(16.0).padding(20).align_x(Align::Center)
                // Sliders (in place of the canvas ColorPickerWidget)
                .child(
                    slider("hue", (0.0, 360.0), model.hue as f32)
                        .step(1.0)
                        .label("Hue"),
                )
                .child(
                    slider("sat", (0.0, 100.0), (model.saturation * 100.0) as f32)
                        .step(1.0)
                        .label("Saturation"),
                )
                .child(
                    slider("val", (0.0, 100.0), (model.value * 100.0) as f32)
                        .step(1.0)
                        .label("Value"),
                )
                // Swatch + labels row
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
