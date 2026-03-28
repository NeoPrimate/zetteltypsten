use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

// =============================================================================
// Public types
// =============================================================================

/// Section number like [1, 2, 3] = "1.2.3."
pub type SectionNumber = Vec<u32>;

/// Full book configuration loaded from a single `book.toml`.
#[derive(Debug, Clone)]
pub struct BookConfig {
    pub title: String,
    pub authors: Vec<String>,
    pub description: String,
    pub language: String,
    pub src: PathBuf,
    pub build_dir: PathBuf,
    pub create_missing: bool,
    pub html: HtmlConfig,
    // Chapter tree
    pub prefix_chapters: Vec<Chapter>,
    pub parts: Vec<Part>,
    pub suffix_chapters: Vec<Chapter>,
}

/// A part groups numbered chapters under a title.
#[derive(Debug, Clone)]
pub struct Part {
    pub title: String,
    pub chapters: Vec<Chapter>,
}

/// A single chapter (may have nested sub-chapters).
#[derive(Debug, Clone)]
pub struct Chapter {
    pub title: String,
    pub file: Option<PathBuf>,       // None = draft chapter
    pub number: Option<SectionNumber>,
    pub children: Vec<Chapter>,
}

/// HTML output configuration.
#[derive(Debug, Clone)]
pub struct HtmlConfig {
    pub default_theme: String,
    pub preferred_dark_theme: String,
    pub git_repo_url: Option<String>,
    pub git_repo_icon: String,
    pub edit_url_template: Option<String>,
    pub additional_css: Vec<PathBuf>,
    pub additional_js: Vec<PathBuf>,
    pub no_section_label: bool,
    pub site_url: String,
    pub search: SearchConfig,
    pub print: PrintConfig,
    pub fold: FoldConfig,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub enable: bool,
    pub limit_results: u32,
}

#[derive(Debug, Clone)]
pub struct PrintConfig {
    pub enable: bool,
}

#[derive(Debug, Clone)]
pub struct FoldConfig {
    pub enable: bool,
    pub level: u8,
}

// =============================================================================
// TOML deserialization structs
// =============================================================================

#[derive(Deserialize, Default)]
struct TomlRoot {
    #[serde(default)]
    book: TomlBook,
    #[serde(default)]
    build: TomlBuild,
    #[serde(default)]
    output: TomlOutput,
    #[serde(default)]
    prefix: Vec<TomlChapter>,
    #[serde(default)]
    part: Vec<TomlPart>,
    #[serde(default)]
    suffix: Vec<TomlChapter>,
}

#[derive(Deserialize, Default)]
struct TomlBook {
    title: Option<String>,
    authors: Option<Vec<String>>,
    description: Option<String>,
    language: Option<String>,
    src: Option<String>,
}

#[derive(Deserialize, Default)]
struct TomlBuild {
    #[serde(rename = "build-dir")]
    build_dir: Option<String>,
    #[serde(rename = "create-missing")]
    create_missing: Option<bool>,
}

#[derive(Deserialize, Default)]
struct TomlOutput {
    #[serde(default)]
    html: TomlHtml,
}

#[derive(Deserialize, Default)]
struct TomlHtml {
    #[serde(rename = "default-theme")]
    default_theme: Option<String>,
    #[serde(rename = "preferred-dark-theme")]
    preferred_dark_theme: Option<String>,
    #[serde(rename = "git-repository-url")]
    git_repo_url: Option<String>,
    #[serde(rename = "git-repository-icon")]
    git_repo_icon: Option<String>,
    #[serde(rename = "edit-url-template")]
    edit_url_template: Option<String>,
    #[serde(rename = "additional-css")]
    additional_css: Option<Vec<String>>,
    #[serde(rename = "additional-js")]
    additional_js: Option<Vec<String>>,
    #[serde(rename = "no-section-label")]
    no_section_label: Option<bool>,
    #[serde(rename = "site-url")]
    site_url: Option<String>,
    #[serde(default)]
    search: TomlSearch,
    #[serde(default)]
    print: TomlPrint,
    #[serde(default)]
    fold: TomlFold,
}

#[derive(Deserialize, Default)]
struct TomlSearch {
    enable: Option<bool>,
    #[serde(rename = "limit-results")]
    limit_results: Option<u32>,
}

#[derive(Deserialize, Default)]
struct TomlPrint {
    enable: Option<bool>,
}

#[derive(Deserialize, Default)]
struct TomlFold {
    enable: Option<bool>,
    level: Option<u8>,
}

