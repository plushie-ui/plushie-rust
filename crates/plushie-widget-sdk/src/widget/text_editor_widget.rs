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
    Color, Font, InputPurpose, Length, LineHeight, PlushieType, Style as CoreStyle, TextDirection,
    TextMotion, Wrapping,
};

// ---------------------------------------------------------------------------
// Key binding helpers
// ---------------------------------------------------------------------------

/// Convert a logical text motion into an iced Motion.
fn text_motion(motion: TextMotion, direction: TextDirection) -> text_editor::Motion {
    use text_editor::Motion;

    match motion {
        TextMotion::Backward => match direction {
            TextDirection::Rtl => Motion::Right,
            TextDirection::Auto | TextDirection::Ltr => Motion::Left,
        },
        TextMotion::Forward => match direction {
            TextDirection::Rtl => Motion::Left,
            TextDirection::Auto | TextDirection::Ltr => Motion::Right,
        },
        TextMotion::Up => Motion::Up,
        TextMotion::Down => Motion::Down,
        TextMotion::WordBackward => match direction {
            TextDirection::Rtl => Motion::WordRight,
            TextDirection::Auto | TextDirection::Ltr => Motion::WordLeft,
        },
        TextMotion::WordForward => match direction {
            TextDirection::Rtl => Motion::WordLeft,
            TextDirection::Auto | TextDirection::Ltr => Motion::WordRight,
        },
        TextMotion::LineStart => Motion::Home,
        TextMotion::LineEnd => Motion::End,
        TextMotion::PageUp => Motion::PageUp,
        TextMotion::PageDown => Motion::PageDown,
        TextMotion::DocumentStart => Motion::DocumentStart,
        TextMotion::DocumentEnd => Motion::DocumentEnd,
    }
}

/// Parse a pre-extracted motion string into an iced Motion.
fn parse_motion(s: &str, direction: TextDirection) -> Option<text_editor::Motion> {
    use text_editor::Motion;

    match s {
        "left" => Some(Motion::Left),
        "right" => Some(Motion::Right),
        "word_left" => Some(Motion::WordLeft),
        "word_right" => Some(Motion::WordRight),
        "home" => Some(Motion::Home),
        "end" => Some(Motion::End),
        _ => TextMotion::wire_decode(&Value::String(s.to_owned()))
            .map(|motion| text_motion(motion, direction)),
    }
}

