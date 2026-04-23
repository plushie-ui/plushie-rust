use std::any::TypeId;
use std::collections::BTreeMap;

use plushie_core::protocol::{PropValue, TreeNode};
use plushie_core::types::{
    A11y, CanvasFill, CanvasShape, CircleShape, ClipRect, Dash, DragAxis, DragBounds, FillRule,
    GroupShape, HasPopup, HitRect, ImageShape, LineCap, LineJoin, LineShape, Live, Orientation,
    PathCommand, PathShape, RectShape, Role, ShapeStyle, Stroke, SvgShape, TextShape, Transform,
    decode_commands, decode_transforms, encode_transforms, extract_canvas_layers,
};
use serde_json::Value;

fn assert_public_type<T: 'static>() {
    let _ = TypeId::of::<T>();
}

#[test]
fn a11y_and_canvas_types_are_public_from_types_module() {
    assert_public_type::<A11y>();
    assert_public_type::<HasPopup>();
    assert_public_type::<Live>();
    assert_public_type::<Orientation>();
    assert_public_type::<Role>();

    assert_public_type::<ClipRect>();
    assert_public_type::<DragAxis>();
    assert_public_type::<DragBounds>();
    assert_public_type::<CanvasFill>();
    assert_public_type::<FillRule>();
    assert_public_type::<HitRect>();
    assert_public_type::<PathCommand>();
    assert_public_type::<CanvasShape>();
    assert_public_type::<CircleShape>();
    assert_public_type::<GroupShape>();
    assert_public_type::<ImageShape>();
    assert_public_type::<LineShape>();
    assert_public_type::<PathShape>();
    assert_public_type::<RectShape>();
    assert_public_type::<SvgShape>();
    assert_public_type::<TextShape>();
    assert_public_type::<ShapeStyle>();
    assert_public_type::<Dash>();
    assert_public_type::<LineCap>();
    assert_public_type::<LineJoin>();
    assert_public_type::<Stroke>();
    assert_public_type::<Transform>();

    let _: fn(&Value) -> Vec<PathCommand> = decode_commands;
    let _: fn(&Value) -> Vec<Transform> = decode_transforms;
    let _: fn(&[Transform]) -> PropValue = encode_transforms;
    let _: fn(&TreeNode) -> BTreeMap<String, Vec<CanvasShape>> = extract_canvas_layers;
}
