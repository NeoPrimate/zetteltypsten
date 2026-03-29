//! Vault content extractor — parses Typst source to pull out headings, tags,
//! labels, links, and cross-note `@ref`s.
//!
//! Most extractions are backed by [`typst_syntax::parse`] so they work on the
//! full AST rather than fragile ad-hoc string patterns.  The one exception is
//! `#link(…)` extraction, which uses a fast byte-scan (the AST approach for
//! arbitrary nested function calls would be equivalent in correctness but
//! substantially more code for minimal gain).

use typst_syntax::{parse, SyntaxKind, SyntaxNode};
use zt_core::link::UnresolvedLink;
use zt_core::tag::Tag;

// ── AST walk helpers ─────────────────────────────────────────────────────────

/// Depth-first walk over every node in the syntax tree.
fn walk(node: &SyntaxNode, f: &mut impl FnMut(&SyntaxNode)) {
    f(node);
    for child in node.children() {
        walk(child, f);
    }
}

/// Collect the visible source text of `node`'s subtree, skipping any subtrees
/// whose root kind is listed in `skip`.
fn collect_text_skip(node: &SyntaxNode, skip: &[SyntaxKind]) -> String {
    if skip.contains(&node.kind()) {
        return String::new();
    }
    // Leaf nodes carry their text directly; composite nodes recurse.
    let own_text = node.text();
    if !own_text.is_empty() {
        return own_text.to_string();
    }
    node.children()
        .map(|child| collect_text_skip(child, skip))
        .collect()
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Extract heading levels and text from Typst source.
///
/// Returns `(depth, heading_text)` pairs where `depth` is the number of `=`
/// markers (1 = top-level, 2 = section, etc.).  Trailing `<label>` markup is
/// stripped from each heading text.
pub fn extract_headings(source: &str) -> Vec<(usize, String)> {
    let root = parse(source);
    let mut headings = Vec::new();

    walk(&root, &mut |node| {
        if node.kind() != SyntaxKind::Heading {
            return;
        }

        // The heading marker (===…) tells us the depth.
        let depth = node
            .children()
            .find(|c| c.kind() == SyntaxKind::HeadingMarker)
            .map(|m| m.text().len())
            .unwrap_or(0);

        if depth == 0 {
            return;
        }

        // Strip the marker, surrounding space, and any trailing labels.
        let text = collect_text_skip(
            node,
            &[SyntaxKind::HeadingMarker, SyntaxKind::Label, SyntaxKind::Space],
        )
        .trim()
        .to_string();

        if !text.is_empty() {
            headings.push((depth, text));
        }
    });

    headings
}

/// Extract the note title — the text of the first top-level heading.
///
/// Returns `None` if there is no `= Heading` in the source.
pub fn extract_title(source: &str) -> Option<String> {
    extract_headings(source)
        .into_iter()
        .find(|(depth, _)| *depth == 1)
        .map(|(_, text)| text)
}

/// Extract all `<label>` declarations from Typst source.
///
/// Skips tag-related labels (`<tags>`, `<tag>`, `<tag-*>`) that are used
/// solely to anchor `#metadata(…)` tag declarations.
pub fn extract_labels(source: &str) -> Vec<String> {
    let root = parse(source);
    let mut labels = Vec::new();

    walk(&root, &mut |node| {
        if node.kind() != SyntaxKind::Label {
            return;
        }
        // A Label node's text is the full `<name>` including angle brackets.
        let raw = node.text();
        let name = raw.trim_start_matches('<').trim_end_matches('>');
        if name.is_empty() {
            return;
        }
        // Skip tag-sentinel labels.
        if matches!(name, "tags" | "tag") || name.starts_with("tag-") {
            return;
        }
        labels.push(name.to_string());
    });

    labels
}

/// Extract labels with their associated display text.
///
/// For `= My Title <hello>` → `("hello", "My Title")`.
/// For a standalone `<foo>` not preceded by visible text → `("foo", "foo")`.
pub fn extract_labels_with_text(source: &str) -> Vec<(String, String)> {
    let root = parse(source);
    let mut results = Vec::new();

    walk(&root, &mut |node| {
        if node.kind() != SyntaxKind::Label {
            return;
        }
        let raw = node.text();
        let name = raw.trim_start_matches('<').trim_end_matches('>');
        if name.is_empty() {
            return;
        }
        if matches!(name, "tags" | "tag") || name.starts_with("tag-") {
            return;
        }

        // Walk upward through parents to find the enclosing heading, if any.
        // Since `typst_syntax` doesn't provide parent pointers, we re-scan the
        // source line containing this label for a heading prefix.
        let label_byte = find_label_byte_offset(source, raw);
        let display = label_byte
            .and_then(|offset| enclosing_heading_text(source, offset))
            .unwrap_or_else(|| name.to_string());

        results.push((name.to_string(), display));
    });

    results
}

/// Locate the byte offset of the first occurrence of `raw_label` (e.g. `<foo>`)
/// in `source`.
fn find_label_byte_offset(source: &str, raw_label: &str) -> Option<usize> {
    source.find(raw_label)
}

/// Given a byte offset that falls inside a label, return the heading text of
/// the line that contains it, or `None` if the line is not a heading.
fn enclosing_heading_text(source: &str, byte_offset: usize) -> Option<String> {
    let offset = byte_offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    let line = source[line_start..line_end].trim();

    // Strip leading `=` markers
    let depth = line.chars().take_while(|&c| c == '=').count();
    if depth == 0 {
        return None;
    }
    let after_markers = line[depth..].trim_start();

    // Strip trailing `<label>` from the heading text
    let heading_text = if let Some(label_start) = after_markers.rfind('<') {
        if after_markers.ends_with('>') {
            after_markers[..label_start].trim()
        } else {
            after_markers
        }
    } else {
        after_markers
    };

    if heading_text.is_empty() {
        None
    } else {
        Some(heading_text.to_string())
    }
}

/// Extract `@reference` labels from Typst source (cross-note refs).
///
/// Returns label names referenced (without the leading `@`).  Only top-level
/// markup refs are captured — refs inside code blocks are excluded because the
/// AST correctly distinguishes markup from code contexts.
pub fn extract_refs(source: &str) -> Vec<String> {
    let root = parse(source);
    let mut refs = Vec::new();

    walk(&root, &mut |node| {
        if node.kind() != SyntaxKind::Ref {
            return;
        }
        // The Ref composite has a RefMarker child whose text is `@label-name`.
        if let Some(marker) = node.children().find(|c| c.kind() == SyntaxKind::RefMarker) {
            let name = marker.text().trim_start_matches('@');
            if !name.is_empty() {
                refs.push(name.to_string());
            }
        }
    });

    refs
}

/// Extract tags from native Typst `#metadata()` patterns.
///
/// **Pattern 1** (grouped):  `#metadata(("rust", "gui")) <tags>`
/// **Pattern 2** (individual): `#metadata("rust") <tag-rust>`
///
/// Both are invisible in rendered output and require no imports.
///
/// Implementation note: full function-call AST traversal for arbitrary nested
/// args is complex; a line-based scan is equivalent in correctness because
/// these declarations are always written as single-line statements.
pub fn extract_tags(source: &str) -> Vec<Tag> {
    let mut tags = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        // Pattern 1: #metadata(("tag1", "tag2", ...)) <tags>
        if trimmed.starts_with("#metadata((") && trimmed.contains("<tags>") {
            if let Some(inner_start) = trimmed.find("((") {
                if let Some(inner_end) = trimmed.find("))") {
                    let inner = &trimmed[inner_start + 2..inner_end];
                    extract_quoted_strings(inner, &mut tags);
                }
            }
            continue;
        }

        // Pattern 2: #metadata("tagname") <tag-tagname>  OR  <tag>
        if trimmed.starts_with("#metadata(\"")
            && (trimmed.contains("<tag-") || trimmed.contains(" <tag>"))
        {
            let after_quote = &trimmed[11..]; // skip '#metadata("'
            if let Some(close_quote) = after_quote.find('"') {
                let tag_str = &after_quote[..close_quote];
                if !tag_str.is_empty() {
                    tags.push(Tag::new(tag_str));
                }
            }
        }
    }

    tags
}