#[derive(Deserialize, Clone)]
struct TomlPart {
    title: String,
    #[serde(default)]
    chapter: Vec<TomlChapter>,
}

#[derive(Deserialize, Clone)]
struct TomlChapter {
    title: String,
    file: Option<String>,
    #[serde(default)]
    sub: Vec<TomlChapter>,
}

// =============================================================================
// Implementation
// =============================================================================

impl BookConfig {
    /// Load book configuration from a `book.toml` file.
    pub fn load(book_root: &Path) -> Result<Self> {
        let toml_path = book_root.join(".zetteltypsten/book.toml");
        let content = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;
        let raw: TomlRoot = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", toml_path.display()))?;

        // Convert chapter tree
        let prefix_chapters = raw.prefix.iter().map(|c| convert_chapter(c)).collect();
        let suffix_chapters = raw.suffix.iter().map(|c| convert_chapter(c)).collect();

        let mut parts: Vec<Part> = raw
            .part
            .iter()
            .map(|p| Part {
                title: p.title.clone(),
                chapters: p.chapter.iter().map(|c| convert_chapter(c)).collect(),
            })
            .collect();

        // Assign section numbers
        for part in &mut parts {
            assign_numbers(&mut part.chapters, &mut vec![]);
        }

        Ok(Self {
            title: raw.book.title.unwrap_or_else(|| "Untitled Book".into()),
            authors: raw.book.authors.unwrap_or_default(),
            description: raw.book.description.unwrap_or_default(),
            language: raw.book.language.unwrap_or_else(|| "en".into()),
            src: PathBuf::from(raw.book.src.unwrap_or_else(|| ".".into())),
            build_dir: PathBuf::from(raw.build.build_dir.unwrap_or_else(|| "book".into())),
            create_missing: raw.build.create_missing.unwrap_or(true),
            html: HtmlConfig {
                default_theme: raw.output.html.default_theme.unwrap_or_else(|| "dark".into()),
                preferred_dark_theme: raw
                    .output
                    .html
                    .preferred_dark_theme
                    .unwrap_or_else(|| "mocha".into()),
                git_repo_url: raw.output.html.git_repo_url,
                git_repo_icon: raw
                    .output
                    .html
                    .git_repo_icon
                    .unwrap_or_else(|| "fab-github".into()),
                edit_url_template: raw.output.html.edit_url_template,
                additional_css: raw
                    .output
                    .html
                    .additional_css
                    .unwrap_or_default()
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
                additional_js: raw
                    .output
                    .html
                    .additional_js
                    .unwrap_or_default()
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
                no_section_label: raw.output.html.no_section_label.unwrap_or(false),
                site_url: raw.output.html.site_url.unwrap_or_else(|| "/".into()),
                search: SearchConfig {
                    enable: raw.output.html.search.enable.unwrap_or(true),
                    limit_results: raw.output.html.search.limit_results.unwrap_or(30),
                },
                print: PrintConfig {
                    enable: raw.output.html.print.enable.unwrap_or(true),
                },
                fold: FoldConfig {
                    enable: raw.output.html.fold.enable.unwrap_or(false),
                    level: raw.output.html.fold.level.unwrap_or(0),
                },
            },
            prefix_chapters,
            parts,
            suffix_chapters,
        })
    }

    /// Create a default config (no book.toml file).
    pub fn default_with_title(title: &str) -> Self {
        Self {
            title: title.to_string(),
            authors: vec![],
            description: String::new(),
            language: "en".into(),
            src: PathBuf::from("."),
            build_dir: PathBuf::from("book"),
            create_missing: true,
            html: HtmlConfig {
                default_theme: "dark".into(),
                preferred_dark_theme: "mocha".into(),
                git_repo_url: None,
                git_repo_icon: "fab-github".into(),
                edit_url_template: None,
                additional_css: vec![],
                additional_js: vec![],
                no_section_label: false,
                site_url: "/".into(),
                search: SearchConfig {
                    enable: true,
                    limit_results: 30,
                },
                print: PrintConfig { enable: true },
                fold: FoldConfig {
                    enable: false,
                    level: 0,
                },
            },
            prefix_chapters: vec![],
            parts: vec![],
            suffix_chapters: vec![],
        }
    }

    /// Get a flat list of all chapters with content (for prev/next navigation).
    pub fn flatten_chapters(&self) -> Vec<&Chapter> {
        let mut result = Vec::new();
        for ch in &self.prefix_chapters {
            flatten(ch, &mut result);
        }
        for part in &self.parts {
            for ch in &part.chapters {
                flatten(ch, &mut result);
            }
        }
        for ch in &self.suffix_chapters {
            flatten(ch, &mut result);
        }
        result
    }
}

