fn main() -> iced::Result {
    plushie_renderer::run(plushie_widget_sdk::app::PlushieAppBuilder::new())
}
