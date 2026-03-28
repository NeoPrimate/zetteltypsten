use crate::theme;
use crate::typst_canvas::{self, LinkRegion, PT_TO_PX};
use gpui::*;
use gpui_component::input::{InputEvent, InputState, TabSize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use typst::layout::Frame;
use typst::model::Destination;

actions!(note_view, [ToggleEditMode]);

pub enum NoteViewEvent {
    OpenFile(String),
    Recompiled,
}

impl EventEmitter<NoteViewEvent> for NoteView {}

pub struct NoteView {
    pub input: Entity<InputState>,
    pages: Vec<Frame>,
    /// Link bounds (in window coords, updated each paint) → URL string.
    link_store: Arc<Mutex<Vec<(Bounds<Pixels>, String)>>>,
    pub edit_mode: bool,
    vault_root: PathBuf,
    pub rel_path: String,
    subscribed: bool,
    pending_compile: Option<Task<()>>,
    world: Arc<Mutex<zt_typst::world::ZettelWorld>>,
}

impl NoteView {
    pub fn new(
        vault_root: PathBuf,
        rel_path: String,
        source: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("typst")
                .multi_line(true)
                .line_number(true)
                .searchable(true)
                .indent_guides(true)
                .tab_size(TabSize { tab_size: 2, hard_tabs: false })
                .soft_wrap(true)
                .default_value(source.clone())
        });

        let mut world = zt_typst::world::ZettelWorld::new(vault_root.clone(), &rel_path);
        let preamble = build_preamble(&vault_root, &rel_path, &source);
        let full_source = format!("{preamble}{source}");
        world.set_source(&rel_path, full_source);
        let pages = compile_frames(&world);
        let world = Arc::new(Mutex::new(world));

        Self {
            input,
            pages,
            link_store: Arc::new(Mutex::new(Vec::new())),
            edit_mode: false,
            vault_root,
            rel_path,
            subscribed: false,
            pending_compile: None,
            world,
        }
    }

    pub fn open_file(&mut self, rel_path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let full_path = self.vault_root.join(rel_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to read {}: {}", full_path.display(), e);
                return;
            }
        };

        let preamble = build_preamble(&self.vault_root, rel_path, &source);
        let full_source = format!("{preamble}{source}");

        self.rel_path = rel_path.to_string();
        self.input.update(cx, |s, cx| s.set_value(&source, window, cx));

        let mut world = self.world.lock().unwrap();
        world.set_main(rel_path);
        world.set_source(rel_path, full_source);
        self.pages = compile_frames(&world);
        drop(world);

        cx.notify();
    }

    fn schedule_recompile(&mut self, cx: &mut Context<Self>) {
        self.pending_compile = None;

        let source = self.input.read(cx).value().to_string();
        let rel_path = self.rel_path.clone();
        let vault_root = self.vault_root.clone();
        let world = self.world.clone();

        let bg = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            bg.timer(std::time::Duration::from_millis(50)).await;

            let new_pages = bg
                .spawn(async move {
                    // Auto-save on change.
                    let full_path = vault_root.join(&rel_path);
                    if let Err(e) = std::fs::write(&full_path, &source) {
                        log::error!("Auto-save failed for {}: {}", full_path.display(), e);
                    }
                    let preamble = build_preamble(&vault_root, &rel_path, &source);
                    let full_source = format!("{preamble}{source}");
                    let mut w = world.lock().unwrap();
                    w.set_source(&rel_path, full_source);
                    compile_frames(&w)
                })
                .await;

            cx.update(|cx| {
                this.update(cx, |nv, cx| {
                    nv.pages = new_pages;
                    cx.notify();
                    cx.emit(NoteViewEvent::Recompiled);
                })
                .ok();
            })
            .ok();
        });

        self.pending_compile = Some(task);
    }
}

/// Scan vault for all `<label>` declarations; returns {label → rel_path}.
fn scan_vault_labels(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for entry in walkdir::WalkDir::new(root).max_depth(10).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().is_some_and(|e| e == "typ") {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .into_owned();
            if let Ok(src) = std::fs::read_to_string(entry.path()) {
                for label in zt_typst::compiler::extract_local_labels(&src) {
                    map.entry(label).or_insert(rel.clone());
                }
            }
        }
    }
    map
}

