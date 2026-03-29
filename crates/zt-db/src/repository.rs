//! Typed query repository that wraps [`Database`] and returns `zt-core` domain
//! types instead of raw `String` tuples.
//!
//! # Usage
//!
//! ```rust,no_run
//! # use zt_db::{Database, NoteRepository};
//! # let db = Database::open_in_memory().unwrap();
//! let repo = NoteRepository::new(&db);
//! if let Some(row) = repo.get("notes/foo.typ") {
//!     println!("{}", row.title);
//! }
//! for tag in repo.tags_for("notes/foo.typ") {
//!     println!("{}", tag.as_str());
//! }
//! ```

use crate::Database;
use zt_core::note::NoteId;
use zt_core::tag::Tag;

// ── Row types ─────────────────────────────────────────────────────────────────

/// A complete note record as stored in the database.
#[derive(Clone, Debug)]
pub struct NoteRow {
    /// Stable note identifier derived from its vault-relative path.
    pub id: NoteId,
    /// Note title (file stem / note name).
    pub title: String,
    /// Full source text of the note.
    pub content: String,
    /// Unix timestamp (seconds) of the last modification.
    pub modified_at: i64,
}

/// One entry in the backlink list for a given note.
#[derive(Clone, Debug)]
pub struct BacklinkRow {
    /// The note that links to the queried note.
    pub source_id: NoteId,
    /// Resolved title of the source note (falls back to the id string).
    pub source_title: String,
    /// The line of text that contains the link.
    pub context: String,
}

// ── Repository ────────────────────────────────────────────────────────────────

/// Typed query layer over [`Database`].
///
/// Borrows the database for its lifetime; all methods are synchronous since
/// SQLite queries are fast enough to run on the GPUI main thread.
pub struct NoteRepository<'db> {
    db: &'db Database,
}

impl<'db> NoteRepository<'db> {
    /// Create a new repository view over an existing database connection.
    pub fn new(db: &'db Database) -> Self {
        Self { db }
    }

    // ── Note queries ──────────────────────────────────────────────────────────

    /// Fetch a single note by its vault-relative path (e.g. `"notes/foo.typ"`).
    ///
    /// Returns `None` if the note has not been indexed yet.
    pub fn get(&self, rel_path: &str) -> Option<NoteRow> {
        let note_id = NoteId::from_path(std::path::Path::new(rel_path));
        self.db
            .conn
            .query_row(
                "SELECT id, title, content, modified_at FROM notes WHERE id = ?1",
                [note_id.0.as_str()],
                |row| {
                    Ok(NoteRow {
                        id: NoteId(row.get(0)?),
                        title: row.get(1)?,
                        content: row.get(2)?,
                        modified_at: row.get(3)?,
                    })
                },
            )
            .ok()
    }

    /// Return every indexed note, ordered by title.
    pub fn all_notes(&self) -> Vec<NoteRow> {
        let mut stmt = match self.db.conn.prepare(
            "SELECT id, title, content, modified_at FROM notes ORDER BY title",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("all_notes prepare failed: {e}");
                return vec![];
            }
        };

