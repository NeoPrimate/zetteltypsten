//! Reusable tag badge pill component.

use crate::theme;
use gpui::*;

/// Map a tag string to a stable Catppuccin Macchiato palette color.
///
/// The same tag always maps to the same color across renders (hash-stable).
pub fn tag_color(tag: &str) -> Hsla {
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

/// Render a Catppuccin-colored pill badge for a single tag label.
///
/// # Example
/// ```rust,no_run
/// # use zt_ui::components::tag_badge;
/// # let tags: Vec<zt_core::tag::Tag> = vec![];
/// // tags.iter().map(|t| tag_badge(t.as_str()))
/// ```
pub fn tag_badge(label: &str) -> AnyElement {
    let color = tag_color(label);
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
        .child(label.to_owned())
        .into_any_element()
}