/// Build the cross-note ref preamble for a given source file.
fn build_preamble(vault_root: &Path, rel_path: &str, source: &str) -> String {
    let label_map = scan_vault_labels(vault_root);
    let local_labels = zt_typst::compiler::extract_local_labels(source);
    let empty: HashMap<String, String> = HashMap::new();
    let ref_preamble =
        zt_typst::compiler::build_ref_preamble(&label_map, &local_labels, &empty, &empty);
    // Catppuccin Macchiato: text = #cad3f5, background = #24273a
    format!(
        "#set text(fill: rgb(\"#cad3f5\"))\n#set page(fill: rgb(\"#24273a\"))\n{ref_preamble}"
    )
}

/// Compile and return page frames.
fn compile_frames(world: &zt_typst::world::ZettelWorld) -> Vec<Frame> {
    let result = typst::compile::<typst::layout::PagedDocument>(world);
    match result.output {
        Ok(doc) => doc.pages.iter().map(|p| p.frame.clone()).collect(),
        Err(_) => Vec::new(),
    }
}

impl Render for NoteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mantle = theme::mantle();

        if !self.subscribed {
            self.subscribed = true;
            let input = self.input.clone();
            cx.subscribe(&input, |this: &mut NoteView, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.schedule_recompile(cx);
                }
            })
            .detach();
        }

        if self.edit_mode {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .child(
                            gpui_component::input::Input::new(&self.input)
                                .appearance(false)
                                .bordered(false)
                                .size_full(),
                        ),
                )
        } else {
            let pages = self.pages.clone();
            let link_store = self.link_store.clone();

            let margin = 16.0_f32;
            let total_height: f32 = pages
                .iter()
                .map(|p| p.height().to_pt() as f32 * PT_TO_PX + margin)
                .sum::<f32>()
                + margin;

            let view_canvas = canvas(
                move |bounds, _window, _cx| (bounds, pages.clone()),
                move |_bounds, (bounds, pages), window, _cx| {
                    let bx = f32::from(bounds.origin.x);
                    let by = f32::from(bounds.origin.y);
                    let canvas_w = f32::from(bounds.size.width);
                    let available_w = canvas_w - margin * 2.0;
                    let mut y_offset = by + margin;
                    let mut new_links: Vec<(Bounds<Pixels>, String)> = Vec::new();

                    for page in &pages {
                        let page_w_pt = page.width().to_pt() as f32;
                        let page_h_pt = page.height().to_pt() as f32;
                        let scale = (available_w / (page_w_pt * PT_TO_PX)).min(1.0);
                        let page_w = page_w_pt * PT_TO_PX * scale;
                        let page_h = page_h_pt * PT_TO_PX * scale;
                        let x_offset = bx + (canvas_w - page_w) / 2.0;

                        let origin = point(px(x_offset), px(y_offset));
                        let vp_top = by;
                        let vp_bottom = by + f32::from(bounds.size.height);
                        let mut raw_links: Vec<LinkRegion> = Vec::new();
                        typst_canvas::render_frame_with_viewport(
                            window, page, origin, scale, vp_top, vp_bottom, &mut raw_links,
                        );

                        for lr in raw_links {
                            if let Destination::Url(url) = lr.destination {
                                new_links.push((lr.bounds, url.to_string()));
                            }
                        }

                        y_offset += page_h + margin;
                    }

                    *link_store.lock().unwrap() = new_links;
                },
            )
            .w_full()
            .h(px(total_height));

            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(mantle)
                .child(
                    div()
                        .id("note-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|nv, ev: &MouseDownEvent, _window, cx| {
                                let pos = ev.position;
                                for (bounds, url) in nv.link_store.lock().unwrap().iter() {
                                    if bounds.contains(&pos) && url.starts_with("zt-open:") {
                                        let target =
                                            url.trim_start_matches("zt-open:").to_string();
                                        cx.emit(NoteViewEvent::OpenFile(target));
                                        break;
                                    }
                                }
                            }),
                        )
                        .child(view_canvas),
                )
        }
    }
}
