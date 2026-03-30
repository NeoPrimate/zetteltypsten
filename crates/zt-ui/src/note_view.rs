use crate::file_ops;
use crate::theme;
use crate::typst_canvas::{self, PT_TO_PX};
use gpui::*;
use gpui_component::input::{InputEvent, InputState, TabSize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use typst::layout::Frame;
use typst::model::Destination;
use zt_index::NoteInfo;
use zt_typst::vault_doc::VaultDocument;

actions!(note_view, [ToggleEditMode]);

pub enum NoteViewEvent {
    /// Navigate to a different note.
    OpenFile(String),
    /// Navigate to a specific position within a note (cross-note ref click).
    /// `y_pt` is the absolute Y in the vault document (Typst points).
    NavigateToLocation { note_path: String, y_pt: f32 },
    Recompiled,
    Renamed { old_rel: String, new_rel: String },
    /// A draft note was given a name and the file was created on disk.
    Created(String),
}

impl EventEmitter<NoteViewEvent> for NoteView {}

pub struct NoteView {
    pub input: Entity<InputState>,
    pub title_input: Entity<InputState>,
    /// Frames to render — the slice of the vault document for this note.
    pages: Vec<Frame>,
    /// Link destinations collected during paint, for click handling.
    link_store: Arc<Mutex<Vec<(Bounds<Pixels>, LinkTarget)>>>,
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
    /// Shared vault-wide compiled document.
    vault_doc: Arc<Mutex<VaultDocument>>,
    /// Handle for programmatic scrolling of the view-mode canvas.
    scroll_handle: ScrollHandle,
}

/// A link target collected during rendering.
#[derive(Clone)]
pub enum LinkTarget {
    /// Native Typst location (cross-note ref resolved by compiler).
    Location(typst::introspection::Location),
    /// External URL string.
    Url(String),
}

impl NoteView {
    pub fn new(
        vault_root: PathBuf,
        rel_path: String,
        source: String,
        vault_doc: Arc<Mutex<VaultDocument>>,
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
                .default_value(source)
        });

        let stem = Path::new(&rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&rel_path)
            .to_string();
        let title_input = cx.new(|cx| InputState::new(window, cx).default_value(stem));

        // Extract this note's frames and info from the vault document
        let (pages, note_info) = extract_note_slice(&vault_doc, &rel_path);

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
            vault_doc,
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// Create an unnamed draft note. The file is NOT written to disk until the
    /// user provides a name in the title field and presses Enter.
    pub fn new_draft(
        vault_root: PathBuf,
        draft_dir: String,
        vault_doc: Arc<Mutex<VaultDocument>>,
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
        });

        let title_input = cx.new(|cx| InputState::new(window, cx));
        title_input.update(cx, |s, cx| s.focus(window, cx));

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
            vault_doc,
            scroll_handle: ScrollHandle::new(),
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

        self.rel_path = rel_path.to_string();
        self.input.update(cx, |s, cx| s.set_value(&source, window, cx));

        let stem = Path::new(rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rel_path)
            .to_string();
        self.title_input.update(cx, |s, cx| s.set_value(&stem, window, cx));

        // Extract this note's slice from the vault document
        let (pages, note_info) = extract_note_slice(&self.vault_doc, rel_path);
        self.pages = pages;
        self.note_info = note_info;

        cx.notify();
    }

    fn commit_title_rename(&mut self, new_name: &str, cx: &mut Context<Self>) {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return;
        }

        if self.is_draft {
            let new_filename = format!("{}.typ", new_name);
            match file_ops::create_file(&self.vault_root, &self.draft_dir, &new_filename) {
                Ok(new_rel) => {
                    self.is_draft = false;
                    self.rel_path = new_rel.clone();
                    // Write editor content to the new file
                    let source = self.input.read(cx).value().to_string();
                    let path = self.vault_root.join(&new_rel);
                    let _ = std::fs::write(&path, &source);
                    // Add to vault document and recompile
                    {
                        let mut vd = self.vault_doc.lock().unwrap();
                        vd.add_note(&new_rel);
                        vd.recompile();
                    }
                    let (pages, info) = extract_note_slice(&self.vault_doc, &new_rel);
                    self.pages = pages;
                    self.note_info = info;
                    cx.emit(NoteViewEvent::Created(new_rel));
                    cx.notify();
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
                // Update vault document
                {
                    let mut vd = self.vault_doc.lock().unwrap();
                    vd.rename_note(&old_rel, &new_rel);
                    vd.recompile();
                }
                cx.emit(NoteViewEvent::Renamed { old_rel, new_rel });
            }
            Err(e) => tracing::error!("Rename failed: {e}"),
        }
    }

    /// Scroll to an absolute Y position (in Typst points) within the vault document.
    /// The position is converted to note-relative coordinates.
    pub fn scroll_to_y_abs(&mut self, y_abs_pt: f32) {
        let vd = self.vault_doc.lock().unwrap();
        let Some(boundary) = vd.note_boundary(&self.rel_path) else { return };
        let y_relative = y_abs_pt - boundary.y_start_pt;
        drop(vd);
        if y_relative >= 0.0 {
            let scroll_y = (y_relative * PT_TO_PX - 20.0).max(0.0);
            self.scroll_handle.set_offset(point(px(0.0), px(-scroll_y)));
        }
    }

    /// Explicitly save the current editor content to disk.
    pub fn save_file(&self, cx: &Context<Self>) {
        if self.is_draft || self.rel_path.is_empty() {
            return;
        }
        let source = self.input.read(cx).value().to_string();
        let path = self.vault_root.join(&self.rel_path);
        if let Err(e) = std::fs::write(&path, &source) {
            tracing::error!("Failed to save {}: {}", path.display(), e);
        }
    }

    fn schedule_recompile(&mut self, cx: &mut Context<Self>) {
        self.pending_compile = None;

        let source = self.input.read(cx).value().to_string();
        let rel_path = self.rel_path.clone();
        let vault_root = self.vault_root.clone();
        let vault_doc = self.vault_doc.clone();
        let is_draft = self.is_draft;

        let bg = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            bg.timer(std::time::Duration::from_millis(10)).await;

            let (new_pages, new_info) = bg
                .spawn(async move {
                    // Auto-save to disk
                    if !is_draft && !rel_path.is_empty() {
                        let full_path = vault_root.join(&rel_path);
                        if let Err(e) = std::fs::write(&full_path, &source) {
                            tracing::error!("Auto-save failed for {}: {}", full_path.display(), e);
                        }
                    }
                    if rel_path.is_empty() {
                        return (Vec::new(), NoteInfo::default());
                    }

                    // Update source in vault doc and recompile
                    let mut vd = vault_doc.lock().unwrap();
                    vd.update_note_source(&rel_path, source);
                    vd.recompile();

                    // Extract this note's frame and info
                    let pages: Vec<Frame> = vd.note_frame(&rel_path).into_iter().collect();
                    let info = vd.note_info(&rel_path);
                    drop(vd);

                    (pages, info)
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

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Extract a note's frame and NoteInfo from the vault document.
fn extract_note_slice(
    vault_doc: &Arc<Mutex<VaultDocument>>,
    rel_path: &str,
) -> (Vec<Frame>, NoteInfo) {
    let vd = vault_doc.lock().unwrap();
    let frame = vd.note_frame(rel_path);
    let info = vd.note_info(rel_path);
    let pages = frame.into_iter().collect();
    (pages, info)
}


// ─────────────────────────────────────────────────────────────────────────────
// Render
// ─────────────────────────────────────────────────────────────────────────────

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

        // Title bar
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
            let _vault_doc_for_links = self.vault_doc.clone();
            let _note_rel = self.rel_path.clone();

            let pad = 16.0_f32;
            let text_color = theme::text();
            let total_height: f32 = pages
                .iter()
                .map(|p| p.height().to_pt() as f32 * PT_TO_PX)
                .sum::<f32>()
                + pad;

            let view_canvas = canvas(
                move |bounds, _window, _cx| (bounds, pages.clone()),
                move |_bounds, (bounds, pages), window, _cx| {
                    let bounds_x = f32::from(bounds.origin.x);
                    let bounds_y = f32::from(bounds.origin.y);
                    let canvas_w = f32::from(bounds.size.width);
                    let available_w = canvas_w - pad * 2.0;
                    let mut y_offset = bounds_y;
                    let mut new_links: Vec<(Bounds<Pixels>, LinkTarget)> = Vec::new();

                    for page in &pages {
                        let page_w_pt = page.width().to_pt() as f32;
                        let page_h_pt = page.height().to_pt() as f32;
                        let scale = (available_w / (page_w_pt * PT_TO_PX)).min(1.0);
                        let page_h = page_h_pt * PT_TO_PX * scale;
                        let x_offset = bounds_x + (canvas_w - page_w_pt * PT_TO_PX * scale) / 2.0;
                        let origin = point(px(x_offset), px(y_offset));

                        let vp_top = bounds_y;
                        let vp_bottom = bounds_y + f32::from(bounds.size.height);
                        let mut raw_links = Vec::new();
                        typst_canvas::render_frame_styled(
                            window, page, origin, scale, vp_top, vp_bottom,
                            Some(text_color), &mut raw_links,
                        );

                        for lr in raw_links {
                            let target = match lr.destination {
                                Destination::Url(url) => LinkTarget::Url(url.to_string()),
                                Destination::Location(loc) => LinkTarget::Location(loc),
                                _ => continue,
                            };
                            new_links.push((lr.bounds, target));
                        }

                        y_offset += page_h;
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
                        .track_scroll(&self.scroll_handle)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |nv, ev: &MouseDownEvent, _window, cx| {
                                let pos = ev.position;
                                let links = nv.link_store.lock().unwrap();
                                for (bounds, target) in links.iter() {
                                    if bounds.contains(&pos) {
                                        match target {
                                            LinkTarget::Location(loc) => {
                                                let vd = nv.vault_doc.lock().unwrap();
                                                if let Some(doc) = vd.document() {
                                                    let p = doc.introspector.position(*loc);
                                                    let y_pt = p.point.y.to_pt() as f32;
                                                    if let Some(note_path) = vd.note_at_y(y_pt) {
                                                        let note_path = note_path.to_string();
                                                        drop(vd);
                                                        cx.emit(NoteViewEvent::NavigateToLocation {
                                                            note_path,
                                                            y_pt,
                                                        });
                                                    }
                                                }
                                            }
                                            LinkTarget::Url(_url) => {
                                                // External URL — could open in browser
                                            }
                                        }
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
