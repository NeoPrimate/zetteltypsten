use crate::book_view::BookView;
use crate::editor::{Editor, SaveFile};
use crate::file_ops;
use crate::file_tree::{FileTree, FileTreeEvent, RenameSelected};
use crate::graph_view::{GraphView, GraphViewEvent};
use crate::note_view::{NoteView, NoteViewEvent, ToggleEditMode};
use crate::theme;
use gpui::*;
use gpui_component::{Icon, IconName};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zt_core::note::NoteId;
use zt_index::indexer::VaultIndex;

actions!(workspace, [ToggleLeftSidebar, ToggleRightSidebar]);

/// Which panel is active in the activity bar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Notes = 0,
    Graph = 1,
    Book = 2,
    Pdf = 3,
}

/// Which panel is active in the right sidebar.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RightPanel {
    Outline,
    Tags,
    Inlinks,
    Outlinks,
}

/// A single open note tab.
pub struct NoteTab {
    pub rel_path: String,
    pub title: String,
    pub note_view: Entity<NoteView>,
}

pub struct Workspace {
    vault_root: Option<PathBuf>,
    active_tab: ActiveTab,
    left_visible: bool,
    right_visible: bool,
    right_panel: RightPanel,
    // Multi-tab note area
    note_tabs: Vec<NoteTab>,
    active_note_idx: usize,
    /// Set by the close-button's on_mouse_down; checked by the outer tab's on_click.
    pending_tab_close: Option<usize>,
    // Other views
    editor: Option<Entity<Editor>>,
    graph_view: Option<Entity<GraphView>>,
    book_view: Option<Entity<BookView>>,
    file_tree: Option<Entity<FileTree>>,
    vault_index: Arc<Mutex<VaultIndex>>,
    /// Pending file to open (set by sidebar click / graph nav; opened on next render).
    pending_open: Arc<std::sync::Mutex<Option<String>>>,
    /// Pending cross-note navigation from NoteView link click.
    pending_note_nav: Option<String>,
    /// True after the first startup auto-open; prevents re-opening when all tabs are closed.
    startup_done: bool,
}

// ── Init ─────────────────────────────────────────────────────────────────────

/// Call from main.rs to register keybindings and global actions.
pub fn init(cx: &mut App, workspace: Entity<Workspace>) {
    cx.bind_keys([
        KeyBinding::new("cmd-b", ToggleLeftSidebar, None),
        KeyBinding::new("cmd-r", ToggleRightSidebar, None),
        KeyBinding::new("cmd-e", ToggleEditMode, None),
        KeyBinding::new("cmd-s", SaveFile, None),
        KeyBinding::new("enter", RenameSelected, Some("FileTree")),
    ]);

    let ws = workspace.clone();
    cx.on_action(move |_: &ToggleLeftSidebar, cx: &mut App| {
        ws.update(cx, |ws, cx| {
            if ws.active_tab == ActiveTab::Book {
                if let Some(ref bv) = ws.book_view {
                    bv.update(cx, |bv, cx| bv.toggle_sidebar(cx));
                }
            } else {
                ws.left_visible = !ws.left_visible;
                cx.notify();
            }
        });
    });

    let ws = workspace.clone();
    cx.on_action(move |_: &ToggleRightSidebar, cx: &mut App| {
        ws.update(cx, |ws, cx| {
            ws.right_visible = !ws.right_visible;
            cx.notify();
        });
    });

    let ws = workspace.clone();
    cx.on_action(move |_: &SaveFile, cx: &mut App| {
        ws.update(cx, |ws, cx| {
            if let Some(ref ed) = ws.editor {
                ed.update(cx, |ed, cx| ed.save_file(cx));
            }
        });
    });

    let ws = workspace.clone();
    cx.on_action(move |_: &ToggleEditMode, cx: &mut App| {
        ws.update(cx, |ws, cx| {
            if let Some(tab) = ws.note_tabs.get(ws.active_note_idx) {
                tab.note_view.update(cx, |nv, cx| {
                    nv.edit_mode = !nv.edit_mode;
                    cx.notify();
                });
            }
        });
    });
}

// ── Workspace ────────────────────────────────────────────────────────────────

