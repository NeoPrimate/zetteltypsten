use crate::world::ZettelWorld;
use anyhow::Result;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use typst_html::HtmlDocument;
use typst::WorldExt;

/// A structured diagnostic from Typst compilation.
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    pub from: usize,
    pub to: usize,
    pub severity: &'static str, // "error" or "warning"
    pub message: String,
}

/// Compilation result with both HTML and diagnostics.
pub struct CompileResult {
    pub html: Option<String>,
    pub diagnostics: Vec<DiagnosticInfo>,
    pub error_message: Option<String>,
}

/// Extract structured diagnostics from Typst's SourceDiagnostic list.
fn extract_diagnostics(
    diags: &[typst::diag::SourceDiagnostic],
    world: &ZettelWorld,
) -> Vec<DiagnosticInfo> {
    diags
        .iter()
        .filter_map(|d| {
            let range = world.range(d.span)?;
            Some(DiagnosticInfo {
                from: range.start,
                to: range.end,
                severity: match d.severity {
                    typst::diag::Severity::Error => "error",
                    typst::diag::Severity::Warning => "warning",
                },
                message: d.message.to_string(),
            })
        })
        .collect()
}

/// Serialize diagnostics to JSON for CM6's lint gutter.
pub fn diagnostics_to_json(diags: &[DiagnosticInfo]) -> String {
    let mut json = String::with_capacity(diags.len() * 60);
    json.push('[');
    for (i, d) in diags.iter().enumerate() {
        if i > 0 { json.push(','); }
        let msg = d.message.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        json.push_str(&format!(
            r#"{{"from":{},"to":{},"severity":"{}","message":"{}"}}"#,
            d.from, d.to, d.severity, msg
        ));
    }
    json.push(']');
    json
}

/// Compile Typst source to annotated HTML with span data-attributes for click-to-edit.
///
/// Each HTML element gets a `data-span="start:end"` attribute encoding the byte range
/// in the original source, plus a unique `id="zt-N"` for JS targeting.
pub fn compile_to_annotated_html(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
) -> Result<String> {
    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, source.to_string());

    let doc: HtmlDocument = typst::compile(&world)
        .output
        .map_err(|diagnostics| {
            let messages: Vec<String> = diagnostics
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            anyhow::anyhow!("Typst HTML compilation failed:\n{}", messages.join("\n"))
        })?;

    Ok(encode_annotated_html(&doc, &world, source))
}

/// Compile source text directly to annotated HTML (no vault context).
pub fn compile_string_to_html(source: &str) -> Result<String> {
    let tmp = std::env::temp_dir().join("zetteltypsten_scratch");
    std::fs::create_dir_all(&tmp)?;
    compile_to_annotated_html(&tmp, "main.typ", source)
}

/// Compile to body-only HTML (no <!DOCTYPE>, <html>, <head> wrappers).
/// Returns `<style>...</style><div class="zt-content">...body...</div>`
/// Suitable for embedding via dangerous_inner_html.
pub fn compile_to_body_html(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
) -> Result<String> {
    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, source.to_string());

    let doc: HtmlDocument = typst::compile(&world)
        .output
        .map_err(|diagnostics| {
            let messages: Vec<String> = diagnostics
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            anyhow::anyhow!("Typst HTML compilation failed:\n{}", messages.join("\n"))
        })?;

    Ok(encode_body_html(&doc, &world, source))
}

/// Compile to body HTML and return structured diagnostics alongside.
pub fn compile_to_body_html_with_diags(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
) -> CompileResult {
    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, source.to_string());

    let result = typst::compile::<HtmlDocument>(&world);
    let mut diagnostics = extract_diagnostics(&result.warnings, &world);

    match result.output {
        Ok(doc) => {
            let html = encode_body_html(&doc, &world, source);
            CompileResult { html: Some(html), diagnostics, error_message: None }
        }
        Err(errors) => {
            diagnostics.extend(extract_diagnostics(&errors, &world));
            let messages: Vec<String> = errors.iter().map(|d| d.message.to_string()).collect();
            CompileResult {
                html: None,
                diagnostics,
                error_message: Some(messages.join("\n")),
            }
        }
    }
}

/// Compile to body-only HTML (no vault context).
pub fn compile_string_to_body_html(source: &str) -> Result<String> {
    let tmp = std::env::temp_dir().join("zetteltypsten_scratch");
    std::fs::create_dir_all(&tmp)?;
    compile_to_body_html(&tmp, "main.typ", source)
}

