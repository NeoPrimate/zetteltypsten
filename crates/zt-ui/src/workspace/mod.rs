//! Root workspace entity — three-column layout with activity bar, left sidebar,
//! content area, and optional right inspector panel.

mod graph_sidebar;
mod keybindings;
mod right_panel;
mod tab_bar;

pub use keybindings::init;

use crate::book_view::BookView;
use crate::components::empty_state;
use crate::editor::Editor;
use crate::file_tree::{FileTree, FileTreeEvent};
use crate::graph_view::{GraphView, GraphViewEvent};
use crate::note_view::{NoteView, NoteViewEvent};
use crate::theme;
use gpui::*;
use gpui_component::{Icon, IconName};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zt_core::note::NoteId;
use zt_core::RelPath;
use zt_index::indexer::VaultIndex;

actions!(workspace, [ToggleLeftSidebar, ToggleRightSidebar]);

// ── Resize drag marker types ──────────────────────────────────────────────────

/// Drag marker for the left sidebar resize handle.
#[derive(Clone)]
pub(self) struct LeftResize;

/// Drag marker for the right sidebar resize handle.
#[derive(Clone)]
pub(self) struct RightResize;

/// Invisible drag-ghost view used by both resize handles.
struct ResizeGhost;
impl Render for ResizeGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

// ── Domain types ──────────────────────────────────────────────────────────────

/// Which main panel is shown in the content area.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Notes = 0,
    Graph = 1,
    Book = 2,
    Pdf = 3,
}

/// Which sub-panel is active inside the right inspector sidebar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(self) enum RightPanel {
    Outline,
    Tags,
    Inlinks,
    Outlinks,
}

/// State for a single open note tab.
pub struct NoteTab {
    pub rel_path: RelPath,
    pub title: String,
    pub note_view: Entity<NoteView>,
}


// ── Workspace entity ──────────────────────────────────────────────────────────

pub struct Workspace {
    pub(self) vault_root: Option<PathBuf>,
    pub(self) active_tab: ActiveTab,
    pub(self) left_visible: bool,
    pub(self) right_visible: bool,
    pub(self) right_panel: RightPanel,
    // Sidebar widths (user-draggable)
    pub(self) left_sidebar_width: f32,
    pub(self) right_sidebar_width: f32,
    /// Captured at drag start: (start_mouse_x, start_sidebar_width)
    pub(self) left_resize_origin: Option<(f32, f32)>,
    pub(self) right_resize_origin: Option<(f32, f32)>,
    // Multi-tab note area
    pub(self) note_tabs: Vec<NoteTab>,
    pub(self) active_note_idx: usize,
    /// Set by the close-button's on_mouse_down; checked by the outer tab's on_click.
    pub(self) pending_tab_close: Option<usize>,
    // PDF composer
    pub(self) pdf_editor: Option<Entity<Editor>>,
    pub(self) pdf_doc_list: Vec<String>,
    pub(self) active_pdf_doc: String,
    // Other views
    pub(self) graph_view: Option<Entity<GraphView>>,
    pub(self) book_view: Option<Entity<BookView>>,
    pub(self) file_tree: Option<Entity<FileTree>>,
    pub(self) vault_index: Arc<Mutex<VaultIndex>>,
    /// Pending file to open (set by sidebar click / graph nav; opened on next render).
    pub(self) pending_open: Arc<std::sync::Mutex<Option<String>>>,
    /// Pending cross-note navigation from NoteView link click.
    pub(self) pending_note_nav: Option<String>,
    /// Absolute Y-offset (in Typst points) to scroll to after opening the pending note nav.
    pub(self) pending_scroll_y: Option<f32>,
    /// True after the first startup auto-open; prevents re-opening when all tabs are closed.
    pub(self) startup_done: bool,
    /// Track which multi-file chapters are expanded in the book right panel.
    pub(self) expanded_book_chapters: std::collections::HashSet<String>,
    /// Item being renamed inline in the book panel: (key, input_state).
    /// Key is either a ChapterLoc key or "part-N" for parts.
    pub(self) book_renaming: Option<(String, Entity<gpui_component::input::InputState>)>,
    /// Shared vault-wide compiled document.
    pub(self) vault_doc: Option<Arc<Mutex<zt_typst::vault_doc::VaultDocument>>>,
}

