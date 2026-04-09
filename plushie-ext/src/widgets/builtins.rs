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
                &'a self,
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
                &'a self,
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
// MarkdownWidget: extracted stateful factory (owns parsed markdown items)
pub(crate) struct MarkdownWidget {
    /// Parsed markdown items per (window_id, node_id), with content hash
    /// for invalidation. Rebuilt when the "content" or "code_theme" prop changes.
    items: std::collections::HashMap<(String, String), (u64, Vec<iced::widget::markdown::Item>)>,
}

impl MarkdownWidget {
    const MAX_CONTENT: usize = 1_048_576; // 1 MB

    pub(crate) fn new() -> Self {
        Self {
            items: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for MarkdownWidget {
    fn type_names(&self) -> &[&str] {
        &["markdown"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        use crate::widgets::caches::hash_str;

        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        let mut content_str = crate::prop_helpers::prop_str(props, "content").unwrap_or_default();
        if content_str.len() > Self::MAX_CONTENT {
            log::warn!(
                "[id={}] markdown content ({} bytes) exceeds limit ({} bytes), truncating",
                node.id,
                content_str.len(),
                Self::MAX_CONTENT,
            );
            let mut end = Self::MAX_CONTENT;
            while !content_str.is_char_boundary(end) && end > 0 {
                end -= 1;
            }
            content_str.truncate(end);
        }
        let code_theme_str = crate::prop_helpers::prop_str(props, "code_theme").unwrap_or_default();
        let hash = hash_str(&format!("{content_str}\0{code_theme_str}"));

        if let Some((existing_hash, _)) = self.items.get(&key) {
            if *existing_hash == hash {
                return;
            }
        }

        let code_theme = match code_theme_str.as_str() {
            "base16_mocha" => Some(iced::highlighter::Theme::Base16Mocha),
            "base16_ocean" => Some(iced::highlighter::Theme::Base16Ocean),
            "base16_eighties" => Some(iced::highlighter::Theme::Base16Eighties),
            "solarized_dark" => Some(iced::highlighter::Theme::SolarizedDark),
            "inspired_github" => Some(iced::highlighter::Theme::InspiredGitHub),
            "" => None,
            other => {
                log::warn!("unknown code_theme {:?}, using default", other);
                None
            }
        };
        let items: Vec<_> = if let Some(theme) = code_theme {
            let mut md = iced::widget::markdown::Content::new().code_theme(theme);
            md.push_str(&content_str);
            md.items().to_vec()
        } else {
            iced::widget::markdown::parse(&content_str).collect()
        };
        self.items.insert(key, (hash, items));
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        use crate::prop_helpers::*;
        use crate::widgets::helpers::value_to_length_opt;

        let key = (ctx.window_id.to_string(), node.id.clone());
        let items = match self.items.get(&key) {
            Some((_hash, items)) => items.as_slice(),
            None => {
                log::warn!("markdown cache miss for id={}", node.id);
                return iced::widget::text("(markdown: cache miss)").into();
            }
        };

        let props = node.props.as_object();
        let mut settings = if let Some(text_size) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "text_size")
                .or(ctx.default_text_size)
        {
            iced::widget::markdown::Settings::with_text_size(
                text_size,
                iced::widget::markdown::Style::from(ctx.theme),
            )
        } else {
            iced::widget::markdown::Settings::from(ctx.theme)
        };
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "h1_size")
        {
            settings.h1_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "h2_size")
        {
            settings.h2_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "h3_size")
        {
            settings.h3_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "code_size")
        {
            settings.code_size = iced::Pixels(v);
        }
        if let Some(v) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing")
        {
            settings.spacing = iced::Pixels(v);
        }
        if let Some(lc) = prop_color(props, "link_color") {
            settings.style.link_color = lc;
        }

        let mut md: Element<'a, Message, iced::Theme, R> =
            iced::widget::markdown::view(items, settings).map(Message::MarkdownUrl);

        if let Some(w) = value_to_length_opt(props.and_then(|p| p.get("width"))) {
            md = iced::widget::container(md).width(w).into();
        }

        md
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.items.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(MarkdownWidget::new())
    }
}
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
// ThemerWidget: extracted stateful factory (owns resolved theme cache)
// See interactive::ensure_themer_cache for the original ensure logic.
pub(crate) struct ThemerWidget {
    /// Resolved themes per (window_id, node_id). Populated during prepare,
    /// borrowed during render for child context theming.
    themes: std::collections::HashMap<(String, String), iced::Theme>,
}

impl ThemerWidget {
    pub(crate) fn new() -> Self {
        Self {
            themes: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for ThemerWidget {
    fn type_names(&self) -> &[&str] {
        &["themer"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        if let Some(resolved) = props
            .and_then(|p| p.get("theme"))
            .and_then(crate::theming::resolve_theme_only)
        {
            self.themes.insert(key, resolved);
        } else {
            self.themes.remove(&key);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        let cached_theme = self.themes.get(&key);
        let child_theme = cached_theme.unwrap_or(ctx.theme);
        let child_ctx = ctx.with_theme(child_theme);

        let child: Element<'a, Message, iced::Theme, R> = node
            .children
            .first()
            .map(|c| child_ctx.render_child(c))
            .unwrap_or_else(|| iced::widget::Space::new().into());

        let themer_theme = cached_theme.cloned();
        iced::widget::Themer::new(themer_theme, child).into()
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.themes.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ThemerWidget::new())
    }
}
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
            Box::new(MarkdownWidget::new()),
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
            Box::new(ThemerWidget::new()),
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
