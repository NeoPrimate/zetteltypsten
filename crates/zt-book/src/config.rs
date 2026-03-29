use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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
    /// Flat list of top-level chapters (nesting via `children`).
    pub chapters: Vec<Chapter>,
}

/// A single chapter / section (may have nested sub-sections).
///
/// `file` is `None` for section-only entries (structural headers with no content).
/// `is_part` marks this as a non-interactive section divider (like mdBook's `# Part Title`).
#[derive(Debug, Clone)]
pub struct Chapter {
    pub title: String,
    pub file: Option<PathBuf>,
    pub number: Option<SectionNumber>,
    pub children: Vec<Chapter>,
    /// If true, this is a part title — a bold divider in the TOC with no content
    /// and no nesting. It cannot contain children or be dragged into.
    pub is_part: bool,
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

#[derive(Deserialize, Serialize, Default)]
struct TomlRoot {
    #[serde(default)]
    book: TomlBook,
    #[serde(default, skip_serializing_if = "TomlBuild::is_default")]
    build: TomlBuild,
    #[serde(default, skip_serializing_if = "TomlOutput::is_default")]
    output: TomlOutput,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chapter: Vec<TomlChapter>,
}

#[derive(Deserialize, Serialize, Default)]
struct TomlBook {
    title: Option<String>,
    authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    src: Option<String>,
}

#[derive(Deserialize, Serialize, Default)]
struct TomlBuild {
    #[serde(rename = "build-dir", skip_serializing_if = "Option::is_none")]
    build_dir: Option<String>,
    #[serde(rename = "create-missing", skip_serializing_if = "Option::is_none")]
    create_missing: Option<bool>,
}

impl TomlBuild {
    fn is_default(&self) -> bool {
        self.build_dir.is_none() && self.create_missing.is_none()
    }
}

#[derive(Deserialize, Serialize, Default)]
struct TomlOutput {
    #[serde(default, skip_serializing_if = "TomlHtml::is_default")]
    html: TomlHtml,
}

impl TomlOutput {
    fn is_default(&self) -> bool {
        self.html.is_default()
    }
}

#[derive(Deserialize, Serialize, Default)]
struct TomlHtml {
    #[serde(rename = "default-theme", skip_serializing_if = "Option::is_none")]
    default_theme: Option<String>,
    #[serde(rename = "preferred-dark-theme", skip_serializing_if = "Option::is_none")]
    preferred_dark_theme: Option<String>,
    #[serde(rename = "git-repository-url", skip_serializing_if = "Option::is_none")]
    git_repo_url: Option<String>,
    #[serde(rename = "git-repository-icon", skip_serializing_if = "Option::is_none")]
    git_repo_icon: Option<String>,
    #[serde(rename = "edit-url-template", skip_serializing_if = "Option::is_none")]
    edit_url_template: Option<String>,
    #[serde(rename = "additional-css", skip_serializing_if = "Option::is_none")]
    additional_css: Option<Vec<String>>,
    #[serde(rename = "additional-js", skip_serializing_if = "Option::is_none")]
    additional_js: Option<Vec<String>>,
    #[serde(rename = "no-section-label", skip_serializing_if = "Option::is_none")]
    no_section_label: Option<bool>,
    #[serde(rename = "site-url", skip_serializing_if = "Option::is_none")]
    site_url: Option<String>,
    #[serde(default, skip_serializing_if = "TomlSearch::is_default")]
    search: TomlSearch,
    #[serde(default, skip_serializing_if = "TomlPrint::is_default")]
    print: TomlPrint,
    #[serde(default, skip_serializing_if = "TomlFold::is_default")]
    fold: TomlFold,
}

impl TomlHtml {
    fn is_default(&self) -> bool {
        self.default_theme.is_none()
            && self.preferred_dark_theme.is_none()
            && self.git_repo_url.is_none()
            && self.git_repo_icon.is_none()
            && self.edit_url_template.is_none()
            && self.additional_css.is_none()
            && self.additional_js.is_none()
            && self.no_section_label.is_none()
            && self.site_url.is_none()
            && self.search.is_default()
            && self.print.is_default()
            && self.fold.is_default()
    }
}

#[derive(Deserialize, Serialize, Default)]
struct TomlSearch {
    #[serde(skip_serializing_if = "Option::is_none")]
    enable: Option<bool>,
    #[serde(rename = "limit-results", skip_serializing_if = "Option::is_none")]
    limit_results: Option<u32>,
}

impl TomlSearch {
    fn is_default(&self) -> bool { self.enable.is_none() && self.limit_results.is_none() }
}

#[derive(Deserialize, Serialize, Default)]
struct TomlPrint {
    #[serde(skip_serializing_if = "Option::is_none")]
    enable: Option<bool>,
}

impl TomlPrint {
    fn is_default(&self) -> bool { self.enable.is_none() }
}

#[derive(Deserialize, Serialize, Default)]
struct TomlFold {
    #[serde(skip_serializing_if = "Option::is_none")]
    enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
}

impl TomlFold {
    fn is_default(&self) -> bool { self.enable.is_none() && self.level.is_none() }
}

#[derive(Deserialize, Serialize, Clone)]
struct TomlChapter {
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    part: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sub: Vec<TomlChapter>,
}

fn is_false(b: &bool) -> bool { !*b }

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
        let mut chapters: Vec<Chapter> = raw.chapter.iter().map(|c| convert_chapter(c)).collect();
        assign_numbers(&mut chapters, &mut vec![]);

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
            chapters,
        })
    }

    /// Save the current configuration back to `.zetteltypsten/book.toml`.
    pub fn save(&self, book_root: &Path) -> Result<()> {
        let toml_root = self.to_toml();
        let content = toml::to_string_pretty(&toml_root)
            .context("Failed to serialize book config")?;
        let path = book_root.join(".zetteltypsten/book.toml");
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(())
    }

    fn to_toml(&self) -> TomlRoot {
        let src_str = self.src.to_string_lossy().into_owned();
        let build_str = self.build_dir.to_string_lossy().into_owned();
        TomlRoot {
            book: TomlBook {
                title: Some(self.title.clone()),
                authors: if self.authors.is_empty() { None } else { Some(self.authors.clone()) },
                description: if self.description.is_empty() { None } else { Some(self.description.clone()) },
                language: if self.language == "en" { None } else { Some(self.language.clone()) },
                src: if src_str == "." { None } else { Some(src_str) },
            },
            build: TomlBuild {
                build_dir: if build_str == "book" { None } else { Some(build_str) },
                create_missing: if self.create_missing { None } else { Some(false) },
            },
            output: TomlOutput {
                html: TomlHtml {
                    default_theme: if self.html.default_theme == "dark" { None } else { Some(self.html.default_theme.clone()) },
                    preferred_dark_theme: if self.html.preferred_dark_theme == "mocha" { None } else { Some(self.html.preferred_dark_theme.clone()) },
                    git_repo_url: self.html.git_repo_url.clone(),
                    git_repo_icon: if self.html.git_repo_icon == "fab-github" { None } else { Some(self.html.git_repo_icon.clone()) },
                    edit_url_template: self.html.edit_url_template.clone(),
                    additional_css: if self.html.additional_css.is_empty() { None } else { Some(self.html.additional_css.iter().map(|p| p.to_string_lossy().into_owned()).collect()) },
                    additional_js: if self.html.additional_js.is_empty() { None } else { Some(self.html.additional_js.iter().map(|p| p.to_string_lossy().into_owned()).collect()) },
                    no_section_label: if self.html.no_section_label { Some(true) } else { None },
                    site_url: if self.html.site_url == "/" { None } else { Some(self.html.site_url.clone()) },
                    search: TomlSearch {
                        enable: if self.html.search.enable { None } else { Some(false) },
                        limit_results: if self.html.search.limit_results == 30 { None } else { Some(self.html.search.limit_results) },
                    },
                    print: TomlPrint {
                        enable: if self.html.print.enable { None } else { Some(false) },
                    },
                    fold: TomlFold {
                        enable: if self.html.fold.enable { Some(true) } else { None },
                        level: if self.html.fold.level == 0 { None } else { Some(self.html.fold.level) },
                    },
                },
            },
            chapter: self.chapters.iter().map(chapter_to_toml).collect(),
        }
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
            chapters: vec![],
        }
    }

    /// Get a flat list of all chapters with content (for prev/next navigation).
    pub fn flatten_chapters(&self) -> Vec<&Chapter> {
        let mut result = Vec::new();
        for ch in &self.chapters {
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
        is_part: raw.part,
    }
}