impl Workspace {
    pub fn new(vault_root: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let graph_view = vault_root.as_ref().map(|root| {
            let gv = cx.new(|cx| GraphView::new(root.clone(), cx));
            cx.subscribe(&gv, |ws: &mut Workspace, _, ev: &GraphViewEvent, cx| {
                let GraphViewEvent::OpenFile(rel) = ev;
                *ws.pending_open.lock().unwrap() = Some(rel.clone());
                ws.active_tab = ActiveTab::Notes;
                cx.notify();
            })
            .detach();
            gv
        });

        // Build vault index in the background.
        let vault_index = Arc::new(Mutex::new(VaultIndex::new()));
        if let Some(ref root) = vault_root {
            let vi = vault_index.clone();
            let root = root.clone();
            let bg = cx.background_executor().clone();
            cx.spawn(async move |this, cx| {
                let idx = bg
                    .spawn(async move { VaultIndex::build(&root).ok() })
                    .await;
                if let Some(idx) = idx {
                    *vi.lock().unwrap() = idx;
                }
                cx.update(|cx| {
                    this.update(cx, |_, cx| cx.notify()).ok();
                })
                .ok();
            })
            .detach();
        }

        Self {
            vault_root,
            active_tab: ActiveTab::Notes,
            left_visible: true,
            right_visible: false,
            right_panel: RightPanel::Outline,
            note_tabs: Vec::new(),
            active_note_idx: 0,
            pending_tab_close: None,
            editor: None,
            graph_view,
            book_view: None,
            file_tree: None,
            vault_index,
            pending_open: Arc::new(std::sync::Mutex::new(None)),
            pending_note_nav: None,
            startup_done: false,
        }
    }

    fn open_file(&mut self, rel_path: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ref root) = self.vault_root else {
            return;
        };

        // If already open in a tab, just switch to it.
        if let Some(idx) = self.note_tabs.iter().position(|t| t.rel_path == rel_path) {
            self.active_note_idx = idx;
            // Sync file tree active highlight.
            if let Some(ref ft) = self.file_tree {
                let rel = rel_path.to_string();
                ft.update(cx, |ft, _| ft.set_active(Some(rel)));
            }
            cx.notify();
            return;
        }

