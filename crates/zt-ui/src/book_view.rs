use crate::theme;
use crate::typst_canvas;
use gpui::*;
use gpui_component::{Icon, IconName};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use typst::layout::Frame;
use zt_book::config::BookConfig;

// ---------------------------------------------------------------------------
// BookView events
// ---------------------------------------------------------------------------

pub enum BookViewEvent {
    /// Request to open a file in the Notes tab (double-click chapter).
    OpenFile(String),
}

impl EventEmitter<BookViewEvent> for BookView {}

// ---------------------------------------------------------------------------
// BookView
// ---------------------------------------------------------------------------

pub struct BookView {
    pub vault_root: PathBuf,
    pub config: Option<BookConfig>,
    pub selected_nav_idx: usize,
    pages: Vec<Frame>,
    link_store: Arc<Mutex<Vec<(Bounds<Pixels>, crate::note_view::LinkTarget)>>>,
    _pending_compile: Option<Task<()>>,
    /// Shared vault-wide compiled document.
    vault_doc: Arc<Mutex<zt_typst::vault_doc::VaultDocument>>,
}

impl BookView {
    pub fn new(vault_root: PathBuf, vault_doc: Arc<Mutex<zt_typst::vault_doc::VaultDocument>>, cx: &mut Context<Self>) -> Self {
        let config = BookConfig::load(&vault_root).ok();

        let mut bv = Self {
            vault_root,
            config,
            selected_nav_idx: 0,
            pages: Vec::new(),
            link_store: Arc::new(Mutex::new(Vec::new())),
            _pending_compile: None,
            vault_doc,
        };

        bv.load_chapter(0, cx);
        bv
    }

    fn navigable_count(&self) -> usize {
        self.config
            .as_ref()
            .map(|c| c.flatten_chapters().len())
            .unwrap_or(0)
    }

    pub fn load_chapter(&mut self, nav_idx: usize, cx: &mut Context<Self>) {
        let Some(ref config) = self.config else { return };
        let flat = config.flatten_chapters();
        let Some(chapter) = flat.get(nav_idx) else { return };
        let Some(ref file) = chapter.file else { return };
        let rel_path = file.to_string_lossy().to_string();

        // Extract just this chapter's frame from the vault document
        let vd = self.vault_doc.lock().unwrap();
        self.pages = vd.note_frame(&rel_path).into_iter().collect();
        self.selected_nav_idx = nav_idx;
        drop(vd);

        cx.notify();
    }

