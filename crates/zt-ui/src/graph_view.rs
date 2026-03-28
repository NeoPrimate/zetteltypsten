use crate::theme;
use gpui::*;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zt_core::note::NoteId;

pub enum GraphViewEvent {
    OpenFile(String),
}

impl EventEmitter<GraphViewEvent> for GraphView {}

struct GraphNode {
    rel_path: String,
    title: String,
    x: f32,
    y: f32,
    /// Base radius in pixels (unscaled by zoom), grows logarithmically with in-degree.
    radius: f32,
}

pub struct GraphView {
    vault_root: PathBuf,
    nodes: Vec<GraphNode>,
    edges: Vec<(usize, usize)>,
    vel: Vec<(f32, f32)>,
    // Drag state
    dragging: Option<usize>,
    drag_moved: bool,
    // Pan state (click-drag on background)
    panning: bool,
    pan_start_mouse: Option<(f32, f32)>,
    pan_at_start: (f32, f32),
    // Viewport transform: vx = nx * zoom + pan.0
    zoom: f32,
    pan: (f32, f32),
    // Simulation always runs while on this view.
    simulating: bool,
    /// Canvas pixel bounds from last prepaint (window-absolute).
    canvas_bounds: Arc<Mutex<Option<Bounds<Pixels>>>>,
    /// Local (parent-relative) screen pos per node, written by canvas paint.
    node_screen_pos: Arc<Mutex<Vec<(f32, f32)>>>,
    build_task: Option<Task<()>>,
    /// Ticker task — runs for the lifetime of this view, steps sim at ~60 fps.
    sim_task: Option<Task<()>>,
}

const NODE_R: f32 = 16.0;
const PAD: f32 = 60.0;

impl GraphView {
    pub fn new(vault_root: PathBuf, cx: &mut Context<Self>) -> Self {
        let mut gv = Self {
            vault_root,
            nodes: Vec::new(),
            edges: Vec::new(),
            vel: Vec::new(),
            dragging: None,
            drag_moved: false,
            panning: false,
            pan_start_mouse: None,
            pan_at_start: (0.0, 0.0),
            zoom: 0.55,
            pan: (0.0, 0.0),
            simulating: false,
            canvas_bounds: Arc::new(Mutex::new(None)),
            node_screen_pos: Arc::new(Mutex::new(Vec::new())),
            build_task: None,
            sim_task: None,
        };
        gv.rebuild(cx);

        // Ticker: runs for the lifetime of this view, drives the simulation at ~60 fps.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let alive = cx
                    .update(|cx| {
                        this.update(cx, |gv, cx| {
                            if gv.simulating {
                                gv.step();
                                cx.notify();
                            }
                        })
                        .is_ok()
                    })
                    .unwrap_or(false);
                if !alive {
                    break;
                }
            }
        });
        gv.sim_task = Some(ticker);

        gv
    }

    pub fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.build_task = None;
        self.simulating = false;
        self.dragging = None;
        self.drag_moved = false;
        self.panning = false;
        self.pan_start_mouse = None;

        // Save current positions so the graph doesn't jump on rebuild.
        let saved: HashMap<String, (f32, f32)> = self
            .nodes
            .iter()
            .map(|n| (n.rel_path.clone(), (n.x, n.y)))
            .collect();

        let root = self.vault_root.clone();
        let bg = cx.background_executor().clone();

        let task = cx.spawn(async move |this, cx| {
            let (nodes, edges) = bg
                .spawn(async move {
                    match zt_index::indexer::VaultIndex::build(&root) {
                        Ok(index) => build_graph_data(&index),
                        Err(e) => {
                            log::error!("Graph index error: {e}");
                            (Vec::new(), Vec::new())
                        }
                    }
                })
                .await;

            cx.update(|cx| {
                this.update(cx, |gv, cx| {
                    let n = nodes.len();
                    gv.nodes = nodes;
                    gv.edges = edges;
                    gv.vel = vec![(0.0, 0.0); n];

                    // Restore positions for nodes that existed before rebuild.
                    for node in &mut gv.nodes {
                        if let Some(&(ox, oy)) = saved.get(&node.rel_path) {
                            node.x = ox;
                            node.y = oy;
                        }
                    }

                    gv.simulating = true;
                    gv.node_screen_pos.lock().unwrap().clear();
                    cx.notify();
                })
                .ok();
            })
            .ok();
        });

        self.build_task = Some(task);
    }

    fn step(&mut self) {
        let n = self.nodes.len();
        if n == 0 {
            return;
        }

        let mut forces = vec![(0.0_f32, 0.0_f32); n];

        // Repulsion — all pairs.
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = self.nodes[i].x - self.nodes[j].x;
                let dy = self.nodes[i].y - self.nodes[j].y;
                let dist = (dx * dx + dy * dy).sqrt().max(0.01);
                let f = 0.001 / (dist * dist);
                let fx = f * dx / dist;
                let fy = f * dy / dist;
                forces[i].0 += fx;
                forces[i].1 += fy;
                forces[j].0 -= fx;
                forces[j].1 -= fy;
            }
        }

        // Attraction — edges (spring, rest length 0.3).
        for &(a, b) in &self.edges {
            let dx = self.nodes[b].x - self.nodes[a].x;
            let dy = self.nodes[b].y - self.nodes[a].y;
            let dist = (dx * dx + dy * dy).sqrt().max(0.01);
            let f = 0.01 * (dist - 0.3);
            let fx = f * dx / dist;
            let fy = f * dy / dist;
            forces[a].0 += fx;
            forces[a].1 += fy;
            forces[b].0 -= fx;
            forces[b].1 -= fy;
        }

        // Integrate — dragged node is held in place by the mouse.
        for i in 0..n {
            if self.dragging == Some(i) {
                self.vel[i] = (0.0, 0.0);
                continue;
            }
            forces[i].0 -= 0.01 * self.nodes[i].x; // centre gravity
            forces[i].1 -= 0.01 * self.nodes[i].y;
            self.vel[i].0 = (self.vel[i].0 + forces[i].0) * 0.85;
            self.vel[i].1 = (self.vel[i].1 + forces[i].1) * 0.85;
            self.nodes[i].x += self.vel[i].0;
            self.nodes[i].y += self.vel[i].1;
        }
    }

    /// Convert window-space mouse position to graph (normalised) space,
    /// accounting for the current zoom and pan.
    fn screen_to_graph(&self, wx: f32, wy: f32) -> Option<(f32, f32)> {
        let bounds = (*self.canvas_bounds.lock().unwrap())?;
        let ox = f32::from(bounds.origin.x);
        let oy = f32::from(bounds.origin.y);
        let w = f32::from(bounds.size.width);
        let h = f32::from(bounds.size.height);
        let uw = (w - PAD * 2.0).max(1.0);
        let uh = (h - PAD * 2.0).max(1.0);
        // local pixel → viewport [-1,1] → graph
        let vx = (wx - ox - PAD) / uw * 2.0 - 1.0;
        let vy = (wy - oy - PAD) / uh * 2.0 - 1.0;
        let gx = (vx - self.pan.0) / self.zoom;
        let gy = (vy - self.pan.1) / self.zoom;
        Some((gx, gy))
    }
}