fn chapter_to_toml(ch: &Chapter) -> TomlChapter {
    TomlChapter {
        title: ch.title.clone(),
        file: ch.file.as_ref().map(|f| f.to_string_lossy().into_owned()),
        part: ch.is_part,
        sub: ch.children.iter().map(chapter_to_toml).collect(),
    }
}

fn assign_numbers(chapters: &mut [Chapter], prefix: &mut Vec<u32>) {
    let mut counter = 0u32;
    for ch in chapters.iter_mut() {
        if ch.is_part {
            ch.number = None; // parts are unnumbered
            continue;
        }
        counter += 1;
        let mut num = prefix.clone();
        num.push(counter);
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

    /// Is this a section-only entry (no file)?
    pub fn is_draft(&self) -> bool {
        self.file.is_none()
    }
}

// =============================================================================
// Chapter location + structural mutations
// =============================================================================

/// Identifies a chapter's position in the book tree as a path of indices.
///
/// `path` is non-empty: `path[0]` indexes into `BookConfig::chapters`,
/// each subsequent element indexes into `.children`.
/// E.g. `[2, 0, 1]` = `chapters[2].children[0].children[1]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChapterLoc {
    pub path: Vec<usize>,
}

impl ChapterLoc {
    pub fn new(path: Vec<usize>) -> Self { Self { path } }

