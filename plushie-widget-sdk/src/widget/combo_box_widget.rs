use iced::widget::{combo_box, container, text_input};
use iced::{Element, Length, Theme, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::a11y::A11yOverrides;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

// ---------------------------------------------------------------------------
// ComboBoxWidget (stateful)
// ---------------------------------------------------------------------------

/// Stateful factory owning combo_box::State per (window_id, node_id).
pub(crate) struct ComboBoxWidget {
    /// combo_box::State per (window_id, node_id).
    states: std::collections::HashMap<(String, String), combo_box::State<String>>,
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
        let props = &node.props;
        let opts_val = props.get_value("options");
        let new_options: Vec<String> = opts_val
            .as_ref()
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
            self.states
                .insert(key.clone(), combo_box::State::new(new_options.clone()));
            self.options.insert(key, new_options);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        match self.states.get(&key) {
            Some(state) => render_combo_box_with_state(node, *ctx, state),
            None => {
                log::warn!("combo_box factory cache miss for id={}", node.id);
                iced::widget::text("(combo_box: cache miss)").into()
            }
        }
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        let props = &node.props;
        crate::prop_helpers::prop_str(props, "placeholder").map(A11yOverrides::with_description)
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
// Render logic
// ---------------------------------------------------------------------------

/// Render a combo_box with the provided State.
fn render_combo_box_with_state<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
    state: &'a combo_box::State<String>,
) -> Element<'a, Message, Theme, R> {
    let props = &node.props;
    let selected: Option<String> = prop_str(props, "selected");
    let placeholder = prop_str(props, "placeholder").unwrap_or_default();
    let width = prop_length(props, "width", Length::Fill);
    let padding_val = parse_padding_value(props);
    let id = node.id.clone();
    let input_id = node.id.clone();
    let window_id = ctx.window_id.to_string();
    let input_window_id = window_id.clone();

    let mut cb = combo_box(state, &placeholder, selected.as_ref(), move |selected| {
        Message::Select(window_id.clone(), id.clone(), selected)
    })
    .width(width);

    if let Some(p) = padding_val {
        cb = cb.padding(p);
    }

    // on_input: emit Input events so the host can filter
    cb = cb.on_input(move |v| Message::Input(input_window_id.clone(), input_id.clone(), v));

    if let Some(sz) = prop_f32(props, "size").or(ctx.default_text_size) {
        cb = cb.size(sz);
    }
    let font = props
        .get_value("font")
        .as_ref().map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        cb = cb.font(f);
    }
    if let Some(lh) = parse_line_height(props) {
        cb = cb.line_height(lh);
    }
    if let Some(shaping) = parse_shaping(props) {
        cb = cb.shaping(shaping);
    }
    if let Some(mh) = prop_f32(props, "menu_height") {
        cb = cb.menu_height(mh);
    }
    if let Some(icon) = props
        .get_value("icon")
        .as_ref()
        .and_then(parse_text_input_icon)
    {
        cb = cb.icon(icon);
    }
    if let Some(e) = parse_ellipsis(props) {
        cb = cb.ellipsis(e);
    }

    // Menu style: inline overrides for the dropdown menu
    if let Some(ms) = parse_menu_style(props) {
        cb = cb.menu_style(move |theme: &iced::Theme| {
            use iced::overlay::menu;
            let mut style = menu::default(theme);
            apply_menu_style_overrides(&mut style, &ms);
            style
        });
    }

    if prop_bool_default(props, "on_option_hovered", false) {
        let hover_id = node.id.clone();
        let hover_window_id = ctx.window_id.to_string();
        cb = cb.on_option_hovered(move |val| {
            Message::OptionHovered(hover_window_id.clone(), hover_id.clone(), val)
        });
    }
    if prop_bool_default(props, "on_open", false) {
        let open_id = node.id.clone();
        cb = cb.on_open(Message::Event {
            window_id: ctx.window_id.to_string(),
            id: open_id,
            data: Value::Null,
            family: "open".into(),
        });
    }
    if prop_bool_default(props, "on_close", false) {
        let close_id = node.id.clone();
        cb = cb.on_close(Message::Event {
            window_id: ctx.window_id.to_string(),
            id: close_id,
            data: Value::Null,
            family: "close".into(),
        });
    }

    // Style: string name or style map object (applies to the input field)
    if let Some(style_val) = props.get_value("style") {
        if let Some(style_name) = style_val.as_str() {
            cb = match style_name {
                "default" => cb.input_style(text_input::default),
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "combo_box"
                    );
                    cb
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            cb = cb.input_style(move |theme: &iced::Theme, status| {
                let base_fn: fn(&iced::Theme, text_input::Status) -> text_input::Style =
                    match ov.preset_base.as_deref() {
                        Some("default") => text_input::default,
                        _ => text_input::default,
                    };
                let mut style = base_fn(theme, status);
                apply_text_input_fields(&mut style, &ov.base);
                match status {
                    text_input::Status::Focused { .. } => {
                        if let Some(ref f) = ov.focused {
                            apply_text_input_fields(&mut style, f);
                        }
                    }
                    text_input::Status::Hovered => {
                        if let Some(ref f) = ov.hovered {
                            apply_text_input_fields(&mut style, f);
                        } else {
                            style.background = deviate_background(style.background, 0.1);
                        }
                    }
                    text_input::Status::Disabled => {
                        if let Some(ref f) = ov.disabled {
                            apply_text_input_fields(&mut style, f);
                        } else {
                            style.background = match style.background {
                                iced::Background::Color(c) => {
                                    iced::Background::Color(alpha_color(c, 0.5))
                                }
                                iced::Background::Gradient(g) => {
                                    iced::Background::Gradient(alpha_gradient(g, 0.5))
                                }
                            };
                            style.value = alpha_color(style.value, 0.5);
                            style.border = auto_derive_disabled_border(style.border);
                        }
                    }
                    _ => {}
                }
                style
            });
        }
    }

    container(cb).id(widget::Id::from(node.id.clone())).into()
}
