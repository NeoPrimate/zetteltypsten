use crate::link::Link;
use crate::tag::Tag;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Stable identifier for a note, derived from its vault-relative path.
///
/// For example, `"projects/rust-gui"` for the file `projects/rust-gui.typ`.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct NoteId(pub String);

impl NoteId {
    /// Construct from a vault-relative path, stripping the `.typ` extension.
    pub fn from_path(rel_path: &Path) -> Self {
        let s = rel_path
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        Self(s)
    }

    /// Convert back to the vault-relative `.typ` path.
    pub fn to_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.typ", self.0))
    }

    /// The short display name (last path component).
    pub fn display_name(&self) -> &str {
        self.0
            .rsplit('/')
            .next()
            .unwrap_or(&self.0)
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// In-memory representation of a single note.
pub struct Note {
    pub id: NoteId,
    pub title: String,
    pub content: String,
    pub metadata: NoteMetadata,
    pub modified_at: SystemTime,
}

/// A single heading entry extracted from note source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeadingEntry {
    /// Nesting level: 1 for `=`, 2 for `==`, etc.
    pub depth: usize,
    /// Heading text with labels and whitespace trimmed.
    pub text: String,
}

pub struct NoteMetadata {
    pub tags: Vec<Tag>,
    pub outgoing_links: Vec<Link>,
    pub aliases: Vec<String>,
    /// Ordered list of headings, used to build the outline sidebar.
    pub headings: Vec<HeadingEntry>,
}

impl NoteMetadata {
    pub fn empty() -> Self {
        Self {
            tags: Vec::new(),
            outgoing_links: Vec::new(),
            aliases: Vec::new(),
            headings: Vec::new(),
        }
    }
}

impl Note {
    /// Create a new note from raw file content.
    /// Title is always the file stem (note name), not the first heading.
    pub fn from_content(id: NoteId, content: String) -> Self {
        let title = id.display_name().to_string();
        Self {
            id,
            title,
            content,
            metadata: NoteMetadata::empty(),
            modified_at: SystemTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_id_from_path() {
        let id = NoteId::from_path(Path::new("projects/rust-gui.typ"));
        assert_eq!(id.0, "projects/rust-gui");
    }

    #[test]
    fn note_id_to_path() {
        let id = NoteId("daily/2025-01-01".into());
        assert_eq!(id.to_path(), PathBuf::from("daily/2025-01-01.typ"));
    }
}