    pub fn root(index: usize) -> Self { Self { path: vec![index] } }

    /// Stable string key for HashSets.
    pub fn key(&self) -> String {
        self.path.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("/")
    }

    /// Create a child loc by appending an index.
    pub fn child(&self, index: usize) -> Self {
        let mut p = self.path.clone();
        p.push(index);
        Self { path: p }
    }
}

impl BookConfig {
    // ── Accessors ────────────────────────────────────────────────────────────

    /// Get the sibling list and index for the chapter at `loc`.
    fn sibling_list_and_index(&self, loc: &ChapterLoc) -> Option<(&Vec<Chapter>, usize)> {
        if loc.path.is_empty() { return None; }
        if loc.path.len() == 1 {
            return Some((&self.chapters, loc.path[0]));
        }
        let mut list = &self.chapters;
        for &idx in &loc.path[..loc.path.len() - 2] {
            list = &list.get(idx)?.children;
        }
        let parent_idx = loc.path[loc.path.len() - 2];
        Some((&list.get(parent_idx)?.children, *loc.path.last()?))
    }

    fn sibling_list_and_index_mut(&mut self, loc: &ChapterLoc) -> Option<(&mut Vec<Chapter>, usize)> {
        if loc.path.is_empty() { return None; }
        if loc.path.len() == 1 {
            return Some((&mut self.chapters, loc.path[0]));
        }
        let mut list = &mut self.chapters;
        for &idx in &loc.path[..loc.path.len() - 2] {
            list = &mut list.get_mut(idx)?.children;
        }
        let parent_idx = loc.path[loc.path.len() - 2];
        let last = *loc.path.last()?;
        Some((&mut list.get_mut(parent_idx)?.children, last))
    }

    pub fn chapter_at(&self, loc: &ChapterLoc) -> Option<&Chapter> {
        let (list, idx) = self.sibling_list_and_index(loc)?;
        list.get(idx)
    }

    pub fn chapter_at_mut(&mut self, loc: &ChapterLoc) -> Option<&mut Chapter> {
        let (list, idx) = self.sibling_list_and_index_mut(loc)?;
        list.get_mut(idx)
    }

    // ── Mutations ────────────────────────────────────────────────────────────