/// Parse a JSON binding value into an iced Binding.
fn parse_binding(
    val: &Value,
    id: &str,
    window_id: &str,
    direction: TextDirection,
) -> Option<text_editor::Binding<Message>> {
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
                .and_then(|s| parse_motion(s, direction))
            {
                return Some(Binding::Move(m));
            }
            if let Some(m) = obj
                .get("select")
                .and_then(|v| v.as_str())
                .and_then(|s| parse_motion(s, direction))
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
                    .filter_map(|v| parse_binding(v, id, window_id, direction))
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

fn binding_config_is_valid(val: &Value, direction: TextDirection) -> bool {
    match val {
        Value::String(s) => matches!(
            s.as_str(),
            "default"
                | "copy"
                | "cut"
                | "paste"
                | "select_all"
                | "enter"
                | "backspace"
                | "delete"
                | "unfocus"
                | "select_word"
                | "select_line"
        ),
        Value::Object(obj) => {
            obj.get("move")
                .and_then(|v| v.as_str())
                .and_then(|s| parse_motion(s, direction))
                .is_some()
                || obj
                    .get("select")
                    .and_then(|v| v.as_str())
                    .and_then(|s| parse_motion(s, direction))
                    .is_some()
                || obj.get("insert").and_then(|v| v.as_str()).is_some_and(|s| {
                    let mut chars = s.chars();
                    chars.next().is_some() && chars.next().is_none()
                })
                || obj.get("custom").and_then(|v| v.as_str()).is_some()
                || obj
                    .get("sequence")
                    .and_then(|v| v.as_array())
                    .is_some_and(|seq| seq.iter().any(|v| binding_config_is_valid(v, direction)))
        }
        _ => false,
    }
}

/// Check if a KeyPress matches the modifiers specified in a binding rule.
///
/// Requires an exact match: all required modifiers must be pressed, and
/// no extra modifiers may be active. This prevents a rule for `["shift"]`
/// from firing on `shift+ctrl+A`.
fn match_modifiers(mods: &keyboard::Modifiers, required: &[String]) -> bool {
    let mut expected = keyboard::Modifiers::empty();

    for modifier in required {
        match modifier.as_str() {
            "shift" => expected |= keyboard::Modifiers::SHIFT,
            "ctrl" => expected |= keyboard::Modifiers::CTRL,
            "alt" => expected |= keyboard::Modifiers::ALT,
            "logo" => expected |= keyboard::Modifiers::LOGO,
            _ => return false,
        }
    }

    mods.shift() == expected.shift()
        && mods.control() == expected.control()
        && mods.alt() == expected.alt()
        && mods.logo() == expected.logo()
}

fn named_key_target(named_key: &str) -> Option<keyboard::key::Named> {
    use keyboard::key::Named;

    match named_key {
        "Enter" => Some(Named::Enter),
        "Backspace" => Some(Named::Backspace),
        "Delete" => Some(Named::Delete),
        "Escape" => Some(Named::Escape),
        "Tab" => Some(Named::Tab),
        "Space" => Some(Named::Space),
        "ArrowLeft" => Some(Named::ArrowLeft),
        "ArrowRight" => Some(Named::ArrowRight),
        "ArrowUp" => Some(Named::ArrowUp),
        "ArrowDown" => Some(Named::ArrowDown),
        "Home" => Some(Named::Home),
        "End" => Some(Named::End),
        "PageUp" => Some(Named::PageUp),
        "PageDown" => Some(Named::PageDown),
        "F1" => Some(Named::F1),
        "F2" => Some(Named::F2),
        "F3" => Some(Named::F3),
        "F4" => Some(Named::F4),
        "F5" => Some(Named::F5),
        "F6" => Some(Named::F6),
        "F7" => Some(Named::F7),
        "F8" => Some(Named::F8),
        "F9" => Some(Named::F9),
        "F10" => Some(Named::F10),
        "F11" => Some(Named::F11),
        "F12" => Some(Named::F12),
        _ => None,
    }
}

/// Match a named key string against a KeyPress key.
///
/// Character keys must use the `key` rule field, which matches through
/// `Key::to_latin`.
fn match_named_key(named_key: &str, key: &keyboard::Key) -> bool {
    let Some(target) = named_key_target(named_key) else {
        return false;
    };

    matches!(key, keyboard::Key::Named(n) if *n == target)
}

fn warn_key_rule_issues(rule: &KeyRule, node_id: &str) {
    for modifier in &rule.modifiers {
        match modifier.as_str() {
            "shift" | "ctrl" | "alt" | "logo" => {}
            "command" | "jump" => {
                log::warn!(
                    "text_editor key_binding modifier {:?} is unsupported because it maps to \
                     different physical keys by platform, use `ctrl`, `alt`, `logo`, or \
                     `shift` [id={}]",
                    modifier,
                    node_id
                );
            }
            other => {
                log::warn!(
                    "text_editor key_binding modifier {:?} is unsupported [id={}]",
                    other,
                    node_id
                );
            }
        }
    }

    if let Some(named_key) = &rule.named
        && named_key_target(named_key).is_none()
    {
        log::warn!(
            "text_editor key_binding named key {:?} is unsupported, `named` only matches named \
             keys like Enter or ArrowLeft, use `key` for character keys [id={}]",
            named_key,
            node_id
        );
    }
}

fn parse_modifier_names(value: Option<&Value>, node_id: &str) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    let Some(arr) = value.as_array() else {
        log::warn!(
            "text_editor key_binding modifiers value {:?} is unsupported, expected an array \
             of modifier names [id={}]",
            value,
            node_id
        );
        return vec![String::new()];
    };

    arr.iter()
        .map(|modifier| {
            modifier.as_str().map(str::to_owned).unwrap_or_else(|| {
                log::warn!(
                    "text_editor key_binding modifier value {:?} is unsupported, modifier names \
                     must be strings [id={}]",
                    modifier,
                    node_id
                );
                String::new()
            })
        })
        .collect()
}

