use iced::widget::Image;
use iced::widget::image::FilterMethod;
use iced::{Element, Length, Radians, Rotation, Theme};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widgets::helpers::*;

pub(crate) struct ImageWidget;

impl<R: PlushieRenderer> PlushieWidget<R> for ImageWidget {
    fn type_names(&self) -> &[&str] {
        &["image"]
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let props = node.props.as_object();
        let width = prop_length(props, "width", Length::Shrink);
        let height = prop_length(props, "height", Length::Shrink);
        let content_fit = prop_content_fit(props);

        // source can be a string (file path) or an object with a "handle" field
        // (in-memory image from the registry).
        let source_val = props.and_then(|p| p.get("source"));
        if source_val.is_none() {
            log::warn!("[id={}] image: no 'source' prop specified", node.id);
        }
        let handle: iced::widget::image::Handle = match source_val {
            Some(Value::Object(obj)) => {
                if let Some(name) = obj.get("handle").and_then(|v| v.as_str()) {
                    match ctx.images.get(name) {
                        Some(h) => h.clone(),
                        None => {
                            log::warn!(
                                "[id={}] image: unknown registry handle: {name}",
                                node.id
                            );
                            iced::widget::image::Handle::from_bytes(vec![])
                        }
                    }
                } else {
                    iced::widget::image::Handle::from_bytes(vec![])
                }
            }
            _ => {
                let path = prop_str(props, "source").unwrap_or_default();
                iced::widget::image::Handle::from_path(path)
            }
        };

        let mut img = Image::new(handle).width(width).height(height);
        if let Some(cf) = content_fit {
            img = img.content_fit(cf);
        }
        if let Some(r) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "rotation")
        {
            img = img.rotation(Rotation::from(Radians(r.to_radians())));
        }
        if let Some(o) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "opacity")
        {
            img = img.opacity(o);
        }
        if let Some(br) = prop_animated_f32(
            &ctx.caches.interpolated_props,
            &node.id,
            props,
            "border_radius",
        ) {
            img = img.border_radius(br);
        }
        if let Some(fm_str) = prop_str(props, "filter_method") {
            let fm = match fm_str.to_ascii_lowercase().as_str() {
                "nearest" => FilterMethod::Nearest,
                _ => FilterMethod::Linear,
            };
            img = img.filter_method(fm);
        }
        if let Some(expand) = prop_bool(props, "expand") {
            img = img.expand(expand);
        }
        if let Some(scale) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "scale")
        {
            img = img.scale(scale);
        }
        if let Some(alt) = prop_str(props, "alt") {
            img = img.alt(alt);
        }
        if let Some(desc) = prop_str(props, "description") {
            img = img.description(desc);
        }
        if prop_bool_default(props, "decorative", false) {
            img = img.decorative();
        }
        // crop: {"x": u32, "y": u32, "width": u32, "height": u32}
        if let Some(crop_obj) = props
            .and_then(|p| p.get("crop"))
            .and_then(|v| v.as_object())
        {
            let cx = crop_obj.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let cy = crop_obj.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let cw = crop_obj
                .get("width")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let ch = crop_obj
                .get("height")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            img = img.crop(iced::Rectangle {
                x: cx,
                y: cy,
                width: cw,
                height: ch,
            });
        }

        img.into()
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(ImageWidget)
    }
}