// ── Constructor ───────────────────────────────────────────────────────────────

impl Workspace {
    pub fn new(vault_root: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let graph_view = vault_root.as_ref().map(|root| {
            let graph_view_entity = cx.new(|cx| GraphView::new(root.clone(), cx));
            cx.subscribe(
                &graph_view_entity,
                |workspace: &mut Workspace, _, ev: &GraphViewEvent, cx| {
                    let GraphViewEvent::OpenFile(rel) = ev;
                    *workspace.pending_open.lock().unwrap() = Some(rel.clone());
                    workspace.active_tab = ActiveTab::Notes;
                    cx.notify();
                },
            )
            .detach();
            graph_view_entity
        });

        // Build vault index in the background so startup is non-blocking.
        let vault_index = Arc::new(Mutex::new(VaultIndex::new()));
        if let Some(ref root) = vault_root {
            let vault_idx_arc = vault_index.clone();
            let root = root.clone();
            let bg = cx.background_executor().clone();
            cx.spawn(async move |this, cx| {
                let idx = bg.spawn(async move { VaultIndex::build(&root).ok() }).await;
                if let Some(idx) = idx {
                    *vault_idx_arc.lock().unwrap() = idx;
                }
                cx.update(|cx| {
                    this.update(cx, |_, cx| cx.notify()).ok();
                })
                .ok();
            })
            .detach();
        }

        // Build vault document (compiles all notes as one)
        let vault_doc = vault_root.as_ref().map(|root| {
            Arc::new(Mutex::new(zt_typst::vault_doc::VaultDocument::new(root.clone())))
        });

        Self {
            vault_root,
            active_tab: ActiveTab::Notes,
            left_visible: true,
            right_visible: false,
            right_panel: RightPanel::Outline,
            left_sidebar_width: 220.0,
            right_sidebar_width: 240.0,
            left_resize_origin: None,
            right_resize_origin: None,
            note_tabs: Vec::new(),
            active_note_idx: 0,
            pending_tab_close: None,
            pdf_editor: None,
            pdf_doc_list: Vec::new(),
            active_pdf_doc: String::new(),
            graph_view,
            book_view: None,
            file_tree: None,
            vault_index,
            pending_open: Arc::new(std::sync::Mutex::new(None)),
            pending_note_nav: None,
            pending_scroll_y: None,
            startup_done: false,
            expanded_book_chapters: std::collections::HashSet::new(),
            book_renaming: None,
            vault_doc,
        }
    }

    /// Open a vault-relative path in a new tab, or switch to it if already open.
    pub(self) fn open_file(
        &mut self,
        rel_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ref root) = self.vault_root else {
            return;
        };

        // Already open → just switch to that tab.
        if let Some(idx) = self.note_tabs.iter().position(|t| t.rel_path.as_str() == rel_path) {
            self.active_note_idx = idx;
            if let Some(ref file_tree) = self.file_tree {
                let rel = rel_path.to_string();
                file_tree.update(cx, |file_tree, _| file_tree.set_active(Some(rel)));
            }
            cx.notify();
            return;
        }

