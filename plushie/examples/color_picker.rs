//! HSV color picker using a custom canvas widget.
//!
//! The color picker is a reusable component that handles drag
//! interaction internally. The app receives "change" events with
//! the current HSV values.
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
        (ColorPickerApp {
            hue: 0.0,
            saturation: 1.0,
            value: 1.0,
        }, Command::none())
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

        window("color_picker").title("Color Picker").child(
            column().spacing(16.0).padding(20).align_x(Align::Center)
                .child(text("Color Picker").size(24.0))
                .child(container()
                    .width(200.0).height(200.0)
                    .background(Color::hex(&hex))
                    .border(Border::new().color("#cccccc").width(1.0).radius(8.0))
                )
                .child(column().spacing(8.0)
                    .child(row().spacing(8.0)
                        .child(text("Hue").width(80.0))
                        .child(slider("hue", (0.0, 360.0), model.hue as f32))
                        .child(text(&format!("{:.0}°", model.hue)).width(50.0))
                    )
                    .child(row().spacing(8.0)
                        .child(text("Saturation").width(80.0))
                        .child(slider("sat", (0.0, 100.0), (model.saturation * 100.0) as f32))
                        .child(text(&format!("{:.0}%", model.saturation * 100.0)).width(50.0))
                    )
                    .child(row().spacing(8.0)
                        .child(text("Value").width(80.0))
                        .child(slider("val", (0.0, 100.0), (model.value * 100.0) as f32))
                        .child(text(&format!("{:.0}%", model.value * 100.0)).width(50.0))
                    )
                )
                .child(text(&format!("Hex: {hex}")).size(16.0))
        ).into()
    }
}

fn hsv_to_hex(h: f64, s: f64, v: f64) -> String {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let r = ((r + m) * 255.0) as u8;
    let g = ((g + m) * 255.0) as u8;
    let b = ((b + m) * 255.0) as u8;
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn main() -> plushie::Result {
    plushie::run::<ColorPickerApp>()
}