/// Compile to paged SVG output (for PDF-like preview).
/// Returns one SVG string per page.
pub fn compile_to_svg_pages(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
) -> Result<Vec<SvgPage>> {
    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, source.to_string());

    let doc: typst::layout::PagedDocument = typst::compile(&world)
        .output
        .map_err(|errs| {
            let msgs: Vec<String> = errs.iter().map(|e| e.message.to_string()).collect();
            anyhow::anyhow!("Typst compilation failed: {}", msgs.join("; "))
        })?;

    let mut pages = Vec::new();
    for (i, page) in doc.pages.iter().enumerate() {
        let svg = typst_svg::svg(page);
        let width = page.frame.width().to_pt();
        let height = page.frame.height().to_pt();
        pages.push(SvgPage {
            index: i,
            svg,
            width_pt: width,
            height_pt: height,
        });
    }

    Ok(pages)
}

/// A single rendered page as SVG.
pub struct SvgPage {
    pub index: usize,
    pub svg: String,
    pub width_pt: f64,
    pub height_pt: f64,
}

/// A single rendered page as a PNG data URI.
pub struct PngPage {
    pub data_uri: String,
    pub width_css: f64,
    pub height_css: f64,
}

/// Compile to rasterized PNG pages at the given DPI scale.
/// Returns base64 data URIs suitable for `<img src="...">`.
pub fn compile_to_png_pages(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
    pixel_per_pt: f32,
) -> Result<Vec<PngPage>> {
    use base64::Engine;

    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, source.to_string());

    let doc: typst::layout::PagedDocument = typst::compile(&world)
        .output
        .map_err(|errs| {
            let msgs: Vec<String> = errs.iter().map(|e| e.message.to_string()).collect();
            anyhow::anyhow!("Typst compilation failed: {}", msgs.join("; "))
        })?;

    let mut pages = Vec::new();
    for page in doc.pages.iter() {
        let pixmap = typst_render::render(page, pixel_per_pt);
        let width_px = pixmap.width();
        let height_px = pixmap.height();

        let png_data = pixmap
            .encode_png()
            .map_err(|e| anyhow::anyhow!("PNG encode failed: {e}"))?;

        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_data);
        let data_uri = format!("data:image/png;base64,{b64}");

        // CSS dimensions = pixel dimensions / scale factor
        let width_css = width_px as f64 / pixel_per_pt as f64;
        let height_css = height_px as f64 / pixel_per_pt as f64;

        pages.push(PngPage {
            data_uri,
            width_css,
            height_css,
        });
    }

    Ok(pages)
}

/// Compile to PDF and return as a base64-encoded data URI.
/// Suitable for embedding in `<iframe src="data:application/pdf;base64,...">`.
pub fn compile_to_pdf_base64(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
) -> Result<String> {
    use base64::Engine;
    use typst::foundations::Smart;

    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, source.to_string());

    let doc: typst::layout::PagedDocument = typst::compile(&world)
        .output
        .map_err(|errs| {
            let msgs: Vec<String> = errs.iter().map(|e| e.message.to_string()).collect();
            anyhow::anyhow!("Typst compilation failed: {}", msgs.join("; "))
        })?;

    let options = typst_pdf::PdfOptions {
        ident: Smart::Auto,
        timestamp: None,
        page_ranges: None,
        standards: typst_pdf::PdfStandards::default(),
        tagged: true,
    };

    let pdf_bytes = typst_pdf::pdf(&doc, &options)
        .map_err(|errs| {
            let msgs: Vec<String> = errs.iter().map(|e| e.message.to_string()).collect();
            anyhow::anyhow!("PDF export failed: {}", msgs.join("; "))
        })?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&pdf_bytes);
    Ok(b64)
}

