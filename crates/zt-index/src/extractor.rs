use zt_core::link::UnresolvedLink;
use zt_core::tag::Tag;

/// Extract all `#link("target")` and `#link("target")[display text]` calls
/// from Typst source text.
///
/// Only links whose target looks like a note path (no protocol, no extension)
/// are captured — HTTP URLs and other schemes are ignored.
///
/// This is the fast path — runs on every save without needing to parse the AST.
pub fn extract_links(source: &str) -> Vec<UnresolvedLink> {
    let mut links = Vec::new();
    let pattern = "#link(";

    let mut search_from = 0;
    while let Some(start) = source[search_from..].find(pattern) {
        let abs_start = search_from + start;
        let after_paren = abs_start + pattern.len();

        if let Some(link) = parse_link_call(source, abs_start, after_paren) {
            // Only include note links — skip URLs and anchors
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

    // Expect opening quote for target
    if !rest.starts_with('"') {
        return None;
    }

    // Find closing quote for target
    let target_start = 1;
    let target_end = rest[target_start..].find('"')?;
    let target = &rest[target_start..target_start + target_end];

    if target.is_empty() {
        return None;
    }

    // Find closing paren
    let closing_paren = source[call_start..].find(')')?;
    let span_end = call_start + closing_paren + 1;

    // Look for optional display text in square brackets: )[display text]
    let after_close = &source[span_end..];
    let display = if after_close.starts_with('[') {
        if let Some(bracket_end) = after_close[1..].find(']') {
            let display_text = &after_close[1..1 + bracket_end];
            if !display_text.is_empty() {
                Some(display_text.to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Extend span to include [display] if present
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

/// Extract tags from native Typst `#metadata()` patterns:
///
/// **Pattern 1** (grouped):  `#metadata(("rust", "gui")) <tags>`
/// **Pattern 2** (individual): `#metadata("rust") <tag-rust>`
///
/// Both are invisible in rendered output and require no imports.
pub fn extract_tags(source: &str) -> Vec<Tag> {
    let mut tags = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();

        // Pattern 1: #metadata(("tag1", "tag2", ...)) <tags>
        if trimmed.starts_with("#metadata((") && trimmed.contains("<tags>") {
            // Extract all quoted strings from the inner array
            if let Some(inner_start) = trimmed.find("((") {
                if let Some(inner_end) = trimmed.find("))") {
                    let inner = &trimmed[inner_start + 2..inner_end];
                    extract_quoted_strings(inner, &mut tags);
                }
            }
            continue;
        }

        // Pattern 2: #metadata("tagname") <tag-tagname>
        if trimmed.starts_with("#metadata(\"") && trimmed.contains("<tag-") {
            // Extract the single quoted string
            let after_quote = &trimmed[11..]; // skip '#metadata("'
            if let Some(close_quote) = after_quote.find('"') {
                let tag_str = &after_quote[..close_quote];
                if !tag_str.is_empty() {
                    tags.push(Tag::new(tag_str));
                }
            }
            continue;
        }

        // Pattern 3: #metadata("tagname") <tag>  (bare <tag> label)
        if trimmed.starts_with("#metadata(\"") && trimmed.contains(" <tag>") {
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

/// Helper: extract all quoted strings from a comma-separated list.
fn extract_quoted_strings(input: &str, tags: &mut Vec<Tag>) {
    let mut search = 0;
    let bytes = input.as_bytes();
    while search < bytes.len() {
        if let Some(q_start) = input[search..].find('"') {
            let abs_start = search + q_start + 1;
            if let Some(q_end) = input[abs_start..].find('"') {
                let tag_str = &input[abs_start..abs_start + q_end];
                if !tag_str.is_empty() {
                    tags.push(Tag::new(tag_str));
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

/// Extract all `<label>` declarations from Typst source.
/// Labels look like `<identifier>` where identifier is [a-zA-Z][\w-]*.
/// They appear after headings, equations, figures, or standalone.
pub fn extract_labels(source: &str) -> Vec<String> {
    let mut labels = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let start = i + 1;
            // Must start with a letter
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
                    let label = &source[start..end];
                    // Skip HTML-like tags (common ones) and tag markers
                    if !matches!(
                        label,
                        "p" | "div" | "span" | "br" | "hr" | "img" | "a" | "ul"
                            | "ol" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                            | "table" | "tr" | "td" | "th" | "pre" | "code" | "em"
                            | "strong" | "style" | "script" | "head" | "body" | "html"
                            | "tags" | "tag"
                    ) && !label.starts_with("tag-")
                    {
                        labels.push(label.to_string());
                    }
                    i = end + 1;
                    continue;
                }
            }
        }
        i += 1;
    }
    labels
}

/// Extract labels with their associated display text.
/// For `= My Title <Hello>`, returns `("Hello", "My Title")`.
/// For standalone `<foo>` on a non-heading line, returns `("foo", "foo")`.
pub fn extract_labels_with_text(source: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        // Find all <label> patterns on this line
        let bytes = trimmed.as_bytes();
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
                        let label = &trimmed[start..end];
                        // Skip HTML-like tags and tag markers
                        if !matches!(
                            label,
                            "p" | "div" | "span" | "br" | "hr" | "img" | "a" | "ul"
                                | "ol" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                                | "table" | "tr" | "td" | "th" | "pre" | "code" | "em"
                                | "strong" | "style" | "script" | "head" | "body" | "html"
                                | "tags" | "tag"
                        ) && !label.starts_with("tag-")
                        {
                            // Get the text before the label on this line
                            let before = trimmed[..i].trim();
                            let display = if let Some(heading) = before.strip_prefix("= ") {
                                heading.trim().to_string()
                            } else if let Some(heading) = before.strip_prefix("== ") {
                                heading.trim().to_string()
                            } else if let Some(heading) = before.strip_prefix("=== ") {
                                heading.trim().to_string()
                            } else if !before.is_empty() {
                                before.to_string()
                            } else {
                                label.to_string()
                            };
                            results.push((label.to_string(), display));
                        }
                        i = end + 1;
                        continue;
                    }
                }
            }
            i += 1;
        }
    }
    results
}

/// Extract `@reference` patterns from Typst source (cross-note refs).
/// Returns the label names referenced (without the @).
pub fn extract_refs(source: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\n') {
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
                if end > start {
                    refs.push(source[start..end].to_string());
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    refs
}

/// Extract the title from a Typst source (first `= Heading`).
/// Strips any trailing `<label>` syntax from the heading.
pub fn extract_title(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("= ") {
            let title = heading.trim();
            // Strip trailing <label> if present
            if let Some(label_start) = title.rfind('<') {
                if title.ends_with('>') {
                    let before = title[..label_start].trim();
                    if !before.is_empty() {
                        return Some(before.to_string());
                    }
                }
            }
            return Some(title.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn extract_multiple_links() {
        let source = r#"
Links to #link("a") and #link("b/c")[BC].
Also see #link("d").
"#;
        let links = extract_links(source);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].raw_target, "a");
        assert_eq!(links[1].raw_target, "b/c");
        assert_eq!(links[2].raw_target, "d");
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
        assert_eq!(tags[0].0, "rust");
        assert_eq!(tags[1].0, "gui");
        assert_eq!(tags[2].0, "project/active");
    }

    #[test]
    fn extract_individual_metadata_tags() {
        let source = "#metadata(\"rust\") <tag-rust>\n#metadata(\"gui\") <tag-gui>";
        let tags = extract_tags(source);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].0, "rust");
        assert_eq!(tags[1].0, "gui");
    }

    #[test]
    fn extract_mixed_tag_patterns() {
        let source = "#metadata((\"rust\", \"gui\")) <tags>\n#metadata(\"extra\") <tag-extra>";
        let tags = extract_tags(source);
        assert_eq!(tags.len(), 3);
        assert_eq!(tags[0].0, "rust");
        assert_eq!(tags[1].0, "gui");
        assert_eq!(tags[2].0, "extra");
    }

    #[test]
    fn tag_labels_excluded_from_label_extraction() {
        let source = "= Title <hello>\n#metadata((\"rust\")) <tags>\n#metadata(\"gui\") <tag-gui>";
        let labels = extract_labels(source);
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0], "hello");
    }

    #[test]
    fn extract_no_links() {
        let links = extract_links("Just plain text with no links.");
        assert!(links.is_empty());
    }

    #[test]
    fn extract_title_test() {
        assert_eq!(
            extract_title("= My Note\n\nContent"),
            Some("My Note".into())
        );
        assert_eq!(extract_title("No heading here"), None);
    }
}
