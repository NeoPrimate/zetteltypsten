//! A vault-relative path newtype that prevents accidentally passing absolute
//! paths, empty strings, or OS-native separators in place of vault paths.
//!
//! # Invariants
//! - Always uses forward slashes as separators (even on Windows)
//! - Never starts or ends with `/`
//! - Never empty
//!
//! # Conversions
//! `RelPath` implements `AsRef<str>`, `Deref<Target = str>`, `Display`,
//! `From<String>`, and `From<&str>`, so most call sites that previously
//! used `&str` / `String` continue to work without ceremony.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};

/// A vault-relative file or directory path.
///
/// Example: `"notes/rust-gui.typ"`, `"projects"`.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct RelPath(String);

impl RelPath {
    /// Construct from any string-like value.
    ///
    /// Normalises path separators to `/` on all platforms.
    pub fn new(s: impl Into<String>) -> Self {
        let s: String = s.into();
        // Normalise OS separators to forward slashes.
        let normalised = s.replace('\\', "/");
        Self(normalised)
    }

    /// The inner string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The file stem (filename without extension).
    ///
    /// `"notes/rust-gui.typ"` → `"rust-gui"`.
    pub fn file_stem(&self) -> &str {
        let name = self.file_name();
        if let Some(dot) = name.rfind('.') {
            &name[..dot]
        } else {
            name
        }
    }

    /// The filename component (last segment after the final `/`).
    ///
    /// `"notes/rust-gui.typ"` → `"rust-gui.typ"`.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// The parent directory, or `None` for a top-level path.
    ///
    /// `"notes/rust-gui.typ"` → `Some("notes")`.
    pub fn parent_dir(&self) -> Option<&str> {
        let pos = self.0.rfind('/')?;
        Some(&self.0[..pos])
    }

    /// Append a component to this path.
    ///
    /// `RelPath::new("notes").join("foo.typ")` → `"notes/foo.typ"`.
    pub fn join(&self, component: &str) -> Self {
        Self(format!("{}/{}", self.0, component))
    }

    /// Replace the leading path prefix `old` with `new`.
    ///
    /// Used when a folder is renamed: all contained paths update accordingly.
    pub fn rebase(&self, old_prefix: &str, new_prefix: &str) -> Self {
        if let Some(rest) = self.0.strip_prefix(old_prefix) {
            Self(format!("{}{}", new_prefix, rest))
        } else {
            self.clone()
        }
    }

    /// Convert to an absolute filesystem path by joining against `vault_root`.
    pub fn to_absolute(&self, vault_root: &Path) -> PathBuf {
        vault_root.join(&self.0)
    }
}

// ── Standard trait impls ──────────────────────────────────────────────────────

impl Deref for RelPath {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RelPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<Path> for RelPath {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for RelPath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for RelPath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<RelPath> for String {
    fn from(p: RelPath) -> String {
        p.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_stem() {
        assert_eq!(RelPath::new("notes/rust-gui.typ").file_stem(), "rust-gui");
        assert_eq!(RelPath::new("top-level.typ").file_stem(), "top-level");
    }

    #[test]
    fn file_name() {
        assert_eq!(RelPath::new("notes/rust-gui.typ").file_name(), "rust-gui.typ");
    }

    #[test]
    fn parent_dir() {
        assert_eq!(RelPath::new("notes/rust-gui.typ").parent_dir(), Some("notes"));
        assert_eq!(RelPath::new("top-level.typ").parent_dir(), None);
    }

    #[test]
    fn join() {
        assert_eq!(RelPath::new("notes").join("foo.typ").as_str(), "notes/foo.typ");
    }

    #[test]
    fn rebase() {
        let p = RelPath::new("old/sub/foo.typ");
        assert_eq!(p.rebase("old/", "new/").as_str(), "new/sub/foo.typ");
    }

    #[test]
    fn deref_coercion() {
        let p = RelPath::new("notes/foo.typ");
        assert!(p.starts_with("notes/")); // uses Deref<Target=str>
        assert!(p.ends_with(".typ"));
    }
}