fn key_rule_matches(rule: &KeyRule, key_press: &text_editor::KeyPress) -> bool {
    if !match_modifiers(&key_press.modifiers, &rule.modifiers) {
        return false;
    }

    if let Some(ref key_char) = rule.key {
        let latin = key_press.key.to_latin(key_press.physical_key);
        matches!(latin, Some(c) if c.to_string() == *key_char)
    } else if let Some(ref named_key) = rule.named {
        match_named_key(named_key, &key_press.key)
    } else {
        true
    }
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
    /// Nodes that should emit a paste event when paste edits arrive.
    paste_enabled: std::collections::HashSet<(String, String)>,
}

impl<R: PlushieRenderer> TextEditorWidget<R> {
    pub(crate) fn new() -> Self {
        Self {
            contents: std::collections::HashMap::new(),
            content_hashes: std::collections::HashMap::new(),
            paste_enabled: std::collections::HashSet::new(),
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
        if prop_bool_default(props, "on_paste", false) {
            self.paste_enabled.insert(key.clone());
        } else {
            self.paste_enabled.remove(&key);
        }
        let raw = crate::prop_helpers::prop_str(props, "content").unwrap_or_default();
        let content_str = crate::shared_state::enforce_content_cap(
            &node.id,
            "content",
            raw,
            crate::shared_state::MAX_TEXT_EDITOR_BYTES,
        );
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

    fn handle_message(&mut self, msg: &Message) -> crate::registry::HandleResult {
        use crate::registry::HandleResult;
        use crate::shared_state::hash_str;

        match msg {
            Message::TextEditorAction(window_id, id, action) => {
                let key = (window_id.to_string(), id.to_string());
                if let Some(content) = self.contents.get_mut(&key) {
                    let is_edit = action.is_edit();
                    let pasted_text = match action {
                        text_editor::Action::Edit(text_editor::Edit::Paste(text))
                            if self.paste_enabled.contains(&key) =>
                        {
                            Some(text.as_ref().clone())
                        }
                        _ => None,
                    };
                    content.perform(action.clone());
                    if is_edit {
                        let new_text = content.text();
                        self.content_hashes.insert(key, hash_str(&new_text));
                        let mut events =
                            vec![crate::protocol::OutgoingEvent::input(id.clone(), new_text)];
                        if let Some(pasted_text) = pasted_text {
                            events.push(crate::protocol::OutgoingEvent::generic(
                                "paste",
                                id.clone(),
                                Some(Value::String(pasted_text)),
                            ));
                        }
                        return HandleResult::emit(events);
                    }
                }
                HandleResult::consume()
            }
            _ => HandleResult::Fallthrough,
        }
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        let props = &node.props;
        crate::prop_helpers::prop_str(props, "placeholder").map(A11yOverrides::with_description)
    }

    fn prune_stale(&mut self, live_ids: &std::collections::HashSet<(String, String)>) {
        self.contents.retain(|k, _| live_ids.contains(k));
        self.content_hashes.retain(|k, _| live_ids.contains(k));
        self.paste_enabled.retain(|k| live_ids.contains(k));
    }

    fn fresh_for_session(&self) -> Box<dyn PlushieWidget<R>> {
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
    text_direction: Option<TextDirection>,
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
            text_direction: TextDirection::extract(p, "text_direction"),
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
    let text_direction = tp.text_direction.unwrap_or(TextDirection::Auto);
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
    let size = prop_animated_f32(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "size",
    )
    .or(tp.size)
    .or(ctx.default_text_size);
    if let Some(sz) = size {
        te = te.size(sz);
    }
    let line_height = prop_animated_f32(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "line_height",
    )
    .map(LineHeight::Relative)
    .or(tp.line_height);
    if let Some(lh) = line_height {
        te = te.line_height(iced_convert::line_height(lh));
    }
    if let Some(p) = tp.padding {
        te = te.padding(p);
    }
    let min_height = prop_animated_f32(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "min_height",
    )
    .or(tp.min_height);
    if let Some(minh) = min_height {
        te = te.min_height(minh);
    }
    let max_height = prop_animated_f32(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "max_height",
    )
    .or(tp.max_height);
    if let Some(maxh) = max_height {
        te = te.max_height(maxh);
    }
    if let Some(w) = tp.wrapping {
        te = te.wrapping(iced_convert::wrapping(w));
    }
    // text_editor.width() takes impl Into<Pixels>, not Length
    let width = prop_animated_f32(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "width",
    )
    .or(tp.width);
    if let Some(w) = width {
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
                let modifiers = parse_modifier_names(obj.get("modifiers"), &node.id);
                if key.is_none() && named.is_none() {
                    // Catch-all rules (no key/named) are valid but log a
                    // hint if it looks accidental (has modifiers but no key).
                    if !modifiers.is_empty() {
                        log::warn!(
                            "text_editor key_binding rule has modifiers but no `key` or `named`, \
                             this will match ANY key with those modifiers [id={}]",
                            node.id
                        );
                    }
                }
                let binding_val = obj.get("binding").cloned().unwrap_or(Value::Null);
                let is_default = binding_val.as_str() == Some("default");
                if !binding_config_is_valid(&binding_val, text_direction) {
                    if let Some(action_name) = binding_val.as_str() {
                        log::warn!(
                            "text_editor key_binding: unrecognized binding action {:?} [id={}]",
                            action_name,
                            node.id,
                        );
                    } else {
                        log::warn!(
                            "text_editor key_binding: unsupported binding value {:?} [id={}]",
                            binding_val,
                            node.id,
                        );
                    }
                    return None;
                }
                let rule = KeyRule {
                    key,
                    named,
                    modifiers,
                    binding_val,
                    is_default,
                };
                warn_key_rule_issues(&rule, &node.id);
                Some(rule)
            })
            .collect();

        if !parsed_rules.is_empty() {
            te = te.key_binding(move |key_press: text_editor::KeyPress| {
                for rule in &parsed_rules {
                    if !key_rule_matches(rule, &key_press) {
                        continue;
                    }

                    // Default binding: delegate to iced's built-in handler
                    if rule.is_default {
                        return text_editor::Binding::from_key_press(key_press);
                    }

                    // Parse the specific binding
                    return parse_binding(
                        &rule.binding_val,
                        &editor_id,
                        ctx.window_id,
                        text_direction,
                    );
                }
                // No rule matched: no binding
                None
            });
        }
    }

    // Direct color props for placeholder and selection
    let placeholder_color = prop_animated_color(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "placeholder_color",
    )
    .or_else(|| tp.placeholder_color.as_ref().map(iced_convert::color));
    let selection_color = prop_animated_color(
        &ctx.caches.interpolated_props,
        &node.id,
        &node.props,
        "selection_color",
    )
    .or_else(|| tp.selection_color.as_ref().map(iced_convert::color));
    let cursor_color = ctx.theme_chrome.cursor_color;

    // Style closure, shared between plain and highlighted paths
    #[allow(clippy::type_complexity)]
    let style_fn: Option<Box<dyn Fn(&iced::Theme, text_editor::Status) -> text_editor::Style>> =
        match &tp.style {
            Some(CoreStyle::Preset(name)) => match name.as_str() {
                "default" => {
                    if placeholder_color.is_some()
                        || selection_color.is_some()
                        || cursor_color.is_some()
                    {
                        Some(Box::new(move |theme: &iced::Theme, status| {
                            let mut style = text_editor::default(theme, status);
                            apply_text_editor_cursor_chrome(&mut style, status, cursor_color);
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
                    apply_text_editor_cursor_chrome(&mut style, status, cursor_color);
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
                if placeholder_color.is_some()
                    || selection_color.is_some()
                    || cursor_color.is_some()
                {
                    // No style prop but direct color overrides present
                    Some(Box::new(move |theme: &iced::Theme, status| {
                        let mut style = text_editor::default(theme, status);
                        apply_text_editor_cursor_chrome(&mut style, status, cursor_color);
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
        te = te.on_status_change(move |status| Message::Event {
            window_id: status_wid.clone(),
            id: status_id.clone(),
            value: serde_json::Value::String(status.to_owned()),
            family: "status".into(),
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
        // Set ID before highlight() since .id() is only available on PlainText variant
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{HandleResult, PlushieWidget};
    use serde_json::json;
    use std::sync::Arc;

    fn modifier_names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn key_rule(key: Option<&str>, named: Option<&str>, modifiers: &[&str]) -> KeyRule {
        KeyRule {
            key: key.map(str::to_owned),
            named: named.map(str::to_owned),
            modifiers: modifier_names(modifiers),
            binding_val: Value::Null,
            is_default: false,
        }
    }

    fn key_press(
        key: keyboard::Key,
        physical_key: keyboard::key::Physical,
        modifiers: keyboard::Modifiers,
    ) -> text_editor::KeyPress {
        text_editor::KeyPress {
            key: key.clone(),
            modified_key: key,
            physical_key,
            modifiers,
            text: None,
            status: text_editor::Status::Active,
        }
    }

    #[test]
    fn paste_action_emits_paste_event_when_enabled() {
        let mut widget = TextEditorWidget::<iced::Renderer>::new();
        let node = crate::testing::node_with_props(
            "editor",
            "text_editor",
            json!({"content": "", "on_paste": true}),
        );
        widget.prepare(&node, "main", &iced::Theme::Dark);

        let result = widget.handle_message(&Message::TextEditorAction(
            "main".to_string(),
            "editor".to_string(),
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new("clip".to_string()))),
        ));

        let HandleResult::Handled(events) = result else {
            panic!("text editor action should be handled");
        };
        assert!(events.iter().any(|event| {
            event.family == "paste"
                && event.id == "editor"
                && event.value == Some(Value::String("clip".to_string()))
        }));
    }

    #[test]
    fn paste_action_does_not_emit_paste_event_when_disabled() {
        let mut widget = TextEditorWidget::<iced::Renderer>::new();
        let node = crate::testing::node_with_props(
            "editor",
            "text_editor",
            json!({"content": "", "on_paste": false}),
        );
        widget.prepare(&node, "main", &iced::Theme::Dark);

        let result = widget.handle_message(&Message::TextEditorAction(
            "main".to_string(),
            "editor".to_string(),
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new("clip".to_string()))),
        ));

        let HandleResult::Handled(events) = result else {
            panic!("text editor action should be handled");
        };
        assert!(events.iter().all(|event| event.family != "paste"));
    }

    #[test]
    fn text_motion_aliases_stay_physical_in_rtl() {
        assert_eq!(
            parse_motion("left", TextDirection::Rtl),
            Some(text_editor::Motion::Left)
        );
        assert_eq!(
            parse_motion("right", TextDirection::Rtl),
            Some(text_editor::Motion::Right)
        );
        assert_eq!(
            parse_motion("word_left", TextDirection::Rtl),
            Some(text_editor::Motion::WordLeft)
        );
        assert_eq!(
            parse_motion("word_right", TextDirection::Rtl),
            Some(text_editor::Motion::WordRight)
        );
    }

    #[test]
    fn text_motion_logical_values_map_with_rtl_direction() {
        assert_eq!(
            parse_motion("backward", TextDirection::Rtl),
            Some(text_editor::Motion::Right)
        );
        assert_eq!(
            parse_motion("forward", TextDirection::Rtl),
            Some(text_editor::Motion::Left)
        );
        assert_eq!(
            parse_motion("word_backward", TextDirection::Rtl),
            Some(text_editor::Motion::WordRight)
        );
        assert_eq!(
            parse_motion("word_forward", TextDirection::Rtl),
            Some(text_editor::Motion::WordLeft)
        );
    }

    #[test]
    fn text_motion_logical_values_use_ltr_for_auto() {
        assert_eq!(
            parse_motion("backward", TextDirection::Auto),
            Some(text_editor::Motion::Left)
        );
        assert_eq!(
            parse_motion("forward", TextDirection::Auto),
            Some(text_editor::Motion::Right)
        );
    }

    #[test]
    fn modifier_matching_requires_exact_physical_modifiers() {
        assert!(match_modifiers(
            &keyboard::Modifiers::SHIFT,
            &modifier_names(&["shift"])
        ));
        assert!(!match_modifiers(
            &(keyboard::Modifiers::SHIFT | keyboard::Modifiers::CTRL),
            &modifier_names(&["shift"])
        ));
        assert!(!match_modifiers(
            &keyboard::Modifiers::SHIFT,
            &modifier_names(&["ctrl"])
        ));
        assert!(match_modifiers(
            &(keyboard::Modifiers::SHIFT | keyboard::Modifiers::ALT),
            &modifier_names(&["shift", "alt"])
        ));
    }

    #[test]
    fn platform_modifier_aliases_never_match() {
        assert!(!match_modifiers(
            &keyboard::Modifiers::COMMAND,
            &modifier_names(&["command"])
        ));
        assert!(!match_modifiers(
            &keyboard::Modifiers::ALT,
            &modifier_names(&["command"])
        ));
        assert!(!match_modifiers(
            &keyboard::Modifiers::ALT,
            &modifier_names(&["jump"])
        ));
        assert!(!match_modifiers(
            &keyboard::Modifiers::CTRL,
            &modifier_names(&["jump"])
        ));
        assert!(!match_modifiers(
            &keyboard::Modifiers::empty(),
            &modifier_names(&["bogus"])
        ));
    }

    #[test]
    fn malformed_modifier_values_never_match() {
        let modifiers = parse_modifier_names(
            Some(&serde_json::json!(["ctrl", 123])),
            "editor-with-bad-binding",
        );

        assert!(!match_modifiers(&keyboard::Modifiers::CTRL, &modifiers));
    }

    #[test]
    fn malformed_modifier_lists_never_match() {
        let modifiers =
            parse_modifier_names(Some(&serde_json::json!(123)), "editor-with-bad-binding");

        assert!(!match_modifiers(&keyboard::Modifiers::empty(), &modifiers));
    }

    #[test]
    fn named_key_matching_only_accepts_named_keys() {
        assert!(match_named_key(
            "Enter",
            &keyboard::Key::Named(keyboard::key::Named::Enter)
        ));
        assert!(match_named_key(
            "ArrowLeft",
            &keyboard::Key::Named(keyboard::key::Named::ArrowLeft)
        ));
        assert!(!match_named_key("A", &keyboard::Key::Character("A".into())));
        assert!(!match_named_key(
            "Enter",
            &keyboard::Key::Character("Enter".into())
        ));
    }

    #[test]
    fn character_key_rules_match_through_key_field() {
        let rule = key_rule(Some("A"), None, &[]);
        let press = key_press(
            keyboard::Key::Character("A".into()),
            keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
            keyboard::Modifiers::empty(),
        );

        assert!(key_rule_matches(&rule, &press));
    }

    #[test]
    fn named_character_rules_do_not_match_character_keys() {
        let rule = key_rule(None, Some("A"), &[]);
        let press = key_press(
            keyboard::Key::Character("A".into()),
            keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified),
            keyboard::Modifiers::empty(),
        );

        assert!(!key_rule_matches(&rule, &press));
    }

    #[test]
    fn key_rule_matching_respects_modifiers_and_named_keys() {
        let rule = key_rule(None, Some("ArrowLeft"), &["shift"]);
        let matching_press = key_press(
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
            keyboard::key::Physical::Code(keyboard::key::Code::ArrowLeft),
            keyboard::Modifiers::SHIFT,
        );
        let extra_modifier_press = key_press(
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft),
            keyboard::key::Physical::Code(keyboard::key::Code::ArrowLeft),
            keyboard::Modifiers::SHIFT | keyboard::Modifiers::CTRL,
        );

        assert!(key_rule_matches(&rule, &matching_press));
        assert!(!key_rule_matches(&rule, &extra_modifier_press));
    }

    #[test]
    fn invalid_binding_configs_are_rejected_before_matching() {
        assert!(!binding_config_is_valid(
            &serde_json::json!("launch_missiles"),
            TextDirection::Auto
        ));
        assert!(!binding_config_is_valid(
            &serde_json::json!({"move": "sideways"}),
            TextDirection::Auto
        ));
        assert!(binding_config_is_valid(
            &serde_json::json!("default"),
            TextDirection::Auto
        ));
        assert!(binding_config_is_valid(
            &serde_json::json!({"sequence": ["copy", {"custom": "tag"}]}),
            TextDirection::Auto
        ));
    }
}