/// Compile to body HTML with cross-note `@ref` support.
///
/// `label_map` maps label names to the note ID that declares them.
/// `current_note_labels` are labels declared in the current note (don't stub those).
///
/// Injects a preamble that:
/// 1. Creates invisible stub elements for all cross-note labels
/// 2. Adds a `#show ref` rule that renders cross-note refs as clickable links
pub fn compile_to_body_html_with_refs(
    vault_root: &Path,
    rel_path: &str,
    source: &str,
    label_map: &HashMap<String, String>,     // label → note_id string
    title_map: &HashMap<String, String>,     // note_id → display title
    label_text_map: &HashMap<String, String>, // label → display text for that label
) -> Result<String> {
    // Find labels declared in THIS note (don't stub those)
    let local_labels: Vec<String> = extract_local_labels(source);

    // Build preamble
    let preamble = build_ref_preamble(label_map, &local_labels, title_map, label_text_map);

    // The preamble goes before the user's source. We need to track the offset
    // so that data-span byte ranges still point to the right place in the
    // ORIGINAL source. We'll compile with the preamble prepended, but adjust
    // spans by subtracting the preamble length.
    let preamble_len = preamble.len();
    let full_source = format!("{preamble}{source}");

    let mut world = ZettelWorld::new(vault_root.to_path_buf(), rel_path);
    world.set_source(rel_path, full_source.clone());

    let doc: HtmlDocument = typst::compile(&world)
        .output
        .map_err(|diagnostics| {
            let messages: Vec<String> = diagnostics
                .iter()
                .map(|d| d.message.to_string())
                .collect();
            anyhow::anyhow!("Typst HTML compilation failed:\n{}", messages.join("\n"))
        })?;

    // Encode with the full source (including preamble) but adjust spans
    let html = encode_body_html_with_offset(&doc, &world, &full_source, source, preamble_len);

    // Post-process: rewrite zt-open: links so they don't trigger wry's navigation handler.
    // Change <a href="zt-open:note-id"> to <a href="#" data-zt-open="note-id">
    // This prevents the WebView from trying to navigate to a zt-open: URL.
    let mut result = String::with_capacity(html.len());
    let mut remaining = html.as_str();
    while let Some(pos) = remaining.find("href=\"zt-open:") {
        result.push_str(&remaining[..pos]);
        let after_prefix = &remaining[pos + 14..]; // skip 'href="zt-open:'
        if let Some(quote_end) = after_prefix.find('"') {
            let note_id = &after_prefix[..quote_end];
            result.push_str(&format!("href=\"#\" data-zt-open=\"{}\"", note_id));
            remaining = &after_prefix[quote_end + 1..];
        } else {
            result.push_str(&remaining[pos..pos + 14]);
            remaining = &remaining[pos + 14..];
        }
    }
    result.push_str(remaining);

    Ok(result)
}

