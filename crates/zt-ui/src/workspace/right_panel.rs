//! Right inspector sidebar rendering.
//!
//! Defines [`Workspace::render_right_panel`], which builds the outline,
//! tags, inlinks, and outlinks panels shown when the right sidebar is open.

use super::{graph_sidebar, ActiveTab, RightPanel, Workspace};
use crate::book_view::BookView;
use crate::components::{sidebar_item, tag_badge};
use crate::file_ops;
use crate::theme;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::{Icon, IconName};
use std::path::Path;
use zt_book::config::{BookConfig, Chapter, ChapterLoc};
use zt_core::note::NoteId;

impl Workspace {
    /// Build the right inspector sidebar, or return `None` if it is hidden.
    ///
    /// Returns `Option<AnyElement>` (fully owned) so that callers are not
    /// lifetime-coupled to `&mut self`.
    pub(super) fn render_right_panel(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.right_visible {
            return None;
        }

        // ── Graph tab: filter / groups / display controls ──────────────────────
        if self.active_tab == ActiveTab::Graph {
            let w = self.right_sidebar_width;
            return self.graph_view.as_ref().map(|gv| {
                graph_sidebar::render(gv.clone(), w, cx).into_any_element()
            });
        }

        // ── Book tab: chapter tree ─────────────────────────────────────────────
        if self.active_tab == ActiveTab::Book {
            return self.render_book_right_panel(cx);
        }

        let text_color = theme::text();
        let subtext = theme::subtext0();
        let surface0 = theme::surface0();
        let blue = theme::blue();

        // Pull the compiled note_info and note ID from the active tab.
        let (note_info, note_id_opt) =
            if let Some(tab) = self.note_tabs.get(self.active_note_idx) {
                let note_view_ref = tab.note_view.read(cx);
                let info = note_view_ref.note_info.clone();
                let note_id = if note_view_ref.rel_path.is_empty() {
                    None
                } else {
                    Some(NoteId::from_path(Path::new(&note_view_ref.rel_path)))
                };
                (info, note_id)
            } else {
                (Default::default(), None)
            };

        let vault_idx = self.vault_index.lock().unwrap();

        // Outline — one row per Typst heading
        let outline_items: Vec<AnyElement> = note_info
            .headings
            .iter()
            .map(|(depth, text)| {
                let color = if *depth == 1 { theme::text() } else { theme::subtext0() };
                div()
                    .pl(px(12.0 + (*depth as f32 - 1.0) * 14.0))
                    .pr(px(8.0))
                    .py(px(3.0))
                    .text_sm()
                    .text_color(color)
                    .child(text.clone())
                    .into_any_element()
            })
            .collect();

        // Tags — Catppuccin-colored pills via the shared TagBadge component
        let tag_items: Vec<AnyElement> = note_info
            .tags
            .iter()
            .map(|tag| tag_badge(tag.as_str()))
            .collect();

        // Collect inlinks while the vault index is locked
        let inlinks: Vec<(String, String)> = if let Some(ref note_id) = note_id_opt {
            vault_idx
                .backlinks_with_context(note_id)
                .into_iter()
                .map(|backlink| {
                    let rel = backlink.source_id.to_path().to_string_lossy().into_owned();
                    (backlink.source_title, rel)
                })
                .collect()
        } else {
            vec![]
        };

        // Collect outlinks while the vault index is locked
        let outlinks: Vec<(String, String)> = if let Some(ref note_id) = note_id_opt {
            vault_idx
                .link_graph
                .outgoing(note_id)
                .into_iter()
                .map(|target_id| {
                    let title = vault_idx.title_of(target_id);
                    let rel = target_id.to_path().to_string_lossy().into_owned();
                    (title, rel)
                })
                .collect()
        } else {
            vec![]
        };

        drop(vault_idx);

        // Clickable inlink rows — navigate to the source note on click
        let workspace_entity = cx.entity();
        let pending_open_arc = self.pending_open.clone();

        let inlink_items: Vec<AnyElement> = inlinks
            .into_iter()
            .enumerate()
            .map(|(idx, (title, rel))| {
                let pending_open = pending_open_arc.clone();
                let workspace_ref = workspace_entity.clone();
                let rel_clone = rel.clone();
                div()
                    .id(SharedString::from(format!("inlink-{}", idx)))
                    .px(px(12.0))
                    .py(px(5.0))
                    .text_sm()
                    .text_color(theme::blue())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::surface0()))
                    .on_click(move |_, _, cx| {
                        *pending_open.lock().unwrap() = Some(rel_clone.clone());
                        workspace_ref.update(cx, |_, cx| cx.notify());
                    })
                    .child(title)
                    .into_any_element()
            })
            .collect();

        // Clickable outlink rows
        let outlink_items: Vec<AnyElement> = outlinks
            .into_iter()
            .enumerate()
            .map(|(idx, (title, rel))| {
                let pending_open = pending_open_arc.clone();
                let workspace_ref = workspace_entity.clone();
                let rel_clone = rel.clone();
                div()
                    .id(SharedString::from(format!("outlink-{}", idx)))
                    .px(px(12.0))
                    .py(px(5.0))
                    .text_sm()
                    .text_color(theme::blue())
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::surface0()))
                    .on_click(move |_, _, cx| {
                        *pending_open.lock().unwrap() = Some(rel_clone.clone());
                        workspace_ref.update(cx, |_, cx| cx.notify());
                    })
                    .child(title)
                    .into_any_element()
            })
            .collect();

        let active_right_panel = self.right_panel;

        let make_panel_tab = |id: &'static str, icon: IconName, panel: RightPanel| {
            let is_active = active_right_panel == panel;
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
                .on_click(cx.listener(move |workspace, _: &ClickEvent, _window, cx| {
                    workspace.right_panel = panel;
                    cx.notify();
                }))
                .child(Icon::new(icon).size_4())
        };

        let panel_tabs = div()
            .w_full()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(surface0)
            .child(make_panel_tab("rp-outline", IconName::BookOpen, RightPanel::Outline))
            .child(make_panel_tab("rp-tags", IconName::Asterisk, RightPanel::Tags))
            .child(make_panel_tab("rp-inlinks", IconName::ArrowLeft, RightPanel::Inlinks))
            .child(make_panel_tab("rp-outlinks", IconName::ArrowRight, RightPanel::Outlinks));

        let empty_msg = |msg: &'static str| -> AnyElement {
            div()
                .p(px(12.0))
                .text_sm()
                .text_color(subtext)
                .child(msg)
                .into_any_element()
        };

        let panel_content: AnyElement = match active_right_panel {
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
                    div().p(px(8.0)).flex().flex_wrap().children(tag_items).into_any_element()
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

        Some(
            div()
                .flex()
                .flex_col()
                .h_full()
                .w(px(self.right_sidebar_width))
                .flex_shrink_0()
                .bg(theme::mantle())
                // Titlebar-height spacer aligns with the macOS traffic-light row
                .child(div().w_full().h(px(theme::TITLEBAR_H)).bg(theme::surface0()))
                .child(panel_tabs)
                .child(
                    div()
                        .id("right-panel-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .child(panel_content),
                )
                .into_any_element(),
        )
    }

    /// Start an inline rename for a book chapter or part.
    /// `key` is a ChapterLoc key or "part-N".
    fn start_book_rename(&mut self, key: String, default_value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let inp = cx.new(|cx| InputState::new(window, cx).default_value(default_value.to_string()));
        inp.update(cx, |s, cx| s.focus(window, cx));
        let rename_key = key.clone();
        cx.subscribe(&inp, move |ws: &mut Workspace, state, ev: &InputEvent, cx| {
            match ev {
                InputEvent::PressEnter { .. } => {
                    let new_name = state.read(cx).value().to_string();
                    let new_name = new_name.trim().to_string();
                    if !new_name.is_empty() {
                        ws.commit_book_rename(&rename_key, &new_name, cx);
                    }
                    ws.book_renaming = None;
                    cx.notify();
                }
                InputEvent::Blur => {
                    // On blur, also commit (like Obsidian)
                    let new_name = state.read(cx).value().to_string();
                    let new_name = new_name.trim().to_string();
                    if !new_name.is_empty() {
                        ws.commit_book_rename(&rename_key, &new_name, cx);
                    }
                    ws.book_renaming = None;
                    cx.notify();
                }
                _ => {}
            }
        }).detach();
        self.book_renaming = Some((key, inp));
        cx.notify();
    }

    /// Commit a book rename for a chapter.
    fn commit_book_rename(&mut self, key: &str, new_name: &str, cx: &mut Context<Self>) {
        let Some(ref book_view) = self.book_view else { return };
        let vault_root = book_view.read(cx).vault_root.clone();
        let bve = book_view.clone();

        let loc = ChapterLoc::new(
            key.split('/').filter_map(|s| s.parse::<usize>().ok()).collect()
        );
        if loc.path.is_empty() { return; }

        bve.update(cx, |bv, cx| {
            if let Some(ref mut cfg) = bv.config {
                if let Some(ch) = cfg.chapter_at_mut(&loc) {
                    ch.title = new_name.to_string();
                    let _ = cfg.save_renumbered(&vault_root);
                    bv.reload_config(cx);
                }
            }
        });
    }

    /// Right panel for the Book tab: book title header + scrollable chapter rows
    /// with drag-and-drop reordering, expandable multi-file chapters, and context menus.
    fn render_book_right_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let surface0 = theme::surface0();
        let subtext = theme::subtext0();
        let text_color = theme::text();

        let book_title = self
            .book_view
            .as_ref()
            .and_then(|bv| bv.read(cx).config.as_ref().map(|c| c.title.clone()))
            .unwrap_or_else(|| "Book".to_string());

        let chapter_content: AnyElement = if let Some(ref book_view) = self.book_view {
            let (selected, config_opt, vault_root) = {
                let bv = book_view.read(cx);
                (bv.selected_nav_idx, bv.config.clone(), bv.vault_root.clone())
            };
            if let Some(config) = config_opt {
                let bve = book_view.clone();
                let mut nav_counter = 0usize;
                let mut rows: Vec<AnyElement> = Vec::new();
                let ws = cx.entity().clone();

                // All chapters (flat list with nesting)
                self.collect_chapter_rows(
                    &config.chapters,
                    &mut nav_counter,
                    selected,
                    bve.clone(),
                    0,
                    &[],
                    &vault_root,
                    &mut rows,
                    cx,
                );

                div()
                    .id("book-chapter-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .py(px(4.0))
                    .children(rows)
                    .into_any_element()
            } else {
                div()
                    .p(px(12.0))
                    .text_sm()
                    .text_color(subtext)
                    .child("No book.toml found")
                    .into_any_element()
            }
        } else {
            div().into_any_element()
        };

        Some(
            div()
                .flex()
                .flex_col()
                .h_full()
                .w(px(self.right_sidebar_width))
                .flex_shrink_0()
                .bg(theme::mantle())
                .child(div().w_full().h(px(theme::TITLEBAR_H)).bg(theme::surface0()))
                .child({
                    // + button to add a new chapter
                    let bve_add = self.book_view.clone();
                    let ws_add = cx.entity().clone();

                    div()
                        .w_full()
                        .px(px(12.0))
                        .py(px(6.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .border_b_1()
                        .border_color(surface0)
                        .child(Icon::new(IconName::BookOpen).size_4().text_color(subtext))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(text_color)
                                .child(book_title),
                        )
                        .child(
                            div()
                                .id("book-add-chapter")
                                .cursor_pointer()
                                .text_color(subtext)
                                .hover(|s| s.text_color(theme::text()))
                                .on_click(move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                                    if let Some(ref bve) = bve_add {
                                        let ch_idx = bve.read(cx).config.as_ref()
                                            .map(|c| c.chapters.len()).unwrap_or(0);
                                        bve.update(cx, |bv, cx| {
                                            if let Some(ref mut cfg) = bv.config {
                                                cfg.chapters.push(Chapter {
                                                    title: "New Part".into(),
                                                    file: None,
                                                    number: None,
                                                    children: vec![],
                                                    is_part: true,
                                                });
                                                let _ = cfg.save_renumbered(&bv.vault_root);
                                                bv.reload_config(cx);
                                            }
                                        });
                                        let new_key = ChapterLoc::root(ch_idx).key();
                                        ws_add.update(cx, |ws, cx| {
                                            ws.start_book_rename(new_key, "New Part", window, cx);
                                        });
                                    }
                                })
                                .child(Icon::new(IconName::Plus).size_4()),
                        )
                })
                .child(chapter_content)
                .into_any_element(),
        )
    }

    /// Recursively appends chapter rows into `rows`.
    ///
    /// Three kinds of entry:
    /// - **Part** (`is_part: true`): bold non-interactive section divider
    /// - **Note** (`file: Some(...)`): clickable, loads preview; can have children
    /// - **Section** (`file: None, !is_part`): click toggles collapse; can have children
    ///
    /// Drag-drop is unified:
    /// - Drop ON an item → add as child (both ChapterDrag and file-from-tree)
    /// - Drop on interstitial zone between items → insert at that position
    fn collect_chapter_rows(
        &self,
        chapters: &[Chapter],
        nav_counter: &mut usize,
        selected: usize,
        book_view: Entity<BookView>,
        depth: usize,
        parent_path: &[usize],
        vault_root: &std::path::Path,
        rows: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) {
        let indent_px = 12.0 + depth as f32 * 12.0;

        for (i, ch) in chapters.iter().enumerate() {
            // ── Interstitial drop zone BEFORE this item ──────────────────────
            let insert_loc = {
                let mut p = parent_path.to_vec();
                p.push(i);
                ChapterLoc::new(p)
            };
            self.push_interstitial_drop_zone(
                &format!("gap-{}-{i}", insert_loc.key()),
                indent_px,
                &insert_loc,
                book_view.clone(),
                vault_root,
                rows,
            );

            // ── Item setup ───────────────────────────────────────────────────
            let nav_idx = *nav_counter;
            let has_file = ch.file.is_some();
            let has_children = !ch.children.is_empty();
            if has_file { *nav_counter += 1; }

            let mut loc_path = parent_path.to_vec();
            loc_path.push(i);
            let loc = ChapterLoc::new(loc_path.clone());
            let loc_key = loc.key();
            let is_active = has_file && nav_idx == selected;
            let item_id = SharedString::from(format!("ch-{}", loc_key));
            let first_file: Option<String> = ch.file.as_ref()
                .map(|f| f.to_string_lossy().into_owned());
            let is_collapsed = has_children && !self.expanded_book_chapters.contains(&loc_key);

            let bve = book_view.clone();
            let bve_ctx = book_view.clone();
            let vr_ctx = vault_root.to_path_buf();
            let loc_ctx = loc.clone();
            let ws_ctx = cx.entity().clone();
            let is_renaming_ch = self.book_renaming.as_ref()
                .map(|(k, _)| k == &loc_key).unwrap_or(false);

            let label: String = if ch.section_string().is_empty() {
                ch.title.clone()
            } else {
                format!("{} {}", ch.section_string(), ch.title)
            };

            // ── Part: bold section divider ────────────────────────────────────
            if ch.is_part {
                let mut part_row = div()
                    .id(item_id.clone())
                    .w_full()
                    .pl(px(indent_px))
                    .pr(px(8.0))
                    .pt(px(10.0))
                    .pb(px(4.0))
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme::subtext0())
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center();

                // Label / inline rename
                if is_renaming_ch {
                    if let Some((_, ref inp)) = self.book_renaming {
                        part_row = part_row.child(
                            div().flex_1().child(
                                Input::new(inp).appearance(false).bordered(false).text_sm()
                            )
                        );
                    } else {
                        part_row = part_row.child(label.clone());
                    }
                } else {
                    part_row = part_row.child(label.clone());
                }

                // Drag: reorder parts
                let drag_loc = loc.clone();
                let drag_title = ch.title.clone();
                part_row = part_row.on_drag(
                    ChapterDrag { loc: drag_loc, title: drag_title.clone() },
                    move |_: &ChapterDrag, _, _, cx| cx.new(|_| DragLabel { text: drag_title.clone() }),
                );

                // Context menu for parts
                let part_with_menu = part_row.context_menu({
                    let ws_r = ws_ctx.clone();
                    let rk = loc_key.clone();
                    let rt = ch.title.clone();
                    let bve_rm = bve_ctx.clone();
                    let vr_rm = vr_ctx.clone();
                    let loc_rm = loc_ctx.clone();
                    move |menu, _, _| {
                        let ws_r2 = ws_r.clone();
                        let rk2 = rk.clone();
                        let rt2 = rt.clone();
                        let bve_rm2 = bve_rm.clone();
                        let vr_rm2 = vr_rm.clone();
                        let loc_rm2 = loc_rm.clone();
                        menu.item(
                            PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                                ws_r2.update(cx, |ws, cx| {
                                    ws.start_book_rename(rk2.clone(), &rt2, window, cx);
                                });
                            }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Remove Part").on_click(move |_, _, cx| {
                                bve_rm2.update(cx, |bv, cx| {
                                    if let Some(ref mut cfg) = bv.config {
                                        cfg.remove_chapter(&loc_rm2);
                                        let _ = cfg.save_renumbered(&vr_rm2);
                                        bv.reload_config(cx);
                                    }
                                });
                            }),
                        )
                    }
                });

                rows.push(part_with_menu.into_any_element());
                continue;
            }

            // ── Regular item (note or section) ───────────────────────────────
            let mut row = sidebar_item(&label, indent_px, is_active)
                .id(item_id.clone())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(2.0));

            // Collapse arrow
            if has_children {
                let arrow = if is_collapsed { "▶" } else { "▼" };
                let ws_arrow = ws_ctx.clone();
                let lk_arrow = loc_key.clone();
                row = row.child(
                    div()
                        .id(SharedString::from(format!("arr-{}", loc_key)))
                        .text_xs()
                        .text_color(theme::overlay0())
                        .cursor_pointer()
                        .px(px(2.0))
                        .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            ws_arrow.update(cx, |ws, cx| {
                                if ws.expanded_book_chapters.contains(&lk_arrow) {
                                    ws.expanded_book_chapters.remove(&lk_arrow);
                                } else {
                                    ws.expanded_book_chapters.insert(lk_arrow.clone());
                                }
                                cx.notify();
                            });
                        })
                        .child(arrow)
                );
            }

            // Label / inline rename
            if is_renaming_ch {
                if let Some((_, ref inp)) = self.book_renaming {
                    row = row.child(
                        div().flex_1().child(
                            Input::new(inp).appearance(false).bordered(false).text_sm()
                        )
                    );
                } else {
                    row = row.child(div().flex_1().overflow_x_hidden().child(label.clone()));
                }
            } else {
                row = row.child(div().flex_1().overflow_x_hidden().child(label.clone()));
            }

            // Click handler
            if has_file {
                let bve_click = bve.clone();
                row = row.on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    bve_click.update(cx, |bv, cx| {
                        bv.selected_nav_idx = nav_idx;
                        bv.load_chapter(nav_idx, cx);
                        cx.notify();
                    });
                });
            } else {
                let ws_toggle = ws_ctx.clone();
                let lk_toggle = loc_key.clone();
                row = row.on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    ws_toggle.update(cx, |ws, cx| {
                        if ws.expanded_book_chapters.contains(&lk_toggle) {
                            ws.expanded_book_chapters.remove(&lk_toggle);
                        } else {
                            ws.expanded_book_chapters.insert(lk_toggle.clone());
                        }
                        cx.notify();
                    });
                });
            }

            // Drag: this item is draggable
            let drag_loc = loc.clone();
            let drag_title = ch.title.clone();
            row = row.on_drag(
                ChapterDrag { loc: drag_loc, title: drag_title.clone() },
                move |_: &ChapterDrag, _, _, cx| cx.new(|_| DragLabel { text: drag_title.clone() }),
            );

            // ── Drop ON item → add as child (unified for both drag types) ────
            let bve_ch_drop = bve.clone();
            let vr_ch_drop = vault_root.to_path_buf();
            let loc_ch_drop = loc.clone();
            let ws_exp1 = ws_ctx.clone();
            let lk_exp1 = loc_key.clone();
            row = row
                .drag_over::<ChapterDrag>(|s, _, _, _| s.bg(theme::surface0()))
                .on_drop(move |dragged: &ChapterDrag, _, cx| {
                    bve_ch_drop.update(cx, |bv, cx| {
                        if let Some(ref mut cfg) = bv.config {
                            if let Some(ch) = cfg.remove_chapter(&dragged.loc) {
                                let adjusted = BookConfig::adjust_loc_after_removal(&loc_ch_drop, &dragged.loc);
                                if let Some(parent) = cfg.chapter_at_mut(&adjusted) {
                                    parent.children.push(ch);
                                }
                                let _ = cfg.save_renumbered(&vr_ch_drop);
                                bv.reload_config(cx);
                            }
                        }
                    });
                    ws_exp1.update(cx, |ws, cx| {
                        ws.expanded_book_chapters.insert(lk_exp1.clone());
                        cx.notify();
                    });
                });

            let bve_file_drop = bve.clone();
            let vr_file_drop = vault_root.to_path_buf();
            let loc_file_drop = loc.clone();
            let ws_exp2 = ws_ctx.clone();
            let lk_exp2 = loc_key.clone();
            row = row
                .drag_over::<String>(|s, _, _, _| s.bg(theme::surface0()))
                .on_drop(move |rel_path: &String, _, cx| {
                    let stem = Path::new(rel_path.as_str())
                        .file_stem().and_then(|s| s.to_str())
                        .unwrap_or(rel_path).to_string();
                    bve_file_drop.update(cx, |bv, cx| {
                        if let Some(ref mut cfg) = bv.config {
                            if let Some(chapter) = cfg.chapter_at_mut(&loc_file_drop) {
                                chapter.children.push(Chapter {
                                    title: stem,
                                    file: Some(rel_path.into()),
                                    number: None,
                                    children: vec![],
                                    is_part: false,
                                });
                                let _ = cfg.save_renumbered(&vr_file_drop);
                                bv.reload_config(cx);
                            }
                        }
                    });
                    ws_exp2.update(cx, |ws, cx| {
                        ws.expanded_book_chapters.insert(lk_exp2.clone());
                        cx.notify();
                    });
                });

            // ── Context menu ─────────────────────────────────────────────────
            let row_with_menu = row.context_menu({
                let loc_cm = loc_ctx;
                let bve_cm = bve_ctx;
                let vr_cm = vr_ctx;
                let first_file_cm = first_file.clone();
                let ws_rename = ws_ctx.clone();
                let rename_loc_key = loc_key.clone();
                let rename_title = ch.title.clone();
                move |menu, _, _| {
                    let bve0 = bve_cm.clone();
                    let ff0 = first_file_cm.clone();
                    let ws_r = ws_rename.clone();
                    let rlk = rename_loc_key.clone();
                    let rt = rename_title.clone();
                    let bve1 = bve_cm.clone();
                    let vr1 = vr_cm.clone();
                    let loc1 = loc_cm.clone();
                    let bve2 = bve_cm.clone();
                    let vr2 = vr_cm.clone();
                    let loc2 = loc_cm.clone();
                    let bve3 = bve_cm.clone();
                    let vr3 = vr_cm.clone();
                    let loc3 = loc_cm.clone();

                    let menu = if let Some(ref rel) = ff0 {
                        let rel_c = rel.clone();
                        menu.item(
                            PopupMenuItem::new("Open in Notes").on_click(move |_, _, cx| {
                                bve0.update(cx, |_bv, cx| {
                                    cx.emit(crate::book_view::BookViewEvent::OpenFile(rel_c.clone()));
                                });
                            }),
                        ).separator()
                    } else { menu };

                    menu.item(
                        PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                            ws_r.update(cx, |ws, cx| {
                                ws.start_book_rename(rlk.clone(), &rt, window, cx);
                            });
                        }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Move Up").on_click(move |_, _, cx| {
                            bve1.update(cx, |bv, cx| {
                                if let Some(ref mut cfg) = bv.config {
                                    cfg.move_chapter(&loc1, -1);
                                    let _ = cfg.save_renumbered(&vr1);
                                    bv.reload_config(cx);
                                }
                            });
                        }),
                    )
                    .item(
                        PopupMenuItem::new("Move Down").on_click(move |_, _, cx| {
                            bve2.update(cx, |bv, cx| {
                                if let Some(ref mut cfg) = bv.config {
                                    cfg.move_chapter(&loc2, 1);
                                    let _ = cfg.save_renumbered(&vr2);
                                    bv.reload_config(cx);
                                }
                            });
                        }),
                    )
                    .separator()
                    .item(
                        PopupMenuItem::new("Remove from Book").on_click(move |_, _, cx| {
                            bve3.update(cx, |bv, cx| {
                                if let Some(ref mut cfg) = bv.config {
                                    cfg.remove_chapter(&loc3);
                                    let _ = cfg.save_renumbered(&vr3);
                                    bv.reload_config(cx);
                                }
                            });
                        }),
                    )
                }
            });

            rows.push(row_with_menu.into_any_element());

            // ── Children (if expanded) ───────────────────────────────────────
            if has_children && !is_collapsed {
                self.collect_chapter_rows(
                    &ch.children, nav_counter, selected,
                    book_view.clone(), depth + 1, &loc_path, vault_root, rows, cx,
                );
            } else if has_children && is_collapsed {
                count_nav_indices_recursive(&ch.children, nav_counter);
            }
        }

        // ── Trailing interstitial after last item ────────────────────────────
        let trailing_loc = {
            let mut p = parent_path.to_vec();
            p.push(chapters.len());
            ChapterLoc::new(p)
        };
        self.push_interstitial_drop_zone(
            &format!("gap-end-{}", trailing_loc.key()),
            indent_px,
            &trailing_loc,
            book_view,
            vault_root,
            rows,
        );
    }

    /// A thin drop zone for inserting items between existing rows.
    fn push_interstitial_drop_zone(
        &self,
        id: &str,
        indent_px: f32,
        insert_loc: &ChapterLoc,
        book_view: Entity<BookView>,
        vault_root: &std::path::Path,
        rows: &mut Vec<AnyElement>,
    ) {
        let bve1 = book_view.clone();
        let vr1 = vault_root.to_path_buf();
        let loc1 = insert_loc.clone();
        let bve2 = book_view;
        let vr2 = vault_root.to_path_buf();
        let loc2 = insert_loc.clone();

        rows.push(
            div()
                .id(SharedString::from(id.to_string()))
                .h(px(4.0))
                .w_full()
                .pl(px(indent_px))
                .drag_over::<ChapterDrag>(|s, _, _, _| s.border_t_2().border_color(theme::blue()))
                .on_drop(move |dragged: &ChapterDrag, _, cx| {
                    bve1.update(cx, |bv, cx| {
                        if let Some(ref mut cfg) = bv.config {
                            if let Some(ch) = cfg.remove_chapter(&dragged.loc) {
                                let adjusted = BookConfig::adjust_loc_after_removal(&loc1, &dragged.loc);
                                cfg.insert_chapter(&adjusted, ch);
                                let _ = cfg.save_renumbered(&vr1);
                                bv.reload_config(cx);
                            }
                        }
                    });
                })
                .drag_over::<String>(|s, _, _, _| s.border_t_2().border_color(theme::blue()))
                .on_drop(move |rel_path: &String, _, cx| {
                    let stem = Path::new(rel_path.as_str())
                        .file_stem().and_then(|s| s.to_str())
                        .unwrap_or(rel_path).to_string();
                    bve2.update(cx, |bv, cx| {
                        if let Some(ref mut cfg) = bv.config {
                            cfg.insert_chapter(&loc2, Chapter {
                                title: stem,
                                file: Some(rel_path.into()),
                                number: None,
                                children: vec![],
                                is_part: false,
                            });
                            let _ = cfg.save_renumbered(&vr2);
                            bv.reload_config(cx);
                        }
                    });
                })
                .into_any_element()
        );
    }
}

// ── Drag payloads ────────────────────────────────────────────────────────────

#[derive(Clone)]
struct ChapterDrag {
    loc: ChapterLoc,
    title: String,
}

struct DragLabel {
    text: String,
}

impl Render for DragLabel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(4.0))
            .bg(theme::surface1())
            .rounded(px(4.0))
            .text_sm()
            .text_color(theme::text())
            .child(self.text.clone())
    }
}

/// Count nav indices for collapsed children without producing rows.
fn count_nav_indices_recursive(chapters: &[Chapter], counter: &mut usize) {
    for ch in chapters {
        if ch.file.is_some() {
            *counter += 1;
        }
        count_nav_indices_recursive(&ch.children, counter);
    }
}
