use std::collections::{HashMap, HashSet};
use zt_core::note::NoteId;
use zt_core::tag::Tag;

/// Bidirectional index mapping tags to notes and notes to tags.
pub struct TagIndex {
    tag_to_notes: HashMap<Tag, HashSet<NoteId>>,
    note_to_tags: HashMap<NoteId, HashSet<Tag>>,
}

impl TagIndex {
    pub fn new() -> Self {
        Self {
            tag_to_notes: HashMap::new(),
            note_to_tags: HashMap::new(),
        }
    }

    /// Set the tags for a note (replaces any previous tags).
    pub fn set_tags(&mut self, note: &NoteId, tags: Vec<Tag>) {
        // Remove old tags for this note
        if let Some(old_tags) = self.note_to_tags.remove(note) {
            for tag in &old_tags {
                if let Some(notes) = self.tag_to_notes.get_mut(tag) {
                    notes.remove(note);
                    if notes.is_empty() {
                        self.tag_to_notes.remove(tag);
                    }
                }
            }
        }

        // Add new tags
        let tag_set: HashSet<Tag> = tags.into_iter().collect();
        for tag in &tag_set {
            self.tag_to_notes
                .entry(tag.clone())
                .or_default()
                .insert(note.clone());
        }
        if !tag_set.is_empty() {
            self.note_to_tags.insert(note.clone(), tag_set);
        }
    }

    /// Remove a note from the index entirely.
    pub fn remove_note(&mut self, note: &NoteId) {
        if let Some(tags) = self.note_to_tags.remove(note) {
            for tag in &tags {
                if let Some(notes) = self.tag_to_notes.get_mut(tag) {
                    notes.remove(note);
                    if notes.is_empty() {
                        self.tag_to_notes.remove(tag);
                    }
                }
            }
        }
    }

    /// Get all notes with a given tag.
    pub fn notes_with_tag(&self, tag: &Tag) -> Vec<&NoteId> {
        self.tag_to_notes
            .get(tag)
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    /// Get all tags for a given note.
    pub fn tags_for_note(&self, note: &NoteId) -> Vec<&Tag> {
        self.note_to_tags
            .get(note)
            .map(|s| s.iter().collect())
            .unwrap_or_default()
    }

    /// Get all known tags.
    pub fn all_tags(&self) -> Vec<&Tag> {
        self.tag_to_notes.keys().collect()
    }

    /// Total number of unique tags.
    pub fn tag_count(&self) -> usize {
        self.tag_to_notes.len()
    }
}

impl Default for TagIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_index_basics() {
        let mut idx = TagIndex::new();
        let note_a = NoteId("a".into());
        let note_b = NoteId("b".into());
        let rust = Tag::new("rust");
        let gui = Tag::new("gui");

        idx.set_tags(&note_a, vec![rust.clone(), gui.clone()]);
        idx.set_tags(&note_b, vec![rust.clone()]);

        assert_eq!(idx.notes_with_tag(&rust).len(), 2);
        assert_eq!(idx.notes_with_tag(&gui).len(), 1);
        assert_eq!(idx.tags_for_note(&note_a).len(), 2);
    }

    #[test]
    fn replacing_tags() {
        let mut idx = TagIndex::new();
        let note = NoteId("a".into());
        let rust = Tag::new("rust");
        let go = Tag::new("go");

        idx.set_tags(&note, vec![rust.clone()]);
        assert_eq!(idx.notes_with_tag(&rust).len(), 1);

        idx.set_tags(&note, vec![go.clone()]);
        assert_eq!(idx.notes_with_tag(&rust).len(), 0);
        assert_eq!(idx.notes_with_tag(&go).len(), 1);
    }
}