    pub fn move_chapter(&mut self, loc: &ChapterLoc, delta: isize) -> bool {
        let Some((list, idx)) = self.sibling_list_and_index_mut(loc) else { return false };
        let new_idx = idx as isize + delta;
        if new_idx < 0 || new_idx as usize >= list.len() { return false; }
        let ch = list.remove(idx);
        list.insert(new_idx as usize, ch);
        true
    }

    pub fn remove_chapter(&mut self, loc: &ChapterLoc) -> Option<Chapter> {
        let (list, idx) = self.sibling_list_and_index_mut(loc)?;
        if idx >= list.len() { return None; }
        Some(list.remove(idx))
    }

    pub fn insert_chapter(&mut self, loc: &ChapterLoc, ch: Chapter) {
        if let Some((list, idx)) = self.sibling_list_and_index_mut(loc) {
            let idx = idx.min(list.len());
            list.insert(idx, ch);
        }
    }

    pub fn renumber(&mut self) {
        assign_numbers(&mut self.chapters, &mut vec![]);
    }

    pub fn save_renumbered(&mut self, book_root: &Path) -> Result<()> {
        self.renumber();
        self.save(book_root)
    }

    /// After removing an item at `removed`, adjust `target` location if the
    /// removal shifted its index within the same sibling list.
    ///
    /// Call this AFTER `remove_chapter(removed)` but BEFORE `insert_chapter(target, ch)`.
    pub fn adjust_loc_after_removal(target: &ChapterLoc, removed: &ChapterLoc) -> ChapterLoc {
        let mut adjusted = target.path.clone();
        let rp = &removed.path;

        // Only adjust if they share the same parent
        // (i.e., the removal affected the same sibling list the target is in)
        if rp.len() > adjusted.len() {
            // Removed is deeper than target — no effect on target's sibling list
            return ChapterLoc::new(adjusted);
        }

        let parent_len = rp.len() - 1;
        if parent_len > adjusted.len() - 1 {
            return ChapterLoc::new(adjusted);
        }

        // Check that the parent path matches
        if rp[..parent_len] == adjusted[..parent_len] {
            let removed_idx = rp[parent_len];
            if adjusted[parent_len] > removed_idx {
                adjusted[parent_len] -= 1;
            }
        }

        ChapterLoc::new(adjusted)
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
    fn parse_book_toml() {
        let tmp = std::env::temp_dir().join("zt-book-test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join(".zetteltypsten")).unwrap();

        fs::write(
            tmp.join(".zetteltypsten/book.toml"),
            r#"
[book]
title = "My Book"
authors = ["Alice", "Bob"]

[build]
build-dir = "output"

[[chapter]]
title = "Introduction"
file = "intro.typ"

[[chapter]]
title = "Getting Started"
file = "getting-started.typ"

  [[chapter.sub]]
  title = "Installation"
  file = "gs/install.typ"

  [[chapter.sub]]
  title = "First Note"
  file = "gs/first-note.typ"

[[chapter]]
title = "Features"
file = "features.typ"

[[chapter]]
title = "Draft Section"

[[chapter]]
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

        assert_eq!(config.title, "My Book");
        assert_eq!(config.authors.len(), 2);
        assert_eq!(config.build_dir, PathBuf::from("output"));

        // 5 top-level chapters
        assert_eq!(config.chapters.len(), 5);
        assert_eq!(config.chapters[0].title, "Introduction");

        // Nested sub-chapters
        let gs = &config.chapters[1];
        assert_eq!(gs.title, "Getting Started");
        assert_eq!(gs.section_string(), "2.");
        assert_eq!(gs.children.len(), 2);
        assert_eq!(gs.children[0].title, "Installation");
        assert_eq!(gs.children[0].section_string(), "2.1.");
        assert_eq!(gs.children[1].section_string(), "2.2.");

        // Draft
        let draft = &config.chapters[3];
        assert!(draft.is_draft());
        assert_eq!(draft.section_string(), "4.");

        // Flattened (non-draft, with files): intro + gs + install + first-note + features + appendix = 6
        let flat = config.flatten_chapters();
        assert_eq!(flat.len(), 6);

        // HTML config
        assert_eq!(config.html.default_theme, "dark");
        assert_eq!(config.html.search.limit_results, 20);

        let _ = fs::remove_dir_all(&tmp);
    }
}