        stmt.query_map([], |row| {
            Ok(NoteRow {
                id: NoteId(row.get(0)?),
                title: row.get(1)?,
                content: row.get(2)?,
                modified_at: row.get(3)?,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    // ── Link queries ──────────────────────────────────────────────────────────

    /// Notes that link **to** this note (backlinks), with context snippets.
    pub fn backlinks_for(&self, rel_path: &str) -> Vec<BacklinkRow> {
        let note_id = NoteId::from_path(std::path::Path::new(rel_path));
        let mut stmt = match self.db.conn.prepare(
            "SELECT l.source_id, COALESCE(n.title, l.source_id), l.context
             FROM links l
             LEFT JOIN notes n ON n.id = l.source_id
             WHERE l.target_id = ?1
             ORDER BY n.title",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("backlinks_for prepare failed: {e}");
                return vec![];
            }
        };

        stmt.query_map([note_id.0.as_str()], |row| {
            Ok(BacklinkRow {
                source_id: NoteId(row.get(0)?),
                source_title: row.get(1)?,
                context: row.get(2)?,
            })
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Notes that this note links **to** (outgoing links).
    pub fn outgoing_for(&self, rel_path: &str) -> Vec<(NoteId, String)> {
        let note_id = NoteId::from_path(std::path::Path::new(rel_path));
        let mut stmt = match self.db.conn.prepare(
            "SELECT l.target_id, COALESCE(n.title, l.target_id)
             FROM links l
             LEFT JOIN notes n ON n.id = l.target_id
             WHERE l.source_id = ?1
             ORDER BY n.title",
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("outgoing_for prepare failed: {e}");
                return vec![];
            }
        };

        stmt.query_map([note_id.0.as_str()], |row| {
            Ok((NoteId(row.get(0)?), row.get::<_, String>(1)?))
        })
        .into_iter()
        .flatten()
        .filter_map(|r| r.ok())
        .collect()
    }

    // ── Tag queries ───────────────────────────────────────────────────────────

    /// Tags attached to the given note, as typed [`Tag`] values.
    pub fn tags_for(&self, rel_path: &str) -> Vec<Tag> {
        let note_id = NoteId::from_path(std::path::Path::new(rel_path));
        let mut stmt = match self
            .db
            .conn
            .prepare("SELECT tag FROM tags WHERE note_id = ?1 ORDER BY tag")
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("tags_for prepare failed: {e}");
                return vec![];
            }
        };

        stmt.query_map([note_id.0.as_str()], |row| row.get::<_, String>(0))
            .into_iter()
            .flatten()
            .filter_map(|r| r.ok())
            .map(Tag::new)
            .collect()
    }

    /// All unique tags in the vault.
    pub fn all_tags(&self) -> Vec<Tag> {
        let mut stmt = match self
            .db
            .conn
            .prepare("SELECT DISTINCT tag FROM tags ORDER BY tag")
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("all_tags prepare failed: {e}");
                return vec![];
            }
        };

        stmt.query_map([], |row| row.get::<_, String>(0))
            .into_iter()
            .flatten()
            .filter_map(|r| r.ok())
            .map(Tag::new)
            .collect()
    }

    /// All notes that carry the given tag.
    pub fn notes_with_tag(&self, tag: &Tag) -> Vec<NoteId> {
        let mut stmt = match self
            .db
            .conn
            .prepare("SELECT note_id FROM tags WHERE tag = ?1 ORDER BY note_id")
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("notes_with_tag prepare failed: {e}");
                return vec![];
            }
        };

        stmt.query_map([tag.as_str()], |row| row.get::<_, String>(0))
            .into_iter()
            .flatten()
            .filter_map(|r| r.ok())
            .map(NoteId)
            .collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn get_and_all_notes() {
        let db = Database::open_in_memory().unwrap();
        // Note IDs follow the convention: vault-relative path without extension.
        db.index_note("notes/foo", "= Foo Note\nContent.", 100)
            .unwrap();

        let repo = NoteRepository::new(&db);

        // `get` accepts the `.typ` path and strips the extension internally.
        let row = repo.get("notes/foo.typ").unwrap();
        assert_eq!(row.title, "foo");
        assert_eq!(row.modified_at, 100);

        let all = repo.all_notes();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id.0, "notes/foo");
    }

    #[test]
    fn typed_tags() {
        let db = Database::open_in_memory().unwrap();
        db.index_note(
            "notes/foo",
            "= Foo\n#metadata((\"rust\", \"async\")) <tags>",
            1,
        )
        .unwrap();

        let repo = NoteRepository::new(&db);
        let tags = repo.tags_for("notes/foo.typ");
        let tag_strs: Vec<&str> = tags.iter().map(|t| t.as_str()).collect();
        assert!(tag_strs.contains(&"rust"));
        assert!(tag_strs.contains(&"async"));
    }

    #[test]
    fn typed_backlinks() {
        let db = Database::open_in_memory().unwrap();
        db.index_note("notes/a", "= Note A <hello>\nContent.", 1)
            .unwrap();
        db.index_note(
            "notes/b",
            "= Note B\nSee #link(\"notes/a\")[A].",
            1,
        )
        .unwrap();

        let repo = NoteRepository::new(&db);
        let backlinks = repo.backlinks_for("notes/a.typ");
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].source_id.0, "notes/b");
    }
}
