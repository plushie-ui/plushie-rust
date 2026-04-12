use iced::widget::text_editor;
use iced::{Element, Theme, keyboard, widget};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::a11y::A11yOverrides;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Color, Font, InputPurpose, Length, LineHeight, PlushieType, Style as CoreStyle, Wrapping,
};

// ---------------------------------------------------------------------------
// Key binding helpers
// ---------------------------------------------------------------------------

/// Parse a JSON motion string into an iced Motion.
fn parse_motion(s: &str) -> Option<text_editor::Motion> {
    use text_editor::Motion;
    match s {
        "left" => Some(Motion::Left),
        "right" => Some(Motion::Right),
        "up" => Some(Motion::Up),
        "down" => Some(Motion::Down),
        "word_left" => Some(Motion::WordLeft),
        "word_right" => Some(Motion::WordRight),
        "home" => Some(Motion::Home),
        "end" => Some(Motion::End),
        "page_up" => Some(Motion::PageUp),
        "page_down" => Some(Motion::PageDown),
        "document_start" => Some(Motion::DocumentStart),
        "document_end" => Some(Motion::DocumentEnd),
        _ => None,
    }
}

/// Parse a JSON binding value into an iced Binding.
fn parse_binding(val: &Value, id: &str, window_id: &str) -> Option<text_editor::Binding<Message>> {
    use text_editor::Binding;
    match val {
        Value::String(s) => match s.as_str() {
            "copy" => Some(Binding::Copy),
            "cut" => Some(Binding::Cut),
            "paste" => Some(Binding::Paste),
            "select_all" => Some(Binding::SelectAll),
            "enter" => Some(Binding::Enter),
            "backspace" => Some(Binding::Backspace),
            "delete" => Some(Binding::Delete),
            "unfocus" => Some(Binding::Unfocus),
            "select_word" => Some(Binding::SelectWord),
            "select_line" => Some(Binding::SelectLine),
            // "default" is handled at the rule-matching level, not here
            _ => None,
        },
        Value::Object(obj) => {
            if let Some(m) = obj
                .get("move")
                .and_then(|v| v.as_str())
                .and_then(parse_motion)
            {
                return Some(Binding::Move(m));
            }
            if let Some(m) = obj
                .get("select")
                .and_then(|v| v.as_str())
                .and_then(parse_motion)
            {
                return Some(Binding::Select(m));
            }
            if let Some(c) = obj
                .get("insert")
                .and_then(|v| v.as_str())
                .and_then(|s| s.chars().next())
            {
                return Some(Binding::Insert(c));
            }
            if let Some(tag) = obj.get("custom").and_then(|v| v.as_str()) {
                let event_id = id.to_string();
                return Some(Binding::Custom(Message::Event {
                    window_id: window_id.to_string(),
                    id: event_id,
                    value: serde_json::json!(tag),
                    family: "key_binding".to_string(),
                }));
            }
            if let Some(seq) = obj.get("sequence").and_then(|v| v.as_array()) {
                let bindings: Vec<_> = seq
                    .iter()
                    .filter_map(|v| parse_binding(v, id, window_id))
                    .collect();
                if !bindings.is_empty() {
                    return Some(Binding::Sequence(bindings));
                }
            }
            None
        }
        _ => None,
    }
}

/// Check if a KeyPress matches the modifiers specified in a binding rule.
///
/// Requires an exact match: all required modifiers must be pressed, and
/// no extra modifiers may be active. This prevents a rule for `["shift"]`
/// from firing on `shift+ctrl+A`.
fn match_modifiers(mods: &keyboard::Modifiers, required: &[String]) -> bool {
    let want_shift = required.iter().any(|m| m == "shift");
    let want_ctrl = required.iter().any(|m| m == "ctrl");
    let want_alt = required.iter().any(|m| m == "alt");
    let want_logo = required.iter().any(|m| m == "logo");
    let want_command = required.iter().any(|m| m == "command");
    let want_jump = required.iter().any(|m| m == "jump");

    mods.shift() == want_shift
        && mods.control() == want_ctrl
        && mods.alt() == want_alt
        && mods.logo() == want_logo
        && mods.command() == want_command
        && mods.jump() == want_jump
}

