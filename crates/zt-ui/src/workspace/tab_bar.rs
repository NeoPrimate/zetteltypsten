//! Tab bar rendering for the Notes and PDF views.
//!
//! Defines [`Workspace::render_tab_bar`] and [`Workspace::render_pdf_tab_bar`],
//! inherent methods that build the horizontal strip of open tabs at the top of
//! the content area.

use super::Workspace;
use crate::{file_ops, theme, utils::LogErr};
use gpui::*;

impl Workspace {
    /// Build the tab bar strip (TITLEBAR_H tall) for the Notes content area.
    ///
    /// Each tab shows the file stem and a close button. A `+` button on the
    /// right creates a new untitled file. The active tab is visually highlighted
    /// with a rounded-top-corners pill shape.
    ///
    /// Returns an owned `AnyElement` so that callers are not lifetime-coupled to `&mut self`.
    pub(super) fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let text_color = theme::text();
        let subtext = theme::subtext0();
        let surface0 = theme::surface0();

        // Push tabs safely past the macOS traffic lights when the left sidebar is hidden.
        let tab_left_pad = if self.left_visible { 4.0f32 } else { 76.0f32 };
        let active_idx = self.active_note_idx;

        let mut tab_elements: Vec<AnyElement> = self
            .note_tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let is_active = i == active_idx;
                let tab_id = SharedString::from(format!("tab-{}", i));
                let close_id = SharedString::from(format!("tab-close-{}", i));

                // Base tab pill — rounded top corners, sits flush with the bar bottom
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
                    .bg(if is_active { theme::mantle() } else { gpui::transparent_black() })
                    .text_sm()
                    .text_color(if is_active { text_color } else { subtext })
                    .on_click(cx.listener(move |workspace, _: &ClickEvent, _, cx| {
                        if let Some(close_idx) = workspace.pending_tab_close.take() {
                            workspace.close_tab(close_idx, cx);
                        } else {
                            workspace.active_note_idx = i;
                            if let Some(ref file_tree) = workspace.file_tree {
                                let rel = workspace
                                    .note_tabs
                                    .get(i)
                                    .map(|t| t.rel_path.to_string());
                                file_tree.update(cx, |file_tree, _| file_tree.set_active(rel));
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
                                cx.listener(move |workspace, _: &MouseDownEvent, _, _| {
                                    workspace.pending_tab_close = Some(i);
                                }),
                            )
                            .child("×"),
                    );

                // Hover highlight only for inactive tabs
                let tab_pill = if is_active {
                    tab_pill
                } else {
                    tab_pill.hover(|s| s.bg(theme::surface0()).text_color(theme::text()))
                };

                tab_pill.into_any_element()
            })
            .collect();

        // ＋ new-note button — opens a draft tab with the title field focused
        tab_elements.push(
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
                .on_click(cx.listener(|workspace, _: &ClickEvent, window, cx| {
                    workspace.open_new_note("", window, cx);
                }))
                .child("+")
                .into_any_element(),
        );

        div()
            .id("note-tab-bar")
            .w_full()
            .h(px(theme::TITLEBAR_H))
            .bg(surface0)
            .flex()
            .flex_row()
            .items_end()          // tabs sit at the bottom of the titlebar row
            .pl(px(tab_left_pad)) // safe zone left of macOS traffic lights
            .pr(px(4.0))
            .gap(px(2.0))
            .overflow_hidden()
            .children(tab_elements)
            .into_any_element()
    }
}
