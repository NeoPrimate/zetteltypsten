use crate::compiled::NoteInfo;
use crate::extractor;
use crate::link_graph::LinkGraph;
use crate::tag_index::TagIndex;
use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use zt_core::note::NoteId;

/// Holds the complete index state for a vault.
pub struct VaultIndex {
    pub link_graph: LinkGraph,
    pub tag_index: TagIndex,
    /// Map from NoteId to extracted title.
    pub titles: HashMap<NoteId, String>,
    /// Map from NoteId to the context line for each outgoing link
    /// (used for backlink display).
    pub link_contexts: HashMap<(NoteId, NoteId), String>,
    /// Map from label name to the NoteId that declares it.
    /// E.g., "intro" → NoteId("notes/my-paper")
    pub label_map: HashMap<String, NoteId>,
    /// Map from label name to its display text (heading text or context).
    /// E.g., "Hello" → "My Title" (from `= My Title <Hello>`)
    pub label_text_map: HashMap<String, String>,
}

impl VaultIndex {
    pub fn new() -> Self {
        Self {
            link_graph: LinkGraph::new(),
            tag_index: TagIndex::new(),
            titles: HashMap::new(),
            link_contexts: HashMap::new(),
            label_map: HashMap::new(),
            label_text_map: HashMap::new(),
        }
    }

    /// Build the full index by scanning all .typ files in a vault.
    pub fn build(vault_root: &Path) -> Result<Self> {
        let mut index = Self::new();
        let files = zt_fs::scanner::scan_typ_files(vault_root)?;

        for entry in &files {
            let content = std::fs::read_to_string(&entry.path)?;
            let note_id = NoteId::from_path(&entry.rel_path);
            index.index_note(&note_id, &content);
        }

        Ok(index)
    }

    /// Index (or re-index) a single note's content.
    pub fn index_note(&mut self, note_id: &NoteId, content: &str) {
        // Title is always the file stem (note name), not the first heading.
        let title = note_id.display_name().to_string();
        self.titles.insert(note_id.clone(), title);

        // Extract and index tags
        let tags = extractor::extract_tags(content);
        self.tag_index.set_tags(note_id, tags);

        // Extract labels declared in this note (with display text)
        let labels_with_text = extractor::extract_labels_with_text(content);
        // Remove old labels for this note
        self.label_map.retain(|_, nid| nid != note_id);
        self.label_text_map.retain(|k, _| self.label_map.contains_key(k));
        for (label, display) in &labels_with_text {
            self.label_map.insert(label.clone(), note_id.clone());
            self.label_text_map.insert(label.clone(), display.clone());
        }

        // Extract #link() calls, clear old outgoing, add new
        self.link_graph.clear_outgoing(note_id);
        let links = extractor::extract_links(content);
        for link in &links {
            let target_id = NoteId(link.raw_target.clone());
            self.link_graph.add_link(note_id, &target_id);

            // Store context line for backlink display
            let context = extract_context_line(content, link.span.start);
            self.link_contexts
                .insert((note_id.clone(), target_id), context);
        }

        // Extract @ref references — these create links to the note that owns the label
        let refs = extractor::extract_refs(content);
        for ref_label in &refs {
            if let Some(target_note) = self.label_map.get(ref_label) {
                if target_note != note_id {
                    self.link_graph.add_link(note_id, target_note);
                }
            }
        }

        // Ensure the note itself is in the graph
        self.link_graph.add_note(note_id.clone());
    }

