//! Canvas types: hit regions, transforms, interactive elements, and drag state.

use iced::widget::canvas;
use iced::{Point, Radians, Vector};

use crate::PlushieRenderer;

/// Maximum number of shapes per canvas layer. Layers exceeding this limit
/// are truncated with a warning to prevent excessive tessellation work from
/// a single oversized payload.
pub(crate) const MAX_SHAPES_PER_LAYER: usize = 10_000;

// ---------------------------------------------------------------------------
// Interactive elements: hit testing and interaction state
// ---------------------------------------------------------------------------

/// Geometric region for hit testing an interactive element.
///
/// Currently only `Rect` is constructed (groups auto-infer bounding boxes).
/// `Circle` and `Line` are retained for `hit_test` dispatch, focus ring
/// geometry, and future `hit_rect` extensions that support non-rectangular
/// regions.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Circle/Line not yet constructed but used in match arms
pub(crate) enum HitRegion {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        half_width: f32,
    },
}

fn clean_coordinate(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn clean_extent(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

impl HitRegion {
    pub(crate) fn normalized(&self) -> Self {
        match *self {
            Self::Rect { x, y, w, h } => Self::Rect {
                x: clean_coordinate(x),
                y: clean_coordinate(y),
                w: clean_extent(w),
                h: clean_extent(h),
            },
            Self::Circle { cx, cy, r } => Self::Circle {
                cx: clean_coordinate(cx),
                cy: clean_coordinate(cy),
                r: clean_extent(r),
            },
            Self::Line {
                x1,
                y1,
                x2,
                y2,
                half_width,
            } => Self::Line {
                x1: clean_coordinate(x1),
                y1: clean_coordinate(y1),
                x2: clean_coordinate(x2),
                y2: clean_coordinate(y2),
                half_width: clean_extent(half_width),
            },
        }
    }
}

/// 2D affine transform matrix for mapping between coordinate spaces.
///
/// Stored as a 2x3 matrix `[a, b, c, d, tx, ty]` representing:
/// ```text
/// | a  b  tx |
/// | c  d  ty |
/// | 0  0   1 |
/// ```
///
/// Transforms are composed by multiplication. Points are transformed as:
/// ```text
/// x' = a*x + b*y + tx
/// y' = c*x + d*y + ty
/// ```
///
/// Used to map cursor positions from canvas space into an element's local
/// coordinate space for hit testing. Each [`InteractiveElement`] stores
/// the accumulated matrix from all ancestor group transforms and its
/// inverse (precomputed for efficient per-frame hit testing).
#[derive(Debug, Clone, Copy)]
pub(crate) struct TransformMatrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl TransformMatrix {
    /// The identity matrix (no transformation).
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Append a translation to this matrix.
    pub fn translate(self, x: f32, y: f32) -> Self {
        Self {
            a: self.a,
            b: self.b,
            c: self.c,
            d: self.d,
            tx: self.a * x + self.b * y + self.tx,
            ty: self.c * x + self.d * y + self.ty,
        }
    }

    /// Append a rotation (in radians) to this matrix.
    pub fn rotate(self, angle: f32) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();
        Self {
            a: self.a * cos + self.b * sin,
            b: self.b * cos - self.a * sin,
            c: self.c * cos + self.d * sin,
            d: self.d * cos - self.c * sin,
            tx: self.tx,
            ty: self.ty,
        }
    }

    /// Append a non-uniform scale to this matrix.
    pub fn scale(self, sx: f32, sy: f32) -> Self {
        Self {
            a: self.a * sx,
            b: self.b * sy,
            c: self.c * sx,
            d: self.d * sy,
            tx: self.tx,
            ty: self.ty,
        }
    }

    /// Transform a point from the source coordinate space to the
    /// destination coordinate space.
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty,
        )
    }

    /// Compose this matrix with another: `self * other`.
    ///
    /// The result maps points through `other` first, then through `self`.
    /// This matches the "append" convention: `parent.compose(&local)`
    /// produces a matrix that applies the local transform in the parent's
    /// coordinate space.
    pub fn compose(&self, other: &Self) -> Self {
        Self {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            tx: self.a * other.tx + self.b * other.ty + self.tx,
            ty: self.c * other.tx + self.d * other.ty + self.ty,
        }
    }

    /// Compute the inverse matrix, or `None` if the matrix is singular
    /// (determinant is zero, meaning the transform collapses a dimension).
    pub fn inverse(&self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self {
            a: self.d * inv_det,
            b: -self.b * inv_det,
            c: -self.c * inv_det,
            d: self.a * inv_det,
            tx: (self.b * self.ty - self.d * self.tx) * inv_det,
            ty: (self.c * self.tx - self.a * self.ty) * inv_det,
        })
    }

    /// Decompose this matrix into translation, rotation, and scale.
    ///
    /// Returns `(tx, ty, angle, sx, sy)` where the matrix can be
    /// reconstructed as `translate(tx, ty) * rotate(angle) * scale(sx, sy)`.
    ///
    /// This decomposition is exact for matrices built from translate,
    /// rotate, and scale operations (no shear). For matrices with shear,
    /// the result is an approximation.
    pub fn decompose(&self) -> (f32, f32, f32, f32, f32) {
        const MIN_SCALE: f32 = 1e-10;

        let tx = if self.tx.is_finite() { self.tx } else { 0.0 };
        let ty = if self.ty.is_finite() { self.ty } else { 0.0 };

        let angle = self.c.atan2(self.a);
        let angle = if angle.is_finite() { angle } else { 0.0 };

        let sx = (self.a * self.a + self.c * self.c).sqrt();
        let sx = if sx.is_finite() {
            sx.max(MIN_SCALE)
        } else {
            1.0
        };

        // Use determinant / sx to get sy with correct sign (handles reflection).
        let det = self.a * self.d - self.b * self.c;
        let sy = if det.is_finite() {
            let sy = det / sx;
            if sy.is_finite() {
                let sign = if sy.is_sign_negative() { -1.0 } else { 1.0 };
                sign * sy.abs().max(MIN_SCALE)
            } else {
                1.0
            }
        } else {
            1.0
        };

        (tx, ty, angle, sx, sy)
    }

    /// Apply this matrix's transform to an iced canvas [`Frame`].
    ///
    /// Decomposes the matrix into translate + rotate + scale and applies
    /// them in order. The caller must call `frame.push_transform()` before
    /// and `frame.pop_transform()` after.
    ///
    /// [`Frame`]: iced::widget::canvas::Frame
    pub fn apply_to_frame<R: PlushieRenderer>(&self, frame: &mut canvas::Frame<R>) {
        let (tx, ty, angle, sx, sy) = self.decompose();
        frame.translate(Vector::new(tx, ty));
        if angle.abs() > 1e-6 {
            frame.rotate(Radians(angle));
        }
        if (sx - 1.0).abs() > 1e-6 || (sy - 1.0).abs() > 1e-6 {
            frame.scale_nonuniform(Vector::new(sx, sy));
        }
    }

    /// Build a matrix from a group's `"transforms"` JSON array.
    ///
    /// Applies each transform entry in order: translate, rotate, scale.
    /// Returns the composed matrix.
    #[cfg(test)]
    pub fn from_transforms(transforms: &[serde_json::Value]) -> Self {
        let mut m = Self::identity();
        for t in transforms {
            let t_type = t.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match t_type {
                "translate" => {
                    let x = t.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let y = t.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    m = m.translate(x, y);
                }
                "rotate" => {
                    let deg = t.get("angle").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    m = m.rotate(deg.to_radians());
                }
                "scale" => {
                    if let Some(factor) = t.get("factor").and_then(|v| v.as_f64()) {
                        let f = factor as f32;
                        m = m.scale(f, f);
                    } else {
                        let sx = t.get("x").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                        let sy = t.get("y").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                        m = m.scale(sx, sy);
                    }
                }
                _ => {}
            }
        }
        m
    }

    /// Build a matrix from typed [`Transform`] values.
    pub fn from_typed_transforms(transforms: &[plushie_core::types::Transform]) -> Self {
        use plushie_core::types::Transform;

        let mut m = Self::identity();
        for t in transforms {
            match t {
                Transform::Translate { x, y } => m = m.translate(*x, *y),
                Transform::Rotate { angle } => m = m.rotate(angle.radians()),
                Transform::Scale { x, y } => m = m.scale(*x, *y),
                Transform::ScaleUniform { factor } => m = m.scale(*factor, *factor),
            }
        }
        m
    }
}

