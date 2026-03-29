//! Reusable sidebar row item component used by the file tree.

use crate::theme;
use gpui::*;

/// Render a single sidebar row (file or folder label).
///
/// - `label` — display text for the item
/// - `indent_px` — left padding in pixels (controls nesting depth)
/// - `is_active` — whether this item is the currently open/selected file
///
/// The caller is responsible for attaching `.id(...)`, `.on_click(...)`,
/// `.context_menu(...)`, and any drag/drop handlers after calling this.
///
/// # Example
/// ```rust,no_run
/// # use zt_ui::components::sidebar_item;
/// let _ = sidebar_item("my-note.typ", 16.0, true);
/// ```
pub fn sidebar_item(label: &str, indent_px: f32, is_active: bool) -> gpui::Div {
    div()
        .w_full()
        .pl(px(indent_px))
        .pr(px(8.0))
        .py(px(3.0))
        .text_sm()
        .cursor_pointer()
        .text_color(if is_active { theme::text_primary() } else { theme::text_muted() })
        .bg(if is_active { theme::bg_surface() } else { gpui::transparent_black() })
        .hover(|s| s.bg(theme::bg_hover()).text_color(theme::text_primary()))
}
