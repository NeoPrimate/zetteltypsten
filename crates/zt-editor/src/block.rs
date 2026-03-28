use std::ops::Range;

/// The kind of a parsed block.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockKind {
    /// `= Heading`, `== Heading`, etc.
    Heading { level: u8 },
    /// One or more consecutive non-blank text lines.
    Paragraph,
    /// Fenced code block (``` ... ```).
    CodeBlock,
    /// A list item starting with `- `, `+ `, or `1. `.
    ListItem { ordered: bool },
    /// Display math (`$ ... $` on its own line).
    MathBlock,
    /// Top-level directives: `#set`, `#show`, `#import`, `#let`, `#include`.
    Directive,
    /// An empty line (block separator).
    BlankLine,
}

/// A parsed block of Typst source.
#[derive(Debug, Clone)]
pub struct Block {
    pub kind: BlockKind,
    /// Byte range in the original source.
    pub byte_range: Range<usize>,
    /// The raw source text of this block.
    pub source: String,
}

/// Parse Typst source into a flat list of blocks.
pub fn parse_blocks(source: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut offset = 0;
    let lines: Vec<&str> = source.split('\n').collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        let line_byte_len = line.len() + if i + 1 < lines.len() { 1 } else { 0 }; // +1 for \n

        if trimmed.is_empty() {
            // Blank line
            blocks.push(Block {
                kind: BlockKind::BlankLine,
                byte_range: offset..offset + line_byte_len,
                source: line.to_string(),
            });
            offset += line_byte_len;
            i += 1;
        } else if trimmed.starts_with("```") {
            // Code block — collect until closing ```
            let start = offset;
            let mut block_text = String::from(line);
            offset += line_byte_len;
            i += 1;
            while i < lines.len() {
                let next_line = lines[i];
                let next_len = next_line.len() + if i + 1 < lines.len() { 1 } else { 0 };
                block_text.push('\n');
                block_text.push_str(next_line);
                offset += next_len;
                i += 1;
                if next_line.trim() == "```" {
                    break;
                }
            }
            blocks.push(Block {
                kind: BlockKind::CodeBlock,
                byte_range: start..offset,
                source: block_text,
            });
        } else if trimmed.starts_with("= ") || trimmed.starts_with("== ")
            || trimmed.starts_with("=== ") || trimmed == "="
        {
            // Heading
            let level = trimmed.bytes().take_while(|&b| b == b'=').count() as u8;
            blocks.push(Block {
                kind: BlockKind::Heading { level },
                byte_range: offset..offset + line_byte_len,
                source: line.to_string(),
            });
            offset += line_byte_len;
            i += 1;
        } else if trimmed.starts_with("- ") || trimmed.starts_with("+ ") {
            // Unordered list item
            blocks.push(Block {
                kind: BlockKind::ListItem { ordered: false },
                byte_range: offset..offset + line_byte_len,
                source: line.to_string(),
            });
            offset += line_byte_len;
            i += 1;
        } else if trimmed.len() > 2
            && trimmed.as_bytes()[0].is_ascii_digit()
            && trimmed.contains(". ")
        {
            // Ordered list item (e.g. "1. ")
            blocks.push(Block {
                kind: BlockKind::ListItem { ordered: true },
                byte_range: offset..offset + line_byte_len,
                source: line.to_string(),
            });
            offset += line_byte_len;
            i += 1;
        } else if trimmed == "$" || (trimmed.starts_with("$ ") && trimmed.ends_with(" $")) {
            // Display math — single line or collect until closing $
            if trimmed == "$" {
                let start = offset;
                let mut block_text = String::from(line);
                offset += line_byte_len;
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i];
                    let next_len = next_line.len() + if i + 1 < lines.len() { 1 } else { 0 };
                    block_text.push('\n');
                    block_text.push_str(next_line);
                    offset += next_len;
                    i += 1;
                    if next_line.trim() == "$" {
                        break;
                    }
                }
                blocks.push(Block {
                    kind: BlockKind::MathBlock,
                    byte_range: start..offset,
                    source: block_text,
                });
            } else {
                blocks.push(Block {
                    kind: BlockKind::MathBlock,
                    byte_range: offset..offset + line_byte_len,
                    source: line.to_string(),
                });
                offset += line_byte_len;
                i += 1;
            }
        } else if trimmed.starts_with("#set ")
            || trimmed.starts_with("#show ")
            || trimmed.starts_with("#import ")
            || trimmed.starts_with("#let ")
            || trimmed.starts_with("#include ")
        {
            // Directive
            blocks.push(Block {
                kind: BlockKind::Directive,
                byte_range: offset..offset + line_byte_len,
                source: line.to_string(),
            });
            offset += line_byte_len;
            i += 1;
        } else {
            // Paragraph — group consecutive non-blank, non-special lines
            let start = offset;
            let mut para_text = String::from(line);
            offset += line_byte_len;
            i += 1;
            while i < lines.len() {
                let next_line = lines[i];
                let next_trimmed = next_line.trim_start();
                // Stop grouping on blank line, heading, list, code fence, directive, or math
                if next_trimmed.is_empty()
                    || next_trimmed.starts_with("= ")
                    || next_trimmed.starts_with("== ")
                    || next_trimmed.starts_with("```")
                    || next_trimmed.starts_with("- ")
                    || next_trimmed.starts_with("+ ")
                    || next_trimmed.starts_with("#set ")
                    || next_trimmed.starts_with("#show ")
                    || next_trimmed.starts_with("#import ")
                    || next_trimmed.starts_with("#let ")
                    || next_trimmed.starts_with("#include ")
                    || next_trimmed == "$"
                {
                    break;
                }
                let next_len = next_line.len() + if i + 1 < lines.len() { 1 } else { 0 };
                para_text.push('\n');
                para_text.push_str(next_line);
                offset += next_len;
                i += 1;
            }
            blocks.push(Block {
                kind: BlockKind::Paragraph,
                byte_range: start..offset,
                source: para_text,
            });
        }
    }

    blocks
}