/// Match a named key string against a KeyPress key.
fn match_named_key(named_key: &str, key: &keyboard::Key) -> bool {
    use keyboard::key::Named;
    let target = match named_key {
        "Enter" => Named::Enter,
        "Backspace" => Named::Backspace,
        "Delete" => Named::Delete,
        "Escape" => Named::Escape,
        "Tab" => Named::Tab,
        "Space" => Named::Space,
        "ArrowLeft" => Named::ArrowLeft,
        "ArrowRight" => Named::ArrowRight,
        "ArrowUp" => Named::ArrowUp,
        "ArrowDown" => Named::ArrowDown,
        "Home" => Named::Home,
        "End" => Named::End,
        "PageUp" => Named::PageUp,
        "PageDown" => Named::PageDown,
        "F1" => Named::F1,
        "F2" => Named::F2,
        "F3" => Named::F3,
        "F4" => Named::F4,
        "F5" => Named::F5,
        "F6" => Named::F6,
        "F7" => Named::F7,
        "F8" => Named::F8,
        "F9" => Named::F9,
        "F10" => Named::F10,
        "F11" => Named::F11,
        "F12" => Named::F12,
        _ => return false,
    };
    matches!(key, keyboard::Key::Named(n) if *n == target)
}

/// Pre-parsed key binding rule for closure capture.
struct KeyRule {
    key: Option<String>,
    named: Option<String>,
    modifiers: Vec<String>,
    binding_val: Value,
    is_default: bool,
}

// ---------------------------------------------------------------------------
// TextEditorWidget (stateful)
// ---------------------------------------------------------------------------

/// Stateful text editor factory (owns `text_editor::Content<R>`).
///
/// Parameterized on R because `Content<R>` is renderer-generic.
pub(crate) struct TextEditorWidget<R: PlushieRenderer> {
    /// text_editor Content per (window_id, node_id). Preserves cursor,
    /// undo history, and selection across renders.
    contents: std::collections::HashMap<(String, String), text_editor::Content<R>>,
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
        use crate::shared_state::hash_str;

        let key = (window_id.to_string(), node.id.clone());
        let props = &node.props;
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
            self.contents
                .insert(key.clone(), text_editor::Content::with_text(&content_str));
            self.content_hashes.insert(key, prop_hash);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        match self.contents.get(&key) {
            Some(content) => render_text_editor_with_content(node, *ctx, content),
            None => {
                log::warn!("text_editor factory cache miss for id={}", node.id);
                iced::widget::text("(text_editor: cache miss)").into()
            }
        }
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<crate::protocol::OutgoingEvent>> {
        use crate::shared_state::hash_str;

        match msg {
            Message::TextEditorAction(window_id, id, action) => {
                let key = (window_id.to_string(), id.to_string());
                if let Some(content) = self.contents.get_mut(&key) {
                    let is_edit = action.is_edit();
                    content.perform(action.clone());
                    if is_edit {
                        let new_text = content.text();
                        self.content_hashes.insert(key, hash_str(&new_text));
                        return Some(vec![crate::protocol::OutgoingEvent::input(
                            id.clone(),
                            new_text,
                        )]);
                    }
                }
                Some(vec![])
            }
            _ => None,
        }
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        let props = &node.props;
        crate::prop_helpers::prop_str(props, "placeholder").map(A11yOverrides::with_description)
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

// ---------------------------------------------------------------------------
// Render logic
// ---------------------------------------------------------------------------

struct TextEditorProps {
    placeholder: Option<String>,
    height: Option<Length>,
    width: Option<f32>,
    min_height: Option<f32>,
    max_height: Option<f32>,
    font: Option<Font>,
    size: Option<f32>,
    line_height: Option<LineHeight>,
    padding: Option<f32>,
    wrapping: Option<Wrapping>,
    input_purpose: Option<InputPurpose>,
    highlight_syntax: Option<String>,
    highlight_theme: Option<String>,
    placeholder_color: Option<Color>,
    selection_color: Option<Color>,
    style: Option<CoreStyle>,
}

impl TextEditorProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            placeholder: String::extract(p, "placeholder"),
            height: Length::extract(p, "height"),
            width: f32::extract(p, "width"),
            min_height: f32::extract(p, "min_height"),
            max_height: f32::extract(p, "max_height"),
            font: Font::extract(p, "font"),
            size: f32::extract(p, "size"),
            line_height: LineHeight::extract(p, "line_height"),
            padding: f32::extract(p, "padding"),
            wrapping: Wrapping::extract(p, "wrapping"),
            input_purpose: InputPurpose::extract(p, "input_purpose")
                .or_else(|| InputPurpose::extract(p, "ime_purpose")),
            highlight_syntax: String::extract(p, "highlight_syntax"),
            highlight_theme: String::extract(p, "highlight_theme"),
            placeholder_color: Color::extract(p, "placeholder_color"),
            selection_color: Color::extract(p, "selection_color"),
            style: CoreStyle::extract(p, "style"),
        }
    }
}