        let full_path = root.join(rel_path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to read {}: {}", full_path.display(), e);
                return;
            }
        };

        let root_clone = root.clone();
        let rel = rel_path.to_string();

        // Create the NoteView entity.
        let nv = cx.new(|cx| {
            NoteView::new(root_clone.clone(), rel.clone(), source.clone(), window, cx)
        });
        cx.subscribe(&nv, |ws: &mut Workspace, nv_entity, ev: &NoteViewEvent, cx| {
            match ev {
                NoteViewEvent::OpenFile(target) => {
                    let p = target.split('#').next().unwrap_or(target).to_string();
                    ws.pending_note_nav = Some(p);
                    cx.notify();
                }
                NoteViewEvent::Recompiled => {
                    let rel = nv_entity.read(cx).rel_path.clone();
                    let source = nv_entity.read(cx).input.read(cx).value().to_string();
                    let note_id = NoteId::from_path(Path::new(&rel));
                    ws.vault_index.lock().unwrap().index_note(&note_id, &source);
                    if let Some(ref gv) = ws.graph_view {
                        gv.update(cx, |gv, cx| gv.rebuild(cx));
                    }
                    cx.notify();
                }
            }
        })
        .detach();

        let title = Path::new(rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rel_path)
            .to_string();

        self.note_tabs.push(NoteTab { rel_path: rel.clone(), title, note_view: nv });
        self.active_note_idx = self.note_tabs.len() - 1;

        // Sync file tree active highlight.
        if let Some(ref ft) = self.file_tree {
            let r = rel.clone();
            ft.update(cx, |ft, _| ft.set_active(Some(r)));
        }

        // Keep editor in sync for the PDF tab.
        self.editor = Some(cx.new(|cx| Editor::new(root_clone, rel, source, window, cx)));

        cx.notify();
    }

    fn close_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.note_tabs.len() {
            self.note_tabs.remove(idx);
            self.active_note_idx =
                self.active_note_idx.min(self.note_tabs.len().saturating_sub(1));
            // Sync file tree active highlight.
            if let Some(ref ft) = self.file_tree {
                let new_rel = self.note_tabs.get(self.active_note_idx).map(|t| t.rel_path.clone());
                ft.update(cx, |ft, _| ft.set_active(new_rel));
            }
            cx.notify();
        }
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ── Lazy inits ────────────────────────────────────────────────────────

        // Auto-open first .typ file on startup (once only).
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

        // Lazy-create FileTree when vault root is known.
        if self.file_tree.is_none() {
            if let Some(ref root) = self.vault_root {
                let root = root.clone();
                let ft = cx.new(|cx| FileTree::new(root, cx));
                cx.subscribe(&ft, |ws: &mut Workspace, _, ev: &FileTreeEvent, cx| {
                    match ev {
                        FileTreeEvent::OpenFile(rel) => {
                            *ws.pending_open.lock().unwrap() = Some(rel.clone());
                            cx.notify();
                        }
                        FileTreeEvent::FileRenamed { old_rel, new_rel } => {
                            let updates: Vec<(Entity<NoteView>, String)> = ws
                                .note_tabs
                                .iter_mut()
                                .filter_map(|tab| {
                                    if &tab.rel_path == old_rel {
                                        tab.rel_path = new_rel.clone();
                                        tab.title = Path::new(new_rel)
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or(new_rel)
                                            .to_string();
                                        Some((tab.note_view.clone(), new_rel.clone()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            for (nv, nr) in updates {
                                nv.update(cx, |nv, _| nv.rel_path = nr);
                            }
                            cx.notify();
                        }
                        FileTreeEvent::FileDeleted(rel) => {
                            ws.note_tabs.retain(|t| &t.rel_path != rel);
                            ws.active_note_idx = ws
                                .active_note_idx
                                .min(ws.note_tabs.len().saturating_sub(1));
                            cx.notify();
                        }
                        FileTreeEvent::FileMoved { old_rel, new_rel } => {
                            let updates: Vec<(Entity<NoteView>, String)> = ws
                                .note_tabs
                                .iter_mut()
                                .filter_map(|tab| {
                                    if &tab.rel_path == old_rel {
                                        tab.rel_path = new_rel.clone();
                                        tab.title = Path::new(new_rel)
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or(new_rel)
                                            .to_string();
                                        Some((tab.note_view.clone(), new_rel.clone()))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            for (nv, nr) in updates {
                                nv.update(cx, |nv, _| nv.rel_path = nr);
                            }
                            cx.notify();
                        }
                        FileTreeEvent::FileCreated(rel) => {
                            *ws.pending_open.lock().unwrap() = Some(rel.clone());
                            cx.notify();
                        }
                        FileTreeEvent::FolderDeleted(folder_rel) => {
                            let prefix = format!("{}/", folder_rel);
                            ws.note_tabs.retain(|t| !t.rel_path.starts_with(&prefix));
                            ws.active_note_idx = ws
                                .active_note_idx
                                .min(ws.note_tabs.len().saturating_sub(1));
                            if let Some(ref ft) = ws.file_tree {
                                let new_rel = ws
                                    .note_tabs
                                    .get(ws.active_note_idx)
                                    .map(|t| t.rel_path.clone());
                                ft.update(cx, |ft, _| ft.set_active(new_rel));
                            }
                            cx.notify();
                        }
                        FileTreeEvent::FolderRenamed { old_rel, new_rel } => {
                            let old_prefix = format!("{}/", old_rel);
                            let new_prefix = format!("{}/", new_rel);
                            let updates: Vec<(Entity<NoteView>, String)> = ws
                                .note_tabs
                                .iter_mut()
                                .filter_map(|tab| {
                                    if tab.rel_path.starts_with(&old_prefix) {
                                        let rest = tab.rel_path[old_prefix.len()..].to_string();
                                        let updated = format!("{}{}", new_prefix, rest);
                                        tab.rel_path = updated.clone();
                                        tab.title = Path::new(&updated)
                                            .file_stem()
                                            .and_then(|s| s.to_str())
                                            .unwrap_or(&updated)
                                            .to_string();
                                        Some((tab.note_view.clone(), updated))
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            for (nv, nr) in updates {
                                nv.update(cx, |nv, _| nv.rel_path = nr);
                            }
                            cx.notify();
                        }
                    }
                })
                .detach();
                // Sync initial active file.
                if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                    let rel = tab.rel_path.clone();
                    ft.update(cx, |ft, _| ft.set_active(Some(rel)));
                }
                self.file_tree = Some(ft);
            }
        }

        // Lazy-create BookView.
        if self.active_tab == ActiveTab::Book && self.book_view.is_none() {
            if let Some(ref root) = self.vault_root {
                let root = root.clone();
                self.book_view = Some(cx.new(|cx| BookView::new(root, cx)));
            }
        }

        // ── Consume pending opens ─────────────────────────────────────────────
        let pending = self.pending_open.lock().unwrap().take();
        if let Some(rel_path) = pending {
            self.open_file(&rel_path, window, cx);
        }
        let pending_nav = self.pending_note_nav.take();
        if let Some(rel_path) = pending_nav {
            self.open_file(&rel_path, window, cx);
        }

        // ── Theme colors ──────────────────────────────────────────────────────
        let base = theme::base();
        let crust = theme::crust();
        let surface0 = theme::surface0();
        let text_color = theme::text();
        let subtext = theme::subtext0();
        let blue = theme::blue();

        // ── Activity bar ──────────────────────────────────────────────────────
        let act_tab = self.active_tab;

        let make_icon_btn =
            |id: &'static str, svg_path: &'static str, tab: ActiveTab, cx: &mut Context<Self>| {
                let is_active = act_tab == tab;
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
                    .on_click(cx.listener(move |ws, _: &ClickEvent, _window, cx| {
                        ws.active_tab = tab;
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
            .child(make_icon_btn("act-notes", "icons/file.svg", ActiveTab::Notes, cx))
            .child(make_icon_btn("act-graph", "icons/map.svg", ActiveTab::Graph, cx))
            .child(make_icon_btn("act-book", "icons/book-open.svg", ActiveTab::Book, cx))
            .child(make_icon_btn("act-pdf", "icons/eye.svg", ActiveTab::Pdf, cx));

        // ── Left sidebar ──────────────────────────────────────────────────────
        let vault_name = self
            .vault_root
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Vault".to_string());

        let left_sidebar = if self.left_visible
            && self.active_tab != ActiveTab::Graph
            && self.active_tab != ActiveTab::Book
        {
            let sidebar_col = div()
                .flex()
                .flex_col()
                .h_full()
                .w(px(220.0))
                .bg(crust)
                .border_r_1()
                .border_color(surface0)
                // Empty titlebar placeholder (macOS traffic-light row)
                .child(div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0))
                // Vault name header — lives BELOW the titlebar row
                .child(
                    div()
                        .w_full()
                        .px(px(12.0))
                        .py(px(6.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .border_b_1()
                        .border_color(surface0)
                        .child(Icon::new(IconName::FolderOpen).size_4().text_color(subtext))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text_color)
                                .child(vault_name),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .children(self.file_tree.as_ref().map(|ft| {
                            div().size_full().overflow_hidden().child(ft.clone())
                        })),
                );
            Some(sidebar_col)
        } else {
            None
        };

        // ── Right sidebar ─────────────────────────────────────────────────────
        let right_sidebar = if self.right_visible {
            let (note_source, note_id_opt) =
                if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                    let nv_ref = tab.note_view.read(cx);
                    let source = nv_ref.input.read(cx).value().to_string();
                    let note_id = NoteId::from_path(Path::new(&nv_ref.rel_path));
                    (source, Some(note_id))
                } else {
                    (String::new(), None)
                };

            let vi = self.vault_index.lock().unwrap();

            // Outline
            let outline_items: Vec<AnyElement> = parse_outline(&note_source)
                .into_iter()
                .map(|(depth, text)| {
                    let color = match depth {
                        1 => theme::text(),
                        _ => theme::subtext0(),
                    };
                    div()
                        .pl(px(12.0 + (depth - 1) as f32 * 14.0))
                        .pr(px(8.0))
                        .py(px(3.0))
                        .text_sm()
                        .text_color(color)
                        .child(text)
                        .into_any_element()
                })
                .collect();

            // Tags — small Catppuccin-colored pills
            let tag_items: Vec<AnyElement> =
                zt_index::extractor::extract_tags(&note_source)
                    .into_iter()
                    .map(|tag| {
                        let color = tag_color(tag.0.as_str());
                        div()
                            .m(px(2.0))
                            .px(px(7.0))
                            .py(px(1.0))
                            .rounded_full()
                            .bg(color.opacity(0.15))
                            .border_1()
                            .border_color(color.opacity(0.4))
                            .text_xs()
                            .text_color(color)
                            .child(tag.0.clone())
                            .into_any_element()
                    })
                    .collect();

            // Collect inlink and outlink data (title + rel_path) while vi is locked.
            let inlinks: Vec<(String, String)> = if let Some(ref nid) = note_id_opt {
                vi.backlinks_with_context(nid)
                    .into_iter()
                    .map(|bl| {
                        let rel = bl.source_id.to_path().to_string_lossy().into_owned();
                        (bl.source_title, rel)
                    })
                    .collect()
            } else {
                vec![]
            };

            let outlinks: Vec<(String, String)> = if let Some(ref nid) = note_id_opt {
                vi.link_graph
                    .outgoing(nid)
                    .into_iter()
                    .map(|tid| {
                        let title = vi.title_of(tid);
                        let rel = tid.to_path().to_string_lossy().into_owned();
                        (title, rel)
                    })
                    .collect()
            } else {
                vec![]
            };

            drop(vi);

            // Build inlink items with click-to-navigate.
            let ws_entity = cx.entity();
            let pending_open_c = self.pending_open.clone();

            let inlink_items: Vec<AnyElement> = inlinks
                .into_iter()
                .enumerate()
                .map(|(idx, (title, rel))| {
                    let po = pending_open_c.clone();
                    let ws = ws_entity.clone();
                    let rel2 = rel.clone();
                    div()
                        .id(SharedString::from(format!("inlink-{}", idx)))
                        .px(px(12.0))
                        .py(px(5.0))
                        .text_sm()
                        .text_color(theme::blue())
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::surface0()))
                        .on_click(move |_, _, cx| {
                            *po.lock().unwrap() = Some(rel2.clone());
                            ws.update(cx, |_, cx| cx.notify());
                        })
                        .child(title)
                        .into_any_element()
                })
                .collect();

            // Build outlink items with click-to-navigate.
            let outlink_items: Vec<AnyElement> = outlinks
                .into_iter()
                .enumerate()
                .map(|(idx, (title, rel))| {
                    let po = pending_open_c.clone();
                    let ws = ws_entity.clone();
                    let rel2 = rel.clone();
                    div()
                        .id(SharedString::from(format!("outlink-{}", idx)))
                        .px(px(12.0))
                        .py(px(5.0))
                        .text_sm()
                        .text_color(theme::blue())
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::surface0()))
                        .on_click(move |_, _, cx| {
                            *po.lock().unwrap() = Some(rel2.clone());
                            ws.update(cx, |_, cx| cx.notify());
                        })
                        .child(title)
                        .into_any_element()
                })
                .collect();

            let right_panel = self.right_panel;

            let make_tab_btn = |id: &'static str, icon: IconName, panel: RightPanel| {
                let is_active = right_panel == panel;
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .h(px(32.0))
                    .cursor_pointer()
                    .text_color(if is_active { blue } else { subtext })
                    .border_b_2()
                    .border_color(if is_active { blue } else { gpui::transparent_black() })
                    .hover(|s| s.text_color(theme::text()))
                    .on_click(cx.listener(move |ws, _: &ClickEvent, _window, cx| {
                        ws.right_panel = panel;
                        cx.notify();
                    }))
                    .child(Icon::new(icon).size_4())
            };

            let tab_row = div()
                .w_full()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(surface0)
                .child(make_tab_btn("rp-outline", IconName::BookOpen, RightPanel::Outline))
                .child(make_tab_btn("rp-tags", IconName::Asterisk, RightPanel::Tags))
                .child(make_tab_btn("rp-inlinks", IconName::ArrowLeft, RightPanel::Inlinks))
                .child(make_tab_btn("rp-outlinks", IconName::ArrowRight, RightPanel::Outlinks));

            let empty_msg = |msg: &'static str| {
                div()
                    .p(px(12.0))
                    .text_sm()
                    .text_color(subtext)
                    .child(msg)
                    .into_any_element()
            };

            let panel_content: AnyElement = match right_panel {
                RightPanel::Outline => {
                    if outline_items.is_empty() {
                        empty_msg("No headings")
                    } else {
                        div().py(px(4.0)).children(outline_items).into_any_element()
                    }
                }
                RightPanel::Tags => {
                    if tag_items.is_empty() {
                        empty_msg("No tags")
                    } else {
                        div()
                            .p(px(8.0))
                            .flex()
                            .flex_wrap()
                            .children(tag_items)
                            .into_any_element()
                    }
                }
                RightPanel::Inlinks => {
                    if inlink_items.is_empty() {
                        empty_msg("No inlinks")
                    } else {
                        div().py(px(4.0)).children(inlink_items).into_any_element()
                    }
                }
                RightPanel::Outlinks => {
                    if outlink_items.is_empty() {
                        empty_msg("No outlinks")
                    } else {
                        div().py(px(4.0)).children(outlink_items).into_any_element()
                    }
                }
            };

            let make_bar = || div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0);
            Some(
                div()
                    .flex()
                    .flex_col()
                    .h_full()
                    .w(px(240.0))
                    .bg(theme::mantle())
                    .border_l_1()
                    .border_color(surface0)
                    .child(make_bar())
                    .child(tab_row)
                    .child(
                        div()
                            .id("right-panel-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(panel_content),
                    ),
            )
        } else {
            None
        };

        // ── Main content area ─────────────────────────────────────────────────
        let content = match self.active_tab {
            ActiveTab::Notes => {
                // ── Tab bar ───────────────────────────────────────────────────
                // The tab bar occupies TITLEBAR_H (36px) — same as other columns.
                // Tabs sit near the bottom of that row with rounded top corners,
                // giving them a "browser tab" appearance.
                // Left padding keeps tabs safely right of the macOS traffic lights.
                let tab_left_pad = if self.left_visible { 4.0f32 } else { 76.0f32 };
                let active_idx = self.active_note_idx;
                let tab_bar = {
                    let mut tab_els: Vec<AnyElement> = self
                        .note_tabs
                        .iter()
                        .enumerate()
                        .map(|(i, tab)| {
                            let is_active = i == active_idx;
                            let tab_id = SharedString::from(format!("tab-{}", i));
                            let close_id = SharedString::from(format!("tab-close-{}", i));

                            // Base tab pill — rounded top corners, sits at bottom of bar
                            let tab_pill = div()
                                .id(tab_id)
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_center()
                                .px(px(10.0))
                                .h(px(28.0))
                                .gap(px(6.0))
                                .max_w(px(180.0))
                                .min_w(px(60.0))
                                .cursor_pointer()
                                .rounded_tl(px(6.0))
                                .rounded_tr(px(6.0))
                                .bg(if is_active {
                                    theme::mantle()
                                } else {
                                    gpui::transparent_black()
                                })
                                .text_sm()
                                .text_color(if is_active { text_color } else { subtext })
                                .on_click(cx.listener(move |ws, _: &ClickEvent, _, cx| {
                                    if let Some(close_i) = ws.pending_tab_close.take() {
                                        ws.close_tab(close_i, cx);
                                    } else {
                                        ws.active_note_idx = i;
                                        if let Some(ref ft) = ws.file_tree {
                                            let rel = ws
                                                .note_tabs
                                                .get(i)
                                                .map(|t| t.rel_path.clone());
                                            ft.update(cx, |ft, _| ft.set_active(rel));
                                        }
                                        cx.notify();
                                    }
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(tab.title.clone()),
                                )
                                .child(
                                    div()
                                        .id(close_id)
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .rounded_sm()
                                        .cursor_pointer()
                                        .text_color(subtext)
                                        .hover(|s| s.bg(theme::surface0()).text_color(text_color))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |ws, _: &MouseDownEvent, _, _| {
                                                ws.pending_tab_close = Some(i);
                                            }),
                                        )
                                        .child("×"),
                                );

                            // Hover effect only for inactive tabs
                            let tab_pill = if is_active {
                                tab_pill
                            } else {
                                tab_pill.hover(|s| {
                                    s.bg(theme::surface0()).text_color(theme::text())
                                })
                            };

                            tab_pill.into_any_element()
                        })
                        .collect();

                    // ＋ new-file button
                    tab_els.push(
                        div()
                            .id("tab-new")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(subtext)
                            .hover(|s| s.bg(theme::surface0()).text_color(text_color))
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                if let Some(ref root) = ws.vault_root.clone() {
                                    let used: std::collections::HashSet<String> = ws
                                        .note_tabs
                                        .iter()
                                        .map(|t| t.rel_path.clone())
                                        .collect();
                                    let name = (1usize..)
                                        .map(|i| format!("untitled-{}.typ", i))
                                        .find(|n| {
                                            !root.join(n).exists() && !used.contains(n)
                                        })
                                        .unwrap_or_else(|| "untitled.typ".to_string());
                                    match file_ops::create_file(root, "", &name) {
                                        Ok(rel) => ws.open_file(&rel, window, cx),
                                        Err(e) => log::error!("Create file: {}", e),
                                    }
                                }
                            }))
                            .child("+")
                            .into_any_element(),
                    );

                    // Outer tab bar: full TITLEBAR_H height, items aligned to bottom
                    div()
                        .id("note-tab-bar")
                        .w_full()
                        .h(px(theme::TITLEBAR_H))
                        .bg(surface0)
                        .flex()
                        .flex_row()
                        .items_end()           // tabs sit at the bottom of the bar
                        .pl(px(tab_left_pad))  // safe zone left of traffic lights
                        .pr(px(4.0))
                        .gap(px(2.0))
                        .overflow_hidden()
                        .children(tab_els)
                };

                // ── Active note view ──────────────────────────────────────────
                let note_content: AnyElement =
                    if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(tab.note_view.clone())
                            .into_any_element()
                    } else {
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(
                                        div().text_color(blue).text_xl().child("Zetteltypsten"),
                                    )
                                    .child(
                                        div()
                                            .text_color(subtext)
                                            .text_sm()
                                            .child("Open a file from the sidebar"),
                                    ),
                            )
                            .into_any_element()
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
                if let Some(ref gv) = self.graph_view {
                    div().flex_1().overflow_hidden().child(gv.clone())
                } else {
                    div().flex_1().flex().items_center().justify_center().child(
                        div().text_color(subtext).text_sm().child("No vault open"),
                    )
                }
            }
            ActiveTab::Book => {
                if let Some(ref bv) = self.book_view {
                    div().flex_1().overflow_hidden().child(bv.clone())
                } else {
                    div().flex_1().flex().items_center().justify_center().child(
                        div().text_color(subtext).text_sm().child("No vault open"),
                    )
                }
            }
            ActiveTab::Pdf => {
                if let Some(ref editor) = self.editor {
                    div().flex_1().overflow_hidden().child(editor.clone())
                } else {
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(12.0))
                                .child(div().text_color(blue).text_xl().child("Zetteltypsten"))
                                .child(
                                    div()
                                        .text_color(subtext)
                                        .text_sm()
                                        .child("Open a file from the sidebar"),
                                ),
                        )
                }
            }
        };

        // ── Full layout ───────────────────────────────────────────────────────
        div()
            .size_full()
            .bg(base)
            .text_color(text_color)
            .flex()
            .flex_row()
            .overflow_hidden()
            .child(activity_bar)
            .children(left_sidebar)
            .child(content)
            .children(right_sidebar)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse `= H1`, `== H2` … headings from Typst source.
fn parse_outline(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let eq_count = trimmed.chars().take_while(|&c| c == '=').count();
            if eq_count == 0 {
                return None;
            }
            let rest = &trimmed[eq_count..];
            if rest.starts_with(' ') {
                Some((eq_count, rest.trim().to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Map a tag to a stable Catppuccin palette color.
fn tag_color(tag: &str) -> Hsla {
    let palette = [
        theme::blue(),
        theme::green(),
        theme::red(),
        theme::peach(),
        theme::yellow(),
        theme::teal(),
        theme::mauve(),
        theme::sky(),
    ];
    let hash = tag.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    palette[hash % palette.len()]
}
