//! Direct mode runner: in-process rendering via iced.
//!
//! Embeds the plushie renderer directly in the application binary.
//! The user's `App::view()` produces a `View` which is converted to
//! a TreeNode, rendered through the WidgetRegistry, and displayed by
//! iced. Events from iced are converted to SDK `Event` types and
//! delivered to `App::update()`.

use plushie_renderer_lib::App as RendererApp;
use plushie_widget_sdk::widget::widget_set::iced_widget_set;

use crate::App;

/// Run the app in direct mode.
///
/// Creates a WidgetRegistry with all built-in widgets, initializes
/// the user's App, and starts an iced daemon that renders the view
/// tree in-process.
pub fn run<A: App>() -> crate::Result {
    // Build the widget registry with built-in widgets.
    let builder = plushie_widget_sdk::app::PlushieAppBuilder::<plushie_widget_sdk::iced::Renderer>::new()
        .widget_set(&iced_widget_set());
    let registry = builder.build();

    // TODO: Bridge App::init/update/view with iced daemon lifecycle.
    // This requires:
    // 1. Calling A::init() to get the initial model
    // 2. Wrapping the model + registry in an iced App
    // 3. Converting between SDK types and renderer types
    //
    // For now, verify the infrastructure compiles and the registry
    // can be constructed.
    let _ = registry;
    let _ = std::any::type_name::<A>();

    // Placeholder: start an empty iced daemon to verify the
    // rendering pipeline works end-to-end.
    log::info!("plushie direct mode: registry built with {} widgets",
        plushie_widget_sdk::widget::widget_set::IcedWidgetSet::type_names().len());

    Err("Direct mode runner is under construction".into())
}