        let full_path = root.join(rel_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to read {}: {}", full_path.display(), e);
                return;
            }
        };

        let root_clone = root.clone();
        let rel = rel_path.to_string();

        let vd = self.vault_doc.clone().unwrap();
        let note_view = cx.new(|cx| {
            NoteView::new(root_clone.clone(), rel.clone(), source.clone(), vd, window, cx)
        });
        self.attach_note_view_handler(&note_view, cx);

        let title = Path::new(rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rel_path)
            .to_string();

        self.note_tabs.push(NoteTab { rel_path: RelPath::new(rel.clone()), title, note_view });
        self.active_note_idx = self.note_tabs.len() - 1;

        if let Some(ref file_tree) = self.file_tree {
            let r = rel.clone();
            file_tree.update(cx, |file_tree, _| file_tree.set_active(Some(r)));
        }

        cx.notify();
    }

    /// Open a new unnamed draft note. The file is not created on disk until the
    /// user types a name and presses Enter in the title field.
    pub(self) fn open_new_note(&mut self, dir: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref vault_root) = self.vault_root else { return };
        let vault_root = vault_root.clone();

        let vd = self.vault_doc.clone().unwrap();
        let note_view = cx.new(|cx| {
            NoteView::new_draft(vault_root.clone(), dir.to_string(), vd, window, cx)
        });
        self.attach_note_view_handler(&note_view, cx);

        self.note_tabs.push(NoteTab {
            rel_path: RelPath::new(String::new()),
            title: "New Note".to_string(),
            note_view,
        });
        self.active_note_idx = self.note_tabs.len() - 1;
        self.active_tab = ActiveTab::Notes;
        cx.notify();
    }

    /// Subscribe to all NoteViewEvents for `note_view`, updating workspace state.
    fn attach_note_view_handler(&mut self, note_view: &Entity<NoteView>, cx: &mut Context<Self>) {
        cx.subscribe(
            note_view,
            |ws: &mut Workspace, nve, ev: &NoteViewEvent, cx| match ev {
                NoteViewEvent::OpenFile(target) => {
                    ws.pending_note_nav = Some(target.clone());
                    ws.pending_scroll_y = None;
                    cx.notify();
                }
                NoteViewEvent::NavigateToLocation { note_path, y_pt } => {
                    ws.pending_note_nav = Some(note_path.clone());
                    ws.pending_scroll_y = Some(*y_pt);
                    cx.notify();
                }
                NoteViewEvent::Recompiled => {
                    let nv_ref = nve.read(cx);
                    let rel = nv_ref.rel_path.clone();
                    // Skip indexing for draft notes with no path yet.
                    if !rel.is_empty() {
                        let note_id = NoteId::from_path(Path::new(&rel));
                        let info = nv_ref.note_info.clone();
                        ws.vault_index.lock().unwrap().index_note_compiled(&note_id, &info);
                        if let Some(ref graph_view) = ws.graph_view {
                            graph_view.update(cx, |gv, cx| gv.rebuild(cx));
                        }
                    }
                    cx.notify();
                }
                NoteViewEvent::Renamed { old_rel: _, new_rel } => {
                    // Find the tab by entity ID so rename-from-title always matches.
                    let nv_id = nve.entity_id();
                    for tab in &mut ws.note_tabs {
                        if tab.note_view.entity_id() == nv_id {
                            tab.rel_path = RelPath::new(new_rel.clone());
                            tab.title = Path::new(new_rel.as_str())
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or(new_rel.as_str())
                                .to_string();
                            break;
                        }
                    }
                    if let Some(ref ft) = ws.file_tree {
                        let new = new_rel.clone();
                        ft.update(cx, |ft, _| ft.set_active(Some(new)));
                    }
                    cx.notify();
                }
                NoteViewEvent::Created(new_rel) => {
                    let nv_id = nve.entity_id();
                    for tab in &mut ws.note_tabs {
                        if tab.note_view.entity_id() == nv_id {
                            tab.rel_path = RelPath::new(new_rel.clone());
                            tab.title = Path::new(new_rel.as_str())
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or(new_rel.as_str())
                                .to_string();
                            break;
                        }
                    }
                    if let Some(ref ft) = ws.file_tree {
                        let new = new_rel.clone();
                        ft.update(cx, |ft, _| ft.set_active(Some(new)));
                    }
                    cx.notify();
                }
            },
        )
        .detach();
    }

    /// Close the tab at `idx`, clamping `active_note_idx` if needed.
    pub(self) fn close_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.note_tabs.len() {
            self.note_tabs.remove(idx);
            self.active_note_idx =
                self.active_note_idx.min(self.note_tabs.len().saturating_sub(1));
            if let Some(ref file_tree) = self.file_tree {
                let new_rel = self
                    .note_tabs
                    .get(self.active_note_idx)
                    .map(|t| t.rel_path.to_string());
                file_tree.update(cx, |file_tree, _| file_tree.set_active(new_rel));
            }
            cx.notify();
        }
    }

    /// Open a vault-relative path in the PDF tab list, or switch to it if already open.
    /// Load a PDF document from `.zetteltypsten/documents/` into the editor.
    ///
    /// The editor uses a virtual rel_path `__pdf_doc__.typ` at the vault root
    /// so that `#include "note.typ"` resolves relative to the vault, not to
    /// the `.zetteltypsten/documents/` directory.
    pub(self) fn load_pdf_doc(&mut self, filename: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref root) = self.vault_root else { return };
        let doc_dir = root.join(".zetteltypsten/documents");
        let _ = std::fs::create_dir_all(&doc_dir);
        let doc_path = doc_dir.join(filename);
        let source = std::fs::read_to_string(&doc_path).unwrap_or_default();
        // Use a virtual path at vault root so #include resolves relative to vault
        let virtual_rel = "__pdf_doc__.typ".to_string();
        let root_clone = root.clone();
        let actual_path = format!(".zetteltypsten/documents/{}", filename);

        self.pdf_editor = Some(cx.new(|cx| {
            let mut editor = Editor::new(root_clone, virtual_rel, source, window, cx);
            // Store the real disk path for save operations
            editor.set_save_path(actual_path);
            editor
        }));
        self.active_pdf_doc = filename.to_string();
        cx.notify();
    }

    /// Scan `.zetteltypsten/documents/` for PDF document files.
    pub(self) fn scan_pdf_docs(&mut self) {
        let Some(ref root) = self.vault_root else { return };
        let doc_dir = root.join(".zetteltypsten/documents");
        let _ = std::fs::create_dir_all(&doc_dir);
        let mut docs: Vec<String> = std::fs::read_dir(&doc_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "typ"))
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        docs.sort();
        if docs.is_empty() {
            // Create a default document
            let default = "document.typ";
            let _ = std::fs::write(doc_dir.join(default), "");
            docs.push(default.to_string());
        }
        self.pdf_doc_list = docs;
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ── Lazy inits ────────────────────────────────────────────────────────

        // Auto-open the first .typ file exactly once at startup.
        if !self.startup_done && self.note_tabs.is_empty() {
            self.startup_done = true;
            if let Some(ref root) = self.vault_root.clone() {
                if let Some(entry) = walkdir::WalkDir::new(root)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .find(|e| e.path().extension().is_some_and(|ext| ext == "typ"))
                {
                    let rel = entry
                        .path()
                        .strip_prefix(root)
                        .unwrap_or(entry.path())
                        .to_string_lossy()
                        .into_owned();
                    self.open_file(&rel, window, cx);
                }
            }
        }

        // Lazy-create the FileTree entity (requires vault root to be known).
        if self.file_tree.is_none() {
            if let Some(ref root) = self.vault_root {
                let root = root.clone();
                let file_tree = cx.new(|cx| FileTree::new(root, cx));
                cx.subscribe(
                    &file_tree,
                    |workspace: &mut Workspace, _, ev: &FileTreeEvent, cx| {
                        match ev {
                            FileTreeEvent::OpenFile(rel) => {
                                *workspace.pending_open.lock().unwrap() =
                                    Some(rel.to_string());
                                cx.notify();
                            }
                            FileTreeEvent::FileRenamed { old_rel, new_rel } => {
                                let updates: Vec<(Entity<NoteView>, String)> = workspace
                                    .note_tabs
                                    .iter_mut()
                                    .filter_map(|tab| {
                                        if tab.rel_path == *old_rel {
                                            tab.rel_path = new_rel.clone();
                                            tab.title = new_rel.file_stem().to_string();
                                            Some((tab.note_view.clone(), new_rel.to_string()))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                for (note_view, new_path) in updates {
                                    note_view.update(cx, |note_view, _| {
                                        note_view.rel_path = new_path;
                                    });
                                }
                                cx.notify();
                            }
                            FileTreeEvent::FileDeleted(rel) => {
                                workspace.note_tabs.retain(|t| t.rel_path != *rel);
                                workspace.active_note_idx = workspace
                                    .active_note_idx
                                    .min(workspace.note_tabs.len().saturating_sub(1));
                                cx.notify();
                            }
                            FileTreeEvent::FileMoved { old_rel, new_rel } => {
                                let updates: Vec<(Entity<NoteView>, String)> = workspace
                                    .note_tabs
                                    .iter_mut()
                                    .filter_map(|tab| {
                                        if tab.rel_path == *old_rel {
                                            tab.rel_path = new_rel.clone();
                                            tab.title = new_rel.file_stem().to_string();
                                            Some((tab.note_view.clone(), new_rel.to_string()))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                for (note_view, new_path) in updates {
                                    note_view.update(cx, |note_view, _| {
                                        note_view.rel_path = new_path;
                                    });
                                }
                                cx.notify();
                            }
                            FileTreeEvent::FileCreated(rel) => {
                                *workspace.pending_open.lock().unwrap() =
                                    Some(rel.to_string());
                                cx.notify();
                            }
                            FileTreeEvent::FolderDeleted(folder_rel) => {
                                let prefix = format!("{}/", folder_rel);
                                workspace.note_tabs.retain(|t| !t.rel_path.starts_with(&prefix));
                                workspace.active_note_idx = workspace
                                    .active_note_idx
                                    .min(workspace.note_tabs.len().saturating_sub(1));
                                if let Some(ref file_tree) = workspace.file_tree {
                                    let new_rel = workspace
                                        .note_tabs
                                        .get(workspace.active_note_idx)
                                        .map(|t| t.rel_path.to_string());
                                    file_tree.update(cx, |file_tree, _| {
                                        file_tree.set_active(new_rel);
                                    });
                                }
                                cx.notify();
                            }
                            FileTreeEvent::FolderRenamed { old_rel, new_rel } => {
                                let old_prefix = format!("{}/", old_rel);
                                let new_prefix = format!("{}/", new_rel);
                                let updates: Vec<(Entity<NoteView>, String)> = workspace
                                    .note_tabs
                                    .iter_mut()
                                    .filter_map(|tab| {
                                        if tab.rel_path.starts_with(&old_prefix) {
                                            let suffix =
                                                tab.rel_path[old_prefix.len()..].to_string();
                                            let updated = format!("{}{}", new_prefix, suffix);
                                            tab.rel_path = RelPath::new(updated.clone());
                                            tab.title = tab.rel_path.file_stem().to_string();
                                            Some((tab.note_view.clone(), updated))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                for (note_view, new_path) in updates {
                                    note_view.update(cx, |note_view, _| {
                                        note_view.rel_path = new_path;
                                    });
                                }
                                cx.notify();
                            }
                        }
                    },
                )
                .detach();

                // Sync the tree's active-highlight to whichever tab is already open.
                if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                    let rel = tab.rel_path.to_string();
                    file_tree.update(cx, |file_tree, _| file_tree.set_active(Some(rel)));
                }
                self.file_tree = Some(file_tree);
            }
        }

        // Lazy-create BookView (only needed when the Book tab is active).
        if self.active_tab == ActiveTab::Book && self.book_view.is_none() {
            if let Some(ref root) = self.vault_root {
                let root = root.clone();
                let vd = self.vault_doc.clone().unwrap();
                let bv = cx.new(|cx| BookView::new(root, vd, cx));
                cx.subscribe(&bv, |workspace, _bv, ev: &crate::book_view::BookViewEvent, cx| {
                    match ev {
                        crate::book_view::BookViewEvent::OpenFile(rel) => {
                            *workspace.pending_open.lock().unwrap() = Some(rel.clone());
                            workspace.active_tab = ActiveTab::Notes;
                            cx.notify();
                        }
                    }
                }).detach();
                self.book_view = Some(bv);
            }
        }

        // Lazy-create PDF editor (only when PDF tab first activated).
        if self.active_tab == ActiveTab::Pdf && self.pdf_editor.is_none() {
            self.scan_pdf_docs();
            let first = self.pdf_doc_list.first().cloned().unwrap_or_else(|| "document.typ".into());
            self.load_pdf_doc(&first, window, cx);
        }

        // ── Consume pending opens ─────────────────────────────────────────────
        let pending = self.pending_open.lock().unwrap().take();
        if let Some(rel_path) = pending {
            // File clicks always open in Notes tab (PDF tab has its own documents)
            if self.active_tab == ActiveTab::Pdf {
                self.active_tab = ActiveTab::Notes;
            }
            self.open_file(&rel_path, window, cx);
        }
        let pending_nav = self.pending_note_nav.take();
        let pending_scroll_y = self.pending_scroll_y.take();
        if let Some(rel_path) = pending_nav {
            self.open_file(&rel_path, window, cx);
            // Scroll to the target position within the note
            if let Some(y_pt) = pending_scroll_y {
                if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                    tab.note_view.update(cx, |nv, _cx| {
                        nv.scroll_to_y_abs(y_pt);
                    });
                }
            }
        }

        // ── Theme colors ──────────────────────────────────────────────────────
        let base = theme::base();
        let crust = theme::crust();
        let surface0 = theme::surface0();
        let text_color = theme::text();
        let subtext = theme::subtext0();
        let blue = theme::blue();

        // ── Activity bar ──────────────────────────────────────────────────────
        let active_tab = self.active_tab;

        let make_activity_btn =
            |id: &'static str, svg_path: &'static str, tab: ActiveTab, cx: &mut Context<Self>| {
                let is_active = active_tab == tab;
                let icon_color = if is_active { blue } else { subtext };
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(48.0))
                    .h(px(48.0))
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::surface0()))
                    .on_click(cx.listener(move |workspace, _: &ClickEvent, _window, cx| {
                        workspace.active_tab = tab;
                        cx.notify();
                    }))
                    .child(svg().path(svg_path).size(px(20.0)).text_color(icon_color))
            };

        let activity_bar = div()
            .flex()
            .flex_col()
            .w(px(48.0))
            .h_full()
            .bg(crust)
            .border_r_1()
            .border_color(surface0)
            .child(div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0))
            .child(make_activity_btn("act-notes", "icons/file.svg", ActiveTab::Notes, cx))
            .child(make_activity_btn("act-graph", "icons/map.svg", ActiveTab::Graph, cx))
            .child(make_activity_btn("act-book", "icons/book-open.svg", ActiveTab::Book, cx))
            .child(make_activity_btn("act-pdf", "icons/eye.svg", ActiveTab::Pdf, cx));

        // ── Left sidebar ──────────────────────────────────────────────────────
        let vault_name = self
            .vault_root
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Vault".to_string());

        // ── Left sidebar — file tree (always, for all tabs) ───────────────────
        let left_sidebar_w = px(self.left_sidebar_width);
        let left_sidebar: Option<AnyElement> = if self.left_visible {
            let sidebar = div()
                .flex()
                .flex_col()
                .h_full()
                .w(left_sidebar_w)
                .flex_shrink_0()
                .bg(crust)
                // Titlebar spacer
                .child(div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0))
                // File tree fills remaining space
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .children(self.file_tree.as_ref().map(|ft| {
                            div().size_full().overflow_hidden().child(ft.clone())
                        })),
                )
                // Footer: vault name + settings icon
                .child(
                    div()
                        .w_full()
                        .h(px(40.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .border_t_1()
                        .border_color(surface0)
                        .child(
                            div()
                                .flex_1()
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(Icon::new(IconName::FolderOpen).size_3().text_color(subtext))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(subtext)
                                        .overflow_hidden()
                                        .child(vault_name),
                                ),
                        )
                        .child(
                            div()
                                .id("settings-btn")
                                .w(px(22.0))
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .cursor_pointer()
                                .text_color(subtext)
                                .hover(|s| s.bg(surface0).text_color(text_color))
                                .child(Icon::new(IconName::Settings).size_4()),
                        ),
                );

            // Resize handle — 4 px wide strip at the right edge, acts as visual border
            let left_resize_start_w = self.left_sidebar_width;
            let left_handle = div()
                .id("left-resize-handle")
                .w(px(4.0))
                .h_full()
                .flex_shrink_0()
                .bg(surface0)
                .cursor_col_resize()
                .hover(|s| s.bg(blue.opacity(0.5)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, ev: &MouseDownEvent, _win, _cx| {
                        ws.left_resize_origin =
                            Some((f32::from(ev.position.x), left_resize_start_w));
                    }),
                )
                .on_drag(LeftResize, |_, _, _, cx| cx.new(|_| ResizeGhost));

            Some(
                div()
                    .flex()
                    .flex_row()
                    .h_full()
                    .child(sidebar)
                    .child(left_handle)
                    .into_any_element(),
            )
        } else {
            None
        };

        // ── Right sidebar (delegated to right_panel submodule) ────────────────
        let right_sidebar_raw = self.render_right_panel(cx);
        let right_resize_start_w = self.right_sidebar_width;
        let right_sidebar: Option<AnyElement> = right_sidebar_raw.map(|panel| {
            let right_handle = div()
                .id("right-resize-handle")
                .w(px(4.0))
                .h_full()
                .flex_shrink_0()
                .bg(surface0)
                .cursor_col_resize()
                .hover(|s| s.bg(blue.opacity(0.5)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, ev: &MouseDownEvent, _win, _cx| {
                        ws.right_resize_origin =
                            Some((f32::from(ev.position.x), right_resize_start_w));
                    }),
                )
                .on_drag(RightResize, |_, _, _, cx| cx.new(|_| ResizeGhost));

            div()
                .flex()
                .flex_row()
                .h_full()
                .child(right_handle)
                .child(panel)
                .into_any_element()
        });

        // ── Main content area ─────────────────────────────────────────────────
        let content = match self.active_tab {
            ActiveTab::Notes => {
                let tab_bar = self.render_tab_bar(cx);

                let note_content: AnyElement =
                    if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(tab.note_view.clone())
                            .into_any_element()
                    } else {
                        // Empty state — no tabs open
                        empty_state("Zetteltypsten", "Open a file from the sidebar")
                    };

                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(tab_bar)
                    .child(note_content)
            }
            ActiveTab::Graph => {
                if let Some(ref graph_view) = self.graph_view {
                    div().flex_1().overflow_hidden().child(graph_view.clone())
                } else {
                    div().flex_1().child(empty_state("No vault open", "Open a vault to view the graph"))
                }
            }
            ActiveTab::Book => {
                if let Some(ref book_view) = self.book_view {
                    div().flex_1().overflow_hidden().child(book_view.clone())
                } else {
                    div().flex_1().child(empty_state("No vault open", "Open a vault to view the book"))
                }
            }
            ActiveTab::Pdf => {
                let pdf_content: AnyElement =
                    if let Some(ref editor) = self.pdf_editor {
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(editor.clone())
                            .into_any_element()
                    } else {
                        empty_state("PDF Composer", "Switch to PDF tab to start composing")
                    };
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(pdf_content)
            }
        };

        // ── Full three-column layout ───────────────────────────────────────────
        div()
            .size_full()
            .bg(base)
            .text_color(text_color)
            .flex()
            .flex_row()
            .overflow_hidden()
            // Left sidebar resize
            .on_drag_move::<LeftResize>(cx.listener(|ws, ev: &DragMoveEvent<LeftResize>, _win, cx| {
                if let Some((start_x, start_w)) = ws.left_resize_origin {
                    let delta = f32::from(ev.event.position.x) - start_x;
                    ws.left_sidebar_width = (start_w + delta).clamp(120.0, 520.0);
                    cx.notify();
                }
            }))
            // Right sidebar resize (dragging left makes it wider)
            .on_drag_move::<RightResize>(cx.listener(|ws, ev: &DragMoveEvent<RightResize>, _win, cx| {
                if let Some((start_x, start_w)) = ws.right_resize_origin {
                    let delta = start_x - f32::from(ev.event.position.x);
                    ws.right_sidebar_width = (start_w + delta).clamp(160.0, 520.0);
                    cx.notify();
                }
            }))
            .child(activity_bar)
            .children(left_sidebar)
            .child(content)
            .children(right_sidebar)
    }
}