/// Arrow key navigation mode for canvas interactive elements.
///
/// Controls how arrow keys behave at the boundaries of the element list.
/// Set via the `"arrow_mode"` prop on the canvas widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ArrowMode {
    /// Arrows wrap around at boundaries (last -> first, first -> last).
    /// Always captures the event. This is the standard roving tabindex
    /// pattern for composite widgets like toolbars and radio groups.
    #[default]
    Wrap,
    /// Arrows stop at first/last element (no wrapping). Captures the event.
    Clamp,
    /// Arrows navigate in order and stop at boundaries. Captures the event.
    Linear,
    /// Arrows are not handled by the canvas at all. Elements are only
    /// navigable via Tab. Useful when arrow keys have app-specific meaning.
    None,
}

impl ArrowMode {
    #[cfg(test)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "wrap" => Self::Wrap,
            "clamp" => Self::Clamp,
            "linear" => Self::Linear,
            "none" => Self::None,
            _ => {
                log::warn!("canvas: unknown arrow_mode '{s}', defaulting to 'wrap'");
                Self::Wrap
            }
        }
    }
}

impl From<plushie_core::types::ArrowMode> for ArrowMode {
    fn from(mode: plushie_core::types::ArrowMode) -> Self {
        match mode {
            plushie_core::types::ArrowMode::Wrap => Self::Wrap,
            plushie_core::types::ArrowMode::Clamp => Self::Clamp,
            plushie_core::types::ArrowMode::Linear => Self::Linear,
            plushie_core::types::ArrowMode::None => Self::None,
        }
    }
}

