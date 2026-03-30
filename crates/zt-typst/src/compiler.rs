use crate::world::ZettelWorld;
use anyhow::Result;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use typst_html::HtmlDocument;

// =============================================================================
// Book HTML export — compile with cross-note @ref preamble
// =============================================================================

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

    // 5. Create invisible stub elements for cross-note labels
    //    so @ref doesn't error with "label not found"
    for (label, _) in &cross_labels {
        let _ = writeln!(preamble, "#metadata(none) <{label}>");
    }
    preamble.push('\n');

    preamble
}

// =============================================================================
// HTML encoder — walks HtmlDocument DOM for book export
// =============================================================================

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

/// CSS scoped to the .zt-content container (for book HTML export).
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
            let prev_end = start - 1;
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

        let _ = write!(self.buf, r#" id="{id}""#);
        if let Some(range) = self.resolve_span(el.span) {
            let _ = write!(self.buf, r#" data-span="{}:{}""#, range.start, range.end);
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_note_ref_generates_data_zt_open() {
        let vault = std::env::temp_dir().join("zt_test_refs");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(vault.join("note-a.typ"), "= My Title <Hello>\n\nNote A.\n").unwrap();
        std::fs::write(vault.join("note-b.typ"), "See @Hello for details.\n").unwrap();

        let mut label_map = HashMap::new();
        label_map.insert("Hello".to_string(), "note-a".to_string());

        let mut title_map = HashMap::new();
        title_map.insert("note-a".to_string(), "My Title".to_string());

        let mut label_text_map = HashMap::new();
        label_text_map.insert("Hello".to_string(), "My Title".to_string());

        let source = std::fs::read_to_string(vault.join("note-b.typ")).unwrap();
        let html = compile_to_body_html_with_refs(
            &vault, "note-b.typ", &source, &label_map, &title_map, &label_text_map,
        ).unwrap();

        assert!(html.contains("data-zt-open=\"note-a#Hello\""),
            "Expected data-zt-open attribute with label fragment, got:\n{}", html);
        assert!(!html.contains("href=\"zt-open:"),
            "Should not have zt-open: href, got:\n{}", html);
        assert!(html.contains("My Title"),
            "Expected title 'My Title' in link text, got:\n{}", html);

        let _ = std::fs::remove_dir_all(&vault);
    }
}
