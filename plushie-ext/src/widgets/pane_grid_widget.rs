use std::collections::{HashMap, HashSet};

use iced::widget::{pane_grid, text};
use iced::{Element, Length, Theme};

use crate::PlushieRenderer;
use crate::message::Message;
use crate::protocol::{OutgoingEvent, TreeNode};
use crate::registry::PlushieWidget;
use crate::render_ctx::RenderCtx;
use crate::widgets::helpers::*;

/// Stateful pane_grid factory (owns `pane_grid::State`).
///
/// Prepare reconciles panes against tree children (add/remove).
/// Handle_message resolves opaque pane handles to node IDs and
/// mutates state on resize/drag/click.
pub(crate) struct PaneGridWidget {
    /// pane_grid layout state per (window_id, node_id).
    states: HashMap<(String, String), pane_grid::State<String>>,
}

impl PaneGridWidget {
    pub(crate) fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }
}

impl<R: PlushieRenderer> PlushieWidget<R> for PaneGridWidget {
    fn type_names(&self) -> &[&str] {
        &["pane_grid"]
    }

    fn prepare(&mut self, node: &TreeNode, window_id: &str, _theme: &iced::Theme) {
        let key = (window_id.to_string(), node.id.clone());
        let props = node.props.as_object();
        let axis = match crate::prop_helpers::prop_str(props, "split_axis").as_deref() {
            Some("horizontal") => pane_grid::Axis::Horizontal,
            _ => pane_grid::Axis::Vertical,
        };
        let child_ids: HashSet<String> = node.children.iter().map(|c| c.id.clone()).collect();

        if let Some(state) = self.states.get_mut(&key) {
            // Prune panes whose child nodes no longer exist.
            let stale_panes: Vec<pane_grid::Pane> = state
                .panes
                .iter()
                .filter(|(_pane, id)| !child_ids.contains(*id))
                .map(|(pane, _id)| *pane)
                .collect();
            for pane in stale_panes {
                state.close(pane);
            }
            // Add panes for new children.
            let existing_ids: HashSet<String> = state.panes.values().cloned().collect();
            let new_child_ids: Vec<String> = node
                .children
                .iter()
                .filter(|c| !existing_ids.contains(&c.id))
                .map(|c| c.id.clone())
                .collect();
            for new_id in new_child_ids {
                if let Some((&anchor, _)) = state.panes.iter().next() {
                    let _ = state.split(axis, anchor, new_id);
                }
            }
        } else {
            let child_list: Vec<String> = node.children.iter().map(|c| c.id.clone()).collect();
            let new_state = if child_list.is_empty() {
                let (state, _) = pane_grid::State::new("default".to_string());
                state
            } else if child_list.len() == 1 {
                let (state, _) = pane_grid::State::new(child_list[0].clone());
                state
            } else {
                let (mut state, first_pane) = pane_grid::State::new(child_list[0].clone());
                let mut last_pane = first_pane;
                for id in child_list.iter().skip(1) {
                    if let Some((new_pane, _)) = state.split(axis, last_pane, id.clone()) {
                        last_pane = new_pane;
                    }
                }
                state
            };
            self.states.insert(key, new_state);
        }
    }

