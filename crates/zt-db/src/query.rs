use std::collections::HashMap;

use crate::Database;

/// Information about a single backlink.
#[derive(Clone, Debug)]
pub struct BacklinkInfo {
    pub source_id: String,
    pub source_title: String,
    pub context: String,
}

impl Database {
    /// Get all notes that link TO the given note, with context.
    pub fn backlinks(&self, note_id: &str) -> Vec<BacklinkInfo> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT l.source_id, COALESCE(n.title, l.source_id), l.context
                 FROM links l
                 LEFT JOIN notes n ON n.id = l.source_id
                 WHERE l.target_id = ?1
                 ORDER BY n.title",
            )
            .unwrap_or_else(|e| panic!("backlinks query failed: {e}"));

        stmt.query_map([note_id], |row| {
            Ok(BacklinkInfo {
                source_id: row.get(0)?,
                source_title: row.get(1)?,
                context: row.get(2)?,
            })
        })
        .unwrap_or_else(|e| panic!("backlinks map failed: {e}"))
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Get all notes that this note links TO, with titles.
    pub fn outgoing(&self, note_id: &str) -> Vec<(String, String)> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT l.target_id, COALESCE(n.title, l.target_id)
                 FROM links l
                 LEFT JOIN notes n ON n.id = l.target_id
                 WHERE l.source_id = ?1
                 ORDER BY n.title",
            )
            .unwrap_or_else(|e| panic!("outgoing query failed: {e}"));

        stmt.query_map([note_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap_or_else(|e| panic!("outgoing map failed: {e}"))
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get all tags for a note.
    pub fn tags_for_note(&self, note_id: &str) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM tags WHERE note_id = ?1 ORDER BY tag")
            .unwrap();

        stmt.query_map([note_id], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get all unique tags in the vault.
    pub fn all_tags(&self) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT tag FROM tags ORDER BY tag")
            .unwrap();

        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get all notes with a given tag.
    pub fn notes_with_tag(&self, tag: &str) -> Vec<String> {
        let mut stmt = self
            .conn
            .prepare("SELECT note_id FROM tags WHERE tag = ?1 ORDER BY note_id")
            .unwrap();

        stmt.query_map([tag], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get the title of a note.
    pub fn title_of(&self, note_id: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT title FROM notes WHERE id = ?1",
                [note_id],
                |r| r.get(0),
            )
            .ok()
    }

    /// Get all (note_id, title) pairs — for quick switcher.
    pub fn all_titles(&self) -> Vec<(String, String)> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title FROM notes ORDER BY title")
            .unwrap();

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get all nodes for graph visualization: (id, title).
    pub fn all_nodes(&self) -> Vec<(String, String)> {
        self.all_titles()
    }

    /// Get all edges for graph visualization: (source_id, target_id).
    pub fn all_edges(&self) -> Vec<(String, String)> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_id, target_id FROM links")
            .unwrap();

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get the label → note_id map for cross-note @ref compilation.
    pub fn label_map(&self) -> HashMap<String, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, note_id FROM labels")
            .unwrap();

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get the note_id → title map for @ref display text.
    pub fn title_map(&self) -> HashMap<String, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title FROM notes")
            .unwrap();

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get the label → display_text map for @ref rendering.
    pub fn label_text_map(&self) -> HashMap<String, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, display_text FROM labels")
            .unwrap();

        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }

    /// Get total note count.
    pub fn note_count(&self) -> usize {
        self.conn
            .query_row("SELECT count(*) FROM notes", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    /// Get total link count.
    pub fn link_count(&self) -> usize {
        self.conn
            .query_row("SELECT count(*) FROM links", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[test]
    fn backlinks_and_outgoing() {
        let db = Database::open_in_memory().unwrap();

        db.index_note("note-a", "= Note A <hello>\nContent.", 1)
            .unwrap();
        db.index_note(
            "note-b",
            "= Note B\nSee #link(\"note-a\")[Note A].",
            1,
        )
        .unwrap();

        let backlinks = db.backlinks("note-a");
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].source_id, "note-b");
        assert_eq!(backlinks[0].source_title, "Note B");

        let outgoing = db.outgoing("note-b");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].0, "note-a");
    }

    #[test]
    fn label_and_title_maps() {
        let db = Database::open_in_memory().unwrap();
        db.index_note("note-a", "= My Title <hello>", 1).unwrap();

        let lm = db.label_map();
        assert_eq!(lm.get("hello"), Some(&"note-a".to_string()));

        let tm = db.title_map();
        assert_eq!(tm.get("note-a"), Some(&"My Title".to_string()));

        let ltm = db.label_text_map();
        assert_eq!(ltm.get("hello"), Some(&"My Title".to_string()));
    }

    #[test]
    fn graph_data() {
        let db = Database::open_in_memory().unwrap();
        db.index_note("a", "= A\n#link(\"b\")[B]", 1).unwrap();
        db.index_note("b", "= B", 1).unwrap();

        let nodes = db.all_nodes();
        assert_eq!(nodes.len(), 2);

        let edges = db.all_edges();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0], ("a".to_string(), "b".to_string()));
    }
}
