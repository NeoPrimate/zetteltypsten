use crate::theme;
use crate::typst_canvas;
use gpui::*;
use gpui_component::highlighter::{Diagnostic, DiagnosticSeverity};
use gpui_component::input::{InputEvent, InputState, Position, Rope, TabSize};
use typst::layout::Frame;

actions!(editor, [SaveFile]);

/// Auto-closing bracket pairs for Typst
const AUTO_PAIRS: &[(char, char)] = &[
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('"', '"'),
    ('$', '$'),
];

pub struct Editor {
    pub input: Entity<InputState>,
    pages: Vec<Frame>,
    vault_root: std::path::PathBuf,
    rel_path: String,
    /// If set, save to this path instead of rel_path (for PDF docs with virtual paths).
    save_path: Option<String>,
    subscribed: bool,
    pending_compile: Option<Task<()>>,
    world: std::sync::Arc<std::sync::Mutex<zt_typst::world::ZettelWorld>>,
}

impl Editor {
    pub fn new(
        vault_root: std::path::PathBuf,
        rel_path: String,
        source: String,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("typst")
                .multi_line(true)
                .line_number(true)
                .searchable(true)
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .soft_wrap(true)
                .default_value(source.clone())
        });

        let mut world = zt_typst::world::ZettelWorld::new(vault_root.clone(), &rel_path);
        world.set_source(&rel_path, source);
        let (pages, _diags) = Self::compile_with_world(&world);
        let world = std::sync::Arc::new(std::sync::Mutex::new(world));

        Self {
            input,
            pages,
            vault_root,
            rel_path,
            save_path: None,
            subscribed: false,
            pending_compile: None,
            world,
        }
    }

    /// Set an alternate save path (for PDF documents with virtual compilation paths).
    pub fn set_save_path(&mut self, path: String) {
        self.save_path = Some(path);
    }

    fn compile_with_world(
        world: &zt_typst::world::ZettelWorld,
    ) -> (Vec<Frame>, Vec<typst::diag::SourceDiagnostic>) {
        let result = typst::compile::<typst::layout::PagedDocument>(world);
        let warnings: Vec<_> = result.warnings.into_iter().collect();
        match result.output {
            Ok(doc) => {
                let frames = doc.pages.iter().map(|p| p.frame.clone()).collect();
                (frames, warnings)
            }
            Err(errors) => {
                let mut diags: Vec<_> = errors.into_iter().collect();
                diags.extend(warnings);
                (Vec::new(), diags)
            }
        }
    }

    fn push_diagnostics(
        &self,
        diags: &[typst::diag::SourceDiagnostic],
        source: &str,
        cx: &mut Context<Self>,
    ) {
        let world = self.world.lock().unwrap();
        let mut entries: Vec<(Position, Position, DiagnosticSeverity, String)> = Vec::new();

        for d in diags {
            let severity = match d.severity {
                typst::diag::Severity::Error => DiagnosticSeverity::Error,
                typst::diag::Severity::Warning => DiagnosticSeverity::Warning,
            };
            if let Some(range) = world.range(d.span) {
                let (sl, sc) = byte_offset_to_line_col(source, range.start);
                let (el, ec) = byte_offset_to_line_col(source, range.end);
                entries.push((
                    Position { line: sl as u32, character: sc as u32 },
                    Position { line: el as u32, character: ec as u32 },
                    severity,
                    d.message.to_string(),
                ));
            } else {
                entries.push((
                    Position { line: 0, character: 0 },
                    Position { line: 0, character: 1 },
                    severity,
                    d.message.to_string(),
                ));
            }
        }
        drop(world);

        self.input.update(cx, |input_state, _cx| {
            if let Some(diag_set) = input_state.diagnostics_mut() {
                diag_set.reset(&Rope::from_str(source));
                for (start, end, severity, message) in entries {
                    diag_set.push(
                        Diagnostic::new(start..end, message)
                            .with_severity(severity)
                            .with_source("typst"),
                    );
                }
            }
        });
    }

    pub fn save_file(&mut self, cx: &mut Context<Self>) {
        let source = self.input.read(cx).value().to_string();
        let rel = self.save_path.as_deref().unwrap_or(&self.rel_path);
        let path = self.vault_root.join(rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&path, &source) {
            tracing::error!("Failed to save {}: {}", path.display(), e);
        }
    }

    /// Export the current document as PDF. Returns the output path on success.
    pub fn export_pdf(&self, cx: &Context<Self>) -> Option<std::path::PathBuf> {
        use typst::foundations::Smart;

        let source = self.input.read(cx).value().to_string();
        let world = self.world.lock().unwrap();

        let doc = match typst::compile::<typst::layout::PagedDocument>(&*world).output {
            Ok(doc) => doc,
            Err(errs) => {
                for e in errs.iter().take(3) {
                    tracing::error!("PDF export compile error: {e:?}");
                }
                return None;
            }
        };

        let options = typst_pdf::PdfOptions {
            ident: Smart::Auto,
            timestamp: None,
            page_ranges: None,
            standards: typst_pdf::PdfStandards::default(),
            tagged: true,
        };

        let pdf_bytes = match typst_pdf::pdf(&doc, &options) {
            Ok(bytes) => bytes,
            Err(errs) => {
                for e in errs.iter().take(3) {
                    tracing::error!("PDF export error: {e:?}");
                }
                return None;
            }
        };

        // Write to .zetteltypsten/exports/<name>.pdf
        let export_dir = self.vault_root.join(".zetteltypsten/exports");
        let _ = std::fs::create_dir_all(&export_dir);
        let stem = std::path::Path::new(&self.rel_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        let pdf_path = export_dir.join(format!("{stem}.pdf"));
        if let Err(e) = std::fs::write(&pdf_path, &pdf_bytes) {
            tracing::error!("Failed to write PDF: {}", e);
            return None;
        }
        tracing::info!("Exported PDF to {}", pdf_path.display());
        Some(pdf_path)
    }

    fn schedule_recompile(&mut self, cx: &mut Context<Self>) {
        self.pending_compile = None;

        let source = self.input.read(cx).value().to_string();
        let rel_path = self.rel_path.clone();
        let world = self.world.clone();

        let bg = cx.background_executor().clone();
        let task = cx.spawn(async move |this, cx| {
            bg.timer(std::time::Duration::from_millis(50)).await;

            let (new_pages, diags, src) = bg
                .spawn(async move {
                    let mut w = world.lock().unwrap();
                    w.set_source(&rel_path, source.clone());
                    let (pages, diags) = Self::compile_with_world(&w);
                    (pages, diags, source)
                })
                .await;

            cx.update(|cx| {
                this.update(cx, |editor, cx| {
                    editor.pages = new_pages;
                    editor.push_diagnostics(&diags, &src, cx);
                    cx.notify();
                })
                .ok();
            })
            .ok();
        });

        self.pending_compile = Some(task);
    }
}