    fn render<'a>(
        &'a self,
        node: &'a TreeNode,
        ctx: &RenderCtx<'a, R>,
    ) -> Element<'a, Message, Theme, R> {
        let key = (ctx.window_id.to_string(), node.id.clone());
        match self.states.get(&key) {
            Some(state) => render_pane_grid_with_state(node, *ctx, state),
            None => text("(pane_grid: no state)").into(),
        }
    }

    fn handle_message(&mut self, msg: &Message) -> Option<Vec<OutgoingEvent>> {
        match msg {
            Message::PaneFocusCycle(window_id, grid_id, pane) => {
                let key = (window_id.to_string(), grid_id.to_string());
                if let Some(state) = self.states.get(&key) {
                    let pane_id = state.get(*pane).cloned().unwrap_or_default();
                    Some(vec![
                        OutgoingEvent::pane_focus_cycle(grid_id.clone(), pane_id)
                            .with_window_id(window_id.clone()),
                    ])
                } else {
                    Some(vec![])
                }
            }
            Message::PaneResized(window_id, grid_id, evt) => {
                let key = (window_id.to_string(), grid_id.to_string());
                if let Some(state) = self.states.get_mut(&key) {
                    state.resize(evt.split, evt.ratio);
                }
                Some(vec![
                    OutgoingEvent::pane_resized(
                        grid_id.clone(),
                        format!("{:?}", evt.split),
                        evt.ratio,
                    )
                    .with_window_id(window_id.clone()),
                ])
            }
            Message::PaneDragged(window_id, grid_id, evt) => {
                let key = (window_id.to_string(), grid_id.to_string());
                match evt {
                    pane_grid::DragEvent::Picked { pane } => {
                        if let Some(state) = self.states.get(&key) {
                            let pane_id = state.get(*pane).cloned().unwrap_or_default();
                            Some(vec![
                                OutgoingEvent::pane_dragged(
                                    grid_id.clone(),
                                    "picked",
                                    pane_id,
                                    None,
                                    None,
                                    None,
                                )
                                .with_window_id(window_id.clone()),
                            ])
                        } else {
                            Some(vec![])
                        }
                    }
                    pane_grid::DragEvent::Dropped { pane, target } => {
                        if let Some(state) = self.states.get_mut(&key) {
                            let pane_id = state.get(*pane).cloned().unwrap_or_default();
                            let (target_pane, region, edge) = match target {
                                pane_grid::Target::Edge(e) => {
                                    let edge_str = match e {
                                        pane_grid::Edge::Top => "top",
                                        pane_grid::Edge::Bottom => "bottom",
                                        pane_grid::Edge::Left => "left",
                                        pane_grid::Edge::Right => "right",
                                    };
                                    (None, None, Some(edge_str))
                                }
                                pane_grid::Target::Pane(p, region) => {
                                    let target_id = state.get(*p).cloned().unwrap_or_default();
                                    let region_str = match region {
                                        pane_grid::Region::Center => "center",
                                        pane_grid::Region::Edge(pane_grid::Edge::Top) => "top",
                                        pane_grid::Region::Edge(pane_grid::Edge::Bottom) => {
                                            "bottom"
                                        }
                                        pane_grid::Region::Edge(pane_grid::Edge::Left) => "left",
                                        pane_grid::Region::Edge(pane_grid::Edge::Right) => "right",
                                    };
                                    (Some(target_id), Some(region_str), None)
                                }
                            };
                            state.drop(*pane, *target);
                            Some(vec![
                                OutgoingEvent::pane_dragged(
                                    grid_id.clone(),
                                    "dropped",
                                    pane_id,
                                    target_pane,
                                    region,
                                    edge,
                                )
                                .with_window_id(window_id.clone()),
                            ])
                        } else {
                            Some(vec![])
                        }
                    }
                    pane_grid::DragEvent::Canceled { pane } => {
                        if let Some(state) = self.states.get(&key) {
                            let pane_id = state.get(*pane).cloned().unwrap_or_default();
                            Some(vec![
                                OutgoingEvent::pane_dragged(
                                    grid_id.clone(),
                                    "canceled",
                                    pane_id,
                                    None,
                                    None,
                                    None,
                                )
                                .with_window_id(window_id.clone()),
                            ])
                        } else {
                            Some(vec![])
                        }
                    }
                }
            }
            Message::PaneClicked(window_id, grid_id, pane) => {
                let key = (window_id.to_string(), grid_id.to_string());
                if let Some(state) = self.states.get(&key) {
                    let pane_id = state.get(*pane).cloned().unwrap_or_default();
                    Some(vec![
                        OutgoingEvent::pane_clicked(grid_id.clone(), pane_id)
                            .with_window_id(window_id.clone()),
                    ])
                } else {
                    Some(vec![])
                }
            }
            _ => None,
        }
    }

    fn handle_widget_op(
        &mut self,
        node_id: &str,
        op: &str,
        payload: &serde_json::Value,
    ) -> Option<Vec<OutgoingEvent>> {
        // Find state by node_id (any window).
        let key = self
            .states
            .keys()
            .find(|(_, nid)| nid == node_id)
            .cloned()?;
        let state = self.states.get_mut(&key)?;

        match op {
            "pane_split" => {
                let pane_id = payload
                    .get("pane")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let new_pane_id = payload
                    .get("new_pane_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let axis = match payload
                    .get("axis")
                    .and_then(|v| v.as_str())
                    .unwrap_or("vertical")
                {
                    "horizontal" => pane_grid::Axis::Horizontal,
                    _ => pane_grid::Axis::Vertical,
                };
                if let Some(pane) = find_pane_by_id(state, pane_id) {
                    let _ = state.split(axis, pane, new_pane_id);
                }
                Some(vec![])
            }
            "pane_close" => {
                let pane_id = payload
                    .get("pane")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if let Some(pane) = find_pane_by_id(state, pane_id) {
                    let _ = state.close(pane);
                }
                Some(vec![])
            }
            "pane_swap" => {
                let a_id = payload
                    .get("a")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let b_id = payload
                    .get("b")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if let (Some(a), Some(b)) =
                    (find_pane_by_id(state, a_id), find_pane_by_id(state, b_id))
                {
                    state.swap(a, b);
                }
                Some(vec![])
            }
            "pane_maximize" => {
                let pane_id = payload
                    .get("pane")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if let Some(pane) = find_pane_by_id(state, pane_id) {
                    state.maximize(pane);
                }
                Some(vec![])
            }
            "pane_restore" => {
                state.restore();
                Some(vec![])
            }
            _ => None,
        }
    }

    fn cleanup(&mut self, node_id: &str, window_id: &str) {
        let key = (window_id.to_string(), node_id.to_string());
        self.states.remove(&key);
    }

    fn clone_for_session(&self) -> Box<dyn PlushieWidget<R>> {
        Box::new(PaneGridWidget::new())
    }
}

