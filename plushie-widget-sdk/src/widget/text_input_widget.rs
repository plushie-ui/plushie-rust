use iced::widget::text_input;
use iced::{Element, Theme};

use crate::PlushieRenderer;
use crate::a11y::A11yOverrides;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{
    Color, Font, HorizontalAlignment, InputPurpose, Length, LineHeight, Padding, PlushieType,
};

struct TextInputProps {
    value: Option<String>,
    placeholder: Option<String>,
    padding: Option<Padding>,
    width: Option<Length>,
    size: Option<f32>,
    font: Option<Font>,
    line_height: Option<LineHeight>,
    align_x: Option<HorizontalAlignment>,
    input_purpose: Option<InputPurpose>,
    placeholder_color: Option<Color>,
    selection_color: Option<Color>,
}

impl TextInputProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            value: String::extract(p, "value"),
            placeholder: String::extract(p, "placeholder"),
            padding: Padding::extract(p, "padding"),
            width: Length::extract(p, "width"),
            size: f32::extract(p, "size"),
            font: Font::extract(p, "font"),
            line_height: LineHeight::extract(p, "line_height"),
            align_x: HorizontalAlignment::extract(p, "align_x"),
            input_purpose: InputPurpose::extract(p, "input_purpose")
                .or_else(|| InputPurpose::extract(p, "ime_purpose")),
            placeholder_color: Color::extract(p, "placeholder_color"),
            selection_color: Color::extract(p, "selection_color"),
        }
    }
}

pub(crate) struct TextInputWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for TextInputWidget {
    fn type_names(&self) -> &[&str] {
        &["text_input"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        render_text_input(node, *ctx)
    }

    fn infer_a11y(&self, node: &TreeNode) -> Option<A11yOverrides> {
        let props = &node.props;
        crate::prop_helpers::prop_str(props, "placeholder").map(A11yOverrides::with_description)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TextInputWidget)
    }
}

fn render_text_input<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let tp = TextInputProps::from_node(node);
    let props = &node.props;

    let value = tp.value.unwrap_or_default();
    let placeholder = tp.placeholder.unwrap_or_default();
    let width = tp
        .width
        .as_ref()
        .map(iced_convert::length)
        .unwrap_or(iced::Length::Fill);
    let size = tp.size.or(ctx.default_text_size);
    let secure = prop_bool_default(props, "secure", false);
    let id = node.id.clone();
    let has_on_submit = prop_bool_default(props, "on_submit", false);

    let window_id = ctx.window_id.to_string();
    let mut ti = text_input(&placeholder, &value)
        .on_input(move |v| Message::Input(window_id.clone(), id.clone(), v))
        .width(width)
        .secure(secure);

    if let Some(ref p) = tp.padding {
        ti = ti.padding(iced_convert::padding(p));
    }

    if let Some(purpose) = tp.input_purpose {
        ti = ti.input_purpose(iced_convert::input_purpose(purpose));
    }

    if let Some(s) = size {
        ti = ti.size(s);
    }
    let font = tp
        .font
        .map(|f| iced_convert::font(&f))
        .or(ctx.default_font);
    if let Some(f) = font {
        ti = ti.font(f);
    }
    if let Some(ref lh) = tp.line_height {
        ti = ti.line_height(iced_convert::line_height(*lh));
    }
    if let Some(ax) = tp.align_x {
        ti = ti.align_x(iced_convert::horizontal_alignment(ax));
    }

    if has_on_submit {
        let submit_window_id = ctx.window_id.to_string();
        let submit_id = node.id.clone();
        let submit_value = value.clone();
        ti = ti.on_submit(Message::Submit(submit_window_id, submit_id, submit_value));
    }

    if prop_bool_default(props, "on_paste", false) {
        let paste_window_id = ctx.window_id.to_string();
        let paste_id = node.id.clone();
        ti = ti
            .on_paste(move |text| Message::Paste(paste_window_id.clone(), paste_id.clone(), text));
    }

    {
        let status_wid = ctx.window_id.to_string();
        let status_id = node.id.clone();
        ti = ti.on_status_change(move |status| {
            Message::StatusChanged(status_wid.clone(), status_id.clone(), status.to_string())
        });
    }

    if let Some(icon) = props
        .get_value("icon")
        .as_ref()
        .and_then(parse_text_input_icon)
    {
        ti = ti.icon(icon);
    }

    // Widget ID: default to node.id, allow prop override.
    let widget_id = prop_str(props, "id").unwrap_or_else(|| node.id.clone());
    ti = ti.id(widget_id);

    // Direct color props for placeholder and selection, applied on top of
    // any style preset or StyleMap.
    let placeholder_color = tp.placeholder_color.as_ref().map(iced_convert::color);
    let selection_color = tp.selection_color.as_ref().map(iced_convert::color);

    // Style: string name or style map object
    let has_color_overrides = placeholder_color.is_some() || selection_color.is_some();
    if let Some(style_val) = props.get_value("style") {
        if let Some(style_name) = style_val.as_str() {
            ti = match style_name {
                "default" => {
                    if has_color_overrides {
                        ti.style(move |theme: &iced::Theme, status| {
                            let mut style = text_input::default(theme, status);
                            if let Some(pc) = placeholder_color {
                                style.placeholder = pc;
                            }
                            if let Some(sc) = selection_color {
                                style.selection = sc;
                            }
                            style
                        })
                    } else {
                        ti.style(text_input::default)
                    }
                }
                _ => {
                    log::warn!(
                        "unknown style {:?} for widget type {:?}, using default",
                        style_name,
                        "text_input"
                    );
                    ti
                }
            };
        } else if let Some(obj) = style_val.as_object() {
            let ov = get_style_overrides(&node.id, obj, ctx.caches);
            ti = ti.style(move |theme: &iced::Theme, status| {
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
                if let Some(pc) = placeholder_color {
                    style.placeholder = pc;
                }
                if let Some(sc) = selection_color {
                    style.selection = sc;
                }
                style
            });
        }
    } else if has_color_overrides {
        // No style prop but direct color overrides present
        ti = ti.style(move |theme: &iced::Theme, status| {
            let mut style = text_input::default(theme, status);
            if let Some(pc) = placeholder_color {
                style.placeholder = pc;
            }
            if let Some(sc) = selection_color {
                style.selection = sc;
            }
            style
        });
    }

    ti.into()
}
