use chrono::{Datelike, Local};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use typst::diag::FileResult;
use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Feature, Library, LibraryExt, World};
use typst_kit::fonts::{FontSearcher, FontSlot};

/// Cached font data shared across all ZettelWorld instances.
/// Font scanning is expensive (~1s), so we do it exactly once.
struct FontCache {
    book: LazyHash<FontBook>,
    slots: Vec<FontSlot>,
}

static FONT_CACHE: OnceLock<FontCache> = OnceLock::new();

fn get_font_cache() -> &'static FontCache {
    FONT_CACHE.get_or_init(|| {
        let fonts = FontSearcher::new().search();
        FontCache {
            book: LazyHash::new(fonts.book.clone()),
            slots: fonts.fonts,
        }
    })
}

/// Eagerly initialise the font cache on a background thread so
/// the first file open doesn't stall.  Safe to call multiple times.
pub fn warm_font_cache() {
    std::thread::spawn(|| {
        get_font_cache();
    });
}

/// Implements `typst::World` for vault-scoped compilation.
///
/// Each note is compiled independently with the vault root as the
/// file system root, so `#import` paths are relative to the vault.
pub struct ZettelWorld {
    /// The vault root directory on disk.
    root: PathBuf,
    /// The file currently being compiled.
    main_id: FileId,
    /// Cached sources keyed by FileId.
    sources: HashMap<FileId, Source>,
    /// Typst standard library.
    library: LazyHash<Library>,
}

impl ZettelWorld {
    /// Create a new world rooted at the given vault path.
    ///
    /// `main_path` is the vault-relative path to the note being compiled,
    /// e.g. `"notes/hello.typ"`.
    pub fn new(root: PathBuf, main_path: &str) -> Self {
        // Ensure fonts are loaded (cached globally after first call).
        let _ = get_font_cache();
        let main_id = FileId::new(None, VirtualPath::new(main_path));

        Self {
            root,
            main_id,
            sources: HashMap::new(),
            library: LazyHash::new(
                Library::builder()
                    .with_features([Feature::Html].into_iter().collect())
                    .build(),
            ),
        }
    }

    /// Update (or insert) the source text for a given file.
    pub fn set_source(&mut self, path: &str, text: String) {
        let id = FileId::new(None, VirtualPath::new(path));
        let source = Source::new(id, text);
        self.sources.insert(id, source);
    }

    /// Resolve a Span to a byte range in the source text.
    /// Returns None if the span can't be resolved (e.g., built-in).
    pub fn range(&self, span: typst::syntax::Span) -> Option<std::ops::Range<usize>> {
        let id = span.id()?;
        let source = self.sources.get(&id)?;
        let range = source.range(span)?;
        Some(range)
    }

    /// Change which file is the main compilation target.
    pub fn set_main(&mut self, path: &str) {
        self.main_id = FileId::new(None, VirtualPath::new(path));
    }

    /// Resolve a FileId to an absolute path on disk.
    fn resolve(&self, id: FileId) -> Result<PathBuf, typst::diag::FileError> {
        let vpath = id.vpath();
        let rel = vpath
            .as_rooted_path()
            .strip_prefix("/")
            .unwrap_or(vpath.as_rooted_path());
        let abs = self.root.join(rel);
        if abs.exists() {
            Ok(abs)
        } else {
            Err(typst::diag::FileError::NotFound(abs))
        }
    }
}

impl World for ZettelWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &get_font_cache().book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(source) = self.sources.get(&id) {
            return Ok(source.clone());
        }

        let path = self.resolve(id)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|_| typst::diag::FileError::NotFound(path))?;
        Ok(Source::new(id, text))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve(id)?;
        let bytes = std::fs::read(&path)
            .map_err(|_| typst::diag::FileError::NotFound(path))?;
        Ok(Bytes::new(bytes))
    }

    fn font(&self, index: usize) -> Option<Font> {
        get_font_cache().slots.get(index)?.get()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        let now = Local::now();
        Datetime::from_ymd(
            now.year(),
            now.month().try_into().ok()?,
            now.day().try_into().ok()?,
        )
    }
}