fn flatten<'a>(ch: &'a Chapter, result: &mut Vec<&'a Chapter>) {
    if ch.file.is_some() {
        result.push(ch);
    }
    for child in &ch.children {
        flatten(child, result);
    }
}

fn convert_chapter(raw: &TomlChapter) -> Chapter {
    Chapter {
        title: raw.title.clone(),
        file: raw.file.as_ref().map(PathBuf::from),
        number: None,
        children: raw.sub.iter().map(|c| convert_chapter(c)).collect(),
    }
}

fn assign_numbers(chapters: &mut [Chapter], prefix: &mut Vec<u32>) {
    for (i, ch) in chapters.iter_mut().enumerate() {
        let mut num = prefix.clone();
        num.push((i + 1) as u32);
        ch.number = Some(num.clone());

        if !ch.children.is_empty() {
            assign_numbers(&mut ch.children, &mut num);
        }
    }
}

impl Chapter {
    /// Format section number as "1.2.3."
    pub fn section_string(&self) -> String {
        match &self.number {
            Some(nums) => {
                let s: Vec<String> = nums.iter().map(|n| n.to_string()).collect();
                format!("{}.", s.join("."))
            }
            None => String::new(),
        }
    }

    /// Is this a draft chapter (no file)?
    pub fn is_draft(&self) -> bool {
        self.file.is_none()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_unified_book_toml() {
        let tmp = std::env::temp_dir().join("zt-book-unified-test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join(".zetteltypsten")).unwrap();

        fs::write(
            tmp.join(".zetteltypsten/book.toml"),
            r#"
[book]
title = "My Book"
authors = ["Alice", "Bob"]
language = "en"

[build]
build-dir = "output"

[[prefix]]
title = "Introduction"
file = "intro.typ"

[[part]]
title = "User Guide"

  [[part.chapter]]
  title = "Getting Started"
  file = "getting-started.typ"

    [[part.chapter.sub]]
    title = "Installation"
    file = "gs/install.typ"

    [[part.chapter.sub]]
    title = "First Note"
    file = "gs/first-note.typ"

  [[part.chapter]]
  title = "Features"
  file = "features.typ"

  [[part.chapter]]
  title = "Draft Chapter"

[[part]]
title = "Reference"

  [[part.chapter]]
  title = "Configuration"
  file = "ref/config.typ"

[[suffix]]
title = "Appendix"
file = "appendix.typ"

[output.html]
default-theme = "dark"
git-repository-url = "https://github.com/test/repo"
site-url = "/my-book/"

[output.html.search]
enable = true
limit-results = 20
"#,
        )
        .unwrap();

        let config = BookConfig::load(&tmp).unwrap();

        // Metadata
        assert_eq!(config.title, "My Book");
        assert_eq!(config.authors.len(), 2);
        assert_eq!(config.build_dir, PathBuf::from("output"));

        // Prefix
        assert_eq!(config.prefix_chapters.len(), 1);
        assert_eq!(config.prefix_chapters[0].title, "Introduction");

        // Parts
        assert_eq!(config.parts.len(), 2);
        assert_eq!(config.parts[0].title, "User Guide");
        assert_eq!(config.parts[0].chapters.len(), 3);

        // Nested chapters
        let gs = &config.parts[0].chapters[0];
        assert_eq!(gs.title, "Getting Started");
        assert_eq!(gs.section_string(), "1.");
        assert_eq!(gs.children.len(), 2);
        assert_eq!(gs.children[0].title, "Installation");
        assert_eq!(gs.children[0].section_string(), "1.1.");
        assert_eq!(gs.children[1].section_string(), "1.2.");

        // Draft
        let draft = &config.parts[0].chapters[2];
        assert!(draft.is_draft());
        assert_eq!(draft.section_string(), "3.");

        // Second part
        assert_eq!(config.parts[1].chapters[0].section_string(), "1.");

        // Suffix
        assert_eq!(config.suffix_chapters.len(), 1);

        // Flattened chapters (non-draft, with files)
        // intro + getting-started + install + first-note + features + config + appendix = 7
        let flat = config.flatten_chapters();
        assert_eq!(flat.len(), 7);

        // HTML config
        assert_eq!(config.html.default_theme, "dark");
        assert_eq!(config.html.search.limit_results, 20);

        let _ = fs::remove_dir_all(&tmp);
    }
}
