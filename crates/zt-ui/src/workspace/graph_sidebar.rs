//! Graph view right sidebar — Filter, Groups, and Display accordion panels.

use crate::graph_view::GraphView;
use crate::theme;
use gpui::*;
use gpui_component::slider::Slider;
use gpui_component::{Icon, IconName};

use super::Workspace;

/// Render the graph control panel for the workspace right sidebar.
///
/// Returns a `gpui::Div` styled identically to the notes right inspector
/// (dynamic width, background, titlebar spacer, header row).
pub(super) fn render(
    graph_view: Entity<GraphView>,
    width: f32,
    cx: &mut Context<Workspace>,
) -> gpui::Div {
    let gv = graph_view.read(cx);
    let surface0 = theme::surface0();
    let text_color = theme::text();
    let subtext = theme::subtext0();
    let blue = theme::blue();

    // Snapshot state needed for building child elements
    let filter_open = gv.sidebar_filter_open;
    let groups_open = gv.sidebar_groups_open;
    let display_open = gv.sidebar_display_open;
    let all_tags = gv.all_tags.clone();
    let tag_groups = gv.tag_groups.clone();
    let label_slider = gv.label_slider.clone();
    let node_slider = gv.node_slider.clone();
    let edge_slider = gv.edge_slider.clone();
    let _ = gv;

    // ── Accordion header helper ───────────────────────────────────────────────
    // Builds a clickable header row with a chevron and section title.
    let make_accordion_header =
        |id: &'static str,
         title: &'static str,
         open: bool,
         gve: Entity<GraphView>,
         toggle_fn: fn(&mut GraphView) -> ()| {
            div()
                .id(id)
                .w_full()
                .px(px(12.0))
                .py(px(6.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(subtext)
                .hover(|s| s.text_color(text_color))
                .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    gve.update(cx, |gv, cx| {
                        toggle_fn(gv);
                        cx.notify();
                    });
                })
                .child(if open {
                    Icon::new(IconName::ChevronDown)
                        .size_3()
                        .text_color(subtext)
                        .into_any_element()
                } else {
                    Icon::new(IconName::ChevronRight)
                        .size_3()
                        .text_color(subtext)
                        .into_any_element()
                })
                .child(title)
        };

    // ── Filter accordion (deferred — inputs stored but not wired) ─────────────
    let gve = graph_view.clone();
    let filter_header = make_accordion_header(
        "graph-filter-header",
        "FILTER",
        filter_open,
        gve,
        |gv| gv.sidebar_filter_open = !gv.sidebar_filter_open,
    );

    let filter_body = if filter_open {
        Some(
            div()
                .px(px(12.0))
                .pb(px(8.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(subtext)
                        .italic()
                        .child("Filtering coming soon"),
                ),
        )
    } else {
        None
    };

    // ── Groups accordion ──────────────────────────────────────────────────────
    // Palette of colors to cycle through when assigning a group color.
    let palette = vec![
        theme::blue(),
        theme::green(),
        theme::red(),
        theme::peach(),
        theme::yellow(),
        theme::teal(),
        theme::mauve(),
        theme::sky(),
    ];

    let gve = graph_view.clone();
    let groups_header = make_accordion_header(
        "graph-groups-header",
        "GROUPS",
        groups_open,
        gve.clone(),
        |gv| gv.sidebar_groups_open = !gv.sidebar_groups_open,
    );

    let groups_body = if groups_open {
        // Show each tag. Tags that already have a group show a colored swatch.
        // Clicking the swatch cycles the color. Clicking "+" on unassigned tags adds a group.
        let mut tag_rows: Vec<AnyElement> = Vec::new();

        for tag in &all_tags {
            let tag_clone = tag.clone();
            let tag_clone2 = tag.clone();
            let gve2 = gve.clone();
            let gve3 = gve.clone();
            let palette_clone = palette.clone();

            let existing_color: Option<Hsla> = tag_groups
                .iter()
                .find_map(|(t, c)| if t == tag { Some(*c) } else { None });

            let swatch: AnyElement = if let Some(color) = existing_color {
                // Colored swatch — click cycles to next palette color
                div()
                    .id(SharedString::from(format!("group-swatch-{}", tag)))
                    .w(px(12.0))
                    .h(px(12.0))
                    .rounded_sm()
                    .bg(color)
                    .border_1()
                    .border_color(color.opacity(0.6))
                    .cursor_pointer()
                    .flex_shrink_0()
                    .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let tc = tag_clone.clone();
                        let pc = palette_clone.clone();
                        gve2.update(cx, |gv, cx| {
                            if let Some(entry) = gv.tag_groups.iter_mut().find(|(t, _)| *t == tc) {
                                // Cycle to next palette color
                                let current_idx =
                                    pc.iter().position(|c| *c == entry.1).unwrap_or(0);
                                entry.1 = pc[(current_idx + 1) % pc.len()];
                            }
                            cx.notify();
                        });
                    })
                    .into_any_element()
            } else {
                // Empty swatch placeholder (no group assigned)
                div()
                    .w(px(12.0))
                    .h(px(12.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(subtext.opacity(0.3))
                    .flex_shrink_0()
                    .into_any_element()
            };

            // "+" / "×" button
            let action_btn: AnyElement = if existing_color.is_some() {
                // Remove button
                div()
                    .id(SharedString::from(format!("group-remove-{}", tag)))
                    .w(px(14.0))
                    .h(px(14.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(subtext)
                    .hover(|s| s.bg(theme::surface0()).text_color(theme::text()))
                    .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let tc = tag_clone2.clone();
                        gve3.update(cx, |gv, cx| {
                            gv.tag_groups.retain(|(t, _)| *t != tc);
                            cx.notify();
                        });
                    })
                    .child("×")
                    .into_any_element()
            } else {
                let tag_add = tag.clone();
                let gve_add = gve.clone();
                let palette_add = palette.clone();
                // Add button
                div()
                    .id(SharedString::from(format!("group-add-{}", tag)))
                    .w(px(14.0))
                    .h(px(14.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .text_xs()
                    .text_color(subtext)
                    .hover(|s| s.bg(theme::surface0()).text_color(theme::text()))
                    .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                        let tc = tag_add.clone();
                        let pc = palette_add.clone();
                        gve_add.update(cx, |gv, cx| {
                            // Pick next unused palette color
                            let used: Vec<Hsla> = gv.tag_groups.iter().map(|(_, c)| *c).collect();
                            let color = pc
                                .iter()
                                .find(|c| !used.contains(c))
                                .copied()
                                .unwrap_or(pc[0]);
                            gv.tag_groups.push((tc, color));
                            cx.notify();
                        });
                    })
                    .child("+")
                    .into_any_element()
            };

            tag_rows.push(
                div()
                    .w_full()
                    .px(px(12.0))
                    .py(px(3.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(swatch)
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_xs()
                            .text_color(if existing_color.is_some() {
                                text_color
                            } else {
                                subtext
                            })
                            .child(tag.clone()),
                    )
                    .child(action_btn)
                    .into_any_element(),
            );
        }

        if tag_rows.is_empty() {
            tag_rows.push(
                div()
                    .px(px(12.0))
                    .pb(px(8.0))
                    .text_xs()
                    .text_color(subtext)
                    .child("No tags in vault")
                    .into_any_element(),
            );
        }

        Some(div().flex().flex_col().pb(px(4.0)).children(tag_rows))
    } else {
        None
    };

    // ── Display accordion ─────────────────────────────────────────────────────
    let gve = graph_view.clone();
    let display_header = make_accordion_header(
        "graph-display-header",
        "DISPLAY",
        display_open,
        gve,
        |gv| gv.sidebar_display_open = !gv.sidebar_display_open,
    );

    let display_body = if display_open {
        let make_slider_row =
            |label: &'static str, slider: Entity<gpui_component::slider::SliderState>| {
                div()
                    .w_full()
                    .px(px(12.0))
                    .py(px(4.0))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(div().text_xs().text_color(subtext).child(label))
                    .child(
                        Slider::new(&slider)
                            .horizontal()
                            .w_full()
                            .bg(theme::surface0())
                            .text_color(blue),
                    )
            };

        let _ = cx;
        Some(
            div()
                .flex()
                .flex_col()
                .pb(px(8.0))
                .child(make_slider_row("Labels", label_slider))
                .child(make_slider_row("Node Size", node_slider))
                .child(make_slider_row("Edge Thickness", edge_slider)),
        )
    } else {
        None
    };

    // ── Assemble panel ────────────────────────────────────────────────────────
    div()
        .flex()
        .flex_col()
        .h_full()
        .w(px(width))
        .flex_shrink_0()
        .bg(theme::mantle())
        // Titlebar spacer
        .child(div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0))
        // "Graph" header row (matches vault name row in file tree)
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
                .child(
                    svg()
                        .path("icons/map.svg")
                        .size(px(16.0))
                        .text_color(subtext),
                )
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_color)
                        .child("Graph"),
                ),
        )
        // Scrollable accordion content
        .child(
            div()
                .id("graph-sidebar-scroll")
                .flex_1()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .child(filter_header)
                .children(filter_body)
                .child(div().w_full().h(px(1.0)).bg(surface0)) // divider
                .child(groups_header)
                .children(groups_body)
                .child(div().w_full().h(px(1.0)).bg(surface0)) // divider
                .child(display_header)
                .children(display_body),
        )
}