    /// Reload book.toml from disk and refresh the current chapter.
    pub fn reload_config(&mut self, cx: &mut Context<Self>) {
        self.config = BookConfig::load(&self.vault_root).ok();
        let idx = self.selected_nav_idx;
        self.load_chapter(idx, cx);
        cx.notify();
    }

}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl Render for BookView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface0 = theme::surface0();
        let mantle = theme::mantle();
        let crust = theme::crust();
        let _text_color = theme::text();
        let blue = theme::blue();
        let subtext = theme::subtext0();
        let selected = self.selected_nav_idx;
        let nav_count = self.navigable_count();
        let make_bar = move || div().w_full().h(px(theme::TITLEBAR_H)).bg(surface0);

        // ── No book.toml ─────────────────────────────────────────────────────
        if self.config.is_none() {
            return div()
                .size_full()
                .flex()
                .flex_col()
                .bg(mantle)
                .child(make_bar())
                .child(
                    div().flex_1().flex().items_center().justify_center().child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(10.0))
                            .child(div().text_color(blue).text_xl().child("Book"))
                            .child(
                                div()
                                    .text_color(subtext)
                                    .text_sm()
                                    .child("Create .zetteltypsten/book.toml to define your book"),
                            ),
                    ),
                );
        }

        // ── Content canvas ───────────────────────────────────────────────────
        let pages = self.pages.clone();
        let margin = 24.0_f32;
        let total_height: f32 = pages
            .iter()
            .map(|p| p.height().to_pt() as f32 * typst_canvas::PT_TO_PX + margin)
            .sum::<f32>()
            + margin;

        let link_store = self.link_store.clone();
        let text_color = theme::text();
        let content_canvas = canvas(
            move |bounds, _window, _cx| (bounds, pages.clone()),
            move |_bounds, (bounds, pages), window, _cx| {
                let bx = f32::from(bounds.origin.x);
                let by = f32::from(bounds.origin.y);
                let canvas_w = f32::from(bounds.size.width);
                let available_w = canvas_w - margin * 2.0;
                let mut y_offset = by + margin;
                let mut new_links: Vec<(Bounds<Pixels>, crate::note_view::LinkTarget)> = Vec::new();

                for page in &pages {
                    let page_w_pt = page.width().to_pt() as f32;
                    let page_h_pt = page.height().to_pt() as f32;
                    let scale =
                        (available_w / (page_w_pt * typst_canvas::PT_TO_PX)).min(1.0);
                    let page_w = page_w_pt * typst_canvas::PT_TO_PX * scale;
                    let page_h = page_h_pt * typst_canvas::PT_TO_PX * scale;
                    let x_offset = bx + (canvas_w - page_w) / 2.0;

                    let origin = point(px(x_offset), px(y_offset));
                    let vp_top = by;
                    let vp_bottom = by + f32::from(bounds.size.height);
                    let mut raw_links = Vec::new();
                    typst_canvas::render_frame_styled(
                        window, page, origin, scale, vp_top, vp_bottom,
                        Some(text_color), &mut raw_links,
                    );

                    for lr in raw_links {
                        let target = match lr.destination {
                            typst::model::Destination::Url(url) => {
                                crate::note_view::LinkTarget::Url(url.to_string())
                            }
                            typst::model::Destination::Location(loc) => {
                                crate::note_view::LinkTarget::Location(loc)
                            }
                            _ => continue,
                        };
                        new_links.push((lr.bounds, target));
                    }

                    y_offset += page_h + margin;
                }

                *link_store.lock().unwrap() = new_links;
            },
        )
        .w_full()
        .h(px(total_height));

        // ── Prev / Next footer ───────────────────────────────────────────────
        let has_prev = selected > 0;
        let has_next = selected + 1 < nav_count;

        let footer = div()
            .w_full()
            .h(px(40.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(10.0))
            .border_t_1()
            .border_color(surface0)
            .bg(crust)
            .child(if has_prev {
                div()
                    .id("book-prev")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(subtext)
                    .hover(|s| s.bg(surface0).text_color(text_color))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |bv, _ev: &MouseDownEvent, _window, cx| {
                            if bv.selected_nav_idx > 0 {
                                bv.selected_nav_idx -= 1;
                                let idx = bv.selected_nav_idx;
                                bv.load_chapter(idx, cx);
                                cx.notify();
                            }
                        }),
                    )
                    .child(Icon::new(IconName::ChevronLeft).size_4())
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if has_next {
                div()
                    .id("book-next")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(subtext)
                    .hover(|s| s.bg(surface0).text_color(text_color))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |bv, _ev: &MouseDownEvent, _window, cx| {
                            let next = bv.selected_nav_idx + 1;
                            if next < bv.navigable_count() {
                                bv.selected_nav_idx = next;
                                bv.load_chapter(next, cx);
                                cx.notify();
                            }
                        }),
                    )
                    .child(Icon::new(IconName::ChevronRight).size_4())
                    .into_any_element()
            } else {
                div().into_any_element()
            });

        // ── Full layout: content only (chapter sidebar is in the workspace left panel) ──
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(mantle)
            .child(make_bar())
            .child(
                div()
                    .id("book-content-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|bv, ev: &MouseDownEvent, _window, cx| {
                            let pos = ev.position;
                            let links = bv.link_store.lock().unwrap();
                            for (bounds, target) in links.iter() {
                                if bounds.contains(&pos) {
                                    match target {
                                        crate::note_view::LinkTarget::Location(loc) => {
                                            let vd = bv.vault_doc.lock().unwrap();
                                            if let Some(doc) = vd.document() {
                                                let p = doc.introspector.position(*loc);
                                                let y_pt = p.point.y.to_pt() as f32;
                                                if let Some(note_path) = vd.note_at_y(y_pt) {
                                                    let target = format!("{note_path}#__loc_{y_pt}");
                                                    drop(links);
                                                    drop(vd);
                                                    cx.emit(BookViewEvent::OpenFile(target));
                                                }
                                            }
                                        }
                                        crate::note_view::LinkTarget::Url(_url) => {
                                            // External URL — could open in browser
                                        }
                                    }
                                    break;
                                }
                            }
                        }),
                    )
                    .child(content_canvas),
            )
            .child(footer)
    }
}

