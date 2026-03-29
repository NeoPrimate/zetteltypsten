use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{BookConfig, Chapter, SectionNumber};

/// A compiled book ready for export.
#[derive(Debug)]
pub struct CompiledBook {
    pub config: BookConfig,
    pub chapters: Vec<CompiledChapter>,
}

/// A single compiled chapter.
#[derive(Debug, Clone)]
pub struct CompiledChapter {
    pub title: String,
    pub file: Option<PathBuf>,
    pub number: Option<SectionNumber>,
    pub html: String,
    pub plain_text: String,
    pub is_draft: bool,
}

/// Build a book from a directory containing `.zetteltypsten/book.toml`.
pub fn build_book(book_root: &Path) -> Result<CompiledBook> {
    let config = BookConfig::load(book_root)
        .unwrap_or_else(|_| BookConfig::default_with_title("Untitled Book"));

    let src_root = book_root.join(&config.src);

    // Use SQLite database for cross-note @ref resolution
    let db = zt_db::Database::open(book_root)
        .unwrap_or_else(|_| zt_db::Database::open_in_memory().unwrap());
    let _ = db.sync(book_root);

    let label_map = db.label_map();
    let title_map = db.title_map();
    let label_text_map = db.label_text_map();

    let refs = RefMaps {
        label_map,
        title_map,
        label_text_map,
    };

    let mut chapters = Vec::new();

    for ch in &config.chapters {
        compile_tree(ch, &src_root, &refs, &mut chapters)?;
    }

    tracing::info!(
        "Book compiled: {} chapters ({} with content)",
        chapters.len(),
        chapters.iter().filter(|c| !c.is_draft).count()
    );

    Ok(CompiledBook { config, chapters })
}

struct RefMaps {
    label_map: HashMap<String, String>,
    title_map: HashMap<String, String>,
    label_text_map: HashMap<String, String>,
}

fn compile_tree(
    ch: &Chapter,
    src_root: &Path,
    refs: &RefMaps,
    chapters: &mut Vec<CompiledChapter>,
) -> Result<()> {
    let compiled = compile_one(ch, src_root, refs)?;
    chapters.push(compiled);
    for child in &ch.children {
        compile_tree(child, src_root, refs, chapters)?;
    }
    Ok(())
}

fn compile_one(ch: &Chapter, src_root: &Path, refs: &RefMaps) -> Result<CompiledChapter> {
    let Some(ref file) = ch.file else {
        return Ok(CompiledChapter {
            title: ch.title.clone(),
            file: None,
            number: ch.number.clone(),
            html: String::new(),
            plain_text: String::new(),
            is_draft: true,
        });
    };

    let file_path = src_root.join(file);
    let source = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read chapter: {}", file_path.display()))?;

    let rel_path = file.to_string_lossy().to_string();

    let html = zt_typst::compiler::compile_to_body_html_with_refs(
        src_root,
        &rel_path,
        &source,
        &refs.label_map,
        &refs.title_map,
        &refs.label_text_map,
    )
    .with_context(|| format!("Failed to compile: {}", rel_path))?;

    // Prepend chapter title as an h1
    let title_html = format!("<h1 class=\"chapter-title\">{}</h1>\n", ch.title);
    let html = format!("{title_html}{html}");

    let plain_text = strip_html_tags(&html);

    Ok(CompiledChapter {
        title: ch.title.clone(),
        file: Some(file.clone()),
        number: ch.number.clone(),
        html,
        plain_text,
        is_draft: false,
    })
}

/// Naive HTML tag stripper for search indexing.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                result.push(' ');
            }
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    let mut prev_space = false;
    result
        .chars()
        .filter(|&c| {
            if c.is_whitespace() {
                if prev_space {
                    return false;
                }
                prev_space = true;
            } else {
                prev_space = false;
            }
            true
        })
        .collect()
}

impl CompiledBook {
    /// Get a flat ordered list of navigable (non-draft) chapters.
    pub fn navigable_chapters(&self) -> Vec<&CompiledChapter> {
        self.chapters.iter().filter(|c| !c.is_draft).collect()
    }

    /// Get prev/next for a chapter by its index in self.chapters.
    pub fn neighbors(&self, index: usize) -> (Option<&CompiledChapter>, Option<&CompiledChapter>) {
        let nav = self.navigable_chapters();
        let mut nav_idx = None;
        let mut count = 0;
        for (i, ch) in self.chapters.iter().enumerate() {
            if ch.is_draft {
                continue;
            }
            if i == index {
                nav_idx = Some(count);
                break;
            }
            count += 1;
        }
        let Some(ni) = nav_idx else {
            return (None, None);
        };
        let prev = if ni > 0 { nav.get(ni - 1).copied() } else { None };
        let next = nav.get(ni + 1).copied();
        (prev, next)
    }
}
