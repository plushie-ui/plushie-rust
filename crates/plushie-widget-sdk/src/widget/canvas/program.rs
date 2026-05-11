//! CanvasProgram struct, impl methods, and canvas::Program trait impl.
//! Also includes overlay helpers (merge_shape_style, draw_focus_ring, draw_tooltip).

use iced::widget::canvas;
use iced::{Color, Pixels, Point, Radians, Size, Vector, keyboard, mouse};

use plushie_core::types::CanvasShape;

use super::interaction::*;
use super::shapes::*;
use super::types::*;
use crate::PlushieRenderer;
use crate::canvas_engine::{CanvasLayerCaches, canvas_theme_hash};
use serde_json::json;

use crate::message::{Message, serialize_modifiers};
use crate::protocol::KeyModifiers;

const PAGE_NAVIGATION_STEP: usize = 5;

/// Replace non-finite f32 with 0.0 for safe JSON serialization.
fn sanitize_f32(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

/// Build a modifiers JSON value for pointer event data payloads.
fn modifiers_json(mods: &KeyModifiers) -> serde_json::Value {
    json!({
        "shift": mods.shift,
        "ctrl": mods.ctrl,
        "alt": mods.alt,
        "logo": mods.logo,
        "command": mods.command,
    })
}

/// Build a pointer press/release/move Message::Event with the same wire
/// format as OutgoingEvent::pointer_press/release/move.
#[allow(clippy::too_many_arguments)]
fn pointer_event(
    family: &str,
    window_id: &str,
    id: &str,
    x: f32,
    y: f32,
    button: Option<&str>,
    pointer_type: &str,
    finger: Option<u64>,
    modifiers: KeyModifiers,
) -> Message {
    let mut data = json!({
        "x": sanitize_f32(x),
        "y": sanitize_f32(y),
        "pointer": pointer_type,
        "modifiers": modifiers_json(&modifiers),
    });
    if let Some(btn) = button {
        data["button"] = json!(btn);
    }
    if let Some(f) = finger {
        data["finger"] = json!(f);
    }
    Message::Event {
        window_id: window_id.to_string(),
        id: id.to_string(),
        family: family.to_string(),
        value: data,
    }
}

/// Build a pointer scroll Message::Event with the same wire format as
/// OutgoingEvent::pointer_scroll.
#[allow(clippy::too_many_arguments)]
fn pointer_scroll_event(
    window_id: &str,
    id: &str,
    x: f32,
    y: f32,
    delta_x: f32,
    delta_y: f32,
    pointer_type: &str,
    modifiers: KeyModifiers,
) -> Message {
    Message::Event {
        window_id: window_id.to_string(),
        id: id.to_string(),
        family: "scroll".to_string(),
        value: json!({
            "x": sanitize_f32(x),
            "y": sanitize_f32(y),
            "delta_x": sanitize_f32(delta_x),
            "delta_y": sanitize_f32(delta_y),
            "pointer": pointer_type,
            "modifiers": modifiers_json(&modifiers),
        }),
    }
}

pub(crate) struct CanvasProgram<'a, R: PlushieRenderer = iced::Renderer> {
    /// Sorted layer data: (layer_name, shapes array).
    pub layers: Vec<(String, Vec<CanvasShape>)>,
    /// Per-layer caches from SharedState.
    pub caches: Option<&'a CanvasLayerCaches<R>>,
    pub background: Option<Color>,
    pub window_id: String,
    pub id: String,
    pub on_press: bool,
    pub on_release: bool,
    pub on_move: bool,
    pub on_scroll: bool,
    /// Reference to the image registry for resolving in-memory image handles.
    pub images: &'a crate::image_registry::ImageRegistry,
    /// Interactive elements parsed during prepare.
    pub interactive_elements: &'a [InteractiveElement],
    /// Arrow key navigation mode.
    pub arrow_mode: ArrowMode,
    /// Pending programmatic focus from `focus` widget_op (canvas element path).
    /// Consumed at the top of `update()` to set `focused_id`.
    pub pending_focus: Option<String>,
}

