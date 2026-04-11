//! Canvas-specific types for shapes, fills, strokes, transforms, and interactivity.

mod clip;
mod drag;
mod fill;
mod hit;
mod shape;
mod shape_style;
mod stroke;
mod transform;

pub use clip::ClipRect;
pub use drag::{DragAxis, DragBounds};
pub use fill::{CanvasFill, FillRule};
pub use hit::HitRect;
pub use shape::{
    extract_canvas_layers, CanvasShape, CircleShape, GroupShape, ImageShape, LineShape, PathShape,
    RectShape, SvgShape, TextShape,
};
pub use shape_style::ShapeStyle;
pub use stroke::{Dash, LineCap, LineJoin, Stroke};
pub use transform::{decode_transforms, encode_transforms, Transform};