/// Helper: extract all quoted strings from a comma-separated list into `out`.
fn extract_quoted_strings(input: &str, out: &mut Vec<Tag>) {
    let mut search = 0;
    while search < input.len() {
        if let Some(q_start) = input[search..].find('"') {
            let abs_start = search + q_start + 1;
            if let Some(q_end) = input[abs_start..].find('"') {
                let tag_str = &input[abs_start..abs_start + q_end];
                if !tag_str.is_empty() {
                    out.push(Tag::new(tag_str));
                }
                search = abs_start + q_end + 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
}

/// Extract all `#link("target")` and `#link("target")[display text]` calls
/// from Typst source text.
///
/// HTTP URLs and other non-note link targets are filtered out.  This function
/// uses a fast byte-scan rather than the full AST because deeply nested
/// function-call traversal would be substantially more code for identical
/// correctness on well-formed Typst source.
pub fn extract_links(source: &str) -> Vec<UnresolvedLink> {
    let mut links = Vec::new();
    let pattern = "#link(";

    let mut search_from = 0;
    while let Some(start) = source[search_from..].find(pattern) {
        let abs_start = search_from + start;
        let after_paren = abs_start + pattern.len();

        if let Some(link) = parse_link_call(source, abs_start, after_paren) {
            // Only include note links — skip URLs and anchors.
            if !link.raw_target.contains("://")
                && !link.raw_target.starts_with("http")
                && !link.raw_target.starts_with("mailto:")
                && !link.raw_target.starts_with('#')
            {
                links.push(link);
            }
        }

        search_from = after_paren;
    }

    links
}

/// Parse a single `#link("target")[display]` call.
fn parse_link_call(
    source: &str,
    call_start: usize,
    after_paren: usize,
) -> Option<UnresolvedLink> {
    let rest = &source[after_paren..];
    let rest = rest.trim_start();

    if !rest.starts_with('"') {
        return None;
    }

    let target_start = 1;
    let target_end = rest[target_start..].find('"')?;
    let target = &rest[target_start..target_start + target_end];

    if target.is_empty() {
        return None;
    }

    let closing_paren = source[call_start..].find(')')?;
    let span_end = call_start + closing_paren + 1;

    let after_close = &source[span_end..];
    let display = if after_close.starts_with('[') {
        if let Some(bracket_end) = after_close[1..].find(']') {
            let text = &after_close[1..1 + bracket_end];
            if !text.is_empty() { Some(text.to_string()) } else { None }
        } else {
            None
        }
    } else {
        None
    };

    let final_end = if display.is_some() {
        let bracket_close = source[span_end..].find(']').unwrap_or(0);
        span_end + bracket_close + 1
    } else {
        span_end
    };

    Some(UnresolvedLink {
        raw_target: target.to_string(),
        display_text: display,
        span: call_start..final_end,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_basic() {
        let source = "= Top Level\n== Section\n=== Sub <lbl>\n";
        let headings = extract_headings(source);
        assert_eq!(headings.len(), 3);
        assert_eq!(headings[0], (1, "Top Level".to_string()));
        assert_eq!(headings[1], (2, "Section".to_string()));
        assert_eq!(headings[2], (3, "Sub".to_string()));
    }

    #[test]
    fn title_from_first_heading() {
        assert_eq!(extract_title("= My Note\n\nContent"), Some("My Note".into()));
        assert_eq!(extract_title("No heading here"), None);
    }

    #[test]
    fn labels_ast() {
        let source = "= Title <hello>\n#metadata((\"rust\")) <tags>\n#metadata(\"gui\") <tag-gui>\n<standalone>";
        let labels = extract_labels(source);
        // Only `hello` and `standalone` — `tags` and `tag-gui` are filtered.
        assert!(labels.contains(&"hello".to_string()));
        assert!(labels.contains(&"standalone".to_string()));
        assert!(!labels.iter().any(|l| l == "tags" || l.starts_with("tag-")));
    }

    #[test]
    fn labels_with_text_heading() {
        let source = "= My Title <hello>\n";
        let pairs = extract_labels_with_text(source);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], ("hello".to_string(), "My Title".to_string()));
    }

    #[test]
    fn refs_ast() {
        let source = "See @my-section for details.\nAlso @other.";
        let refs = extract_refs(source);
        assert!(refs.contains(&"my-section".to_string()));
        assert!(refs.contains(&"other".to_string()));
    }

    #[test]
    fn extract_simple_link() {
        let source = r#"See #link("notes/rust-gui") for details."#;
        let links = extract_links(source);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].raw_target, "notes/rust-gui");
        assert_eq!(links[0].display_text, None);
    }

    #[test]
    fn extract_link_with_display() {
        let source = r#"Check #link("projects/zetteltypsten")[the editor] out."#;
        let links = extract_links(source);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].raw_target, "projects/zetteltypsten");
        assert_eq!(links[0].display_text.as_deref(), Some("the editor"));
    }

    #[test]
    fn ignore_url_links() {
        let source = r#"See #link("https://example.com")[example] and #link("notes/foo")."#;
        let links = extract_links(source);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].raw_target, "notes/foo");
    }

    #[test]
    fn extract_grouped_metadata_tags() {
        let source = r#"#metadata(("rust", "gui", "project/active")) <tags>"#;
        let tags = extract_tags(source);
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].as_str(), "rust");
        assert_eq!(tags[1].as_str(), "gui");
        assert_eq!(tags[2].as_str(), "project/active");
    }

    #[test]
    fn extract_individual_metadata_tags() {
        let source = "#metadata(\"rust\") <tag-rust>\n#metadata(\"gui\") <tag-gui>";
        let tags = extract_tags(source);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].as_str(), "rust");
        assert_eq!(tags[1].as_str(), "gui");
    }
}