/// Render a text_editor with the provided Content.
fn render_text_editor_with_content<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
    content: &'a text_editor::Content<R>,
) -> Element<'a, Message, Theme, R> {
    let tp = TextEditorProps::from_node(node);
    let props = &node.props;

    let height = tp
        .height
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Shrink);
    let placeholder = tp.placeholder.unwrap_or_default();
    let id = node.id.clone();

    let editor_id = id;
    let mut te = text_editor::TextEditor::<'_, _, Message, iced::Theme, R>::new(content)
        .on_action(move |action| {
            Message::TextEditorAction(ctx.window_id.to_string(), editor_id.clone(), action)
        })
        .height(height);

    if !placeholder.is_empty() {
        te = te.placeholder(placeholder);
    }
    let font = tp.font.map(|f| iced_convert::font(&f)).or(ctx.default_font);
    if let Some(f) = font {
        te = te.font(f);
    }
    if let Some(sz) = tp.size.or(ctx.default_text_size) {
        te = te.size(sz);
    }
    if let Some(ref lh) = tp.line_height {
        te = te.line_height(iced_convert::line_height(*lh));
    }
    if let Some(p) = tp.padding {
        te = te.padding(p);
    }
    if let Some(minh) = tp.min_height {
        te = te.min_height(minh);
    }
    if let Some(maxh) = tp.max_height {
        te = te.max_height(maxh);
    }
    if let Some(w) = tp.wrapping {
        te = te.wrapping(iced_convert::wrapping(w));
    }
    // text_editor.width() takes impl Into<Pixels>, not Length
    if let Some(w) = tp.width {
        te = te.width(w);
    }

    // Key bindings: declarative rules parsed into a closure
    let key_bindings_val = props.get_value("key_bindings");
    if let Some(rules) = key_bindings_val.as_ref().and_then(|v| v.as_array()) {
        let editor_id = node.id.clone();
        let parsed_rules: Vec<KeyRule> = rules
            .iter()
            .filter_map(|rule| {
                let obj = rule.as_object()?;
                let key = obj.get("key").and_then(|v| v.as_str()).map(str::to_owned);
                let named = obj.get("named").and_then(|v| v.as_str()).map(str::to_owned);
                let modifiers: Vec<String> = obj
                    .get("modifiers")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                if key.is_none() && named.is_none() {
                    // Catch-all rules (no key/named) are valid but log a
                    // hint if it looks accidental (has modifiers but no key).
                    if !modifiers.is_empty() {
                        log::warn!(
                            "text_editor key_binding rule has modifiers but no `key` or `named` -- \
                             this will match ANY key with those modifiers [id={}]",
                            node.id
                        );
                    }
                }
                let binding_val = obj.get("binding").cloned().unwrap_or(Value::Null);
                let is_default = binding_val.as_str() == Some("default");
                // Validate binding action name
                if let Some(action_name) = binding_val.as_str() {
                    match action_name {
                        "default" | "copy" | "cut" | "paste" | "select_all" | "enter"
                        | "backspace" | "delete" | "unfocus" | "select_word" | "select_line" => {}
                        other => {
                            log::warn!(
                                "text_editor key_binding: unrecognized binding action {:?} [id={}]",
                                other,
                                node.id,
                            );
                        }
                    }
                }
                Some(KeyRule {
                    key,
                    named,
                    modifiers,
                    binding_val,
                    is_default,
                })
            })
            .collect();

        if !parsed_rules.is_empty() {
            te = te.key_binding(move |key_press: text_editor::KeyPress| {
                for rule in &parsed_rules {
                    // Check modifiers first
                    if !match_modifiers(&key_press.modifiers, &rule.modifiers) {
                        continue;
                    }

                    // Check key match
                    if let Some(ref key_char) = rule.key {
                        // Match via to_latin for layout-independent character matching
                        let latin = key_press.key.to_latin(key_press.physical_key);
                        match latin {
                            Some(c) if c.to_string() == *key_char => {}
                            _ => continue,
                        }
                    } else if let Some(ref named_key) = rule.named
                        && !match_named_key(named_key, &key_press.key)
                    {
                        continue;
                    }
                    // else: no key/named constraint -- matches any key (catch-all rule)

                    // Default binding: delegate to iced's built-in handler
                    if rule.is_default {
                        return text_editor::Binding::from_key_press(key_press);
                    }

                    // Parse the specific binding
                    return parse_binding(&rule.binding_val, &editor_id, ctx.window_id);
                }
                // No rule matched -- no binding
                None
            });
        }
    }

    // Direct color props for placeholder and selection
    let placeholder_color = tp.placeholder_color.as_ref().map(iced_convert::color);
    let selection_color = tp.selection_color.as_ref().map(iced_convert::color);

    // Style closure, shared between plain and highlighted paths
    #[allow(clippy::type_complexity)]
    let style_fn: Option<Box<dyn Fn(&iced::Theme, text_editor::Status) -> text_editor::Style>> =
        match &tp.style {
            Some(CoreStyle::Preset(name)) => match name.as_str() {
                "default" => {
                    if placeholder_color.is_some() || selection_color.is_some() {
                        Some(Box::new(move |theme: &iced::Theme, status| {
                            let mut style = text_editor::default(theme, status);
                            if let Some(pc) = placeholder_color {
                                style.placeholder = pc;
                            }
                            if let Some(sc) = selection_color {
                                style.selection = sc;
                            }
                            style
                        }))
                    } else {
                        Some(Box::new(text_editor::default))
                    }
                }
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        name,
                        "text_editor"
                    );
                    None
                }
            },
            Some(CoreStyle::Custom(style_map)) => {
                let ov = style_overrides_from_style_map(&node.id, style_map, ctx.caches);
                Some(Box::new(move |theme: &iced::Theme, status| {
                    let base_fn: fn(&iced::Theme, text_editor::Status) -> text_editor::Style =
                        match ov.preset_base.as_deref() {
                            Some("default") => text_editor::default,
                            _ => text_editor::default,
                        };
                    let mut style = base_fn(theme, status);
                    apply_text_editor_fields(&mut style, &ov.base);
                    match status {
                        text_editor::Status::Focused { .. } => {
                            if let Some(ref f) = ov.focused {
                                apply_text_editor_fields(&mut style, f);
                            }
                        }
                        text_editor::Status::Hovered => {
                            if let Some(ref f) = ov.hovered {
                                apply_text_editor_fields(&mut style, f);
                            } else {
                                style.background = deviate_background(style.background, 0.1);
                            }
                        }
                        text_editor::Status::Disabled => {
                            if let Some(ref f) = ov.disabled {
                                apply_text_editor_fields(&mut style, f);
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
                    if let Some(pc) = placeholder_color {
                        style.placeholder = pc;
                    }
                    if let Some(sc) = selection_color {
                        style.selection = sc;
                    }
                    style
                }))
            }
            None => {
                if placeholder_color.is_some() || selection_color.is_some() {
                    // No style prop but direct color overrides present
                    Some(Box::new(move |theme: &iced::Theme, status| {
                        let mut style = text_editor::default(theme, status);
                        if let Some(pc) = placeholder_color {
                            style.placeholder = pc;
                        }
                        if let Some(sc) = selection_color {
                            style.selection = sc;
                        }
                        style
                    }))
                } else {
                    None
                }
            }
        };

    if let Some(purpose) = tp.input_purpose {
        te = te.input_purpose(iced_convert::input_purpose(purpose));
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        te = te.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    let wid = widget::Id::from(node.id.clone());

    // Syntax highlighting changes the generic type parameter, so we must
    // branch here and produce Element from each path separately.
    if let Some(syntax) = tp.highlight_syntax {
        let theme = match tp.highlight_theme.as_deref() {
            Some("base16_mocha") => iced::highlighter::Theme::Base16Mocha,
            Some("base16_ocean") => iced::highlighter::Theme::Base16Ocean,
            Some("base16_eighties") => iced::highlighter::Theme::Base16Eighties,
            Some("inspired_github") => iced::highlighter::Theme::InspiredGitHub,
            _ => iced::highlighter::Theme::SolarizedDark,
        };
        // Set ID before highlight() -- .id() is only available on PlainText variant
        te = te.id(wid);
        let mut hl = te.highlight(&syntax, theme);
        if let Some(sf) = style_fn {
            hl = hl.style(sf);
        }
        return hl.into();
    }

    {
        if let Some(sf) = style_fn {
            te = te.style(sf);
        }
        te = te.id(wid);
        te.into()
    }
}