/// Find a pane by its ID string in a pane_grid::State.
fn find_pane_by_id(
    state: &pane_grid::State<String>,
    pane_id: &str,
) -> Option<pane_grid::Pane> {
    state
        .panes
        .iter()
        .find(|(_, id)| id.as_str() == pane_id)
        .map(|(pane, _)| *pane)
}

/// Render a pane_grid with the provided State.
///
/// The pane_grid renders as nested containers with no inherent semantic
/// structure. Hosts should set `a11y.label` on each pane node and use
/// `a11y.role = "group"` on the grid node for accessibility.
fn render_pane_grid_with_state<'a, R: PlushieRenderer>(
    node: &'a TreeNode,
    ctx: RenderCtx<'a, R>,
    state: &'a pane_grid::State<String>,
) -> Element<'a, Message, Theme, R> {
    let props = node.props.as_object();
    let spacing = prop_animated_f32(&ctx.caches.interpolated_props, &node.id, props, "spacing")
        .unwrap_or(2.0);
    let width = prop_length(props, "width", Length::Fill);
    let height = prop_length(props, "height", Length::Fill);

    // Pre-render children into a map keyed by plushie ID. Also extract
    // title props from child nodes before the closure consumes the elements.
    let mut child_map: HashMap<String, Element<'a, Message, Theme, R>> = HashMap::new();
    let mut title_map: HashMap<String, String> = HashMap::new();
    for c in &node.children {
        child_map.insert(c.id.clone(), ctx.render_child(c));
        if let Some(title) = prop_str(c.props.as_object(), "title") {
            title_map.insert(c.id.clone(), title);
        }
    }

    // We need to move child_map into the closure but PaneGrid::new
    // requires FnMut, so use a RefCell to allow mutation.
    let child_map = std::cell::RefCell::new(child_map);

    let node_id = node.id.clone();
    let node_id2 = node.id.clone();
    let node_id3 = node.id.clone();
    let node_id4 = node.id.clone();
    let window_id = ctx.window_id.to_string();
    let window_id2 = window_id.clone();
    let window_id3 = window_id.clone();
    let window_id4 = window_id.clone();

    let mut pg = pane_grid::PaneGrid::new(state, |_pane, pane_id, _is_maximized| {
        let child_element: Element<'a, Message, Theme, R> = child_map
            .borrow_mut()
            .remove(pane_id)
            .unwrap_or_else(|| text(format!("(pane: {})", pane_id)).into());
        let content = pane_grid::Content::new(child_element);
        if let Some(title_text) = title_map.get(pane_id) {
            let title_bar = pane_grid::TitleBar::new(text(title_text.clone()).size(14.0));
            content.title_bar(title_bar)
        } else {
            content
        }
    })
    .width(width)
    .height(height)
    .spacing(spacing);

    let min_size = prop_f32(props, "min_size").unwrap_or(10.0).max(1.0);
    let leeway = prop_f32(props, "leeway").unwrap_or(min_size);

    pg = pg.on_click(move |pane| Message::PaneClicked(window_id3.clone(), node_id3.clone(), pane));
    pg = pg.on_resize(leeway, move |evt| {
        Message::PaneResized(window_id.clone(), node_id.clone(), evt)
    });
    pg = pg.on_drag(move |evt| Message::PaneDragged(window_id2.clone(), node_id2.clone(), evt));
    pg = pg.on_focus_cycle(move |pane| {
        Message::PaneFocusCycle(window_id4.clone(), node_id4.clone(), pane)
    });

    // Divider styling
    let divider_color = prop_color(props, "divider_color");
    let divider_width = prop_f32(props, "divider_width");
    if divider_color.is_some() || divider_width.is_some() {
        pg = pg.style(move |theme: &iced::Theme| {
            let mut style = pane_grid::default(theme);
            if let Some(dc) = divider_color {
                style.hovered_split.color = dc;
                style.picked_split.color = dc;
            }
            if let Some(dw) = divider_width {
                style.hovered_split.width = dw;
                style.picked_split.width = dw;
            }
            style
        });
    }

    pg.into()
}
