//! Built-in widget implementations for the "iced" widget set.
//!
//! Contains both thin wrapper factories (stateless widgets that delegate
//! to existing `render_*` functions via the [`builtin_widget!`] macro) and
//! fully extracted stateful factories that own their per-instance state.
//!
//! Extracted stateful factories:
//! - [`ThemerWidget`] -- owns resolved theme cache
//! - [`MarkdownWidget`] -- owns parsed markdown items
//! - [`TextEditorWidget`] -- owns `text_editor::Content<R>` (R-generic)
//! - [`ComboBoxWidget`] -- owns `combo_box::State`
//! - [`SliderWidget`] / [`VerticalSliderWidget`] -- own slide value tracking
//! - [`PaneGridWidget`] -- owns `pane_grid::State` with full message handling
//!
//! The [`iced_widget_set`] function returns a [`WidgetSet`] containing all
//! built-in widgets, suitable for registering as the default set.

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

builtin_widget!(ColumnWidget, ["column"], layout::render_column);
builtin_widget!(RowWidget, ["row"], layout::render_row);
builtin_widget!(ContainerWidget, ["container"], layout::render_container);
builtin_widget!(StackWidget, ["stack"], layout::render_stack);
builtin_widget!(GridWidget, ["grid"], layout::render_grid);
builtin_widget!(PinWidget, ["pin"], layout::render_pin);
builtin_widget!(
    KeyedColumnWidget,
    ["keyed_column"],
    layout::render_keyed_column
);
builtin_widget!(FloatWidget, ["float"], layout::render_float);
builtin_widget!(ResponsiveWidget, ["responsive"], layout::render_responsive);
builtin_widget!(ScrollableWidget, ["scrollable"], layout::render_scrollable);
// PaneGridWidget: extracted stateful factory (owns pane_grid::State).
// Has complex prepare (pane reconciliation) and handle_message
// (resolve pane handles to IDs, mutate state on resize/drop).
pub(crate) struct PaneGridWidget {
    /// pane_grid layout state per (window_id, node_id).
    states: std::collections::HashMap<(String, String), iced::widget::pane_grid::State<String>>,
}

