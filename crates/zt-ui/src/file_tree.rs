use crate::{file_ops, theme};
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

actions!(file_tree, [RenameSelected]);

// ── Events ───────────────────────────────────────────────────────────────────

pub enum FileTreeEvent {
    OpenFile(String),
    FileRenamed { old_rel: String, new_rel: String },
    FileDeleted(String),
    FileMoved { old_rel: String, new_rel: String },
    FileCreated(String),
    FolderDeleted(String),
    FolderRenamed { old_rel: String, new_rel: String },
}

impl EventEmitter<FileTreeEvent> for FileTree {}

// ── Drag preview ─────────────────────────────────────────────────────────────

struct DragLabel {
    text: String,
}

impl Render for DragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(4.0))
            .bg(theme::surface0())
            .rounded_md()
            .text_sm()
            .text_color(theme::text())
            .child(self.text.clone())
    }
}

// ── FileTree entity ───────────────────────────────────────────────────────────

pub struct FileTree {
    vault_root: PathBuf,
    /// rel_paths of folders that are currently collapsed (all others are expanded).
    collapsed: HashSet<String>,
    /// rel_path of the file currently being renamed (None = not renaming).
    renaming: Option<String>,
    rename_input: Option<Entity<InputState>>,
    /// rel_path of the currently-active file (highlighted).
    active_rel_path: Option<String>,
    /// rel_path of the last-clicked item (file or folder) — used for Enter-to-rename.
    selected: Option<String>,
    /// Focus handle so the tree can receive keyboard events.
    focus_handle: FocusHandle,
}

