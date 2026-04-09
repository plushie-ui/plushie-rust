//! Thin wrapper factories for all 36 built-in widget types.
//!
//! Each wrapper implements [`PlushieWidget`] by delegating to the existing
//! `render_*` functions. These are transitional: as each widget is fully
//! extracted into a proper `PlushieWidget` impl with owned state, the
//! corresponding wrapper is removed.
//!
//! The [`iced_widget_set`] function returns a [`WidgetSet`] containing all
//! built-in wrappers, suitable for registering as the default set.

use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::extensions::RenderCtx;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::{PlushieWidget, WidgetSet};
use crate::widgets::a11y::A11yOverrides;

use super::{canvas, display, input, interactive, layout, table};

// ---------------------------------------------------------------------------
// Macro: generate a zero-sized wrapper struct + PlushieWidget impl
// ---------------------------------------------------------------------------

/// Generate a zero-sized struct that delegates `render()` to an existing
/// `render_*` function. For widgets with a11y auto-inference, an optional
/// `infer_a11y` closure can be provided.
macro_rules! builtin_widget {
    // Basic: no a11y inference
    ($struct_name:ident, $names:expr, $render_fn:path) => {
        pub(crate) struct $struct_name;

        impl<R: PlushieRenderer> PlushieWidget<R> for $struct_name {
            fn type_names(&self) -> &[&str] {
                &$names
            }

            fn render<'a>(
                &self,
                node: &'a TreeNode,
                ctx: &RenderCtx<'a, R>,
            ) -> Element<'a, Message, Theme, R> {
                $render_fn(node, *ctx)
            }

            fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
                Box::new($struct_name)
            }
        }
    };

    // With a11y auto-inference
    ($struct_name:ident, $names:expr, $render_fn:path, infer_a11y: $a11y_fn:expr) => {
        pub(crate) struct $struct_name;

        impl<R: PlushieRenderer> PlushieWidget<R> for $struct_name {
            fn type_names(&self) -> &[&str] {
                &$names
            }

            fn render<'a>(
                &self,
                node: &'a TreeNode,
                ctx: &RenderCtx<'a, R>,
            ) -> Element<'a, Message, Theme, R> {
                $render_fn(node, *ctx)
            }

            fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
                let infer: fn(&TreeNode) -> Option<A11yOverrides> = $a11y_fn;
                infer(node)
            }

            fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
                Box::new($struct_name)
            }
        }
    };
}

// ---------------------------------------------------------------------------
// A11y inference helpers
// ---------------------------------------------------------------------------

fn infer_placeholder_as_description(node: &TreeNode) -> Option<A11yOverrides> {
    let props = node.props.as_object();
    crate::prop_helpers::prop_str(props, "placeholder").map(A11yOverrides::with_description)
}

// ---------------------------------------------------------------------------
// Layout widgets (11)
// ---------------------------------------------------------------------------

builtin_widget!(ColumnWidget,          ["column"],       layout::render_column);
builtin_widget!(RowWidget,             ["row"],          layout::render_row);
builtin_widget!(ContainerWidget,       ["container"],    layout::render_container);
builtin_widget!(StackWidget,           ["stack"],        layout::render_stack);
builtin_widget!(GridWidget,            ["grid"],         layout::render_grid);
builtin_widget!(PinWidget,             ["pin"],          layout::render_pin);
builtin_widget!(KeyedColumnWidget,     ["keyed_column"], layout::render_keyed_column);
builtin_widget!(FloatWidget,           ["float"],        layout::render_float);
builtin_widget!(ResponsiveWidget,      ["responsive"],   layout::render_responsive);
builtin_widget!(ScrollableWidget,      ["scrollable"],   layout::render_scrollable);
builtin_widget!(PaneGridWidget,        ["pane_grid"],    layout::render_pane_grid);

// ---------------------------------------------------------------------------
// Display widgets (9, counting rich_text alias)
// ---------------------------------------------------------------------------

builtin_widget!(TextWidget,            ["text"],         display::render_text);
builtin_widget!(RichTextWidget,        ["rich_text", "rich"], display::render_rich_text);
builtin_widget!(SpaceWidget,           ["space"],        display::render_space);
builtin_widget!(RuleWidget,            ["rule"],         display::render_rule);
builtin_widget!(ProgressBarWidget,     ["progress_bar"], display::render_progress_bar);
builtin_widget!(ImageWidget,           ["image"],        display::render_image);
builtin_widget!(SvgWidget,             ["svg"],          display::render_svg);
builtin_widget!(MarkdownWidget,        ["markdown"],     display::render_markdown);
builtin_widget!(QrCodeWidget,          ["qr_code"],      display::render_qr_code);