/// Extract `<label>` declarations from source (simple scan).
pub fn extract_local_labels(source: &str) -> Vec<String> {
    let mut labels = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let start = i + 1;
            if start < bytes.len() && bytes[start].is_ascii_alphabetic() {
                let mut end = start;
                while end < bytes.len()
                    && (bytes[end].is_ascii_alphanumeric()
                        || bytes[end] == b'-'
                        || bytes[end] == b'_'
                        || bytes[end] == b':')
                {
                    end += 1;
                }
                if end < bytes.len() && bytes[end] == b'>' && end > start {
                    labels.push(source[start..end].to_string());
                    i = end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    labels
}

/// Build the Typst preamble for cross-note @ref support.
pub fn build_ref_preamble(
    label_map: &HashMap<String, String>,
    local_labels: &[String],
    title_map: &HashMap<String, String>,
    label_text_map: &HashMap<String, String>,
) -> String {
    let mut preamble = String::new();

    // Collect cross-note labels (not declared in this note), skip empty labels
    let cross_labels: Vec<(&String, &String)> = label_map
        .iter()
        .filter(|(label, _)| !label.is_empty() && !local_labels.contains(label))
        .collect();

    if cross_labels.is_empty() {
        return preamble;
    }

    // 1. Build a top-level dictionary of cross-note labels → note IDs
    preamble.push_str("#let __zt_cross_labels = (\n");
    for (label, note_id) in &cross_labels {
        let escaped_label = label.replace('"', "\\\"");
        let escaped_note = note_id.replace('"', "\\\"");
        let _ = writeln!(preamble, "  \"{escaped_label}\": \"{escaped_note}\",");
    }
    preamble.push_str(")\n\n");

    // 2. Build a title dictionary: note_id → display title (deduplicated)
    let unique_notes: std::collections::HashSet<&String> =
        cross_labels.iter().map(|(_, nid)| *nid).collect();
    preamble.push_str("#let __zt_titles = (\n");
    for note_id in &unique_notes {
        let escaped_note = note_id.replace('"', "\\\"");
        let title = title_map
            .get(*note_id)
            .cloned()
            .unwrap_or_else(|| note_id.replace('"', "\\\""));
        let escaped_title = title.replace('"', "\\\"");
        let _ = writeln!(preamble, "  \"{escaped_note}\": \"{escaped_title}\",");
    }
    preamble.push_str(")\n\n");

    // 3. Build a label→display text dictionary from label_text_map.
    preamble.push_str("#let __zt_label_text = (\n");
    for (label, _) in &cross_labels {
        let escaped_label = label.replace('"', "\\\"");
        let display = label_text_map
            .get(label.as_str())
            .cloned()
            .unwrap_or_else(|| label.to_string());
        let escaped_display = display.replace('"', "\\\"");
        let _ = writeln!(preamble, "  \"{escaped_label}\": \"{escaped_display}\",");
    }
    preamble.push_str(")\n\n");

    // 4. Top-level show rule — NOT inside a block, so it applies globally.
    //    repr(it.target) returns "<label>" so we strip the angle brackets.
    //    Each label gets its own display text and link includes the label for scrolling.
    preamble.push_str(r##"#show ref: it => {
  let raw = repr(it.target)
  let key = raw.slice(1, raw.len() - 1)
  if key in __zt_cross_labels {
    let note = __zt_cross_labels.at(key)
    let display = if key in __zt_label_text { __zt_label_text.at(key) } else { key }
    link("zt-open:" + note + "#" + key)[#display]
  } else {
    it
  }
}
"##);
    preamble.push('\n');

    // 4. Create invisible stub elements for cross-note labels
    //    so @ref doesn't error with "label not found"
    for (label, _) in &cross_labels {
        let _ = writeln!(preamble, "#metadata(none) <{label}>");
    }
    preamble.push('\n');

    preamble
}

// ---------------------------------------------------------------------------
// Annotated HTML encoder — walks the HtmlDocument DOM, adding data-span
// attributes and injecting CSS/JS for the live preview.
// ---------------------------------------------------------------------------

fn encode_annotated_html(doc: &HtmlDocument, world: &ZettelWorld, source: &str) -> String {
    let mut enc = HtmlEncoder {
        buf: String::with_capacity(4096),
        id_counter: 0,
        world,
        source: source.to_string(),
        span_offset: 0,
        original_source_len: source.len(),
    };
    // We write our own DOCTYPE + html wrapper so we can inject <style> and <script>.
    enc.buf.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
    enc.buf.push_str(r#"<meta charset="utf-8">"#);
    enc.buf.push('\n');
    enc.buf
        .push_str(r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#);
    enc.buf.push('\n');
    enc.buf.push_str("<style>\n");
    enc.buf.push_str(LIVE_PREVIEW_CSS);
    enc.buf.push_str("\n</style>\n");
    enc.buf.push_str("</head>\n<body>\n");

    // Render the body content (skip the <html>/<head>/<body> wrappers from Typst).
    let body_children = find_body_children(&doc.root);
    for node in body_children {
        enc.write_node(node);
    }

    enc.buf.push_str("\n<script>\n");
    enc.buf.push_str(LIVE_PREVIEW_JS);
    enc.buf.push_str("\n</script>\n");
    enc.buf.push_str("</body>\n</html>");
    enc.buf
}

/// Encode just the body content with inline style — no document wrapper.
fn encode_body_html(doc: &HtmlDocument, world: &ZettelWorld, source: &str) -> String {
    let mut enc = HtmlEncoder {
        buf: String::with_capacity(4096),
        id_counter: 0,
        world,
        source: source.to_string(),
        span_offset: 0,
        original_source_len: source.len(),
    };

    // Scoped CSS so it doesn't leak into the parent Dioxus page
    enc.buf.push_str("<style scoped>\n");
    enc.buf.push_str(BODY_SCOPED_CSS);
    enc.buf.push_str("\n</style>\n");
    enc.buf.push_str("<div class=\"zt-content\">\n");

    let body_children = find_body_children(&doc.root);
    for node in body_children {
        enc.write_node(node);
    }

    enc.buf.push_str("\n</div>");
    enc.buf
}

/// Encode body HTML with a preamble offset for span adjustment.
/// `full_source` includes the preamble, `original_source` is the user's text.
fn encode_body_html_with_offset(
    doc: &HtmlDocument,
    world: &ZettelWorld,
    full_source: &str,
    original_source: &str,
    preamble_len: usize,
) -> String {
    let mut enc = HtmlEncoder {
        buf: String::with_capacity(4096),
        id_counter: 0,
        world,
        source: full_source.to_string(),
        span_offset: preamble_len,
        original_source_len: original_source.len(),
    };

    enc.buf.push_str("<style scoped>\n");
    enc.buf.push_str(BODY_SCOPED_CSS);
    enc.buf.push_str("\n</style>\n");
    enc.buf.push_str("<div class=\"zt-content\">\n");

    let body_children = find_body_children(&doc.root);
    for node in body_children {
        enc.write_node(node);
    }

    enc.buf.push_str("\n</div>");
    enc.buf
}

/// CSS scoped to the .zt-content container (for embedding in Dioxus).
const BODY_SCOPED_CSS: &str = r#"
.zt-content {
    color: #cdd6f4;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, sans-serif;
    font-size: 15px;
    line-height: 1.7;
    padding: 24px 32px;
    -webkit-font-smoothing: antialiased;
}
.zt-content h1, .zt-content h2, .zt-content h3, .zt-content h4, .zt-content h5, .zt-content h6 {
    color: #89b4fa;
    margin: 1.2em 0 0.4em 0;
    line-height: 1.3;
}
.zt-content h1 { font-size: 1.8em; }
.zt-content h2 { font-size: 1.4em; }
.zt-content h3 { font-size: 1.2em; }
.zt-content p { margin: 0.6em 0; }
.zt-content a { color: #89b4fa; text-decoration: underline; }
.zt-content a:hover { color: #b4befe; }
.zt-content code {
    background: #313244;
    padding: 2px 6px;
    border-radius: 4px;
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", monospace;
    font-size: 0.9em;
}
.zt-content pre {
    background: #181825;
    padding: 12px 16px;
    border-radius: 8px;
    overflow-x: auto;
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", monospace;
    font-size: 0.9em;
    line-height: 1.5;
}
.zt-content pre code { background: none; padding: 0; }
.zt-content strong, .zt-content b { color: #f5c2e7; }
.zt-content em, .zt-content i { color: #f5e0dc; }
.zt-content ul, .zt-content ol { padding-left: 24px; margin: 0.5em 0; }
.zt-content li { margin: 0.2em 0; }
.zt-content blockquote {
    border-left: 3px solid #585b70;
    margin: 0.6em 0;
    padding: 0.2em 0 0.2em 16px;
    color: #a6adc8;
}
.zt-content hr { border: none; border-top: 1px solid #313244; margin: 1.5em 0; }
.zt-content table { border-collapse: collapse; margin: 1em 0; }
.zt-content th, .zt-content td {
    border: 1px solid #313244;
    padding: 6px 12px;
    text-align: left;
}
.zt-content th { background: #181825; font-weight: 600; }
.zt-content svg { max-width: 100%; }
.zt-content [data-span] { cursor: text; border-radius: 3px; transition: outline 0.15s; }
.zt-content [data-span]:hover { outline: 1px solid rgba(137, 180, 250, 0.25); }
"#;

/// Walk past <html><head>...</head><body> wrappers to get the actual content.
fn find_body_children(root: &typst_html::HtmlElement) -> &[typst_html::HtmlNode] {
    use typst_html::HtmlTag;
    let tag_html = HtmlTag::constant("html");
    let tag_body = HtmlTag::constant("body");
    // root is usually <html>, look for <body>
    if root.tag == tag_html {
        for child in &root.children {
            if let typst_html::HtmlNode::Element(el) = child {
                if el.tag == tag_body {
                    return &el.children;
                }
            }
        }
    }
    // Fallback: return root's children directly
    &root.children
}

struct HtmlEncoder<'a> {
    buf: String,
    id_counter: usize,
    world: &'a ZettelWorld,
    /// Cached source text for span expansion.
    source: String,
    /// Byte offset to subtract from spans (to account for injected preamble).
    span_offset: usize,
    /// Original source length (to skip spans that fall in the preamble).
    original_source_len: usize,
}

impl<'a> HtmlEncoder<'a> {
    fn source_text(&self) -> &str {
        &self.source
    }

    fn next_id(&mut self) -> String {
        let id = self.id_counter;
        self.id_counter += 1;
        format!("zt-{id}")
    }

    fn resolve_span(&self, span: typst_syntax::Span) -> Option<std::ops::Range<usize>> {
        if span.is_detached() {
            return None;
        }
        let range = self.world.range(span)?;

        // Skip spans that fall entirely within the preamble
        if range.end <= self.span_offset {
            return None;
        }

        // Adjust for preamble offset
        let start = range.start.saturating_sub(self.span_offset);
        let end = range.end.saturating_sub(self.span_offset);

        // Clamp to original source length
        if start >= self.original_source_len {
            return None;
        }
        let end = end.min(self.original_source_len);

        Some(self.expand_to_statement(start..end))
    }

    /// Expand a byte range to cover complete lines and enclosing block constructs.
    /// If the range is inside a `#for`, `#while`, `#if`, etc., expand to include
    /// the full statement so click-to-edit shows the whole block.
    fn expand_to_statement(&self, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
        let source = self.source_text();
        let bytes = source.as_bytes();
        let mut start = range.start;
        let mut end = range.end;

        // Expand to full lines
        while start > 0 && bytes[start - 1] != b'\n' {
            start -= 1;
        }
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }

        // Expand upward for #for / #while / #if / #let / #show / #set blocks
        for _ in 0..20 {
            if start == 0 {
                break;
            }
            // Find the previous line
            let prev_end = start - 1; // skip \n
            let mut prev_start = prev_end;
            while prev_start > 0 && bytes[prev_start - 1] != b'\n' {
                prev_start -= 1;
            }
            let prev_line = source[prev_start..prev_end].trim_start();
            if prev_line.starts_with("#for ")
                || prev_line.starts_with("#while ")
                || prev_line.starts_with("#if ")
                || prev_line.starts_with("#else")
                || prev_line.starts_with("#let ")
                || prev_line.starts_with("#show ")
                || prev_line.starts_with("#set ")
                || prev_line.ends_with('{')
                || prev_line.ends_with('[')
            {
                start = prev_start;
            } else {
                break;
            }
        }

        // Expand downward to close any open brackets
        let mut depth: i32 = 0;
        for b in source[start..end].bytes() {
            match b {
                b'{' | b'[' | b'(' => depth += 1,
                b'}' | b']' | b')' => depth -= 1,
                _ => {}
            }
        }
        if depth > 0 {
            let mut pos = end;
            while pos < bytes.len() && depth > 0 {
                match bytes[pos] {
                    b'{' | b'[' | b'(' => depth += 1,
                    b'}' | b']' | b')' => depth -= 1,
                    _ => {}
                }
                pos += 1;
            }
            // Extend to end of line
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            end = pos;
        }

        start..end
    }

    fn write_node(&mut self, node: &typst_html::HtmlNode) {
        use typst_html::HtmlNode;
        match node {
            HtmlNode::Tag(_) => {} // introspection marker, skip
            HtmlNode::Text(text, _span) => {
                html_escape_text(&mut self.buf, text);
            }
            HtmlNode::Element(el) => self.write_element(el),
            HtmlNode::Frame(frame) => self.write_frame(frame),
        }
    }

    fn write_element(&mut self, el: &typst_html::HtmlElement) {
        let id = self.next_id();
        let tag_str = el.tag.resolve();

        self.buf.push('<');
        self.buf.push_str(&tag_str);

        // Add our data attributes
        let _ = write!(self.buf, r#" id="{id}""#);
        if let Some(range) = self.resolve_span(el.span) {
            let _ = write!(self.buf, r#" data-span="{}:{}""#, range.start, range.end);
        }

        // Original attributes
        for (attr, value) in &el.attrs.0 {
            self.buf.push(' ');
            self.buf.push_str(&attr.resolve());
            self.buf.push_str("=\"");
            html_escape_attr(&mut self.buf, value);
            self.buf.push('"');
        }

        self.buf.push('>');

        if is_void_tag(&tag_str) {
            return;
        }

        for child in &el.children {
            self.write_node(child);
        }

        self.buf.push_str("</");
        self.buf.push_str(&tag_str);
        self.buf.push('>');
    }

    fn write_frame(&mut self, frame: &typst_html::HtmlFrame) {
        let svg = typst_svg::svg_frame(&frame.inner)
            .replace("<svg class", "<svg style=\"overflow: visible;\" class");
        self.buf.push_str(&svg);
    }
}

/// Check if an HTML tag is a void element (self-closing, no children).
fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input"
        | "link" | "meta" | "param" | "source" | "track" | "wbr"
    )
}

fn html_escape_text(buf: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => buf.push_str("&amp;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            _ => buf.push(c),
        }
    }
}

fn html_escape_attr(buf: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => buf.push_str("&amp;"),
            '"' => buf.push_str("&quot;"),
            '<' => buf.push_str("&lt;"),
            '>' => buf.push_str("&gt;"),
            _ => buf.push(c),
        }
    }
}

/// CSS for the live preview, matching the app's Catppuccin Mocha theme.
const LIVE_PREVIEW_CSS: &str = r#"
* { box-sizing: border-box; }
body {
    background: #1e1e2e;
    color: #cdd6f4;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, sans-serif;
    font-size: 15px;
    line-height: 1.7;
    padding: 24px 32px;
    margin: 0;
    -webkit-font-smoothing: antialiased;
}
h1, h2, h3, h4, h5, h6 {
    color: #89b4fa;
    margin: 1.2em 0 0.4em 0;
    line-height: 1.3;
}
h1 { font-size: 1.8em; }
h2 { font-size: 1.4em; }
h3 { font-size: 1.2em; }
p { margin: 0.6em 0; }
a { color: #89b4fa; text-decoration: underline; }
a:hover { color: #b4befe; }
code {
    background: #313244;
    padding: 2px 6px;
    border-radius: 4px;
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", monospace;
    font-size: 0.9em;
}
pre {
    background: #181825;
    padding: 12px 16px;
    border-radius: 8px;
    overflow-x: auto;
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", monospace;
    font-size: 0.9em;
    line-height: 1.5;
}
pre code { background: none; padding: 0; }
strong, b { color: #f5c2e7; }
em, i { color: #f5e0dc; }
ul, ol { padding-left: 24px; margin: 0.5em 0; }
li { margin: 0.2em 0; }
blockquote {
    border-left: 3px solid #585b70;
    margin: 0.6em 0;
    padding: 0.2em 0 0.2em 16px;
    color: #a6adc8;
}
hr { border: none; border-top: 1px solid #313244; margin: 1.5em 0; }
table { border-collapse: collapse; margin: 1em 0; }
th, td {
    border: 1px solid #313244;
    padding: 6px 12px;
    text-align: left;
}
th { background: #181825; font-weight: 600; }
svg { max-width: 100%; }

/* Click-to-edit highlighting */
[data-span] { cursor: text; border-radius: 3px; transition: outline 0.15s; }
[data-span]:hover { outline: 1px solid rgba(137, 180, 250, 0.25); }

/* Active editing textarea */
textarea.zt-editor {
    width: 100%;
    background: #181825;
    color: #a6e3a1;
    font-family: "SF Mono", "Fira Code", "JetBrains Mono", monospace;
    font-size: 14px;
    line-height: 1.6;
    border: 1px solid #89b4fa;
    border-radius: 6px;
    padding: 8px 12px;
    resize: vertical;
    outline: none;
    display: block;
    margin: 4px 0;
}
textarea.zt-editor:focus { border-color: #b4befe; box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.2); }
"#;

/// JavaScript for click-to-edit interaction.
/// Communicates with Rust via `window.ipc.postMessage()`.
const LIVE_PREVIEW_JS: &str = r#"
(function() {
    let activeEditor = null;

    document.addEventListener('click', function(e) {
        // Don't handle clicks on the textarea itself
        if (e.target.tagName === 'TEXTAREA') return;

        // If there's an active editor, commit it first
        if (activeEditor) {
            commitEdit();
        }

        // Find the nearest element with a data-span
        const el = e.target.closest('[data-span]');
        if (!el) return;

        const span = el.getAttribute('data-span');
        const id = el.id;

        // Send click to Rust
        window.ipc.postMessage(JSON.stringify({
            type: 'click',
            span: span,
            elementId: id
        }));
    });

    // Called from Rust to replace an element with an editable textarea
    window.replaceWithEditor = function(elementId, source) {
        const el = document.getElementById(elementId);
        if (!el) return;

        const textarea = document.createElement('textarea');
        textarea.value = source;
        textarea.className = 'zt-editor';
        textarea.dataset.originalId = elementId;
        textarea.dataset.span = el.dataset.span;
        textarea.dataset.originalTag = el.tagName;

        // Store the original element
        textarea.dataset.originalHtml = el.outerHTML;

        // Size the textarea to fit content
        el.replaceWith(textarea);
        textarea.style.height = Math.max(60, textarea.scrollHeight + 4) + 'px';
        textarea.focus();

        // Auto-resize on input
        textarea.addEventListener('input', function() {
            this.style.height = 'auto';
            this.style.height = Math.max(60, this.scrollHeight + 4) + 'px';
        });

        activeEditor = textarea;
    };

    function commitEdit() {
        if (!activeEditor) return;
        const textarea = activeEditor;
        activeEditor = null;

        window.ipc.postMessage(JSON.stringify({
            type: 'commit',
            span: textarea.dataset.span,
            newText: textarea.value,
            elementId: textarea.dataset.originalId
        }));
    }

    // Commit on Escape key
    document.addEventListener('keydown', function(e) {
        if (e.key === 'Escape' && activeEditor) {
            commitEdit();
            e.preventDefault();
        }
        // Forward Cmd+S to Rust
        if ((e.metaKey || e.ctrlKey) && e.key === 's') {
            window.ipc.postMessage(JSON.stringify({ type: 'save' }));
            e.preventDefault();
        }
        // Forward Cmd+W to Rust
        if ((e.metaKey || e.ctrlKey) && e.key === 'w') {
            window.ipc.postMessage(JSON.stringify({ type: 'close_tab' }));
            e.preventDefault();
        }
    });
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_to_html_basic() {
        let html = compile_string_to_html("= Hello\n\nWorld").unwrap();
        assert!(html.contains("Hello"));
        assert!(html.contains("World"));
        assert!(html.contains("data-span"));
        assert!(html.contains("zt-editor")); // CSS class in stylesheet
    }

    #[test]
    fn cross_note_ref_generates_data_zt_open() {
        let vault = std::env::temp_dir().join("zt_test_refs");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(vault.join("note-a.typ"), "= My Title <Hello>\n\nNote A.\n").unwrap();
        std::fs::write(vault.join("note-b.typ"), "See @Hello for details.\n").unwrap();

        // label_map: Hello -> note-a
        let mut label_map = HashMap::new();
        label_map.insert("Hello".to_string(), "note-a".to_string());

        // title_map: note-a -> My Title
        let mut title_map = HashMap::new();
        title_map.insert("note-a".to_string(), "My Title".to_string());

        // label_text_map: Hello -> My Title
        let mut label_text_map = HashMap::new();
        label_text_map.insert("Hello".to_string(), "My Title".to_string());

        let source = std::fs::read_to_string(vault.join("note-b.typ")).unwrap();
        let html = compile_to_body_html_with_refs(
            &vault, "note-b.typ", &source, &label_map, &title_map, &label_text_map,
        ).unwrap();

        println!("HTML output:\n{}", html);

        // Should contain data-zt-open with note-id#label, NOT href="zt-open:"
        assert!(html.contains("data-zt-open=\"note-a#Hello\""),
            "Expected data-zt-open attribute with label fragment, got:\n{}", html);
        assert!(!html.contains("href=\"zt-open:"),
            "Should not have zt-open: href, got:\n{}", html);
        // Should show the title, not the label name
        assert!(html.contains("My Title"),
            "Expected title 'My Title' in link text, got:\n{}", html);

        // Cleanup
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn svg_output_check() {
        let vault = std::env::temp_dir().join("zt_test_svg");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(vault.join("test.typ"), "= My Title\n\nThis is note A.\n").unwrap();

        let source = std::fs::read_to_string(vault.join("test.typ")).unwrap();
        let pages = compile_to_svg_pages(&vault, "test.typ", &source).unwrap();
        assert!(!pages.is_empty());

        let svg = &pages[0].svg;
        std::fs::write("/tmp/zt_test_output.svg", svg).unwrap();
        println!("SVG length: {} bytes", svg.len());
        println!("SVG (first 2000 chars):\n{}", &svg[..svg.len().min(2000)]);

        let _ = std::fs::remove_dir_all(&vault);
    }
}
