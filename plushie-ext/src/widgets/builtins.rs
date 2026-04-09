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
        // Delegate to existing render function during transition.
        layout::render_pane_grid(node, *ctx)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        use crate::protocol::OutgoingEvent;
        use iced::widget::pane_grid;

        match msg {
            Message::PaneFocusCycle(window_id, grid_id, pane) => {
                let state = self
                    .states
                    .values()
                    .find(|s| s.panes.values().any(|id| id == grid_id))
                    .or_else(|| {
                        self.states.iter().find(|((_w, n), _)| n == grid_id).map(|(_, s)| s)
                    });
                if let Some(state) = state {
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
                // Find state by grid_id (node_id part of the key)
                if let Some(state) = self
                    .states
                    .iter_mut()
                    .find(|((_w, n), _)| n == grid_id)
                    .map(|(_, s)| s)
                {
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
                let state = self
                    .states
                    .iter_mut()
                    .find(|((_w, n), _)| n == grid_id)
                    .map(|(_, s)| s);

                match evt {
                    pane_grid::DragEvent::Picked { pane } => {
                        if let Some(state) = state {
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
                        if let Some(state) = state {
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
                        if let Some(state) = state {
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
                let state = self
                    .states
                    .values()
                    .find(|s| s.panes.values().any(|id| id == grid_id))
                    .or_else(|| {
                        self.states.iter().find(|((_w, n), _)| n == grid_id).map(|(_, s)| s)
                    });
                if let Some(state) = state {
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
        // Delegate to existing render function during transition.
        // The old ensure_text_editor_cache path still populates WidgetCaches,
        // so render_text_editor reads from ctx.caches.editor_contents.
        // TODO: once ensure_caches_walk is removed, render from self.contents.
        input::render_text_editor(node, *ctx)
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        use crate::widgets::caches::hash_str;

        match msg {
            Message::TextEditorAction(window_id, id, action) => {
                // During transition, Content lives in both self.contents
                // and WidgetCaches. The old process_widget_message path
                // mutates WidgetCaches. This handle_message is ready for
                // when registry message dispatch takes over.
                // For now, find by node_id alone (matching old behavior).
                let key_match = self
                    .contents
                    .keys()
                    .find(|(_, nid)| nid == id)
                    .cloned();
                if let Some(key) = key_match {
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
builtin_widget!(CheckboxWidget,        ["checkbox"],         input::render_checkbox);
builtin_widget!(TogglerWidget,         ["toggler"],          input::render_toggler);
builtin_widget!(RadioWidget,           ["radio"],            input::render_radio);
// SliderWidget: extracted factory with handle_message for value tracking.
// Render delegates to the existing function (no slider-specific cache reads).
pub(crate) struct SliderWidget {
    /// Last slider drag value per node ID, used to provide the final value
    /// on SlideRelease (iced only sends the release event, not the value).
    last_values: std::collections::HashMap<String, f64>,
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
        match msg {
            Message::Slide(window_id, id, value) => {
                self.last_values.insert(id.clone(), *value);
                Some(vec![
                    crate::protocol::OutgoingEvent::slide(id.clone(), *value)
                        .with_window_id(window_id.clone()),
                ])
            }
            Message::SlideRelease(window_id, id) => {
                let value = self.last_values.remove(id).unwrap_or(0.0);
                Some(vec![
                    crate::protocol::OutgoingEvent::slide_release(id.clone(), value)
                        .with_window_id(window_id.clone()),
                ])
            }
            _ => None,
        }
    }

    fn cleanup(&mut self, node_id: &str, _window_id: &str) {
        self.last_values.remove(node_id);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(SliderWidget::new())
    }
}

// VerticalSliderWidget: shares the same value tracking as SliderWidget.
pub(crate) struct VerticalSliderWidget {
    last_values: std::collections::HashMap<String, f64>,
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
        match msg {
            Message::Slide(window_id, id, value) => {
                self.last_values.insert(id.clone(), *value);
                Some(vec![
                    crate::protocol::OutgoingEvent::slide(id.clone(), *value)
                        .with_window_id(window_id.clone()),
                ])
            }
            Message::SlideRelease(window_id, id) => {
                let value = self.last_values.remove(id).unwrap_or(0.0);
                Some(vec![
                    crate::protocol::OutgoingEvent::slide_release(id.clone(), value)
                        .with_window_id(window_id.clone()),
                ])
            }
            _ => None,
        }
    }

    fn cleanup(&mut self, node_id: &str, _window_id: &str) {
        self.last_values.remove(node_id);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(VerticalSliderWidget::new())
    }
}
builtin_widget!(PickListWidget,        ["pick_list"],        input::render_pick_list);
// ComboBoxWidget: extracted stateful factory (owns combo_box::State).
// Render delegates to existing function during transition. Once render
// reads from factory state instead of WidgetCaches, the delegation
// can be inlined.
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
        let options_changed = self.options.get(&key).is_none_or(|cached| *cached != new_options);
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
        // During transition, delegate to existing render function which reads
        // from WidgetCaches. The factory's prepare() keeps WidgetCaches in
        // sync via the old ensure_combo_box_cache path (still runs).
        // TODO: once ensure_caches_walk is removed, render from self.states.
        input::render_combo_box(node, *ctx)
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
            Box::new(QrCodeWidget),
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
