use iced::widget::text_input;
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::a11y::A11yOverrides;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

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
        let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
        crate::prop_helpers::prop_str(props, "placeholder").map(A11yOverrides::with_description)
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(TextInputWidget)
    }
}

/// Parse an input purpose string into the corresponding iced `Purpose`.
///
/// Accepts the canonical `input_purpose` values. The `ime_purpose`
/// prop name is handled by callers as a fallback alias.
fn parse_input_purpose(s: &str) -> Option<iced::advanced::input_method::Purpose> {
    use iced::advanced::input_method::Purpose;
    match s {
        "normal" => Some(Purpose::Normal),
        "secure" => Some(Purpose::Secure),
        "terminal" => Some(Purpose::Terminal),
        "number" => Some(Purpose::Number),
        "decimal" => Some(Purpose::Decimal),
        "phone" => Some(Purpose::Phone),
        "email" => Some(Purpose::Email),
        "url" => Some(Purpose::Url),
        "search" => Some(Purpose::Search),
        _ => {
            log::warn!("unknown input_purpose {:?}, ignoring", s);
            None
        }
    }
}

fn render_text_input<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
) -> Element<'a, Message, Theme, R> {
    let props_cow = node.props.as_value_cow();
        let props = props_cow.as_object();
    let value = prop_str(props, "value").unwrap_or_default();
    let placeholder = prop_str(props, "placeholder").unwrap_or_default();
    let width = prop_length(props, "width", Length::Fill);
    let size = prop_f32(props, "size").or(ctx.default_text_size);
    let padding = parse_padding_value(props);
    let secure = prop_bool_default(props, "secure", false);
    let id = node.id.clone();
    let has_on_submit = prop_bool_default(props, "on_submit", false);

    let window_id = ctx.window_id.to_string();
    let mut ti = text_input(&placeholder, &value)
        .on_input(move |v| Message::Input(window_id.clone(), id.clone(), v))
        .width(width)
        .secure(secure);

    if let Some(p) = padding {
        ti = ti.padding(p);
    }

    if let Some(purpose_str) =
        prop_str(props, "input_purpose").or_else(|| prop_str(props, "ime_purpose"))
    {
        let purpose = parse_input_purpose(&purpose_str);
        if let Some(p) = purpose {
            ti = ti.input_purpose(p);
        }
    }

    if let Some(s) = size {
        ti = ti.size(s);
    }
    let font = props
        .and_then(|p| p.get("font"))
        .map(parse_font)
        .or(ctx.default_font);
    if let Some(f) = font {
        ti = ti.font(f);
    }
    if let Some(lh) = parse_line_height(props) {
        ti = ti.line_height(lh);
    }
    if let Some(ax) = props
        .and_then(|p| p.get("align_x"))
        .and_then(|v| v.as_str())
        .and_then(value_to_horizontal_alignment)
    {
        ti = ti.align_x(ax);
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
        .and_then(|p| p.get("icon"))
        .and_then(parse_text_input_icon)
    {
        ti = ti.icon(icon);
    }

    // Widget ID: default to node.id, allow prop override.
    let widget_id = prop_str(props, "id").unwrap_or_else(|| node.id.clone());
    ti = ti.id(widget_id);

    // Direct color props for placeholder and selection, applied on top of
    // any style preset or StyleMap.
    let placeholder_color = prop_color(props, "placeholder_color");
    let selection_color = prop_color(props, "selection_color");

    // Style: string name or style map object
    let has_color_overrides = placeholder_color.is_some() || selection_color.is_some();
    if let Some(style_val) = props.and_then(|p| p.get("style")) {
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
