use crate::Database;

/// A full-text search result.
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub note_id: String,
    pub title: String,
    pub snippet: String,
}

impl Database {
    /// Full-text search across all notes.
    /// Returns matching notes with title and a snippet of the matching content.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        if query.trim().is_empty() {
            return vec![];
        }

        // Escape FTS5 special characters and add prefix matching
        let fts_query = sanitize_fts_query(query);
        if fts_query.is_empty() {
            return vec![];
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT n.id, n.title,
                        snippet(notes_fts, 1, '<b>', '</b>', '...', 40)
                 FROM notes_fts f
                 JOIN notes n ON n.rowid = f.rowid
                 WHERE notes_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .unwrap_or_else(|e| {
                log::error!("FTS query prepare failed: {e}");
                panic!("FTS query failed");
            });

        stmt.query_map(rusqlite::params![fts_query, limit as i64], |row| {
            Ok(SearchResult {
                note_id: row.get(0)?,
                title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })
        .unwrap_or_else(|e| {
            log::error!("FTS query failed: {e}");
            panic!("FTS query failed");
        })
        .filter_map(|r| r.ok())
        .collect()
    }
}

/// Sanitize a user query for FTS5.
/// Splits into words and adds * for prefix matching.
fn sanitize_fts_query(query: &str) -> String {
    let words: Vec<String> = query
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .map(|w| {
            // Remove FTS5 special chars
            let clean: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if clean.is_empty() {
                String::new()
            } else {
                format!("\"{}\"*", clean)
            }
        })
        .filter(|w| !w.is_empty())
        .collect();

    words.join(" ")
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[test]
    fn full_text_search() {
        let db = Database::open_in_memory().unwrap();

        db.index_note("note-a", "= Rust Programming\nRust is a systems language.", 1)
            .unwrap();
        db.index_note("note-b", "= Python Basics\nPython is interpreted.", 1)
            .unwrap();
        db.index_note("note-c", "= Rust Web\nBuilding web apps with Rust.", 1)
            .unwrap();

        let results = db.search("rust", 10);
        assert_eq!(results.len(), 2);
        // Both rust notes should match
        let ids: Vec<&str> = results.iter().map(|r| r.note_id.as_str()).collect();
        assert!(ids.contains(&"note-a"));
        assert!(ids.contains(&"note-c"));

        let results = db.search("python", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_id, "note-b");
    }

    #[test]
    fn empty_search() {
        let db = Database::open_in_memory().unwrap();
        let results = db.search("", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn sanitize_query() {
        use super::sanitize_fts_query;
        assert_eq!(sanitize_fts_query("hello world"), "\"hello\"* \"world\"*");
        assert_eq!(sanitize_fts_query("rust-lang"), "\"rust-lang\"*");
        assert_eq!(sanitize_fts_query(""), "");
    }
}
