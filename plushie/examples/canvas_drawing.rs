//! Canvas drawing example.
//!
//! Demonstrates the canvas shape builders: rect, circle, line,
//! path, groups, and layers.
//!
//! Run with: `cargo run -p plushie --example canvas_drawing`

use plushie::prelude::*;
use plushie::ui::{canvas, layer, group, rect, circle, line, canvas_text};

struct CanvasApp;

impl App for CanvasApp {
    type Model = Self;

    fn init() -> (Self, Command) {
        (CanvasApp, Command::none())
    }

    fn update(_model: &mut Self, _event: Event) -> Command {
        Command::none()
    }

    fn view(_model: &Self) -> View {
        window("main").title("Canvas Drawing").child(
            canvas("drawing")
                .width(800.0)
                .height(400.0)
                .background(Color::hex("#1a1a2e"))
                .child(layer("shapes")
                    .child(rect(50.0, 50.0, 200.0, 100.0)
                        .fill(Color::hex("#e94560"))
                        .radius(8.0))
                    .child(circle(350.0, 100.0, 60.0)
                        .fill(Color::hex("#0f3460"))
                        .stroke(Color::hex("#e94560"))
                        .stroke_width(3.0))
                    .child(line(100.0, 200.0, 400.0, 300.0)
                        .stroke(Color::hex("#16213e"))
                        .stroke_width(2.0))
                    .child(canvas_text(50.0, 350.0, "Hello Canvas!")
                        .size(24.0)
                        .color(Color::white()))
                )
        ).into()
    }
}

fn main() -> plushie::Result {
    plushie::run::<CanvasApp>()
}
