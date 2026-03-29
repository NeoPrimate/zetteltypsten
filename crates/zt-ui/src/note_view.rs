use crate::file_ops;
use crate::theme;
use crate::typst_canvas::{self, LinkRegion, PT_TO_PX};
use gpui::*;
use gpui_component::input::{InputEvent, InputState, TabSize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use typst::layout::Frame;
use typst::model::Destination;
use zt_index::NoteInfo;

actions!(note_view, [ToggleEditMode]);

pub enum NoteViewEvent {
    OpenFile(String),
    Recompiled,
    Renamed { old_rel: String, new_rel: String },
    /// A draft note was given a name and the file was created on disk.
    Created(String),
}

impl EventEmitter<NoteViewEvent> for NoteView {}

pub struct NoteView {
    pub input: Entity<InputState>,
    pub title_input: Entity<InputState>,
    pages: Vec<Frame>,
    /// Link bounds (in window coords, updated each paint) → URL string.
    link_store: Arc<Mutex<Vec<(Bounds<Pixels>, String)>>>,
    pub edit_mode: bool,
    vault_root: PathBuf,
    pub rel_path: String,
    /// Semantic information from the last successful compile.
    pub note_info: NoteInfo,
    /// True while the file has not yet been created on disk (unnamed new note).
    is_draft: bool,
    /// Vault-relative directory where the file will be created when named.
    draft_dir: String,
    subscribed: bool,
    title_subscribed: bool,
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

        let stem = Path::new(&rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&rel_path)
            .to_string();
        let title_input = cx.new(|cx| InputState::new(window, cx).default_value(stem));

        let mut world = zt_typst::world::ZettelWorld::new(vault_root.clone(), &rel_path);
        let preamble = build_preamble(&vault_root, &rel_path, &source);
        let full_source = format!("{preamble}{source}");
        world.set_source(&rel_path, full_source);
        let (pages, note_info) = compile_note(&world);
        let world = Arc::new(Mutex::new(world));

        Self {
            input,
            title_input,
            pages,
            link_store: Arc::new(Mutex::new(Vec::new())),
            edit_mode: false,
            vault_root,
            rel_path,
            note_info,
            is_draft: false,
            draft_dir: String::new(),
            subscribed: false,
            title_subscribed: false,
            pending_compile: None,
            world,
        }
    }

    /// Create an unnamed draft note. The file is NOT written to disk until the
    /// user provides a name in the title field and presses Enter.
    pub fn new_draft(vault_root: PathBuf, draft_dir: String, window: &mut Window, cx: &mut App) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("typst")
                .multi_line(true)
                .line_number(true)
                .searchable(true)
                .indent_guides(true)
                .tab_size(TabSize { tab_size: 2, hard_tabs: false })
                .soft_wrap(true)
        });

        let title_input = cx.new(|cx| InputState::new(window, cx));
        // Focus the title field immediately so the user can start typing the name.
        title_input.update(cx, |s, cx| s.focus(window, cx));

        // Use a placeholder rel-path for the ZettelWorld — it won't touch the disk
        // because we set the source manually to an empty string.
        let placeholder = "__new__.typ";
        let mut world = zt_typst::world::ZettelWorld::new(vault_root.clone(), placeholder);
        world.set_source(placeholder, String::new());
        let world = Arc::new(Mutex::new(world));

        Self {
            input,
            title_input,
            pages: Vec::new(),
            link_store: Arc::new(Mutex::new(Vec::new())),
            edit_mode: false,
            vault_root,
            rel_path: String::new(),
            note_info: NoteInfo::default(),
            is_draft: true,
            draft_dir,
            subscribed: false,
            title_subscribed: false,
            pending_compile: None,
            world,
        }
    }

    pub fn open_file(&mut self, rel_path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let full_path = self.vault_root.join(rel_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to read {}: {}", full_path.display(), e);
                return;
            }
        };

        let preamble = build_preamble(&self.vault_root, rel_path, &source);
        let full_source = format!("{preamble}{source}");

        self.rel_path = rel_path.to_string();
        self.input.update(cx, |s, cx| s.set_value(&source, window, cx));

        let stem = Path::new(rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rel_path)
            .to_string();
        self.title_input.update(cx, |s, cx| s.set_value(&stem, window, cx));

        let mut world = self.world.lock().unwrap();
        world.set_main(rel_path);
        world.set_source(rel_path, full_source);
        let (pages, note_info) = compile_note(&world);
        self.pages = pages;
        self.note_info = note_info;
        drop(world);

        cx.notify();
    }

    fn commit_title_rename(&mut self, new_name: &str, cx: &mut Context<Self>) {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return;
        }

        if self.is_draft {
            // Create the file on disk for the first time.
            let new_filename = format!("{}.typ", new_name);
            match file_ops::create_file(&self.vault_root, &self.draft_dir, &new_filename) {
                Ok(new_rel) => {
                    self.is_draft = false;
                    self.rel_path = new_rel.clone();
                    // Point the world at the real path so auto-save works correctly.
                    let source = self.input.read(cx).value().to_string();
                    {
                        let mut world = self.world.lock().unwrap();
                        world.set_main(&new_rel);
                        let preamble = build_preamble(&self.vault_root, &new_rel, &source);
                        world.set_source(&new_rel, format!("{preamble}{source}"));
                    }
                    cx.emit(NoteViewEvent::Created(new_rel));
                }
                Err(e) => tracing::error!("Create note failed: {e}"),
            }
            return;
        }

        let current_stem = Path::new(&self.rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if new_name == current_stem {
            return;
        }
        let new_filename = format!("{}.typ", new_name);
        let old_rel = self.rel_path.clone();
        match file_ops::rename_file(&self.vault_root, &old_rel, &new_filename) {
            Ok(new_rel) => {
                self.rel_path = new_rel.clone();
                cx.emit(NoteViewEvent::Renamed { old_rel, new_rel });
            }
            Err(e) => tracing::error!("Rename failed: {e}"),
        }
    }

    fn schedule_recompile(&mut self, cx: &mut Context<Self>) {
        self.pending_compile = None;

        let source = self.input.read(cx).value().to_string();
        let rel_path = self.rel_path.clone();
        let vault_root = self.vault_root.clone();
        let world = self.world.clone();
        let is_draft = self.is_draft;

        let bg = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            bg.timer(std::time::Duration::from_millis(10)).await;

            let (new_pages, new_info) = bg
                .spawn(async move {
                    // Skip auto-save for draft notes (no file on disk yet).
                    if !is_draft && !rel_path.is_empty() {
                        let full_path = vault_root.join(&rel_path);
                        if let Err(e) = std::fs::write(&full_path, &source) {
                            tracing::error!("Auto-save failed for {}: {}", full_path.display(), e);
                        }
                    }
                    if rel_path.is_empty() {
                        return (Vec::new(), NoteInfo::default());
                    }
                    let preamble = build_preamble(&vault_root, &rel_path, &source);
                    let full_source = format!("{preamble}{source}");
                    let mut w = world.lock().unwrap();
                    w.set_source(&rel_path, full_source);
                    compile_note(&w)
                })
                .await;

            cx.update(|cx| {
                this.update(cx, |nv, cx| {
                    nv.pages = new_pages;
                    nv.note_info = new_info;
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

/// Compile and return page frames plus extracted semantic info.
fn compile_note(world: &zt_typst::world::ZettelWorld) -> (Vec<Frame>, NoteInfo) {
    let result = typst::compile::<typst::layout::PagedDocument>(world);
    match result.output {
        Ok(doc) => {
            let info = zt_index::compiled::extract_from_compiled(&doc);
            let frames = doc.pages.iter().map(|p| p.frame.clone()).collect();
            (frames, info)
        }
        Err(_) => (Vec::new(), NoteInfo::default()),
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

        if !self.title_subscribed {
            self.title_subscribed = true;
            let title_input = self.title_input.clone();
            cx.subscribe(&title_input, |this: &mut NoteView, state, event: &InputEvent, cx| {
                match event {
                    InputEvent::PressEnter { .. } => {
                        let val = state.read(cx).value().to_string();
                        this.commit_title_rename(&val, cx);
                    }
                    InputEvent::Blur => {
                        let val = state.read(cx).value().to_string();
                        this.commit_title_rename(&val, cx);
                    }
                    _ => {}
                }
            })
            .detach();
        }

        // Title bar — always visible above both edit and view modes.
        let title_bar = div()
            .w_full()
            .px(px(24.0))
            .pt(px(20.0))
            .pb(px(12.0))
            .border_b_1()
            .border_color(theme::surface0())
            .text_size(px(26.0))
            .font_weight(FontWeight::BOLD)
            .text_color(theme::text())
            .child(
                gpui_component::input::Input::new(&self.title_input)
                    .appearance(false)
                    .bordered(false)
                    .size_full(),
            );

        if self.edit_mode {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(title_bar)
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
                    let bounds_x = f32::from(bounds.origin.x);
                    let bounds_y = f32::from(bounds.origin.y);
                    let canvas_w = f32::from(bounds.size.width);
                    let available_w = canvas_w - margin * 2.0;
                    let mut y_offset = bounds_y + margin;
                    let mut new_links: Vec<(Bounds<Pixels>, String)> = Vec::new();

                    for page in &pages {
                        let page_w_pt = page.width().to_pt() as f32;
                        let page_h_pt = page.height().to_pt() as f32;
                        let scale = (available_w / (page_w_pt * PT_TO_PX)).min(1.0);
                        let page_w = page_w_pt * PT_TO_PX * scale;
                        let page_h = page_h_pt * PT_TO_PX * scale;
                        let x_offset = bounds_x + (canvas_w - page_w) / 2.0;

                        let origin = point(px(x_offset), px(y_offset));
                        let vp_top = bounds_y;
                        let vp_bottom = bounds_y + f32::from(bounds.size.height);
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
                .child(title_bar)
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
