//! Centered empty-state placeholder used across multiple content views.

use crate::theme;
use gpui::*;

/// Render a centered placeholder with a prominent title and a muted subtitle.
///
/// Used in the Notes, Graph, Book, and PDF views when no content is open.
///
/// # Example
/// ```rust,no_run
/// # use zt_ui::components::empty_state;
/// let _ = empty_state("Zetteltypsten", "Open a file from the sidebar");
/// ```
pub fn empty_state(title: &str, subtitle: &str) -> AnyElement {
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
                .child(div().text_color(theme::accent()).text_xl().child(title.to_owned()))
                .child(
                    div()
                        .text_color(theme::text_muted())
                        .text_sm()
                        .child(subtitle.to_owned()),
                ),
        )
        .into_any_element()
}