/// Convert byte offset to (line, column) — 0-indexed.
fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let surface0 = theme::surface0();
        let mantle = theme::mantle();

        if !self.subscribed {
            self.subscribed = true;
            let input = self.input.clone();
            cx.subscribe(
                &input,
                |this: &mut Editor, _state, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.schedule_recompile(cx);
                    }
                },
            )
            .detach();
        }

        let pages = self.pages.clone();

        let margin = 16.0_f32;
        let total_height: f32 = pages
            .iter()
            .map(|page| page.height().to_pt() as f32 * typst_canvas::PT_TO_PX * 0.7 + margin)
            .sum::<f32>()
            + margin;

        let preview_canvas = canvas(
            move |bounds, _window, _cx| (bounds, pages.clone()),
            move |_bounds, (bounds, pages), window, _cx| {
                let bx = f32::from(bounds.origin.x);
                let by = f32::from(bounds.origin.y);
                let canvas_w = f32::from(bounds.size.width);

                let margin = 16.0_f32;
                let available_w = canvas_w - margin * 2.0;
                let mut y_offset = by + margin;

                for page in &pages {
                    let page_w_pt = page.width().to_pt() as f32;
                    let page_h_pt = page.height().to_pt() as f32;
                    let scale =
                        (available_w / (page_w_pt * typst_canvas::PT_TO_PX)).min(1.0);
                    let page_w = page_w_pt * typst_canvas::PT_TO_PX * scale;
                    let page_h = page_h_pt * typst_canvas::PT_TO_PX * scale;
                    let x_offset = bx + (canvas_w - page_w) / 2.0;

                    window.paint_quad(PaintQuad {
                        bounds: Bounds {
                            origin: point(px(x_offset), px(y_offset)),
                            size: size(px(page_w), px(page_h)),
                        },
                        corner_radii: Corners::all(px(2.0)),
                        background: gpui::white().into(),
                        border_widths: Edges::all(px(0.0)),
                        border_color: Hsla::transparent_black(),
                        border_style: BorderStyle::Solid,
                    });

                    let origin = point(px(x_offset), px(y_offset));
                    let mut links = Vec::new();
                    let vp_top = by;
                    let vp_bottom = by + f32::from(bounds.size.height);
                    typst_canvas::render_frame_with_viewport(
                        window, page, origin, scale, vp_top, vp_bottom, &mut links,
                    );

                    y_offset += page_h + margin;
                }
            },
        )
        .w_full()
        .h(px(total_height));

        div()
            .size_full()
            .flex()
            .flex_row()
            .child(
                // Left pane: editor fills the full height (tab bar already provides top spacing)
                div()
                    .flex()
                    .flex_col()
                    .w(relative(0.5))
                    .h_full()
                    .bg(mantle)
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                gpui_component::input::Input::new(&self.input)
                                    .appearance(false)
                                    .bordered(false)
                                    .size_full(),
                            ),
                    ),
            )
            .child(
                // Right pane: scrollable preview fills the full height
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .bg(mantle)
                    .child(
                        div()
                            .id("preview-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .child(preview_canvas),
                    ),
            )
    }
}
