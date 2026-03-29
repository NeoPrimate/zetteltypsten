//! Global key binding registration and action handler wiring.
//!
//! Call [`init`] once from `main.rs` after the window has opened.

use super::{ActiveTab, Workspace};
use crate::editor::SaveFile;
use crate::file_tree::RenameSelected;
use crate::note_view::ToggleEditMode;
use gpui::*;

/// Register all global key bindings and attach workspace-level action handlers.
///
/// Must be called after the GPUI window is created so that the `Workspace`
/// entity handle is valid.
pub fn init(cx: &mut App, workspace: Entity<Workspace>) {
    cx.bind_keys([
        KeyBinding::new("cmd-b", super::ToggleLeftSidebar, None),
        KeyBinding::new("cmd-r", super::ToggleRightSidebar, None),
        KeyBinding::new("cmd-e", ToggleEditMode, None),
        KeyBinding::new("cmd-s", SaveFile, None),
        KeyBinding::new("enter", RenameSelected, Some("FileTree")),
    ]);

    let workspace_ref = workspace.clone();
    cx.on_action(move |_: &super::ToggleLeftSidebar, cx: &mut App| {
        workspace_ref.update(cx, |workspace, cx| {
            workspace.left_visible = !workspace.left_visible;
            cx.notify();
        });
    });

    let workspace_ref = workspace.clone();
    cx.on_action(move |_: &super::ToggleRightSidebar, cx: &mut App| {
        workspace_ref.update(cx, |workspace, cx| {
            workspace.right_visible = !workspace.right_visible;
            cx.notify();
        });
    });

    let workspace_ref = workspace.clone();
    cx.on_action(move |_: &SaveFile, cx: &mut App| {
        workspace_ref.update(cx, |workspace, cx| {
            match workspace.active_tab {
                ActiveTab::Pdf => {
                    if let Some(tab) = workspace.pdf_tabs.get(workspace.active_pdf_idx) {
                        tab.editor.update(cx, |editor, cx| editor.save_file(cx));
                    }
                }
                _ => {
                    if let Some(tab) = workspace.note_tabs.get(workspace.active_note_idx) {
                        tab.editor.update(cx, |editor, cx| editor.save_file(cx));
                    }
                }
            }
        });
    });

    let workspace_ref = workspace.clone();
    cx.on_action(move |_: &ToggleEditMode, cx: &mut App| {
        workspace_ref.update(cx, |workspace, cx| {
            if let Some(tab) = workspace.note_tabs.get(workspace.active_note_idx) {
                tab.note_view.update(cx, |note_view, cx| {
                    note_view.edit_mode = !note_view.edit_mode;
                    cx.notify();
                });
            }
        });
    });
}