    /// Index (or re-index) a single note from compiled document information.
    ///
    /// This is the preferred path when a note has just been compiled — it uses
    /// the fully resolved semantic data from the Typst introspector instead of
    /// re-parsing the source text.
    pub fn index_note_compiled(&mut self, note_id: &NoteId, info: &NoteInfo) {
        // Title is always the file stem (note name), not the first heading.
        let title = note_id.display_name().to_string();
        self.titles.insert(note_id.clone(), title);

        // Tags
        let tags: Vec<zt_core::tag::Tag> = info.tags.iter().map(|s| s.clone().into()).collect();
        self.tag_index.set_tags(note_id, tags);

        // Labels
        self.label_map.retain(|_, nid| nid != note_id);
        self.label_text_map.retain(|k, _| self.label_map.contains_key(k));
        for (label, display) in &info.labels {
            self.label_map.insert(label.clone(), note_id.clone());
            self.label_text_map.insert(label.clone(), display.clone());
        }

        // Outlinks (rel paths from zt-open: URLs)
        self.link_graph.clear_outgoing(note_id);
        for target in &info.outlinks {
            let target_id = NoteId(target.clone());
            self.link_graph.add_link(note_id, &target_id);
            self.link_contexts
                .insert((note_id.clone(), target_id), String::new());
        }

        // @ref references → link to note owning the label
        for ref_label in &info.refs {
            if let Some(target_note) = self.label_map.get(ref_label) {
                if target_note != note_id {
                    let target_note = target_note.clone();
                    self.link_graph.add_link(note_id, &target_note);
                }
            }
        }

        self.link_graph.add_note(note_id.clone());
    }

    /// Remove a note from the index.
    pub fn remove_note(&mut self, note_id: &NoteId) {
        self.link_graph.clear_outgoing(note_id);
        self.tag_index.remove_note(note_id);
        self.titles.remove(note_id);
        // Remove link contexts where this note is the source
        self.link_contexts.retain(|(src, _), _| src != note_id);
    }

    /// Get backlinks for a note with their context.
    pub fn backlinks_with_context(&self, note_id: &NoteId) -> Vec<BacklinkInfo> {
        self.link_graph
            .backlinks(note_id)
            .into_iter()
            .map(|source| {
                let title = self
                    .titles
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| source.display_name().to_string());
                let context = self
                    .link_contexts
                    .get(&(source.clone(), note_id.clone()))
                    .cloned()
                    .unwrap_or_default();
                BacklinkInfo {
                    source_id: source.clone(),
                    source_title: title,
                    context,
                }
            })
            .collect()
    }

    /// Get the title for a note, falling back to its display name.
    pub fn title_of(&self, note_id: &NoteId) -> String {
        self.titles
            .get(note_id)
            .cloned()
            .unwrap_or_else(|| note_id.display_name().to_string())
    }
}

impl Default for VaultIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about a single backlink.
#[derive(Clone, Debug)]
pub struct BacklinkInfo {
    pub source_id: NoteId,
    pub source_title: String,
    /// The line of source text containing the link.
    pub context: String,
}

/// Extract the line of text surrounding a byte offset for context display.
fn extract_context_line(content: &str, byte_offset: usize) -> String {
    let offset = byte_offset.min(content.len());
    // Find line start
    let line_start = content[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    // Find line end
    let line_end = content[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(content.len());
    content[line_start..line_end].trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn index_vault_with_links() {
        let tmp = std::env::temp_dir().join("zt_test_index");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        fs::write(
            tmp.join("note-a.typ"),
            r#"= Note A
#metadata(("rust", "test")) <tags>
See #link("note-b") for details.
"#,
        )
        .unwrap();

        fs::write(
            tmp.join("note-b.typ"),
            r#"= Note B
This links back to #link("note-a") and to #link("note-c").
"#,
        )
        .unwrap();

        fs::write(tmp.join("note-c.typ"), "= Note C\nNo links here.\n").unwrap();

        let index = VaultIndex::build(&tmp).unwrap();

        // Check link graph
        let a = NoteId("note-a".into());
        let b = NoteId("note-b".into());
        let c = NoteId("note-c".into());

        assert_eq!(index.link_graph.outgoing(&a).len(), 1); // a -> b
        assert_eq!(index.link_graph.outgoing(&b).len(), 2); // b -> a, b -> c
        assert_eq!(index.link_graph.backlinks(&b).len(), 1); // a -> b
        assert_eq!(index.link_graph.backlinks(&a).len(), 1); // b -> a

        // Check tags
        assert_eq!(index.tag_index.tags_for_note(&a).len(), 2);
        assert_eq!(index.tag_index.tag_count(), 2);

        // Check titles (file stem, not first heading)
        assert_eq!(index.titles.get(&a).unwrap(), "note-a");
        assert_eq!(index.titles.get(&b).unwrap(), "note-b");

        // Check backlinks with context
        let backlinks = index.backlinks_with_context(&b);
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].source_title, "note-a");
        assert!(backlinks[0].context.contains("#link(\"note-b\")"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
