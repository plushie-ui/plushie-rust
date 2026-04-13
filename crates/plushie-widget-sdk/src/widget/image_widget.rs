use iced::widget::Image;
use iced::{Element, Radians, Rotation, Theme};
use serde_json::Value;

use crate::PlushieRenderer;
use crate::iced_convert;
use crate::message::Message;
use crate::protocol::TreeNode;
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widget::helpers::*;

use plushie_core::types::{ContentFit, FilterMethod, Length, PlushieType};

struct ImageProps {
    width: Option<Length>,
    height: Option<Length>,
    content_fit: Option<ContentFit>,
    filter_method: Option<FilterMethod>,
    alt: Option<String>,
    description: Option<String>,
}

impl ImageProps {
    fn from_node(node: &TreeNode) -> Self {
        let p = &node.props;
        Self {
            width: Length::extract(p, "width"),
            height: Length::extract(p, "height"),
            content_fit: ContentFit::extract(p, "content_fit"),
            filter_method: FilterMethod::extract(p, "filter_method"),
            alt: String::extract(p, "alt"),
            description: String::extract(p, "description"),
        }
    }
}

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
        let ip = ImageProps::from_node(node);
        let props = &node.props;

        let width = ip
            .width
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);
        let height = ip
            .height
            .as_ref()
            .map(iced_convert::length)
            .unwrap_or(iced::Length::Shrink);

        // source can be a string (file path) or an object with a "handle" field
        // (in-memory image from the registry). Kept as raw prop access.
        let source_val = props.get_value("source");
        if source_val.is_none() {
            log::warn!("[id={}] image: no 'source' prop specified", node.id);
        }
        let handle: iced::widget::image::Handle = match source_val {
            Some(Value::Object(obj)) => {
                if let Some(name) = obj.get("handle").and_then(|v| v.as_str()) {
                    match ctx.images.get(name) {
                        Some(h) => h.clone(),
                        None => {
                            log::warn!("[id={}] image: unknown registry handle: {name}", node.id);
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
        if let Some(cf) = ip.content_fit {
            img = img.content_fit(iced_convert::content_fit(cf));
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
        if let Some(fm) = ip.filter_method {
            img = img.filter_method(iced_convert::filter_method(fm));
        }
        if let Some(expand) = prop_bool(props, "expand") {
            img = img.expand(expand);
        }
        if let Some(scale) =
            prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "scale")
        {
            img = img.scale(scale);
        }
        if let Some(alt) = ip.alt {
            img = img.alt(alt);
        }
        if let Some(desc) = ip.description {
            img = img.description(desc);
        }
        if prop_bool_default(props, "decorative", false) {
            img = img.decorative();
        }
        // crop: complex object, kept as raw prop access
        let crop_val = props.get_value("crop");
        if let Some(crop_obj) = crop_val.as_ref().and_then(|v| v.as_object()) {
            let cx = crop_obj.get("x").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let cy = crop_obj.get("y").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let cw = crop_obj.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let ch = crop_obj.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
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