impl FileTree {
    pub fn new(vault_root: PathBuf, cx: &mut Context<Self>) -> Self {
        Self {
            vault_root,
            collapsed: HashSet::new(),
            renaming: None,
            rename_input: None,
            active_rel_path: None,
            selected: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_active(&mut self, rel_path: Option<String>) {
        self.active_rel_path = rel_path;
    }

    pub(crate) fn start_rename(&mut self, rel_path: String, cx: &mut Context<Self>) {
        self.renaming = Some(rel_path);
        self.rename_input = None; // created lazily in render()
        cx.notify();
    }

    fn commit_rename(&mut self, new_name: String, cx: &mut Context<Self>) {
        if let Some(ref old_rel) = self.renaming.clone() {
            let old_full = self.vault_root.join(old_rel);
            let is_dir = old_full.is_dir();
            let name_with_ext = if is_dir || new_name.ends_with(".typ") {
                new_name.clone()
            } else {
                format!("{}.typ", new_name)
            };
            match file_ops::rename_file(&self.vault_root, old_rel, &name_with_ext) {
                Ok(new_rel) => {
                    if is_dir {
                        cx.emit(FileTreeEvent::FolderRenamed {
                            old_rel: old_rel.clone(),
                            new_rel,
                        });
                    } else {
                        cx.emit(FileTreeEvent::FileRenamed {
                            old_rel: old_rel.clone(),
                            new_rel,
                        });
                    }
                }
                Err(e) => log::error!("Rename failed: {}", e),
            }
        }
        self.renaming = None;
        self.rename_input = None;
        cx.notify();
    }

    fn do_delete(&mut self, rel_path: String, cx: &mut Context<Self>) {
        match file_ops::delete_file(&self.vault_root, &rel_path) {
            Ok(()) => cx.emit(FileTreeEvent::FileDeleted(rel_path)),
            Err(e) => log::error!("Delete failed: {}", e),
        }
        cx.notify();
    }

    fn do_delete_folder(&mut self, rel_path: String, cx: &mut Context<Self>) {
        let full_path = self.vault_root.join(&rel_path);
        match std::fs::remove_dir_all(&full_path) {
            Ok(()) => cx.emit(FileTreeEvent::FolderDeleted(rel_path)),
            Err(e) => log::error!("Delete folder failed: {}", e),
        }
        cx.notify();
    }

    fn do_add_to_book(&self, rel_path: &str) {
        if let Err(e) = file_ops::add_to_book(&self.vault_root, rel_path) {
            log::error!("Add to book failed: {}", e);
        }
    }

    fn do_create_file(&mut self, cx: &mut Context<Self>) {
        let root = self.vault_root.clone();
        let name = (1usize..)
            .map(|i| format!("untitled-{}.typ", i))
            .find(|n| !root.join(n).exists())
            .unwrap_or_else(|| "untitled.typ".to_string());
        match file_ops::create_file(&self.vault_root, "", &name) {
            Ok(rel) => cx.emit(FileTreeEvent::FileCreated(rel)),
            Err(e) => log::error!("Create file failed: {}", e),
        }
        cx.notify();
    }

    fn do_create_file_in(&mut self, dir: String, cx: &mut Context<Self>) {
        let root = self.vault_root.clone();
        let dir_path = root.join(&dir);
        self.collapsed.remove(&dir);
        let name = (1usize..)
            .map(|i| format!("untitled-{}.typ", i))
            .find(|n| !dir_path.join(n).exists())
            .unwrap_or_else(|| "untitled.typ".to_string());
        match file_ops::create_file(&self.vault_root, &dir, &name) {
            Ok(rel) => cx.emit(FileTreeEvent::FileCreated(rel)),
            Err(e) => log::error!("Create file in folder failed: {}", e),
        }
        cx.notify();
    }

    fn do_create_folder(&mut self, cx: &mut Context<Self>) {
        let root = self.vault_root.clone();
        let name = (1usize..)
            .map(|i| format!("new-folder-{}", i))
            .find(|n| !root.join(n).exists())
            .unwrap_or_else(|| "new-folder".to_string());
        if let Err(e) = file_ops::create_folder(&self.vault_root, "", &name) {
            log::error!("Create folder failed: {}", e);
        }
        cx.notify();
    }

    fn do_create_folder_in(&mut self, parent: String, cx: &mut Context<Self>) {
        let root = self.vault_root.clone();
        let parent_path = root.join(&parent);
        self.collapsed.remove(&parent);
        let name = (1usize..)
            .map(|i| format!("new-folder-{}", i))
            .find(|n| !parent_path.join(n).exists())
            .unwrap_or_else(|| "new-folder".to_string());
        if let Err(e) = file_ops::create_folder(&self.vault_root, &parent, &name) {
            log::error!("Create subfolder failed: {}", e);
        }
        cx.notify();
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for FileTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Lazily create rename InputState when renaming is set.
        if let Some(ref rel_path) = self.renaming.clone() {
            if self.rename_input.is_none() {
                let stem = Path::new(rel_path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.strip_suffix(".typ").unwrap_or(s))
                    .unwrap_or(rel_path)
                    .to_string();
                let inp = cx.new(|cx| InputState::new(window, cx).default_value(stem));
                cx.subscribe(&inp, |ft: &mut FileTree, state, ev: &InputEvent, cx| {
                    match ev {
                        InputEvent::PressEnter { .. } => {
                            let val = state.read(cx).value().to_string();
                            ft.commit_rename(val, cx);
                        }
                        InputEvent::Blur => {
                            ft.renaming = None;
                            ft.rename_input = None;
                            cx.notify();
                        }
                        _ => {}
                    }
                })
                .detach();
                self.rename_input = Some(inp);
            }
        }

        let ft_entity = cx.entity();
        let vault_root = self.vault_root.clone();
        let collapsed = self.collapsed.clone();
        let renaming = self.renaming.clone();
        let rename_input = self.rename_input.clone();
        let active = self.active_rel_path.clone();
        let focus_handle = self.focus_handle.clone();

        let items = build_dir_items(
            &vault_root,
            &vault_root,
            &collapsed,
            &renaming,
            rename_input.as_ref(),
            &active,
            &ft_entity,
            &focus_handle,
            0,
        );

        let ft_cf = ft_entity.clone();
        let ft_cd = ft_entity.clone();
        let ft_root_drop = ft_entity.clone();
        let vr_root_drop = vault_root.clone();

        div()
            .id("file-tree-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_focus(&self.focus_handle)
            .key_context("FileTree")
            .on_action(cx.listener(|ft: &mut FileTree, _: &RenameSelected, _window, cx| {
                if let Some(ref path) = ft.selected.clone() {
                    ft.start_rename(path.clone(), cx);
                }
            }))
            .py(px(4.0))
            .drag_over::<String>(|s, _, _, _| s.bg(theme::surface0().opacity(0.4)))
            .on_drop(move |dragged: &String, _, cx| {
                let old = dragged.clone();
                match file_ops::move_file(&vr_root_drop, &old, "") {
                    Ok(new_rel) => {
                        ft_root_drop.update(cx, |_, cx| {
                            cx.emit(FileTreeEvent::FileMoved {
                                old_rel: old,
                                new_rel,
                            });
                            cx.notify();
                        });
                    }
                    Err(e) => log::error!("Move to root failed: {}", e),
                }
            })
            .children(items)
            .context_menu(move |menu, _, _| {
                let ft_f = ft_cf.clone();
                let ft_d = ft_cd.clone();
                menu.item(
                    PopupMenuItem::new("New File").on_click(move |_, _, cx| {
                        ft_f.update(cx, |ft, cx| ft.do_create_file(cx));
                    }),
                )
                .item(
                    PopupMenuItem::new("New Folder").on_click(move |_, _, cx| {
                        ft_d.update(cx, |ft, cx| ft.do_create_folder(cx));
                    }),
                )
            })
    }
}

// ── Recursive tree builder ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn build_dir_items(
    vault_root: &Path,
    dir: &Path,
    collapsed: &HashSet<String>,
    renaming: &Option<String>,
    rename_input: Option<&Entity<InputState>>,
    active: &Option<String>,
    ft_entity: &Entity<FileTree>,
    focus_handle: &FocusHandle,
    depth: usize,
) -> Vec<AnyElement> {
    let mut items: Vec<AnyElement> = Vec::new();

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return items,
    };

    // Directories first, then alphabetical.
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let indent = depth as f32 * 14.0 + 8.0;

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let rel_path = path
            .strip_prefix(vault_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        if path.is_dir() {
            let is_collapsed = collapsed.contains(&rel_path);
            let is_renaming_folder = renaming.as_deref() == Some(rel_path.as_str());
            let caret = if is_collapsed { "▶" } else { "▼" };

            let ft = ft_entity.clone();
            let rel_toggle = rel_path.clone();
            let vr = vault_root.to_path_buf();
            let rel_drop_target = rel_path.clone();
            let ft_drop = ft_entity.clone();
            let ft_cm = ft_entity.clone();
            let rel_cm = rel_path.clone();
            let fh = focus_handle.clone();

            if is_renaming_folder {
                if let Some(inp) = rename_input {
                    let row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .pl(px(indent))
                        .pr(px(8.0))
                        .py(px(1.0))
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::subtext0())
                                .child(caret),
                        )
                        .child(
                            div().flex_1().child(
                                Input::new(inp).appearance(false).bordered(false),
                            ),
                        );
                    items.push(row.into_any_element());
                }
            } else {
                let folder_row = div()
                    .id(SharedString::from(format!("dir-{}", rel_path)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .pl(px(indent))
                    .pr(px(8.0))
                    .py(px(3.0))
                    .gap(px(6.0))
                    .cursor_pointer()
                    .text_color(theme::subtext0())
                    .hover(|s| s.bg(theme::surface0()).text_color(theme::text()))
                    .drag_over::<String>(|s, _, _, _| s.bg(theme::surface0()))
                    .on_drop(move |dragged: &String, _, cx| {
                        let old = dragged.clone();
                        match file_ops::move_file(&vr, &old, &rel_drop_target) {
                            Ok(new_rel) => {
                                ft_drop.update(cx, |_, cx| {
                                    cx.emit(FileTreeEvent::FileMoved {
                                        old_rel: old,
                                        new_rel,
                                    });
                                    cx.notify();
                                });
                            }
                            Err(e) => log::error!("Move failed: {}", e),
                        }
                    })
                    .on_click(move |_, window, cx| {
                        window.focus(&fh);
                        ft.update(cx, |ft, cx| {
                            ft.selected = Some(rel_toggle.clone());
                            if ft.collapsed.contains(&rel_toggle) {
                                ft.collapsed.remove(&rel_toggle);
                            } else {
                                ft.collapsed.insert(rel_toggle.clone());
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::subtext0())
                            .child(caret),
                    )
                    .child(div().text_sm().text_color(theme::text()).child(name))
                    .context_menu(move |menu, _, _| {
                        let ft = ft_cm.clone();
                        let r = rel_cm.clone();
                        menu.item(
                            PopupMenuItem::new("New File").on_click({
                                let ft = ft.clone();
                                let r = r.clone();
                                move |_, _, cx| {
                                    ft.update(cx, |ft, cx| ft.do_create_file_in(r.clone(), cx));
                                }
                            }),
                        )
                        .item(
                            PopupMenuItem::new("New Folder").on_click({
                                let ft = ft.clone();
                                let r = r.clone();
                                move |_, _, cx| {
                                    ft.update(cx, |ft, cx| ft.do_create_folder_in(r.clone(), cx));
                                }
                            }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Rename").on_click({
                                let ft = ft.clone();
                                let r = r.clone();
                                move |_, _, cx| {
                                    ft.update(cx, |ft, cx| ft.start_rename(r.clone(), cx));
                                }
                            }),
                        )
                        .item(
                            PopupMenuItem::new("Delete").on_click({
                                let ft = ft.clone();
                                let r = r.clone();
                                move |_, _, cx| {
                                    ft.update(cx, |ft, cx| ft.do_delete_folder(r.clone(), cx));
                                }
                            }),
                        )
                    });

                items.push(folder_row.into_any_element());
            }

            if !is_collapsed && !is_renaming_folder {
                let children = build_dir_items(
                    vault_root,
                    &path,
                    collapsed,
                    renaming,
                    rename_input,
                    active,
                    ft_entity,
                    focus_handle,
                    depth + 1,
                );
                items.extend(children);
            }
        } else if path.extension().is_some_and(|e| e == "typ") {
            let display_name = name.strip_suffix(".typ").unwrap_or(&name).to_string();
            let is_active = active.as_deref() == Some(rel_path.as_str());
            let is_renaming = renaming.as_deref() == Some(rel_path.as_str());

            if is_renaming {
                if let Some(inp) = rename_input {
                    let row = div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .pl(px(indent))
                        .pr(px(8.0))
                        .py(px(1.0))
                        .gap(px(4.0))
                        .child(
                            div().flex_1().child(
                                Input::new(inp).appearance(false).bordered(false),
                            ),
                        );
                    items.push(row.into_any_element());
                }
            } else {
                let ft_open = ft_entity.clone();
                let rel_open = rel_path.clone();
                let ft_rename = ft_entity.clone();
                let rel_rename = rel_path.clone();
                let ft_delete = ft_entity.clone();
                let rel_delete = rel_path.clone();
                let ft_book = ft_entity.clone();
                let rel_book = rel_path.clone();
                let rel_drag = rel_path.clone();
                let fh = focus_handle.clone();

                let row = div()
                    .id(SharedString::from(format!("file-{}", rel_path)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .pl(px(indent + 16.0)) // extra indent to align with folder text
                    .pr(px(8.0))
                    .py(px(3.0))
                    .cursor_pointer()
                    .text_color(if is_active { theme::blue() } else { theme::subtext0() })
                    .bg(if is_active {
                        theme::surface0()
                    } else {
                        gpui::transparent_black()
                    })
                    .hover(|s| s.bg(theme::surface0()).text_color(theme::text()))
                    .on_drag(rel_drag, move |path: &String, _pos, _window, cx| {
                        let text = Path::new(path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(path)
                            .to_string();
                        cx.new(|_| DragLabel { text })
                    })
                    .on_click(move |_, window, cx| {
                        window.focus(&fh);
                        let r = rel_open.clone();
                        ft_open.update(cx, |ft, cx| {
                            ft.selected = Some(r.clone());
                            cx.emit(FileTreeEvent::OpenFile(r));
                        });
                    })
                    .context_menu(move |menu, _, _| {
                        let ft_r = ft_rename.clone();
                        let rr = rel_rename.clone();
                        let ft_d = ft_delete.clone();
                        let rd = rel_delete.clone();
                        let ft_b = ft_book.clone();
                        let rb = rel_book.clone();
                        menu.item(
                            PopupMenuItem::new("Rename").on_click(move |_, _, cx| {
                                let r = rr.clone();
                                ft_r.update(cx, |ft, cx| ft.start_rename(r, cx));
                            }),
                        )
                        .item(
                            PopupMenuItem::new("Delete").on_click(move |_, _, cx| {
                                let r = rd.clone();
                                ft_d.update(cx, |ft, cx| ft.do_delete(r, cx));
                            }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("Add to Book").on_click(move |_, _, cx| {
                                let r = rb.clone();
                                ft_b.update(cx, |ft, _cx| ft.do_add_to_book(&r));
                            }),
                        )
                    })
                    .child(div().text_sm().child(display_name));

                items.push(row.into_any_element());
            }
        }
    }

    items
}