/// Axis constraint for draggable shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DragAxis {
    Both,
    X,
    Y,
}

/// Bounds constraint for draggable shapes. Fields are populated during
/// shape parsing and read during drag event handling for clamping.
#[derive(Debug, Clone)]
pub(crate) struct DragBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

/// Parsed interactive configuration for a canvas element.
///
/// An "element" is a group with an `id` field. Interactive fields
/// (`on_click`, `on_hover`, `a11y`, etc.) live at the top level of
/// the group JSON, not in a nested `"interactive"` sub-object.
///
/// Extracted during `ensure_canvas_cache` and stored in `SharedState`
/// so `update()` can hit-test without re-parsing JSON every frame.
#[derive(Debug, Clone)]
pub(crate) struct InteractiveElement {
    /// Unique ID for this element (from the group's `"id"` field).
    pub id: String,
    /// Which layer this element belongs to.
    pub layer: String,
    /// Geometric bounds for hit testing (in local group coordinates).
    /// Use [`inverse_transform`](Self::inverse_transform) to map cursor
    /// positions from canvas space into this local space.
    pub hit_region: HitRegion,
    /// Accumulated transform from canvas origin to this element's local
    /// coordinate space. Composed from all ancestor group transforms.
    pub transform: TransformMatrix,
    /// Precomputed inverse of [`transform`](Self::transform). Used to
    /// map cursor positions from canvas space to local space for hit
    /// testing. `None` if the transform is singular (degenerate).
    pub inverse_transform: Option<TransformMatrix>,
    /// Optional clip rectangle in canvas space. If set, the element is
    /// only hittable when the cursor is inside this rect. Derived from
    /// ancestor group `"clip"` fields, transformed to canvas space.
    pub clip_rect: Option<(f32, f32, f32, f32)>,
    pub on_click: bool,
    pub on_hover: bool,
    pub draggable: bool,
    pub drag_axis: DragAxis,
    pub drag_bounds: Option<DragBounds>,
    /// Cursor to show when hovering (e.g. "pointer", "grab").
    pub cursor: Option<String>,
    /// Whether this element has hover/pressed/focus style overrides.
    pub has_hover_style: bool,
    pub has_pressed_style: bool,
    pub has_focus_style: bool,
    /// Whether to show the default focus ring when this element is focused.
    /// Defaults to `true`. Set to `false` when using `focus_style` instead.
    pub show_focus_ring: bool,
    /// Corner radius for the focus ring. Defaults to `inflate + 1.0` (~3px).
    /// Set to `h/2 + inflate` for pill shapes, `r + inflate` for circles.
    /// Only used when `show_focus_ring` is `true` and the hit region is Rect.
    pub focus_ring_radius: Option<f32>,
    /// Whether this group acts as a Tab stop for two-level navigation.
    /// When true, Tab navigates between focusable groups, and arrows
    /// navigate between elements within the focused group.
    pub focusable: bool,
    /// ID of the parent focusable group, if this element is a child
    /// within a focusable group. `None` for top-level elements and
    /// for focusable groups themselves.
    ///
    /// Used for two-level navigation: Tab moves between elements with
    /// `parent_group == None`, arrows move within elements sharing the
    /// same `parent_group`.
    pub parent_group: Option<String>,
    /// Tooltip text to show on hover.
    pub tooltip: Option<String>,
    /// Accessibility overrides for this element. Parsed from the `a11y`
    /// field on the group using the same [`A11yOverrides`] struct that
    /// all other widgets use (same fields, same parsing, same validation).
    ///
    /// [`A11yOverrides`]: crate::a11y::A11yOverrides
    pub a11y: Option<crate::a11y::A11yOverrides>,
}

