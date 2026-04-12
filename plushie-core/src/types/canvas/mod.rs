//! Canvas-specific types for shapes, fills, strokes, transforms, and interactivity.

mod clip;
mod drag;
mod fill;
mod hit;
mod path;
mod shape;
mod shape_style;
mod stroke;
mod transform;

pub use clip::ClipRect;
pub use drag::{DragAxis, DragBounds};
pub use fill::{CanvasFill, FillRule};
pub use hit::HitRect;
pub use path::{PathCommand, decode_commands};
pub use shape::{
    CanvasShape, CircleShape, GroupShape, ImageShape, LineShape, PathShape, RectShape, SvgShape,
    TextShape, extract_canvas_layers,
};
pub use shape_style::ShapeStyle;
pub use stroke::{Dash, LineCap, LineJoin, Stroke};
pub use transform::{Transform, decode_transforms, encode_transforms};
