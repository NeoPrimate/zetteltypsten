use anyhow::{Context, Result};
use std::path::Path;

use crate::Database;
use zt_index::extractor;

impl Database {
    /// Index a single note: extract metadata and store in DB.
    /// Replaces all existing data for this note_id.
    pub fn index_note(&self, note_id: &str, source: &str, mtime: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        // Clear existing data for this note
        tx.execute("DELETE FROM links WHERE source_id = ?1", [note_id])?;
        tx.execute("DELETE FROM tags WHERE note_id = ?1", [note_id])?;
        tx.execute("DELETE FROM labels WHERE note_id = ?1", [note_id])?;
        tx.execute("DELETE FROM refs WHERE source_id = ?1", [note_id])?;

        // Title is always the file stem (note name), not the first heading.
        let title = zt_core::note::NoteId(note_id.to_string()).display_name().to_string();

        // Upsert note
        tx.execute(
            "INSERT OR REPLACE INTO notes (id, title, content, modified_at) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![note_id, title, source, mtime],
        )?;

        // Extract and store tags
        let tags = extractor::extract_tags(source);
        for tag in &tags {
            tx.execute(
                "INSERT OR IGNORE INTO tags (note_id, tag) VALUES (?1, ?2)",
                rusqlite::params![note_id, tag.0],
            )?;
        }

        // Extract and store labels
        let labels = extractor::extract_labels_with_text(source);
        for (label, display_text) in &labels {
            tx.execute(
                "INSERT OR REPLACE INTO labels (name, note_id, display_text) VALUES (?1, ?2, ?3)",
                rusqlite::params![label, note_id, display_text],
            )?;
        }

        // Extract and store links
        let links = extractor::extract_links(source);
        for link in &links {
            let context = extract_context_line(source, link.span.start);
            tx.execute(
                "INSERT OR IGNORE INTO links (source_id, target_id, context) VALUES (?1, ?2, ?3)",
                rusqlite::params![note_id, link.raw_target, context],
            )?;
        }

        // Extract and store @refs
        let refs = extractor::extract_refs(source);
        for ref_label in &refs {
            tx.execute(
                "INSERT OR IGNORE INTO refs (source_id, label_name) VALUES (?1, ?2)",
                rusqlite::params![note_id, ref_label],
            )?;

            // Resolve ref to a link: find which note owns this label
            let target: Option<String> = tx
                .query_row(
                    "SELECT note_id FROM labels WHERE name = ?1",
                    [ref_label],
                    |r| r.get(0),
                )
                .ok();

            if let Some(target_id) = target {
                let context = format!("@{}", ref_label);
                tx.execute(
                    "INSERT OR IGNORE INTO links (source_id, target_id, context) VALUES (?1, ?2, ?3)",
                    rusqlite::params![note_id, target_id, context],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Remove a note and all its associated data.
    pub fn remove_note(&self, note_id: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM links WHERE source_id = ?1", [note_id])?;
        tx.execute("DELETE FROM tags WHERE note_id = ?1", [note_id])?;
        tx.execute("DELETE FROM labels WHERE note_id = ?1", [note_id])?;
        tx.execute("DELETE FROM refs WHERE source_id = ?1", [note_id])?;
        tx.execute("DELETE FROM notes WHERE id = ?1", [note_id])?;
        tx.commit()?;
        Ok(())
    }

    /// Incremental sync: scan vault, reindex only changed files.
    /// Returns number of notes updated.
    pub fn sync(&self, vault_root: &Path) -> Result<usize> {
        let files = zt_fs::scanner::scan_typ_files(vault_root)?;

        // Get existing mtimes from DB
        let mut stmt = self.conn.prepare("SELECT id, modified_at FROM notes")?;
        let existing: std::collections::HashMap<String, i64> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();

        let mut updated = 0;
        let mut seen = std::collections::HashSet::new();

        for entry in &files {
            let note_id = zt_core::note::NoteId::from_path(&entry.rel_path);
            let id_str = note_id.0.clone();
            seen.insert(id_str.clone());

            // Get file mtime
            let mtime = std::fs::metadata(&entry.path)
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64
                })
                .unwrap_or(0);

            // Skip if unchanged
            if existing.get(&id_str) == Some(&mtime) {
                continue;
            }

            // Read and index
            let content = std::fs::read_to_string(&entry.path)
                .with_context(|| format!("Failed to read {}", entry.path.display()))?;

            self.index_note(&id_str, &content, mtime)?;
            updated += 1;
        }

        // Remove notes whose files no longer exist
        for (id, _) in &existing {
            if !seen.contains(id) {
                self.remove_note(id)?;
                updated += 1;
            }
        }

        // Resolve any pending @refs that couldn't be resolved during individual indexing
        // (because the target note hadn't been indexed yet)
        self.resolve_pending_refs()?;

        tracing::info!("Database synced: {} notes updated", updated);
        Ok(updated)
    }

    /// Public wrapper to resolve pending refs (used by save_active for single-note reindex).
    pub fn sync_refs_only(&self) -> Result<()> {
        self.resolve_pending_refs()
    }

    /// Resolve @refs to links by looking up labels.
    /// Called after sync to handle cross-note refs where the target was indexed after the source.
    fn resolve_pending_refs(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            INSERT OR IGNORE INTO links (source_id, target_id, context)
            SELECT r.source_id, l.note_id, '@' || r.label_name
            FROM refs r
            JOIN labels l ON r.label_name = l.name
            WHERE NOT EXISTS (
                SELECT 1 FROM links
                WHERE source_id = r.source_id AND target_id = l.note_id
            );
            ",
        )?;
        Ok(())
    }
}

/// Extract the line of text surrounding a byte offset for context display.
fn extract_context_line(source: &str, byte_offset: usize) -> String {
    let offset = byte_offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = source[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(source.len());
    source[line_start..line_end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_query_note() {
        let db = Database::open_in_memory().unwrap();

        let source = r#"= My Note <hello>

#metadata(("rust", "test")) <tags>

See #link("other-note")[Other Note] for details.

Also @world reference.
"#;

        db.index_note("my-note", source, 1000).unwrap();

        // Check note stored
        let title: String = db
            .conn
            .query_row("SELECT title FROM notes WHERE id = 'my-note'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(title, "my-note");

        // Check tags
        let tags: Vec<String> = {
            let mut stmt = db
                .conn
                .prepare("SELECT tag FROM tags WHERE note_id = 'my-note' ORDER BY tag")
                .unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert_eq!(tags, vec!["rust", "test"]);

        // Check labels
        let label_note: String = db
            .conn
            .query_row("SELECT note_id FROM labels WHERE name = 'hello'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(label_note, "my-note");

        // Check links
        let link_count: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM links WHERE source_id = 'my-note'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(link_count >= 1); // at least the #link

        // Check refs
        let ref_count: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM refs WHERE source_id = 'my-note'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ref_count, 1); // @world
    }

    #[test]
    fn remove_note_cleans_up() {
        let db = Database::open_in_memory().unwrap();
        db.index_note("test", "= Test\n#metadata((\"tag\")) <tags>", 1)
            .unwrap();

        let count: i64 = db
            .conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        db.remove_note("test").unwrap();

        let count: i64 = db
            .conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);

        let tag_count: i64 = db
            .conn
            .query_row("SELECT count(*) FROM tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tag_count, 0);
    }

    #[test]
    fn cross_note_refs_resolved() {
        let db = Database::open_in_memory().unwrap();

        // Note A declares <hello>
        db.index_note("note-a", "= Note A <hello>\nContent.", 1)
            .unwrap();

        // Note B references @hello
        db.index_note("note-b", "= Note B\nSee @hello for info.", 1)
            .unwrap();

        // Resolve refs
        db.resolve_pending_refs().unwrap();

        // Check that a link from note-b to note-a exists
        let link: Option<String> = db
            .conn
            .query_row(
                "SELECT target_id FROM links WHERE source_id = 'note-b' AND target_id = 'note-a'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert_eq!(link.as_deref(), Some("note-a"));
    }
}