/// Active drag state tracked in `CanvasState`.
#[derive(Debug, Clone)]
pub(crate) struct DragState {
    pub element_id: String,
    pub last: Point,
}

/// Canvas-program-internal state.
#[derive(Default)]
pub(crate) struct CanvasState {
    pub cursor_position: Option<Point>,
    /// ID of the interactive element currently under the cursor.
    pub hovered_element: Option<String>,
    /// ID of the element being pressed (mouse down, not yet released).
    pub pressed_element: Option<String>,
    /// Active drag state (element being dragged).
    pub dragging: Option<DragState>,
    /// ID of the interactive element that has keyboard focus.
    /// ID-based (not index-based) so focus survives element reordering
    /// between renders. When the focused element is removed, focus is
    /// cleared and a blur event is emitted.
    pub focused_id: Option<String>,
    /// ID of the focusable group that currently has group-level focus
    /// in two-level navigation. `None` when navigating at the top level
    /// or when no focusable groups exist.
    pub focused_group: Option<String>,
    /// Tracks the last consumed pending_focus to prevent re-firing.
    /// See pending_focus consumption in update().
    pub last_consumed_pending: Option<String>,
    /// Whether the canvas currently has iced-level focus. Set by
    /// `on_focus_gained`, cleared by `on_focus_lost`. Used to suppress
    /// focus visuals (focus_style, focus ring) when the canvas is
    /// unfocused but `focused_id` is preserved for re-entry.
    pub canvas_focused: bool,
    /// Whether the focus indicator should be visible. `true` for
    /// keyboard navigation (Tab), `false` for mouse clicks.
    /// Matches iced's "focus-visible" pattern.
    pub focus_visible: bool,
    /// Current keyboard modifiers, tracked from ModifiersChanged events.
    /// Included on all outgoing pointer events.
    pub current_modifiers: iced::keyboard::Modifiers,
}