/// Parse inline markup spans within a text block for styled rendering.
/// Returns (plain_text, highlights) where highlights are (byte_range, style).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InlineStyle {
    Bold,
    Italic,
    Code,
    Link,
    Comment,
    FunctionCall,
}

#[derive(Debug, Clone)]
pub struct InlineSpan {
    /// Byte range within the block's source text.
    pub range: Range<usize>,
    pub style: InlineStyle,
    /// For links: the target URL/note-id.
    pub link_target: Option<String>,
}

/// Extract inline style spans from a text block.
pub fn parse_inline_spans(text: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            // Bold: *text*
            b'*' if i + 1 < len && bytes[i + 1] != b' ' => {
                if let Some(end) = find_closing(text, i + 1, b'*') {
                    spans.push(InlineSpan {
                        range: i..end + 1,
                        style: InlineStyle::Bold,
                        link_target: None,
                    });
                    i = end + 1;
                    continue;
                }
            }
            // Italic: _text_
            b'_' if i + 1 < len && bytes[i + 1] != b' ' => {
                if let Some(end) = find_closing(text, i + 1, b'_') {
                    spans.push(InlineSpan {
                        range: i..end + 1,
                        style: InlineStyle::Italic,
                        link_target: None,
                    });
                    i = end + 1;
                    continue;
                }
            }
            // Inline code: `text`
            b'`' if i + 1 < len => {
                if let Some(end) = find_closing(text, i + 1, b'`') {
                    spans.push(InlineSpan {
                        range: i..end + 1,
                        style: InlineStyle::Code,
                        link_target: None,
                    });
                    i = end + 1;
                    continue;
                }
            }
            // Comment: // to end of line
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                let end = text[i..].find('\n').map(|p| i + p).unwrap_or(len);
                spans.push(InlineSpan {
                    range: i..end,
                    style: InlineStyle::Comment,
                    link_target: None,
                });
                i = end;
                continue;
            }
            // #link("target") or #function(...)
            b'#' if i + 1 < len && bytes[i + 1].is_ascii_alphabetic() => {
                // Find the function name
                let name_start = i + 1;
                let name_end = text[name_start..]
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .map(|p| name_start + p)
                    .unwrap_or(len);
                let name = &text[name_start..name_end];

                if name == "link" {
                    // Parse #link("target")[display]
                    if let Some(target) = extract_link_target(text, name_end) {
                        let end = find_link_end(text, name_end);
                        spans.push(InlineSpan {
                            range: i..end,
                            style: InlineStyle::Link,
                            link_target: Some(target),
                        });
                        i = end;
                        continue;
                    }
                }

                // Other function calls
                let end = find_function_call_end(text, name_end);
                spans.push(InlineSpan {
                    range: i..end,
                    style: InlineStyle::FunctionCall,
                    link_target: None,
                });
                i = end;
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    spans
}