impl PaneGridWidget {
    pub(crate) fn new() -> Self {
        Self {
            states: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for PaneGridWidget {
    fn type_names(&self) -> &[&str] {
        &["pane_grid"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        use iced::widget::pane_grid;
        use std::collections::HashSet;

        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        let axis = match crate::prop_helpers::prop_str(props, "split_axis").as_deref() {
            Some("horizontal") => pane_grid::Axis::Horizontal,
            _ => pane_grid::Axis::Vertical,
        };
        let child_ids: HashSet<String> = node.children.iter().map(|c| c.id.clone()).collect();

        if let Some(state) = self.states.get_mut(&key) {
            // Prune panes whose child nodes no longer exist.
            let stale_panes: Vec<pane_grid::Pane> = state
                .panes
                .iter()
                .filter(|(_pane, id)| !child_ids.contains(*id))
                .map(|(pane, _id)| *pane)
                .collect();
            for pane in stale_panes {
                state.close(pane);
            }
            // Add panes for new children.
            let existing_ids: HashSet<String> = state.panes.values().cloned().collect();
            let new_child_ids: Vec<String> = node
                .children
                .iter()
                .filter(|c| !existing_ids.contains(&c.id))
                .map(|c| c.id.clone())
                .collect();
            for new_id in new_child_ids {
                if let Some((&anchor, _)) = state.panes.iter().next() {
                    let _ = state.split(axis, anchor, new_id);
                }
            }
        } else {
            let child_list: Vec<String> = node.children.iter().map(|c| c.id.clone()).collect();
            let new_state = if child_list.is_empty() {
                let (state, _) = pane_grid::State::new("default".to_string());
                state
            } else if child_list.len() == 1 {
                let (state, _) = pane_grid::State::new(child_list[0].clone());
                state
            } else {
                let (mut state, first_pane) = pane_grid::State::new(child_list[0].clone());
                let mut last_pane = first_pane;
                for id in child_list.iter().skip(1) {
                    if let Some((new_pane, _)) = state.split(axis, last_pane, id.clone()) {
                        last_pane = new_pane;
                    }
                }
                state
            };
            self.states.insert(key, new_state);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        match self.states.get(&key) {
            Some(state) => layout::render_pane_grid_with_state(node, *ctx, state),
            None => iced::widget::text("(pane_grid: no state)").into(),
        }
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        use crate::protocol::OutgoingEvent;
        use iced::widget::pane_grid;

        match msg {
            Message::PaneFocusCycle(window_id, grid_id, pane) => {
                let key = (window_id.to_string(), grid_id.to_string());
                if let Some(state) = self.states.get(&key) {
                    let pane_id = state.get(*pane).cloned().unwrap_or_default();
                    Some(vec![
                        OutgoingEvent::pane_focus_cycle(grid_id.clone(), pane_id)
                            .with_window_id(window_id.clone()),
                    ])
                } else {
                    Some(vec![])
                }
            }
            Message::PaneResized(window_id, grid_id, evt) => {
                let key = (window_id.to_string(), grid_id.to_string());
                if let Some(state) = self.states.get_mut(&key) {
                    state.resize(evt.split, evt.ratio);
                }
                Some(vec![
                    OutgoingEvent::pane_resized(
                        grid_id.clone(),
                        format!("{:?}", evt.split),
                        evt.ratio,
                    )
                    .with_window_id(window_id.clone()),
                ])
            }
            Message::PaneDragged(window_id, grid_id, evt) => {
                let key = (window_id.to_string(), grid_id.to_string());
                match evt {
                    pane_grid::DragEvent::Picked { pane } => {
                        if let Some(state) = self.states.get(&key) {
                            let pane_id = state.get(*pane).cloned().unwrap_or_default();
                            Some(vec![
                                OutgoingEvent::pane_dragged(
                                    grid_id.clone(),
                                    "picked",
                                    pane_id,
                                    None,
                                    None,
                                    None,
                                )
                                .with_window_id(window_id.clone()),
                            ])
                        } else {
                            Some(vec![])
                        }
                    }
                    pane_grid::DragEvent::Dropped { pane, target } => {
                        if let Some(state) = self.states.get_mut(&key) {
                            let pane_id = state.get(*pane).cloned().unwrap_or_default();
                            let (target_pane, region, edge) = match target {
                                pane_grid::Target::Edge(e) => {
                                    let edge_str = match e {
                                        pane_grid::Edge::Top => "top",
                                        pane_grid::Edge::Bottom => "bottom",
                                        pane_grid::Edge::Left => "left",
                                        pane_grid::Edge::Right => "right",
                                    };
                                    (None, None, Some(edge_str))
                                }
                                pane_grid::Target::Pane(p, region) => {
                                    let target_id = state.get(*p).cloned().unwrap_or_default();
                                    let region_str = match region {
                                        pane_grid::Region::Center => "center",
                                        pane_grid::Region::Edge(pane_grid::Edge::Top) => "top",
                                        pane_grid::Region::Edge(pane_grid::Edge::Bottom) => {
                                            "bottom"
                                        }
                                        pane_grid::Region::Edge(pane_grid::Edge::Left) => "left",
                                        pane_grid::Region::Edge(pane_grid::Edge::Right) => "right",
                                    };
                                    (Some(target_id), Some(region_str), None)
                                }
                            };
                            state.drop(*pane, *target);
                            Some(vec![
                                OutgoingEvent::pane_dragged(
                                    grid_id.clone(),
                                    "dropped",
                                    pane_id,
                                    target_pane,
                                    region,
                                    edge,
                                )
                                .with_window_id(window_id.clone()),
                            ])
                        } else {
                            Some(vec![])
                        }
                    }
                    pane_grid::DragEvent::Canceled { pane } => {
                        if let Some(state) = self.states.get(&key) {
                            let pane_id = state.get(*pane).cloned().unwrap_or_default();
                            Some(vec![
                                OutgoingEvent::pane_dragged(
                                    grid_id.clone(),
                                    "canceled",
                                    pane_id,
                                    None,
                                    None,
                                    None,
                                )
                                .with_window_id(window_id.clone()),
                            ])
                        } else {
                            Some(vec![])
                        }
                    }
                }
            }
            Message::PaneClicked(window_id, grid_id, pane) => {
                let key = (window_id.to_string(), grid_id.to_string());
                if let Some(state) = self.states.get(&key) {
                    let pane_id = state.get(*pane).cloned().unwrap_or_default();
                    Some(vec![
                        OutgoingEvent::pane_clicked(grid_id.clone(), pane_id)
                            .with_window_id(window_id.clone()),
                    ])
                } else {
                    Some(vec![])
                }
            }
            _ => None,
        }
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.states.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PaneGridWidget::new())
    }
}

// ---------------------------------------------------------------------------
// Display widgets (9, counting rich_text alias)
// ---------------------------------------------------------------------------

builtin_widget!(TextWidget, ["text"], display::render_text);
builtin_widget!(
    RichTextWidget,
    ["rich_text", "rich"],
    display::render_rich_text
);
builtin_widget!(SpaceWidget, ["space"], display::render_space);
builtin_widget!(RuleWidget, ["rule"], display::render_rule);
builtin_widget!(
    ProgressBarWidget,
    ["progress_bar"],
    display::render_progress_bar
);
builtin_widget!(ImageWidget, ["image"], display::render_image);
builtin_widget!(SvgWidget, ["svg"], display::render_svg);
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

        if let Some((existing_hash, _)) = self.items.get(&key)
            && *existing_hash == hash
        {
            return;
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
// QrCodeWidget: extracted stateful factory (owns R-generic canvas::Cache).
pub(crate) struct QrCodeWidget<R: PlushieRenderer> {
    /// Per-qr_code cache with content hash for invalidation.
    /// Keyed by (window_id, node_id).
    caches: std::collections::HashMap<(String, String), (u64, iced::widget::canvas::Cache<R>)>,
}

impl<R: PlushieRenderer> QrCodeWidget<R> {
    pub(crate) fn new() -> Self {
        Self {
            caches: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for QrCodeWidget<R> {
    fn type_names(&self) -> &[&str] {
        &["qr_code"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        let data = crate::prop_helpers::prop_str(props, "data").unwrap_or_default();
        let cell_size = crate::prop_helpers::prop_f32(props, "cell_size").unwrap_or(4.0);
        let ec = crate::prop_helpers::prop_str(props, "error_correction").unwrap_or_default();

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        cell_size.to_bits().hash(&mut hasher);
        ec.hash(&mut hasher);
        let hash = hasher.finish();

        match self.caches.get_mut(&key) {
            Some((existing_hash, cache)) => {
                if *existing_hash != hash {
                    cache.clear();
                    *existing_hash = hash;
                }
            }
            None => {
                self.caches
                    .insert(key, (hash, iced::widget::canvas::Cache::new()));
            }
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        let cache_entry = self.caches.get(&key);
        display::render_qr_code_with_cache(node, cache_entry)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.caches
            .remove(&(window_id.to_string(), node_id.to_string()));
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(QrCodeWidget::new())
    }
}

// ---------------------------------------------------------------------------
// Input widgets (9)
// ---------------------------------------------------------------------------

builtin_widget!(TextInputWidget,       ["text_input"],       input::render_text_input,
    infer_a11y: infer_placeholder_as_description);
// TextEditorWidget: extracted stateful factory (owns text_editor::Content<R>).
// The R-generic Content is why this factory is parameterized on R.
pub(crate) struct TextEditorWidget<R: PlushieRenderer> {
    /// text_editor Content per (window_id, node_id). Preserves cursor,
    /// undo history, and selection across renders.
    contents: std::collections::HashMap<(String, String), iced::widget::text_editor::Content<R>>,
    /// Hash of last-synced "content" prop per (window_id, node_id).
    /// Detects host-side prop changes without clobbering user edits.
    content_hashes: std::collections::HashMap<(String, String), u64>,
}

impl<R: PlushieRenderer> TextEditorWidget<R> {
    const MAX_CONTENT: usize = 10_485_760; // 10 MB

    pub(crate) fn new() -> Self {
        Self {
            contents: std::collections::HashMap::new(),
            content_hashes: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for TextEditorWidget<R> {
    fn type_names(&self) -> &[&str] {
        &["text_editor"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        use crate::widgets::caches::hash_str;

        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        let mut content_str = crate::prop_helpers::prop_str(props, "content").unwrap_or_default();
        if content_str.len() > Self::MAX_CONTENT {
            log::warn!(
                "[id={}] text_editor content ({} bytes) exceeds limit ({} bytes), truncating",
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
        let prop_hash = hash_str(&content_str);
        let prev_hash = self.content_hashes.get(&key).copied();
        if prev_hash != Some(prop_hash) {
            self.contents.insert(
                key.clone(),
                iced::widget::text_editor::Content::with_text(&content_str),
            );
            self.content_hashes.insert(key, prop_hash);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        match self.contents.get(&key) {
            Some(content) => input::render_text_editor_with_content(node, *ctx, content),
            None => {
                log::warn!("text_editor factory cache miss for id={}", node.id);
                iced::widget::text("(text_editor: cache miss)").into()
            }
        }
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        use crate::widgets::caches::hash_str;

        match msg {
            Message::TextEditorAction(window_id, id, action) => {
                let key = (window_id.to_string(), id.to_string());
                if let Some(content) = self.contents.get_mut(&key) {
                    let is_edit = action.is_edit();
                    content.perform(action.clone());
                    if is_edit {
                        let new_text = content.text();
                        self.content_hashes.insert(key, hash_str(&new_text));
                        return Some(vec![
                            crate::protocol::OutgoingEvent::input(id.clone(), new_text)
                                .with_window_id(window_id.clone()),
                        ]);
                    }
                }
                Some(vec![])
            }
            _ => None,
        }
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        infer_placeholder_as_description(node)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.contents.remove(&key);
        self.content_hashes.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TextEditorWidget::new())
    }
}
builtin_widget!(CheckboxWidget, ["checkbox"], input::render_checkbox);
builtin_widget!(TogglerWidget, ["toggler"], input::render_toggler);
builtin_widget!(RadioWidget, ["radio"], input::render_radio);
// ---------------------------------------------------------------------------
// Slider value tracking (shared by SliderWidget and VerticalSliderWidget)
// ---------------------------------------------------------------------------

/// Handle Slide/SlideRelease messages for sliders. Tracks the latest drag
/// value per (window_id, node_id) so SlideRelease can report the final
/// value (iced's release event doesn't carry the value itself).
fn handle_slider_message(
    last_values: &mut std::collections::HashMap<(String, String), f64>,
    msg: &Message,
) -> Option<Vec<crate::protocol::OutgoingEvent>> {
    match msg {
        Message::Slide(window_id, id, value) => {
            last_values.insert((window_id.clone(), id.clone()), *value);
            Some(vec![
                crate::protocol::OutgoingEvent::slide(id.clone(), *value)
                    .with_window_id(window_id.clone()),
            ])
        }
        Message::SlideRelease(window_id, id) => {
            let key = (window_id.clone(), id.clone());
            let value = last_values.remove(&key).unwrap_or(0.0);
            Some(vec![
                crate::protocol::OutgoingEvent::slide_release(id.clone(), value)
                    .with_window_id(window_id.clone()),
            ])
        }
        _ => None,
    }
}

pub(crate) struct SliderWidget {
    last_values: std::collections::HashMap<(String, String), f64>,
}

impl SliderWidget {
    pub(crate) fn new() -> Self {
        Self {
            last_values: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for SliderWidget {
    fn type_names(&self) -> &[&str] {
        &["slider"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        input::render_slider(node, *ctx)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        handle_slider_message(&mut self.last_values, msg)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.last_values
            .remove(&(window_id.to_string(), node_id.to_string()));
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SliderWidget::new())
    }
}

pub(crate) struct VerticalSliderWidget {
    last_values: std::collections::HashMap<(String, String), f64>,
}

impl VerticalSliderWidget {
    pub(crate) fn new() -> Self {
        Self {
            last_values: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for VerticalSliderWidget {
    fn type_names(&self) -> &[&str] {
        &["vertical_slider"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        input::render_vertical_slider(node, *ctx)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        handle_slider_message(&mut self.last_values, msg)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        self.last_values
            .remove(&(window_id.to_string(), node_id.to_string()));
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(VerticalSliderWidget::new())
    }
}
builtin_widget!(PickListWidget, ["pick_list"], input::render_pick_list);
/// Stateful factory owning combo_box::State per (window_id, node_id).
pub(crate) struct ComboBoxWidget {
    /// combo_box::State per (window_id, node_id).
    states: std::collections::HashMap<(String, String), iced::widget::combo_box::State<String>>,
    /// Cached options per (window_id, node_id) for change detection.
    options: std::collections::HashMap<(String, String), Vec<String>>,
}

impl ComboBoxWidget {
    pub(crate) fn new() -> Self {
        Self {
            states: std::collections::HashMap::new(),
            options: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for ComboBoxWidget {
    fn type_names(&self) -> &[&str] {
        &["combo_box"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        let new_options: Vec<String> = props
            .and_then(|p| p.get("options"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let options_changed = self
            .options
            .get(&key)
            .is_none_or(|cached| *cached != new_options);
        if options_changed {
            self.states.insert(
                key.clone(),
                iced::widget::combo_box::State::new(new_options.clone()),
            );
            self.options.insert(key, new_options);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        match self.states.get(&key) {
            Some(state) => input::render_combo_box_with_state(node, *ctx, state),
            None => {
                log::warn!("combo_box factory cache miss for id={}", node.id);
                iced::widget::text("(combo_box: cache miss)").into()
            }
        }
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        infer_placeholder_as_description(node)
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.states.remove(&key);
        self.options.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ComboBoxWidget::new())
    }
}

// ---------------------------------------------------------------------------
// Interactive widgets (7)
// ---------------------------------------------------------------------------

builtin_widget!(ButtonWidget, ["button"], interactive::render_button);
builtin_widget!(
    PointerAreaWidget,
    ["pointer_area"],
    interactive::render_mouse_area
);
builtin_widget!(SensorWidget, ["sensor"], interactive::render_sensor);
builtin_widget!(TooltipWidget, ["tooltip"], interactive::render_tooltip);
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
        // Render from factory-owned state. prepare() populates self.themes
        // via the registry prepare_walk (wired into App::apply and headless).
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
builtin_widget!(WindowWidget, ["window"], interactive::render_window);
builtin_widget!(OverlayWidget, ["overlay"], interactive::render_overlay);

// ---------------------------------------------------------------------------
// Canvas (1)
// ---------------------------------------------------------------------------

/// Stateful factory owning R-generic canvas layer caches, interactive
/// element data, and pending programmatic focus. The most complex
/// built-in widget with 3700+ lines of rendering, hit testing,
/// keyboard navigation, and drag tracking infrastructure.
#[allow(clippy::type_complexity)]
pub(crate) struct CanvasWidget<R: PlushieRenderer> {
    /// Per-canvas, per-layer tessellation caches with content hashing.
    layer_caches: std::collections::HashMap<
        (String, String),
        std::collections::HashMap<String, (u64, iced::widget::canvas::Cache<R>)>,
    >,
    /// Pre-parsed interactive elements per (window_id, canvas_id).
    interactions: std::collections::HashMap<(String, String), Vec<canvas::InteractiveElement>>,
    /// Pending programmatic focus per (window_id, canvas_id).
    pending_focus: std::collections::HashMap<(String, String), String>,
}

impl<R: PlushieRenderer> CanvasWidget<R> {
    pub(crate) fn new() -> Self {
        Self {
            layer_caches: std::collections::HashMap::new(),
            interactions: std::collections::HashMap::new(),
            pending_focus: std::collections::HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for CanvasWidget<R> {
    fn type_names(&self) -> &[&str] {
        &["canvas"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        use crate::widgets::caches::{canvas_layers_from_node, hash_json_value};

        let key = (window_id.to_string(), node.id.clone());
        let layer_map = canvas_layers_from_node(node);

        // Parse interactive elements from all layers.
        let mut interactive_elements = Vec::new();
        for (layer_name, shapes_val) in &layer_map {
            if let Some(shapes_arr) = shapes_val.as_array() {
                canvas::collect_interactive_elements(
                    shapes_arr,
                    layer_name,
                    canvas::TransformMatrix::identity(),
                    None,
                    None,
                    "",
                    &mut interactive_elements,
                );
            }
        }
        // Validate a11y annotations and emit diagnostics as log warnings
        // (prepare has no outgoing event channel).
        let diags = canvas::validate_interactive_elements(&node.id, &interactive_elements);
        for diag in &diags {
            if let Some(msg) = diag
                .data
                .as_ref()
                .and_then(|d| d.get("message"))
                .and_then(|m| m.as_str())
            {
                log::warn!("[canvas {}] {}", node.id, msg);
            }
        }
        self.interactions.insert(key.clone(), interactive_elements);

        // Update or create per-layer tessellation caches.
        let node_caches = self.layer_caches.entry(key).or_default();
        for (layer_name, shapes_val) in &layer_map {
            let hash = {
                let mut hasher = DefaultHasher::new();
                hash_json_value(shapes_val, &mut hasher);
                hasher.finish()
            };
            match node_caches.get_mut(layer_name) {
                Some((existing_hash, cache)) => {
                    if *existing_hash != hash {
                        cache.clear();
                        *existing_hash = hash;
                    }
                }
                None => {
                    node_caches.insert(
                        layer_name.clone(),
                        (hash, iced::widget::canvas::Cache::new()),
                    );
                }
            }
        }
        node_caches.retain(|name, _| layer_map.contains_key(name));
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, iced::Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        // Check both factory-owned pending_focus and SharedState pending_focus
        // (widget_ops.rs writes to SharedState for programmatic focus commands).
        let pending = self
            .pending_focus
            .get(&key)
            .cloned()
            .or_else(|| ctx.caches.canvas_pending_focus.get(&node.id).cloned());
        canvas::render_canvas_with_state(
            node,
            *ctx,
            self.layer_caches.get(&key),
            self.interactions
                .get(&key)
                .map(|v| v.as_slice())
                .unwrap_or(&[]),
            pending,
        )
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        // CanvasElementFocusChanged splits into separate blur + focus events.
        // All other canvas messages use the default message_to_event conversion.
        match msg {
            Message::CanvasElementFocusChanged {
                window_id,
                canvas_id,
                old_element_id,
                new_element_id,
            } => {
                let mut events = Vec::with_capacity(2);
                if let Some(old_id) = old_element_id {
                    events.push(
                        crate::protocol::OutgoingEvent::canvas_element_blurred(
                            canvas_id.clone(),
                            old_id.clone(),
                        )
                        .with_window_id(window_id.clone()),
                    );
                }
                if let Some(new_id) = new_element_id {
                    events.push(
                        crate::protocol::OutgoingEvent::canvas_element_focused(
                            canvas_id.clone(),
                            new_id.clone(),
                        )
                        .with_window_id(window_id.clone()),
                    );
                }
                Some(events)
            }
            _ => None,
        }
    }

    fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        payload: &serde_json::Value,
    ) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        // Handle programmatic focus for canvas elements.
        // The node_id may be "canvas_id/element_id" via prefix routing.
        if op == "focus" {
            if let Some(slash) = node_id.find('/') {
                let canvas_id = &node_id[..slash];
                let element_id = &node_id[slash + 1..];
                // Find the key by canvas_id (any window).
                if let Some(key) = self
                    .interactions
                    .keys()
                    .find(|(_, nid)| nid == canvas_id)
                    .cloned()
                {
                    self.pending_focus.insert(key, element_id.to_string());
                }
            }
            let _ = payload;
            Some(vec![])
        } else {
            None
        }
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.layer_caches.remove(&key);
        self.interactions.remove(&key);
        self.pending_focus.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(CanvasWidget::new())
    }
}

// ---------------------------------------------------------------------------
// Table (1)
// ---------------------------------------------------------------------------

builtin_widget!(TableWidget, ["table"], table::render_table);

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
            Box::new(PaneGridWidget::new()),
            // Display
            Box::new(TextWidget),
            Box::new(RichTextWidget),
            Box::new(SpaceWidget),
            Box::new(RuleWidget),
            Box::new(ProgressBarWidget),
            Box::new(ImageWidget),
            Box::new(SvgWidget),
            Box::new(MarkdownWidget::new()),
            Box::new(QrCodeWidget::new()),
            // Input
            Box::new(TextInputWidget),
            Box::new(TextEditorWidget::new()),
            Box::new(CheckboxWidget),
            Box::new(TogglerWidget),
            Box::new(RadioWidget),
            Box::new(SliderWidget::new()),
            Box::new(VerticalSliderWidget::new()),
            Box::new(PickListWidget),
            Box::new(ComboBoxWidget::new()),
            // Interactive
            Box::new(ButtonWidget),
            Box::new(PointerAreaWidget),
            Box::new(SensorWidget),
            Box::new(TooltipWidget),
            Box::new(ThemerWidget::new()),
            Box::new(WindowWidget),
            Box::new(OverlayWidget),
            // Canvas
            Box::new(CanvasWidget::new()),
            // Table
            Box::new(TableWidget),
        ]
    }
}

/// Create the default iced widget set. Convenience for builder registration.
pub fn iced_widget_set() -> IcedWidgetSet {
    IcedWidgetSet
}