impl<R: PlushieRenderer> CanvasProgram<'_, R> {
    pub fn is_interactive(&self) -> bool {
        self.on_press
            || self.on_release
            || self.on_move
            || self.on_scroll
            || !self.interactive_elements.is_empty()
    }

    /// Collect layer names that need cache bypass due to active
    /// interaction state (hover_style, pressed_style, or focus_style).
    ///
    /// Multiple layers can be active simultaneously (e.g., hover on
    /// layer A while focus is on layer B). All returned layers are
    /// redrawn fresh with style overrides applied.
    pub fn layers_with_active_interaction(&self, state: &CanvasState) -> Vec<String> {
        let mut layers = Vec::new();

        // Hover/pressed style.
        let active_id = state
            .pressed_element
            .as_deref()
            .or(state.hovered_element.as_deref());
        if let Some(id) = active_id
            && let Some(shape) = self.interactive_elements.iter().find(|s| s.id == id)
            && (shape.has_hover_style || shape.has_pressed_style)
        {
            layers.push(shape.layer.clone());
        }

        // Keyboard focus with focus_style (only when canvas has iced focus).
        if state.canvas_focused
            && state.focus_visible
            && let Some(ref focused_id) = state.focused_id
            && let Some(shape) = self
                .interactive_elements
                .iter()
                .find(|s| &s.id == focused_id)
            && shape.has_focus_style
            && !layers.contains(&shape.layer)
        {
            layers.push(shape.layer.clone());
        }

        layers
    }

    /// Get the tooltip text for the currently hovered shape, if any.
    fn active_tooltip(&self, state: &CanvasState) -> Option<String> {
        let hovered_id = state.hovered_element.as_deref()?;
        let shape = self
            .interactive_elements
            .iter()
            .find(|s| s.id == hovered_id)?;
        shape.tooltip.clone()
    }

    /// Resolve the currently focused element ID to its index in the
    /// interactive elements list. Returns `None` if no element is focused
    /// or the focused element no longer exists (removed between renders).
    pub(crate) fn resolve_focus_index(&self, state: &CanvasState) -> Option<usize> {
        let focused_id = state.focused_id.as_deref()?;
        self.interactive_elements
            .iter()
            .position(|e| e.id == focused_id)
    }

    /// Transition focus to a new element by index. Returns a single
    /// [`CanvasElementFocusChanged`](Message::CanvasElementFocusChanged)
    /// message that the emitter splits into separate blur + focus
    /// outgoing events (in that order).
    ///
    /// Pass `None` for `new_index` to clear focus without moving to
    /// another element (e.g., Escape or click-on-empty).
    ///
    /// Returns `None` if no state change occurred (already focused on
    /// the target, or clearing focus when nothing was focused).
    pub(crate) fn set_focus(
        &self,
        state: &mut CanvasState,
        new_index: Option<usize>,
    ) -> Option<Message> {
        let old_id = state.focused_id.take();

        let new_id = new_index
            .filter(|&idx| idx < self.interactive_elements.len())
            .map(|idx| self.interactive_elements[idx].id.clone());

        // No-op if focus didn't actually change.
        if old_id == new_id {
            // Restore the original focused_id since we took it.
            state.focused_id = old_id;
            return None;
        }

        state.focused_id = new_id.clone();

        // Only emit a message if something actually changed.
        if old_id.is_some() || new_id.is_some() {
            Some(Message::CanvasElementFocusChanged {
                window_id: self.window_id.clone(),
                old_element_id: old_id,
                new_element_id: new_id,
            })
        } else {
            None
        }
    }

    /// Get the indices of "top-level" entries for Tab navigation.
    ///
    /// Top-level entries are elements where `parent_group.is_none()`.
    /// This includes standalone elements and focusable groups themselves
    /// (but not children of focusable groups).
    pub(crate) fn top_level_indices(&self) -> Vec<usize> {
        self.interactive_elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.parent_group.is_none())
            .map(|(i, _)| i)
            .collect()
    }

    /// Get the indices of children within a focusable group.
    pub(crate) fn group_child_indices(&self, group_id: &str) -> Vec<usize> {
        self.interactive_elements
            .iter()
            .enumerate()
            .filter(|(_, e)| e.parent_group.as_deref() == Some(group_id))
            .map(|(i, _)| i)
            .collect()
    }

    /// Draw shapes with hover/pressed/focus style overrides applied to the
    /// active element. Used when a layer needs fresh drawing due to
    /// interaction state changes (cache is bypassed).
    ///
    /// Only groups can be interactive elements. Non-group shapes are drawn
    /// directly. When a group is the active element, its children have
    /// per-child style overrides applied. Priority: pressed > hover > focus.
    fn draw_shapes_with_overrides(
        &self,
        frame: &mut canvas::Frame<R>,
        shapes: &[&CanvasShape],
        state: &CanvasState,
        images: &crate::image_registry::ImageRegistry,
        theme: &iced::Theme,
    ) {
        let hovered = state.hovered_element.as_deref();
        let pressed = state.pressed_element.as_deref();
        let focused = if state.canvas_focused && state.focus_visible {
            state.focused_id.as_deref()
        } else {
            None
        };

        for shape in shapes {
            if let CanvasShape::Group(g) = shape {
                let group_id = g.id.as_deref();
                let is_pressed = group_id.is_some_and(|gid| pressed == Some(gid));
                let is_hovered = group_id.is_some_and(|gid| hovered == Some(gid));
                let is_focused = group_id.is_some_and(|gid| focused == Some(gid));

                let has_transforms = !g.transforms.is_empty();
                if has_transforms {
                    frame.push_transform();
                    apply_group_transforms(frame, &g.transforms);
                }

                // Resolve the active style override from the GROUP.
                // Priority: pressed > hover > focus.
                let group_override = if is_pressed {
                    g.pressed_style.as_ref()
                } else {
                    None
                }
                .or(if is_hovered {
                    g.hover_style.as_ref()
                } else {
                    None
                })
                .or(if is_focused {
                    g.focus_style.as_ref()
                } else {
                    None
                });

                let child_refs: Vec<&CanvasShape> = g.children.iter().collect();

                let draw_children = |f: &mut canvas::Frame<R>,
                                     child_refs: &[&CanvasShape],
                                     img: &crate::image_registry::ImageRegistry,
                                     theme: &iced::Theme| {
                    if let Some(overrides) = group_override {
                        for child in child_refs {
                            draw_canvas_shape_with_overrides(f, child, img, theme, overrides);
                        }
                    } else {
                        draw_canvas_shapes(f, child_refs, img, theme);
                    }
                };

                draw_with_group_clip(
                    frame,
                    g.clip.as_ref(),
                    images,
                    theme,
                    &child_refs,
                    draw_children,
                );

                if has_transforms {
                    frame.pop_transform();
                }
            } else {
                draw_canvas_shape(frame, shape, images, theme);
            }
        }
    }

    /// Handle keyboard events for interactive element navigation.
    ///
    /// Extracted from `update()` for readability. Implements the roving
    /// tabindex pattern with two-level navigation for focusable groups.
    pub(super) fn handle_keyboard(
        &self,
        state: &mut CanvasState,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<iced::widget::Action<Message>> {
        if state.dragging.is_some() {
            return Some(iced::widget::Action::capture());
        }

        use keyboard::key::Named;

        let current_idx = self.resolve_focus_index(state);
        if current_idx.is_none() && state.focused_id.is_some() {
            state.focused_group = None;
            if let Some(msg) = self.set_focus(state, None) {
                return Some(iced::widget::Action::publish(msg).and_capture());
            }
        }

        let focus_to = |state: &mut CanvasState,
                        idx: Option<usize>|
         -> Option<iced::widget::Action<Message>> {
            match self.set_focus(state, idx) {
                Some(msg) => Some(iced::widget::Action::publish(msg).and_capture()),
                None => Some(iced::widget::Action::capture()),
            }
        };

        let has_focusable_groups = self.interactive_elements.iter().any(|e| e.focusable);
        let arrow_indices: Vec<usize> = if let Some(ref gid) = state.focused_group {
            self.group_child_indices(gid)
        } else if has_focusable_groups {
            self.top_level_indices()
        } else {
            (0..self.interactive_elements.len()).collect()
        };
        let arrow_pos = current_idx.and_then(|ci| arrow_indices.iter().position(|&i| i == ci));
        let arrow_count = arrow_indices.len();

        // When arrow_mode is "none", forward navigation keys to the host
        // as element key events instead of consuming them for element
        // navigation. This lets the host implement custom value adjustment
        // on focused canvas elements (e.g. slider-like controls).
        // Tab/Shift+Tab and Escape are always handled by the canvas for
        // focus management regardless of arrow_mode.
        if self.arrow_mode == ArrowMode::None
            && let Some(idx) = current_idx
        {
            let is_nav_key = matches!(
                key,
                keyboard::Key::Named(
                    Named::ArrowUp
                        | Named::ArrowDown
                        | Named::ArrowLeft
                        | Named::ArrowRight
                        | Named::Home
                        | Named::End
                        | Named::PageUp
                        | Named::PageDown
                )
            );
            if is_nav_key {
                let element = &self.interactive_elements[idx];
                let mods = serialize_modifiers(modifiers);
                return Some(
                    iced::widget::Action::publish(Message::Event {
                        window_id: self.window_id.clone(),
                        id: element.id.clone(),
                        family: "key_press".to_string(),
                        value: json!({
                            "key": crate::message::serialize_key(key),
                            "modifiers": modifiers_json(&mods),
                        }),
                    })
                    .and_capture(),
                );
            }
        }

        match key {
            keyboard::Key::Named(Named::Tab) if !modifiers.shift() => {
                let top = self.top_level_indices();
                let top_pos = current_idx.and_then(|ci| {
                    if let Some(ref gid) = state.focused_group {
                        top.iter()
                            .position(|&i| self.interactive_elements[i].id == *gid)
                    } else {
                        top.iter().position(|&i| i == ci)
                    }
                });
                match top_pos {
                    None => {
                        if let Some(&first) = top.first() {
                            state.focused_group = None;
                            let elem = &self.interactive_elements[first];
                            if elem.focusable {
                                state.focused_group = Some(elem.id.clone());
                                let children = self.group_child_indices(&elem.id);
                                if let Some(&fc) = children.first() {
                                    focus_to(state, Some(fc))
                                } else {
                                    focus_to(state, Some(first))
                                }
                            } else {
                                focus_to(state, Some(first))
                            }
                        } else {
                            Some(iced::widget::Action::capture())
                        }
                    }
                    Some(pos) if pos + 1 < top.len() => {
                        let next_idx = top[pos + 1];
                        let elem = &self.interactive_elements[next_idx];
                        if elem.focusable {
                            state.focused_group = Some(elem.id.clone());
                            let children = self.group_child_indices(&elem.id);
                            if let Some(&fc) = children.first() {
                                focus_to(state, Some(fc))
                            } else {
                                focus_to(state, Some(next_idx))
                            }
                        } else {
                            state.focused_group = None;
                            focus_to(state, Some(next_idx))
                        }
                    }
                    Some(_) => None,
                }
            }
            keyboard::Key::Named(Named::Tab) if modifiers.shift() => {
                let top = self.top_level_indices();
                let top_pos = current_idx.and_then(|ci| {
                    if let Some(ref gid) = state.focused_group {
                        top.iter()
                            .position(|&i| self.interactive_elements[i].id == *gid)
                    } else {
                        top.iter().position(|&i| i == ci)
                    }
                });
                match top_pos {
                    None => {
                        if let Some(&last) = top.last() {
                            let elem = &self.interactive_elements[last];
                            if elem.focusable {
                                state.focused_group = Some(elem.id.clone());
                                let children = self.group_child_indices(&elem.id);
                                if let Some(&lc) = children.last() {
                                    focus_to(state, Some(lc))
                                } else {
                                    focus_to(state, Some(last))
                                }
                            } else {
                                state.focused_group = None;
                                focus_to(state, Some(last))
                            }
                        } else {
                            Some(iced::widget::Action::capture())
                        }
                    }
                    Some(0) => None,
                    Some(pos) => {
                        let prev_idx = top[pos - 1];
                        let elem = &self.interactive_elements[prev_idx];
                        if elem.focusable {
                            state.focused_group = Some(elem.id.clone());
                            let children = self.group_child_indices(&elem.id);
                            if let Some(&lc) = children.last() {
                                focus_to(state, Some(lc))
                            } else {
                                focus_to(state, Some(prev_idx))
                            }
                        } else {
                            state.focused_group = None;
                            focus_to(state, Some(prev_idx))
                        }
                    }
                }
            }
            keyboard::Key::Named(Named::ArrowDown | Named::ArrowRight)
                if self.arrow_mode != ArrowMode::None && arrow_count > 0 =>
            {
                match (arrow_pos, self.arrow_mode) {
                    (None, _) => focus_to(state, Some(arrow_indices[0])),
                    (Some(pos), ArrowMode::Wrap) => {
                        focus_to(state, Some(arrow_indices[(pos + 1) % arrow_count]))
                    }
                    (Some(pos), _) if pos + 1 < arrow_count => {
                        focus_to(state, Some(arrow_indices[pos + 1]))
                    }
                    (Some(_), ArrowMode::Clamp) => Some(iced::widget::Action::capture()),
                    (Some(_), ArrowMode::Linear) => Some(iced::widget::Action::capture()),
                    _ => None,
                }
            }
            keyboard::Key::Named(Named::ArrowUp | Named::ArrowLeft)
                if self.arrow_mode != ArrowMode::None && arrow_count > 0 =>
            {
                match (arrow_pos, self.arrow_mode) {
                    (None, _) => focus_to(state, Some(*arrow_indices.last().unwrap())),
                    (Some(0), ArrowMode::Wrap) => {
                        focus_to(state, Some(*arrow_indices.last().unwrap()))
                    }
                    (Some(0), ArrowMode::Clamp) => Some(iced::widget::Action::capture()),
                    (Some(0), ArrowMode::Linear) => Some(iced::widget::Action::capture()),
                    (Some(pos), _) => focus_to(state, Some(arrow_indices[pos - 1])),
                }
            }
            keyboard::Key::Named(Named::Enter | Named::Space) => {
                if let Some(idx) = current_idx {
                    let element = &self.interactive_elements[idx];
                    if element.on_click {
                        let center = hit_region_center(&element.hit_region);
                        Some(
                            iced::widget::Action::publish(Message::Event {
                                window_id: self.window_id.clone(),
                                id: element.id.clone(),
                                family: "click".to_string(),
                                value: json!({
                                    "x": sanitize_f32(center.x),
                                    "y": sanitize_f32(center.y),
                                    "button": "keyboard",
                                }),
                            })
                            .and_capture(),
                        )
                    } else {
                        Some(iced::widget::Action::capture())
                    }
                } else {
                    None
                }
            }
            keyboard::Key::Named(Named::Escape) => {
                if state.focused_group.is_some() {
                    let gid = state.focused_group.take().unwrap();
                    let group_idx = self.interactive_elements.iter().position(|e| e.id == gid);
                    match self.set_focus(state, group_idx) {
                        Some(msg) => Some(iced::widget::Action::publish(msg).and_capture()),
                        None => Some(iced::widget::Action::capture()),
                    }
                } else if state.focused_id.is_some() {
                    match self.set_focus(state, None) {
                        Some(msg) => Some(iced::widget::Action::publish(msg).and_capture()),
                        None => Some(iced::widget::Action::capture()),
                    }
                } else {
                    None
                }
            }
            keyboard::Key::Named(Named::Home) if !arrow_indices.is_empty() => {
                focus_to(state, Some(arrow_indices[0]))
            }
            keyboard::Key::Named(Named::End) if !arrow_indices.is_empty() => {
                focus_to(state, Some(*arrow_indices.last().unwrap()))
            }
            keyboard::Key::Named(Named::PageDown) if !arrow_indices.is_empty() => {
                let page_size = PAGE_NAVIGATION_STEP.min(arrow_count);
                let pos = arrow_pos.unwrap_or(0);
                focus_to(
                    state,
                    Some(arrow_indices[(pos + page_size).min(arrow_count - 1)]),
                )
            }
            keyboard::Key::Named(Named::PageUp) if !arrow_indices.is_empty() => {
                let page_size = PAGE_NAVIGATION_STEP.min(arrow_count);
                let pos = arrow_pos.unwrap_or(0);
                focus_to(state, Some(arrow_indices[pos.saturating_sub(page_size)]))
            }
            _ => None,
        }
    }

    /// Handle a key release event. Mirrors `handle_keyboard` but only
    /// forwards nav keys as `CanvasElementKeyRelease` when `arrow_mode`
    /// is `"none"`. Focus management (Tab, Escape) is handled on press
    /// only; release doesn't change focus.
    fn handle_key_release(
        &self,
        state: &mut CanvasState,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<iced::widget::Action<Message>> {
        use keyboard::key::Named;

        if state.dragging.is_some() {
            return Some(iced::widget::Action::capture());
        }

        let current_idx = self.resolve_focus_index(state);

        if self.arrow_mode == ArrowMode::None
            && let Some(idx) = current_idx
        {
            let is_nav_key = matches!(
                key,
                keyboard::Key::Named(
                    Named::ArrowUp
                        | Named::ArrowDown
                        | Named::ArrowLeft
                        | Named::ArrowRight
                        | Named::Home
                        | Named::End
                        | Named::PageUp
                        | Named::PageDown
                )
            );
            if is_nav_key {
                let element = &self.interactive_elements[idx];
                let mods = serialize_modifiers(modifiers);
                return Some(
                    iced::widget::Action::publish(Message::Event {
                        window_id: self.window_id.clone(),
                        id: element.id.clone(),
                        family: "key_release".to_string(),
                        value: json!({
                            "key": crate::message::serialize_key(key),
                            "modifiers": modifiers_json(&mods),
                        }),
                    })
                    .and_capture(),
                );
            }
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Overlay helpers
// ---------------------------------------------------------------------------

/// Merge style overrides into a shape's JSON. The override object can
/// contain `fill`, `stroke`, `stroke_width`, `opacity`, which replace
/// the corresponding fields on the shape.
#[cfg(test)]
pub(crate) fn merge_shape_style(
    shape: &serde_json::Value,
    overrides: &serde_json::Value,
) -> serde_json::Value {
    let mut merged = shape.clone();
    if let (Some(merged_obj), Some(override_obj)) = (merged.as_object_mut(), overrides.as_object())
    {
        for (key, val) in override_obj {
            merged_obj.insert(key.clone(), val.clone());
        }
    }
    merged
}

/// Draw a focus ring around an interactive element.
///
/// The ring shape adapts to the element's hit region geometry:
/// - **Rect**: rounded rectangle inflated by `inflate` on each side
/// - **Circle**: circle inflated by `inflate`
/// - **Line**: capsule (stadium) around the line, inflated by `inflate`
///
/// The element's accumulated transform is applied to the frame so the
/// ring matches the element's visual position, including rotation and scale.
///
/// **Clipping note**: when the hit region fills the entire canvas, the
/// outset ring may be clipped. SDKs should add padding to the canvas
/// (e.g. 4px on each side) to accommodate the focus ring.
fn draw_focus_ring<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    element: &InteractiveElement,
    color: Color,
    stroke_width: f32,
    inflate: f32,
) {
    frame.push_transform();
    element.transform.apply_to_frame(frame);

    let ring_stroke = canvas::Stroke::default()
        .with_color(color)
        .with_width(stroke_width);

    match &element.hit_region {
        HitRegion::Rect { x, y, w, h } => {
            let radius = element.focus_ring_radius.unwrap_or(inflate + 1.0);
            let path = canvas::Path::rounded_rectangle(
                Point::new(x - inflate, y - inflate),
                Size::new(w + inflate * 2.0, h + inflate * 2.0),
                iced::border::Radius::from(radius),
            );
            frame.stroke(&path, ring_stroke);
        }
        HitRegion::Circle { cx, cy, r } => {
            let path = canvas::Path::circle(Point::new(*cx, *cy), r + inflate);
            frame.stroke(&path, ring_stroke);
        }
        HitRegion::Line {
            x1,
            y1,
            x2,
            y2,
            half_width,
        } => {
            // Draw a capsule (stadium shape) around the line.
            // This is a rounded rectangle oriented along the line.
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.01 {
                // Degenerate line: draw a circle at the midpoint.
                let path = canvas::Path::circle(
                    Point::new((x1 + x2) / 2.0, (y1 + y2) / 2.0),
                    half_width + inflate,
                );
                frame.stroke(&path, ring_stroke);
            } else {
                // Rotate so the line is horizontal, draw a rounded rect,
                // then the existing transform handles the visual rotation.
                let angle = dy.atan2(dx);
                let total_half = half_width + inflate;
                frame.push_transform();
                frame.translate(Vector::new(*x1, *y1));
                frame.rotate(Radians(angle));
                let path = canvas::Path::rounded_rectangle(
                    Point::new(-total_half, -total_half),
                    Size::new(len + total_half * 2.0, total_half * 2.0),
                    iced::border::Radius::from(total_half),
                );
                frame.stroke(&path, ring_stroke);
                frame.pop_transform();
            }
        }
    }

    frame.pop_transform();
}

/// Draw a tooltip overlay at the cursor position.
fn draw_tooltip<R: PlushieRenderer>(
    frame: &mut canvas::Frame<R>,
    text: &str,
    cursor: Point,
    bounds: Size,
    theme: &iced::Theme,
) {
    use iced::widget::canvas::Text;

    let palette = theme.palette();
    // Use inverse colors: dark bg on light theme, light bg on dark theme.
    let (bg_color, text_color) = if palette.is_dark {
        (
            Color::from_rgba(0.85, 0.85, 0.85, 0.95),
            Color::from_rgb(0.1, 0.1, 0.1),
        )
    } else {
        (
            Color::from_rgba(0.15, 0.15, 0.15, 0.95),
            Color::from_rgb(0.95, 0.95, 0.95),
        )
    };

    let padding = 6.0;
    let font_size = 13.0;
    // Estimate text width (rough: 0.6 * font_size per char).
    let est_width = text.chars().count() as f32 * font_size * 0.6 + padding * 2.0;
    let est_height = font_size + padding * 2.0;

    // Position tooltip near cursor, clamped to canvas bounds.
    let mut x = cursor.x + 12.0;
    let mut y = cursor.y - est_height - 4.0;
    if x + est_width > bounds.width {
        x = (cursor.x - est_width - 4.0).max(0.0);
    }
    if y < 0.0 {
        y = cursor.y + 20.0;
    }

    // Background
    let bg_rect = iced::Rectangle {
        x,
        y,
        width: est_width,
        height: est_height,
    };
    frame.fill_rectangle(
        Point::new(bg_rect.x, bg_rect.y),
        Size::new(bg_rect.width, bg_rect.height),
        bg_color,
    );

    // Text
    frame.fill_text(Text {
        content: text.to_string(),
        position: Point::new(x + padding, y + padding),
        color: text_color,
        size: Pixels(font_size),
        ..Text::default()
    });
}

// ---------------------------------------------------------------------------
// canvas::Program trait impl
// ---------------------------------------------------------------------------

impl<R: PlushieRenderer> canvas::Program<Message, iced::Theme, R> for CanvasProgram<'_, R> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &iced::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<iced::widget::Action<Message>> {
        // Track modifier state for pointer events.
        if let iced::Event::Keyboard(keyboard::Event::ModifiersChanged(mods)) = event {
            state.current_modifiers = *mods;
        }

        // Keyboard events don't depend on cursor position; handle them
        // before the cursor check so they work when the mouse is outside.
        if matches!(event, iced::Event::Keyboard(..)) {
            if !self.interactive_elements.is_empty() {
                if let iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key, modifiers, ..
                }) = event
                {
                    return self.handle_keyboard(state, key, *modifiers);
                }
                if let iced::Event::Keyboard(keyboard::Event::KeyReleased {
                    key, modifiers, ..
                }) = event
                {
                    return self.handle_key_release(state, key, *modifiers);
                }
            }
            return None;
        }

        // Consume pending programmatic focus (not position-dependent).
        if let Some(ref pending) = self.pending_focus
            && state.last_consumed_pending.as_deref() != Some(pending.as_str())
        {
            let idx = self
                .interactive_elements
                .iter()
                .position(|e| e.id == *pending);
            if let Some(idx) = idx {
                state.last_consumed_pending = Some(pending.clone());
                state.focused_group = self.interactive_elements[idx].parent_group.clone();
                if let Some(msg) = self.set_focus(state, Some(idx)) {
                    return Some(iced::widget::Action::publish(msg));
                }
            }
        }

        let position = match cursor.position_in(bounds) {
            Some(pos) => {
                state.cursor_position = Some(pos);
                pos
            }
            None => {
                // Cursor is outside canvas bounds. Clean up interaction
                // state so we don't have stale hover/drag.
                //
                // DragEnd is processed first (higher priority) because
                // losing a drag-end event leaves the host thinking the
                // drag is still active. ShapeLeave is less critical --
                // the host can infer leave from the drag-end.
                let mut action: Option<iced::widget::Action<Message>> = None;
                if let Some(drag) = state.dragging.take() {
                    let pos = state.cursor_position.unwrap_or(Point::ORIGIN);
                    let msg = Message::Event {
                        window_id: self.window_id.clone(),
                        id: drag.element_id,
                        family: "drag_end".to_string(),
                        value: json!({"x": sanitize_f32(pos.x), "y": sanitize_f32(pos.y)}),
                    };
                    action = Some(iced::widget::Action::publish(msg));
                }
                if let Some(hovered_id) = state.hovered_element.take() {
                    let msg = Message::Event {
                        window_id: self.window_id.clone(),
                        id: hovered_id,
                        family: "exit".to_string(),
                        value: serde_json::Value::Null,
                    };
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }
                state.pressed_element = None;
                state.cursor_position = None;
                return action;
            }
        };

        match event {
            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let mut action: Option<iced::widget::Action<Message>> = None;

                // -- Drag tracking --
                if let Some(ref mut drag) = state.dragging {
                    let shape = self
                        .interactive_elements
                        .iter()
                        .find(|s| s.id == drag.element_id);

                    // Start from raw cursor position, apply bounds
                    // clamping first, then axis constraints. This
                    // ensures axis-constrained drags still respect
                    // bounds on the constrained axis.
                    let mut effective = position;
                    if let Some(shape) = shape
                        && let Some(ref db) = shape.drag_bounds
                    {
                        effective.x = effective.x.clamp(db.min_x, db.max_x);
                        effective.y = effective.y.clamp(db.min_y, db.max_y);
                    }
                    let mut dx = effective.x - drag.last.x;
                    let mut dy = effective.y - drag.last.y;
                    if let Some(shape) = shape {
                        match shape.drag_axis {
                            DragAxis::X => dy = 0.0,
                            DragAxis::Y => dx = 0.0,
                            DragAxis::Both => {}
                        }
                    }
                    // Track the effective (clamped) position so deltas
                    // are consistent across frames.
                    drag.last = effective;
                    let msg = Message::Event {
                        window_id: self.window_id.clone(),
                        id: drag.element_id.clone(),
                        family: "drag".to_string(),
                        value: json!({
                            "x": sanitize_f32(effective.x),
                            "y": sanitize_f32(effective.y),
                            "delta_x": sanitize_f32(dx),
                            "delta_y": sanitize_f32(dy),
                        }),
                    };
                    action = Some(iced::widget::Action::publish(msg).and_capture());
                }

                // -- Hover tracking (skip during active drag) --
                if state.dragging.is_none() {
                    let hit = find_hit_element(position, self.interactive_elements);
                    let new_hovered = hit.map(|s| s.id.clone());
                    let old_hovered = state.hovered_element.take();

                    if new_hovered != old_hovered {
                        // Enter is emitted AFTER leave so that pick_action
                        // keeps Enter when both fire (direct A -> B transition).
                        // The host can infer leave from receiving enter for a
                        // different shape. Losing Enter is worse than losing
                        // Leave: Enter tells the host WHAT is hovered.
                        if let Some(ref old_id) = old_hovered {
                            let msg = Message::Event {
                                window_id: self.window_id.clone(),
                                id: old_id.clone(),
                                family: "exit".to_string(),
                                value: serde_json::Value::Null,
                            };
                            action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                        }
                        if let Some(ref new_id) = new_hovered {
                            let msg = Message::Event {
                                window_id: self.window_id.clone(),
                                id: new_id.clone(),
                                family: "enter".to_string(),
                                value: json!({
                                    "x": sanitize_f32(position.x),
                                    "y": sanitize_f32(position.y),
                                }),
                            };
                            // Override any previous action: Enter takes
                            // priority over Leave and raw canvas move.
                            action = Some(iced::widget::Action::publish(msg));
                        }
                    }
                    state.hovered_element = new_hovered;
                }

                // -- Raw canvas move event --
                if self.on_move {
                    let mods = serialize_modifiers(state.current_modifiers);
                    let msg = pointer_event(
                        "move",
                        &self.window_id,
                        &self.id,
                        position.x,
                        position.y,
                        None,
                        "mouse",
                        None,
                        mods,
                    );
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                action
            }

            iced::Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                let btn_str = serialize_mouse_button_for_canvas(button);
                let mut action: Option<iced::widget::Action<Message>> = None;

                // Mouse interaction clears focus-visible (focus ring only
                // shows for keyboard navigation, not mouse clicks).
                state.focus_visible = false;

                // -- Shape press: start drag or track pressed --
                // Drag and click are mutually exclusive: if a shape is
                // draggable, we start a drag (click never fires for it).
                // If it's only clickable, we track pressed state for
                // click detection on release.
                if matches!(button, mouse::Button::Left) {
                    if let Some(shape) = find_hit_element(position, self.interactive_elements) {
                        // Click-to-focus: move keyboard focus to clicked element.
                        let clicked_idx = self
                            .interactive_elements
                            .iter()
                            .position(|e| e.id == shape.id);
                        if let Some(msg) = self.set_focus(state, clicked_idx) {
                            action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                        }
                        // Update focused_group context for two-level navigation.
                        state.focused_group = shape.parent_group.clone();

                        if shape.draggable {
                            state.dragging = Some(DragState {
                                element_id: shape.id.clone(),
                                last: position,
                            });
                        } else if shape.on_click {
                            state.pressed_element = Some(shape.id.clone());
                        }
                    } else if state.focused_id.is_some() {
                        // Click on empty area: clear focus and group context.
                        state.focused_group = None;
                        if let Some(msg) = self.set_focus(state, None) {
                            action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                        }
                    }
                }

                // -- Raw canvas press event --
                if self.on_press {
                    let mods = serialize_modifiers(state.current_modifiers);
                    let msg = pointer_event(
                        "press",
                        &self.window_id,
                        &self.id,
                        position.x,
                        position.y,
                        Some(&btn_str),
                        "mouse",
                        None,
                        mods,
                    );
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                action
            }

            iced::Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                let btn_str = serialize_mouse_button_for_canvas(button);
                let mut action: Option<iced::widget::Action<Message>> = None;

                if matches!(button, mouse::Button::Left) {
                    // -- Drag end --
                    if let Some(drag) = state.dragging.take() {
                        let msg = Message::Event {
                            window_id: self.window_id.clone(),
                            id: drag.element_id,
                            family: "drag_end".to_string(),
                            value: json!({
                                "x": sanitize_f32(position.x),
                                "y": sanitize_f32(position.y),
                            }),
                        };
                        action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                    }

                    // -- Click detection: pressed shape == current hover --
                    if let Some(pressed_id) = state.pressed_element.take() {
                        let still_over = state
                            .hovered_element
                            .as_ref()
                            .map(|h| h == &pressed_id)
                            .unwrap_or(false);
                        if still_over {
                            let msg = Message::Event {
                                window_id: self.window_id.clone(),
                                id: pressed_id,
                                family: "click".to_string(),
                                value: json!({
                                    "x": sanitize_f32(position.x),
                                    "y": sanitize_f32(position.y),
                                    "button": btn_str,
                                }),
                            };
                            action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                        }
                    }
                }

                // -- Raw canvas release event --
                if self.on_release {
                    let mods = serialize_modifiers(state.current_modifiers);
                    let msg = pointer_event(
                        "release",
                        &self.window_id,
                        &self.id,
                        position.x,
                        position.y,
                        Some(&btn_str),
                        "mouse",
                        None,
                        mods,
                    );
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                action
            }

            iced::Event::Mouse(mouse::Event::WheelScrolled { delta }) if self.on_scroll => {
                let (dx, dy) = match delta {
                    mouse::ScrollDelta::Lines { x, y } => (*x, *y),
                    mouse::ScrollDelta::Pixels { x, y } => (*x, *y),
                };
                let mods = serialize_modifiers(state.current_modifiers);
                Some(iced::widget::Action::publish(pointer_scroll_event(
                    &self.window_id,
                    &self.id,
                    position.x,
                    position.y,
                    dx,
                    dy,
                    "mouse",
                    mods,
                )))
            }

            // -- Touch events --
            iced::Event::Touch(iced::touch::Event::FingerPressed {
                id: finger,
                position: touch_pos,
            }) => {
                let touch_position = match cursor.position_in(bounds) {
                    Some(_) => Point::new(touch_pos.x - bounds.x, touch_pos.y - bounds.y),
                    None => return None,
                };
                let mut action: Option<iced::widget::Action<Message>> = None;
                state.focus_visible = false;

                // Touch press same as left-click for interactive elements
                if let Some(shape) = find_hit_element(touch_position, self.interactive_elements) {
                    let clicked_idx = self
                        .interactive_elements
                        .iter()
                        .position(|e| e.id == shape.id);
                    if let Some(msg) = self.set_focus(state, clicked_idx) {
                        action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                    }
                    state.focused_group = shape.parent_group.clone();

                    if shape.draggable {
                        state.dragging = Some(DragState {
                            element_id: shape.id.clone(),
                            last: touch_position,
                        });
                    } else if shape.on_click {
                        state.pressed_element = Some(shape.id.clone());
                    }
                }

                if self.on_press {
                    let mods = serialize_modifiers(state.current_modifiers);
                    let msg = pointer_event(
                        "press",
                        &self.window_id,
                        &self.id,
                        touch_position.x,
                        touch_position.y,
                        Some("left"),
                        "touch",
                        Some(finger.0),
                        mods,
                    );
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                action
            }

            iced::Event::Touch(iced::touch::Event::FingerMoved {
                id: finger,
                position: touch_pos,
            }) => {
                let touch_position = Point::new(touch_pos.x - bounds.x, touch_pos.y - bounds.y);
                let mut action: Option<iced::widget::Action<Message>> = None;

                // Drag tracking (same as mouse CursorMoved)
                if let Some(ref mut drag) = state.dragging {
                    let mut effective = touch_position;
                    let shape = self
                        .interactive_elements
                        .iter()
                        .find(|s| s.id == drag.element_id);
                    if let Some(shape) = shape
                        && let Some(ref db) = shape.drag_bounds
                    {
                        effective.x = effective.x.clamp(db.min_x, db.max_x);
                        effective.y = effective.y.clamp(db.min_y, db.max_y);
                    }
                    let mut dx = effective.x - drag.last.x;
                    let mut dy = effective.y - drag.last.y;
                    if let Some(shape) = shape {
                        match shape.drag_axis {
                            DragAxis::X => dy = 0.0,
                            DragAxis::Y => dx = 0.0,
                            DragAxis::Both => {}
                        }
                    }
                    drag.last = effective;
                    let msg = Message::Event {
                        window_id: self.window_id.clone(),
                        id: drag.element_id.clone(),
                        family: "drag".to_string(),
                        value: json!({
                            "x": sanitize_f32(effective.x),
                            "y": sanitize_f32(effective.y),
                            "delta_x": sanitize_f32(dx),
                            "delta_y": sanitize_f32(dy),
                        }),
                    };
                    action = Some(iced::widget::Action::publish(msg).and_capture());
                }

                if self.on_move {
                    let mods = serialize_modifiers(state.current_modifiers);
                    let msg = pointer_event(
                        "move",
                        &self.window_id,
                        &self.id,
                        touch_position.x,
                        touch_position.y,
                        None,
                        "touch",
                        Some(finger.0),
                        mods,
                    );
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                action
            }

            iced::Event::Touch(iced::touch::Event::FingerLifted {
                id: finger,
                position: touch_pos,
            }) => {
                let touch_position = Point::new(touch_pos.x - bounds.x, touch_pos.y - bounds.y);
                let mut action: Option<iced::widget::Action<Message>> = None;

                // Drag end
                if let Some(drag) = state.dragging.take() {
                    let msg = Message::Event {
                        window_id: self.window_id.clone(),
                        id: drag.element_id,
                        family: "drag_end".to_string(),
                        value: json!({
                            "x": sanitize_f32(touch_position.x),
                            "y": sanitize_f32(touch_position.y),
                        }),
                    };
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                // Click detection
                if let Some(pressed_id) = state.pressed_element.take() {
                    let still_over = find_hit_element(touch_position, self.interactive_elements)
                        .map(|s| s.id == pressed_id)
                        .unwrap_or(false);
                    if still_over {
                        let msg = Message::Event {
                            window_id: self.window_id.clone(),
                            id: pressed_id,
                            family: "click".to_string(),
                            value: json!({
                                "x": sanitize_f32(touch_position.x),
                                "y": sanitize_f32(touch_position.y),
                                "button": "left",
                            }),
                        };
                        action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                    }
                }

                if self.on_release {
                    let mods = serialize_modifiers(state.current_modifiers);
                    let msg = pointer_event(
                        "release",
                        &self.window_id,
                        &self.id,
                        touch_position.x,
                        touch_position.y,
                        Some("left"),
                        "touch",
                        Some(finger.0),
                        mods,
                    );
                    action = Some(pick_action(action, iced::widget::Action::publish(msg)));
                }

                action
            }

            // Keyboard events are handled before the cursor position check
            // (at the top of update) so they work when the cursor is outside.
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &CanvasState,
        renderer: &R,
        theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<R>> {
        let mut geometries = Vec::new();

        // Background fill: cheap single rect, not cached.
        if let Some(bg) = self.background {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            frame.fill_rectangle(Point::ORIGIN, bounds.size(), bg);
            geometries.push(frame.into_geometry());
        }

        // Determine which layers need fresh drawing due to active interaction.
        let active_layers = self.layers_with_active_interaction(state);

        // Draw each layer, using its cache when available.
        let images = self.images;
        let theme_hash = canvas_theme_hash(theme);
        for (layer_name, shapes) in &self.layers {
            let shape_refs: Vec<&CanvasShape> = shapes.iter().collect();
            let force_redraw = active_layers.iter().any(|l| l == layer_name);

            let geom = if !force_redraw {
                if let Some(record) = self.caches.and_then(|c| c.get(layer_name)) {
                    record.ensure_theme_hash(theme_hash);
                    record.cache.draw(renderer, bounds.size(), |frame| {
                        draw_canvas_shapes(frame, &shape_refs, images, theme);
                    })
                } else {
                    let mut frame = canvas::Frame::new(renderer, bounds.size());
                    draw_canvas_shapes(&mut frame, &shape_refs, images, theme);
                    frame.into_geometry()
                }
            } else {
                // Layer has active interaction (hover/pressed/focus style):
                // clear cache and draw fresh with style overrides applied.
                if let Some(record) = self.caches.and_then(|c| c.get(layer_name)) {
                    record.cache.clear();
                }
                let mut frame = canvas::Frame::new(renderer, bounds.size());
                self.draw_shapes_with_overrides(&mut frame, &shape_refs, state, images, theme);
                frame.into_geometry()
            };
            geometries.push(geom);
        }

        // Tooltip overlay (uncached, drawn on top of all layers).
        if let Some(ref tooltip) = self.active_tooltip(state)
            && let Some(pos) = state.cursor_position
        {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            draw_tooltip(&mut frame, tooltip, pos, bounds.size(), theme);
            geometries.push(frame.into_geometry());
        }

        // Focus ring overlay (uncached, drawn on top of everything).
        // Only drawn when the canvas has iced-level focus.
        if state.canvas_focused
            && state.focus_visible
            && let Some(focused_id) = &state.focused_id
            && let Some(element) = self
                .interactive_elements
                .iter()
                .find(|e| &e.id == focused_id)
            && element.show_focus_ring
        {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            let focus_color = theme.palette().primary.base.color;
            draw_focus_ring(&mut frame, element, focus_color, 2.0, 2.0);
            geometries.push(frame.into_geometry());
        }

        geometries
    }

    fn mouse_interaction(
        &self,
        state: &CanvasState,
        _bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        // Dragging overrides everything.
        if state.dragging.is_some() {
            return mouse::Interaction::Grabbing;
        }
        // Per-shape cursor.
        if let Some(ref hovered_id) = state.hovered_element
            && let Some(shape) = self
                .interactive_elements
                .iter()
                .find(|s| &s.id == hovered_id)
        {
            if let Some(ref cursor_name) = shape.cursor {
                return parse_cursor_interaction(cursor_name);
            }
            // Default cursor for interactive elements without explicit cursor.
            return mouse::Interaction::Pointer;
        }
        // Fallback to canvas-level cursor.
        if self.is_interactive() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }

    fn is_focusable(&self, _state: &CanvasState) -> bool {
        !self.interactive_elements.is_empty()
    }

    fn on_focus_gained(
        &self,
        state: &mut CanvasState,
        focus_visible: bool,
    ) -> Vec<iced::widget::Action<Message>> {
        state.canvas_focused = true;
        state.focus_visible = focus_visible;
        let mut actions = vec![iced::widget::Action::publish(Message::Event {
            window_id: self.window_id.clone(),
            id: self.id.clone(),
            family: "focused".to_string(),
            value: serde_json::Value::Null,
        })];
        // If returning to a canvas that had internal focus, re-announce
        // the focused element, but only if it still exists. If it was
        // removed while the canvas was unfocused, clear the stale ID.
        if let Some(ref id) = state.focused_id {
            let still_exists = self.interactive_elements.iter().any(|e| &e.id == id);
            if still_exists {
                actions.push(iced::widget::Action::publish(Message::Event {
                    window_id: self.window_id.clone(),
                    id: id.clone(),
                    family: "focused".to_string(),
                    value: serde_json::Value::Null,
                }));
            } else {
                // Element was removed while canvas was unfocused.
                // Emit blur for the stale element and clear.
                actions.push(iced::widget::Action::publish(Message::Event {
                    window_id: self.window_id.clone(),
                    id: id.clone(),
                    family: "blurred".to_string(),
                    value: serde_json::Value::Null,
                }));
                state.focused_id = None;
                state.focused_group = None;
            }
        }
        actions
    }

    fn on_focus_lost(&self, state: &mut CanvasState) -> Vec<iced::widget::Action<Message>> {
        state.canvas_focused = false;
        let mut actions = Vec::new();
        // Emit blur for the currently focused element (but DON'T clear
        // focused_id, preserve position so re-entry via Tab returns
        // to the same element).
        if let Some(ref id) = state.focused_id {
            actions.push(iced::widget::Action::publish(Message::Event {
                window_id: self.window_id.clone(),
                id: id.clone(),
                family: "blurred".to_string(),
                value: serde_json::Value::Null,
            }));
        }
        actions.push(iced::widget::Action::publish(Message::Event {
            window_id: self.window_id.clone(),
            id: self.id.clone(),
            family: "blurred".to_string(),
            value: serde_json::Value::Null,
        }));
        actions
    }

    fn active_descendant_id(&self, state: &CanvasState) -> Option<iced::widget::Id> {
        state
            .focused_id
            .as_ref()
            .map(|id| iced::widget::Id::from(id.clone()))
    }

    fn operate_accessible(
        &self,
        _state: &CanvasState,
        canvas_bounds: iced::Rectangle,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        let mut seen_ids = std::collections::HashSet::new();

        // Emit accessible nodes for each interactive element.
        // Focusable groups use traverse() to create parent-child relationships
        // in the accessibility tree. A focusable group without a11y metadata
        // still traverses its children so they remain reachable by AT.
        let canvas_pos = iced::Point::new(canvas_bounds.x, canvas_bounds.y);

        for element in self.interactive_elements {
            // Skip elements without a11y unless they are focusable groups
            // (which must still traverse children for the a11y tree).
            if element.a11y.is_none() && !element.focusable {
                continue;
            }
            if !seen_ids.insert(&element.id) {
                continue;
            }

            let local_rect = hit_region_to_rect(&element.hit_region);
            let element_bounds = transformed_bounds(local_rect, &element.transform, canvas_pos);

            let wid = iced::widget::Id::from(element.id.clone());

            if element.focusable {
                // Focusable group: emit as parent (if it has a11y), then
                // traverse children regardless so they remain reachable.
                if let Some(ref a11y) = element.a11y {
                    operation.accessible(Some(&wid), element_bounds, &a11y.to_accessible());
                }
                operation.traverse(&mut |child_op| {
                    for child in self.interactive_elements.iter() {
                        if child.parent_group.as_deref() != Some(&element.id) {
                            continue;
                        }
                        if let Some(ref child_a11y) = child.a11y {
                            if !seen_ids.insert(&child.id) {
                                continue;
                            }
                            let cr = hit_region_to_rect(&child.hit_region);
                            let child_bounds = transformed_bounds(cr, &child.transform, canvas_pos);
                            let child_wid = iced::widget::Id::from(child.id.clone());
                            child_op.accessible(
                                Some(&child_wid),
                                child_bounds,
                                &child_a11y.to_accessible(),
                            );
                        }
                    }
                });
            } else if element.parent_group.is_none() {
                // Top-level non-group element (a11y guaranteed present by
                // the early skip above).
                let a11y = element.a11y.as_ref().unwrap();
                operation.accessible(Some(&wid), element_bounds, &a11y.to_accessible());
            }
            // Elements with parent_group are emitted inside their group's traverse().
        }
    }
}