// ---------------------------------------------------------------------------
// Graph data extraction
// ---------------------------------------------------------------------------

fn build_graph_data(
    index: &zt_index::indexer::VaultIndex,
) -> (Vec<GraphNode>, Vec<(usize, usize)>) {
    let all_ids: Vec<NoteId> = index.link_graph.all_notes().into_iter().cloned().collect();
    if all_ids.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let n = all_ids.len();
    let id_to_idx: HashMap<&NoteId, usize> =
        all_ids.iter().enumerate().map(|(i, id)| (id, i)).collect();

    let edges: Vec<(usize, usize)> = index
        .link_graph
        .all_edges()
        .into_iter()
        .filter_map(|(src, tgt)| Some((*id_to_idx.get(src)?, *id_to_idx.get(tgt)?)))
        .collect();

    // Compute in-degree for each node.
    let mut in_degree = vec![0usize; n];
    for &(_src, tgt) in &edges {
        if tgt < n {
            in_degree[tgt] += 1;
        }
    }

    // Alternate radii to break circle symmetry so simulation is visibly animated.
    let nodes = all_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let title = index
                .titles
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.display_name().to_string());
            let rel_path = id.to_path().to_string_lossy().into_owned();
            let angle = 2.0 * PI * i as f32 / n as f32;
            let r = if i % 2 == 0 { 0.6_f32 } else { 0.8_f32 };
            // Scale node visual radius logarithmically with in-degree.
            let radius = NODE_R * (1.0 + 0.5 * (in_degree[i] as f32 + 1.0).ln());
            GraphNode {
                rel_path,
                title,
                x: angle.cos() * r,
                y: angle.sin() * r,
                radius,
            }
        })
        .collect();

    (nodes, edges)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl Render for GraphView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface0 = theme::surface0();
        let mantle = theme::mantle();
        let blue = theme::blue();
        let overlay0 = theme::overlay0();
        let text_color = theme::text();
        let subtext = theme::subtext0();

        let bar = div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0);

        if self.nodes.is_empty() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .bg(mantle)
                .on_mouse_move(cx.listener(|_, _, _, _| {}))
                .on_mouse_up(MouseButton::Left, cx.listener(|_, _, _, _| {}))
                .on_scroll_wheel(cx.listener(|_, _, _, _| {}))
                .child(bar)
                .child(
                    div().flex_1().flex().items_center().justify_center().child(
                        div()
                            .text_color(subtext)
                            .text_sm()
                            .child(if self.build_task.is_some() {
                                "Building graph…"
                            } else {
                                "No notes found"
                            }),
                    ),
                );
        }

        // Snapshot for canvas closures (must be Send + 'static).
        let positions_snap: Vec<(f32, f32)> = self.nodes.iter().map(|n| (n.x, n.y)).collect();
        let radii_snap: Vec<f32> = self.nodes.iter().map(|n| n.radius).collect();
        let edges_snap = self.edges.clone();
        let bounds_arc = self.canvas_bounds.clone();
        let screen_pos_arc = self.node_screen_pos.clone();
        let zoom_snap = self.zoom;
        let pan_snap = self.pan;

        let graph_canvas = canvas(
            move |bounds, _window, _cx| {
                *bounds_arc.lock().unwrap() = Some(bounds);
                (
                    bounds,
                    positions_snap.clone(),
                    edges_snap.clone(),
                    radii_snap.clone(),
                )
            },
            move |_bounds, (bounds, nodes, edges, radii), window, _cx| {
                let ox = f32::from(bounds.origin.x);
                let oy = f32::from(bounds.origin.y);
                let w = f32::from(bounds.size.width);
                let h = f32::from(bounds.size.height);
                let uw = (w - PAD * 2.0).max(1.0);
                let uh = (h - PAD * 2.0).max(1.0);

                // Compute local (parent-relative) and absolute positions.
                let mut local: Vec<(f32, f32)> = Vec::with_capacity(nodes.len());
                let mut abs: Vec<(f32, f32)> = Vec::with_capacity(nodes.len());
                for &(nx, ny) in &nodes {
                    // Transform: graph → viewport → local pixel
                    let vx = nx * zoom_snap + pan_snap.0;
                    let vy = ny * zoom_snap + pan_snap.1;
                    let lx = PAD + (vx + 1.0) / 2.0 * uw;
                    let ly = PAD + (vy + 1.0) / 2.0 * uh;
                    local.push((lx, ly));
                    abs.push((ox + lx, oy + ly));
                }
                *screen_pos_arc.lock().unwrap() = local;

                // Edges — window-absolute coords.
                for &(a, b) in &edges {
                    if a >= abs.len() || b >= abs.len() {
                        continue;
                    }
                    let (x1, y1) = abs[a];
                    let (x2, y2) = abs[b];
                    let mut p = PathBuilder::stroke(px(1.0));
                    p.move_to(point(px(x1), px(y1)));
                    p.line_to(point(px(x2), px(y2)));
                    if let Ok(path) = p.build() {
                        window.paint_path(path, overlay0);
                    }
                }

                // Node circles — window-absolute coords, per-node radius.
                for (idx, &(ax, ay)) in abs.iter().enumerate() {
                    let base_r = radii.get(idx).copied().unwrap_or(NODE_R);
                    let r = (base_r * zoom_snap).clamp(4.0, 40.0);
                    window.paint_quad(PaintQuad {
                        bounds: Bounds {
                            origin: point(px(ax - r), px(ay - r)),
                            size: size(px(r * 2.0), px(r * 2.0)),
                        },
                        corner_radii: Corners::all(px(r)),
                        background: blue.into(),
                        border_widths: Edges::all(px(0.0)),
                        border_color: Hsla::transparent_black(),
                        border_style: BorderStyle::Solid,
                    });
                }
            },
        )
        .size_full();

        // Overlay divs use LOCAL positions from the previous frame's paint.
        let local_positions = self.node_screen_pos.lock().unwrap().clone();
        let mut label_divs: Vec<AnyElement> = Vec::new();
        let mut node_divs: Vec<AnyElement> = Vec::new();

        for (i, node) in self.nodes.iter().enumerate() {
            let (lx, ly) = match local_positions.get(i) {
                Some(&p) => p,
                None => continue,
            };
            let node_r_scaled = (node.radius * self.zoom).clamp(4.0, 40.0);

            // Label below circle.
            label_divs.push(
                div()
                    .absolute()
                    .left(px(lx - 40.0))
                    .top(px(ly + node_r_scaled + 4.0))
                    .w(px(80.0))
                    .flex()
                    .justify_center()
                    .text_xs()
                    .text_color(text_color)
                    .overflow_hidden()
                    .child(node.title.clone())
                    .into_any_element(),
            );

            // Hit area over the node circle.
            let hit = node_r_scaled + 4.0;
            node_divs.push(
                div()
                    .id(ElementId::Integer(i as u64))
                    .absolute()
                    .left(px(lx - hit))
                    .top(px(ly - hit))
                    .w(px(hit * 2.0))
                    .h(px(hit * 2.0))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::blue().opacity(0.25)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |gv, _ev: &MouseDownEvent, _window, cx| {
                            gv.dragging = Some(i);
                            gv.drag_moved = false;
                            if i < gv.vel.len() {
                                gv.vel[i] = (0.0, 0.0);
                            }
                            gv.panning = false; // node drag wins over pan
                            gv.simulating = true;
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
            );
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(mantle)
            // ── Scroll to zoom, centered on cursor ───────────────────────
            .on_scroll_wheel(cx.listener(|gv, ev: &ScrollWheelEvent, _window, cx| {
                let delta_y: f32 = match ev.delta {
                    ScrollDelta::Pixels(p) => f32::from(p.y),
                    ScrollDelta::Lines(l) => l.y * 20.0,
                };
                // Negative delta = scroll up = zoom in.
                let factor = (1.0_f32 - delta_y * 0.004).clamp(0.85, 1.15);
                let new_zoom = (gv.zoom * factor).clamp(0.05, 20.0);

                if let Some(bounds) = *gv.canvas_bounds.lock().unwrap() {
                    let ox = f32::from(bounds.origin.x);
                    let oy = f32::from(bounds.origin.y);
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    let uw = (w - PAD * 2.0).max(1.0);
                    let uh = (h - PAD * 2.0).max(1.0);
                    // Cursor in viewport space.
                    let vx_c = (f32::from(ev.position.x) - ox - PAD) / uw * 2.0 - 1.0;
                    let vy_c = (f32::from(ev.position.y) - oy - PAD) / uh * 2.0 - 1.0;
                    // Graph position under cursor (invariant across zoom).
                    let gx_c = (vx_c - gv.pan.0) / gv.zoom;
                    let gy_c = (vy_c - gv.pan.1) / gv.zoom;
                    // Adjust pan so the same graph point stays under cursor.
                    gv.pan.0 = vx_c - gx_c * new_zoom;
                    gv.pan.1 = vy_c - gy_c * new_zoom;
                }

                gv.zoom = new_zoom;
                cx.notify();
            }))
            // ── Mouse down: start node drag OR background pan ─────────────
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|gv, ev: &MouseDownEvent, _window, _cx| {
                    // Only start panning when not on a node (node sets dragging first).
                    if gv.dragging.is_none() {
                        gv.panning = true;
                        gv.pan_start_mouse =
                            Some((f32::from(ev.position.x), f32::from(ev.position.y)));
                        gv.pan_at_start = gv.pan;
                    }
                }),
            )
            // ── Mouse move: update drag or pan ────────────────────────────
            .on_mouse_move(cx.listener(|gv, ev: &MouseMoveEvent, _window, cx| {
                let mx = f32::from(ev.position.x);
                let my = f32::from(ev.position.y);

                if let Some(i) = gv.dragging {
                    // Drag node.
                    if let Some((gx, gy)) = gv.screen_to_graph(mx, my) {
                        gv.nodes[i].x = gx.clamp(-5.0, 5.0);
                        gv.nodes[i].y = gy.clamp(-5.0, 5.0);
                        gv.vel[i] = (0.0, 0.0);
                        gv.drag_moved = true;
                        cx.notify();
                    }
                } else if gv.panning {
                    // Pan the viewport.
                    if let (Some((start_mx, start_my)), Some(bounds)) =
                        (gv.pan_start_mouse, *gv.canvas_bounds.lock().unwrap())
                    {
                        let w = f32::from(bounds.size.width);
                        let h = f32::from(bounds.size.height);
                        let uw = (w - PAD * 2.0).max(1.0);
                        let uh = (h - PAD * 2.0).max(1.0);
                        let dvx = (mx - start_mx) / uw * 2.0;
                        let dvy = (my - start_my) / uh * 2.0;
                        gv.pan = (gv.pan_at_start.0 + dvx, gv.pan_at_start.1 + dvy);
                        cx.notify();
                    }
                }
            }))
            // ── Mouse up: end drag or pan; click = navigate ───────────────
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|gv, _ev: &MouseUpEvent, _window, cx| {
                    if let Some(i) = gv.dragging {
                        if !gv.drag_moved {
                            // Treat as click → navigate to note.
                            let rel = gv.nodes[i].rel_path.clone();
                            cx.emit(GraphViewEvent::OpenFile(rel));
                        }
                        gv.dragging = None;
                        gv.drag_moved = false;
                        gv.simulating = true;
                        cx.notify();
                    }
                    gv.panning = false;
                    gv.pan_start_mouse = None;
                }),
            )
            .child(bar)
            .child(
                div()
                    .flex_1()
                    .relative()
                    .overflow_hidden()
                    .child(graph_canvas)
                    .children(label_divs)
                    .children(node_divs),
            )
    }
}