fn find_closing(text: &str, start: usize, delimiter: u8) -> Option<usize> {
    let bytes = text.as_bytes();
    for i in start..bytes.len() {
        if bytes[i] == delimiter && (i == 0 || bytes[i - 1] != b'\\') {
            return Some(i);
        }
        // Don't cross newlines for inline markup
        if bytes[i] == b'\n' {
            return None;
        }
    }
    None
}

fn extract_link_target(text: &str, after_name: usize) -> Option<String> {
    let rest = &text[after_name..];
    let rest = rest.trim_start();
    if !rest.starts_with("(\"") {
        return None;
    }
    let target_start = 2;
    let target_end = rest[target_start..].find('"')?;
    Some(rest[target_start..target_start + target_end].to_string())
}

fn find_link_end(text: &str, after_name: usize) -> usize {
    let rest = &text[after_name..];
    // Find closing paren
    if let Some(paren) = rest.find(')') {
        let after_paren = after_name + paren + 1;
        // Check for [display text]
        if after_paren < text.len() && text.as_bytes()[after_paren] == b'[' {
            if let Some(bracket) = text[after_paren + 1..].find(']') {
                return after_paren + bracket + 2;
            }
        }
        return after_paren;
    }
    text.len()
}

fn find_function_call_end(text: &str, after_name: usize) -> usize {
    if after_name >= text.len() {
        return after_name;
    }
    let rest = &text[after_name..];
    if rest.starts_with('(') {
        // Find matching paren (simple, no nesting)
        if let Some(close) = rest.find(')') {
            let after_paren = after_name + close + 1;
            // Check for [content]
            if after_paren < text.len() && text.as_bytes()[after_paren] == b'[' {
                if let Some(bracket) = text[after_paren + 1..].find(']') {
                    return after_paren + bracket + 2;
                }
            }
            return after_paren;
        }
    }
    after_name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_heading() {
        let blocks = parse_blocks("= Hello World\n\nSome text.");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0].kind, BlockKind::Heading { level: 1 }));
        assert!(matches!(blocks[1].kind, BlockKind::BlankLine));
        assert!(matches!(blocks[2].kind, BlockKind::Paragraph));
    }

    #[test]
    fn parse_code_block() {
        let src = "```rust\nfn main() {}\n```\n\nAfter.";
        let blocks = parse_blocks(src);
        assert!(matches!(blocks[0].kind, BlockKind::CodeBlock));
        assert!(blocks[0].source.contains("fn main()"));
    }

    #[test]
    fn parse_list_items() {
        let blocks = parse_blocks("- Item one\n- Item two\n+ Item three");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0].kind, BlockKind::ListItem { ordered: false }));
    }

    #[test]
    fn parse_directive() {
        let blocks = parse_blocks("#import \"foo.typ\": bar\n#set text(size: 12pt)");
        assert_eq!(blocks.len(), 2);
        assert!(matches!(blocks[0].kind, BlockKind::Directive));
        assert!(matches!(blocks[1].kind, BlockKind::Directive));
    }

    #[test]
    fn inline_bold_italic() {
        let spans = parse_inline_spans("Hello *bold* and _italic_ world");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].style, InlineStyle::Bold);
        assert_eq!(spans[1].style, InlineStyle::Italic);
    }

    #[test]
    fn inline_link() {
        let spans = parse_inline_spans("See #link(\"notes/foo\")[Foo] here");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].style, InlineStyle::Link);
        assert_eq!(spans[0].link_target.as_deref(), Some("notes/foo"));
    }
}
