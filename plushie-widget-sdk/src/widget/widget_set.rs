//! The default iced widget set.

use crate::PlushieRenderer;
use crate::registry::{PlushieWidget, WidgetSet};

use super::*;

/// The default widget set providing all built-in iced widget wrappers.
pub struct IcedWidgetSet;

impl<R: PlushieRenderer> WidgetSet<R> for IcedWidgetSet {
    fn name(&self) -> &str {
        "iced"
    }

    fn create_widgets(&self) -> Vec<Box<dyn PlushieWidget<R>>> {
        vec![
            // Layout
            Box::new(column_widget::ColumnWidget),
            Box::new(row_widget::RowWidget),
            Box::new(container_widget::ContainerWidget),
            Box::new(stack_widget::StackWidget),
            Box::new(grid_widget::GridWidget),
            Box::new(pin_widget::PinWidget),
            Box::new(keyed_column_widget::KeyedColumnWidget),
            Box::new(float_widget::FloatWidget),
            Box::new(responsive_widget::ResponsiveWidget),
            Box::new(scrollable_widget::ScrollableWidget),
            Box::new(pane_grid_widget::PaneGridWidget::new()),
            // Display
            Box::new(text_widget::TextWidget),
            Box::new(rich_text_widget::RichTextWidget),
            Box::new(space_widget::SpaceWidget),
            Box::new(rule_widget::RuleWidget),
            Box::new(progress_bar_widget::ProgressBarWidget),
            Box::new(image_widget::ImageWidget),
            Box::new(svg_widget::SvgWidget),
            Box::new(markdown_widget::MarkdownWidget::new()),
            Box::new(qr_code_widget::QrCodeWidget::new()),
            // Input
            Box::new(text_input_widget::TextInputWidget),
            Box::new(text_editor_widget::TextEditorWidget::new()),
            Box::new(checkbox_widget::CheckboxWidget),
            Box::new(toggler_widget::TogglerWidget),
            Box::new(radio_widget::RadioWidget),
            Box::new(slider_widget::SliderWidget::new()),
            Box::new(slider_widget::VerticalSliderWidget::new()),
            Box::new(pick_list_widget::PickListWidget),
            Box::new(combo_box_widget::ComboBoxWidget::new()),
            // Interactive
            Box::new(button_widget::ButtonWidget),
            Box::new(pointer_area_widget::PointerAreaWidget),
            Box::new(sensor_widget::SensorWidget),
            Box::new(tooltip_widget::TooltipWidget),
            Box::new(themer_widget::ThemerWidget::new()),
            Box::new(window_widget::WindowWidget),
            Box::new(overlay_widget::OverlayWidget),
            // Canvas
            Box::new(canvas_widget::CanvasWidget::new()),
            // Table
            Box::new(table_widget::TableWidget),
        ]
    }
}

impl IcedWidgetSet {
    /// The complete list of built-in widget type names.
    ///
    /// This is the canonical source of truth for which types the iced
    /// widget set provides. Matches the widgets registered by
    /// `create_widgets()`.
    pub fn type_names() -> &'static [&'static str] {
        &[
            "column",
            "row",
            "container",
            "stack",
            "grid",
            "pin",
            "keyed_column",
            "float",
            "responsive",
            "scrollable",
            "pane_grid",
            "text",
            "rich_text",
            "rich",
            "space",
            "rule",
            "progress_bar",
            "image",
            "svg",
            "markdown",
            "qr_code",
            "text_input",
            "text_editor",
            "checkbox",
            "toggler",
            "radio",
            "slider",
            "vertical_slider",
            "pick_list",
            "combo_box",
            "button",
            "pointer_area",
            "sensor",
            "tooltip",
            "themer",
            "window",
            "overlay",
            "canvas",
            "table",
        ]
    }
}

/// Create the default iced widget set. Convenience for builder registration.
pub fn iced_widget_set() -> IcedWidgetSet {
    IcedWidgetSet
}
