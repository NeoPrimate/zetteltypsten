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

pub struct NoteMetadata {
    pub tags: Vec<Tag>,
    pub outgoing_links: Vec<Link>,
    pub aliases: Vec<String>,
}

impl NoteMetadata {
    pub fn empty() -> Self {
        Self {
            tags: Vec::new(),
            outgoing_links: Vec::new(),
            aliases: Vec::new(),
        }
    }
}

impl Note {
    /// Create a new note from raw file content.
    pub fn from_content(id: NoteId, content: String) -> Self {
        let title = extract_title(&content).unwrap_or_else(|| id.display_name().to_string());
        Self {
            id,
            title,
            content,
            metadata: NoteMetadata::empty(),
            modified_at: SystemTime::now(),
        }
    }
}

/// Extract the first `= Heading` from Typst source as the note title.
/// Strips any trailing `<label>` syntax from the heading.
fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
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
    fn note_id_from_path() {
        let id = NoteId::from_path(Path::new("projects/rust-gui.typ"));
        assert_eq!(id.0, "projects/rust-gui");
    }

    #[test]
    fn note_id_to_path() {
        let id = NoteId("daily/2025-01-01".into());
        assert_eq!(id.to_path(), PathBuf::from("daily/2025-01-01.typ"));
    }

    #[test]
    fn extract_title_from_content() {
        let content = "#import \"_zettel.typ\": *\n\n= My Research Note\n\nSome content.";
        assert_eq!(extract_title(content), Some("My Research Note".into()));
    }

    #[test]
    fn extract_title_missing() {
        assert_eq!(extract_title("Just some text"), None);
    }

    #[test]
    fn extract_title_strips_label() {
        let content = "= My Note <my-label>\n\nSome content.";
        assert_eq!(extract_title(content), Some("My Note".into()));
    }

    #[test]
    fn extract_title_no_label() {
        let content = "= Plain Title\n\nContent.";
        assert_eq!(extract_title(content), Some("Plain Title".into()));
    }
}
