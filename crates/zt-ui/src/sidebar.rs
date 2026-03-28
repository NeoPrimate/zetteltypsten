use gpui::*;
use gpui_component::IconName;
use gpui_component::sidebar::{SidebarMenu, SidebarMenuItem};
use std::path::Path;
use std::sync::Arc;

/// Build SidebarMenu items from a vault directory.
pub fn build_sidebar_menu(
    vault_root: &Path,
    on_file_click: Arc<dyn Fn(&str, &ClickEvent, &mut Window, &mut App) + 'static>,
) -> SidebarMenu {
    let mut menu = SidebarMenu::new();
    let items = build_menu_items(vault_root, vault_root, &on_file_click);
    for item in items {
        menu = menu.child(item);
    }
    menu
}

fn build_menu_items(
    root: &Path,
    dir: &Path,
    on_file_click: &Arc<dyn Fn(&str, &ClickEvent, &mut Window, &mut App) + 'static>,
) -> Vec<SidebarMenuItem> {
    let mut items = Vec::new();

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return items,
    };

    // Sort: directories first, then alphabetically
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let children = build_menu_items(root, &path, on_file_click);
            if children.is_empty() {
                continue;
            }

            let item = SidebarMenuItem::new(name)
                .icon(IconName::Folder)
                .default_open(true)
                .children(children);
            items.push(item);
        } else if path.extension().is_some_and(|e| e == "typ") {
            let display_name = name.strip_suffix(".typ").unwrap_or(&name).to_string();
            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();

            let cb = on_file_click.clone();
            let rel = rel_path.clone();
            let item = SidebarMenuItem::new(display_name)
                .icon(IconName::File)
                .on_click(move |ev, window, cx| {
                    cb(&rel, ev, window, cx);
                });
            items.push(item);
        }
    }

    items
}