// ---------------------------------------------------------------------------
// Input widgets (9)
// ---------------------------------------------------------------------------

builtin_widget!(TextInputWidget,       ["text_input"],       input::render_text_input,
    infer_a11y: infer_placeholder_as_description);
builtin_widget!(TextEditorWidget,      ["text_editor"],      input::render_text_editor,
    infer_a11y: infer_placeholder_as_description);
builtin_widget!(CheckboxWidget,        ["checkbox"],         input::render_checkbox);
builtin_widget!(TogglerWidget,         ["toggler"],          input::render_toggler);
builtin_widget!(RadioWidget,           ["radio"],            input::render_radio);
builtin_widget!(SliderWidget,          ["slider"],           input::render_slider);
builtin_widget!(VerticalSliderWidget,  ["vertical_slider"],  input::render_vertical_slider);
builtin_widget!(PickListWidget,        ["pick_list"],        input::render_pick_list);
builtin_widget!(ComboBoxWidget,        ["combo_box"],        input::render_combo_box,
    infer_a11y: infer_placeholder_as_description);

// ---------------------------------------------------------------------------
// Interactive widgets (7)
// ---------------------------------------------------------------------------

builtin_widget!(ButtonWidget,          ["button"],       interactive::render_button);
builtin_widget!(PointerAreaWidget,     ["pointer_area"], interactive::render_mouse_area);
builtin_widget!(SensorWidget,          ["sensor"],       interactive::render_sensor);
builtin_widget!(TooltipWidget,         ["tooltip"],      interactive::render_tooltip);
builtin_widget!(ThemerWidget,          ["themer"],       interactive::render_themer);
builtin_widget!(WindowWidget,          ["window"],       interactive::render_window);
builtin_widget!(OverlayWidget,         ["overlay"],      interactive::render_overlay);

// ---------------------------------------------------------------------------
// Canvas (1)
// ---------------------------------------------------------------------------

builtin_widget!(CanvasWidget,          ["canvas"],       canvas::render_canvas);

// ---------------------------------------------------------------------------
// Table (1)
// ---------------------------------------------------------------------------

builtin_widget!(TableWidget,           ["table"],        table::render_table);

// ---------------------------------------------------------------------------
// IcedWidgetSet: the default set of all 36 built-in widgets
// ---------------------------------------------------------------------------

/// The default widget set providing all 36 built-in iced widget wrappers.
pub struct IcedWidgetSet;

impl<R: PlushieRenderer> WidgetSet<R> for IcedWidgetSet {
    fn name(&self) -> &str {
        "iced"
    }

    fn create_widgets(&self) -> Vec<Box<dyn PlushieWidget<R>>> {
        vec![
            // Layout
            Box::new(ColumnWidget),
            Box::new(RowWidget),
            Box::new(ContainerWidget),
            Box::new(StackWidget),
            Box::new(GridWidget),
            Box::new(PinWidget),
            Box::new(KeyedColumnWidget),
            Box::new(FloatWidget),
            Box::new(ResponsiveWidget),
            Box::new(ScrollableWidget),
            Box::new(PaneGridWidget),
            // Display
            Box::new(TextWidget),
            Box::new(RichTextWidget),
            Box::new(SpaceWidget),
            Box::new(RuleWidget),
            Box::new(ProgressBarWidget),
            Box::new(ImageWidget),
            Box::new(SvgWidget),
            Box::new(MarkdownWidget),
            Box::new(QrCodeWidget),
            // Input
            Box::new(TextInputWidget),
            Box::new(TextEditorWidget),
            Box::new(CheckboxWidget),
            Box::new(TogglerWidget),
            Box::new(RadioWidget),
            Box::new(SliderWidget),
            Box::new(VerticalSliderWidget),
            Box::new(PickListWidget),
            Box::new(ComboBoxWidget),
            // Interactive
            Box::new(ButtonWidget),
            Box::new(PointerAreaWidget),
            Box::new(SensorWidget),
            Box::new(TooltipWidget),
            Box::new(ThemerWidget),
            Box::new(WindowWidget),
            Box::new(OverlayWidget),
            // Canvas
            Box::new(CanvasWidget),
            // Table
            Box::new(TableWidget),
        ]
    }
}

/// Create the default iced widget set. Convenience for builder registration.
pub fn iced_widget_set() -> IcedWidgetSet {
    IcedWidgetSet
}
