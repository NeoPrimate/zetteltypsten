/// A simple undo/redo stack for text editing operations.
#[derive(Clone)]
pub struct History {
    /// Stack of past buffer snapshots (full text for simplicity in Phase 2).
    undo_stack: Vec<Snapshot>,
    /// Stack of undone snapshots for redo.
    redo_stack: Vec<Snapshot>,
    /// Maximum number of undo entries.
    max_entries: usize,
}

#[derive(Clone)]
struct Snapshot {
    text: String,
    cursor_position: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries: 500,
        }
    }

    /// Save the current state before making a change.
    pub fn push(&mut self, text: String, cursor_position: usize) {
        self.redo_stack.clear();
        self.undo_stack.push(Snapshot {
            text,
            cursor_position,
        });
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.remove(0);
        }
    }

    /// Undo: returns the previous (text, cursor_position) or None.
    pub fn undo(&mut self, current_text: String, current_cursor: usize) -> Option<(String, usize)> {
        let snapshot = self.undo_stack.pop()?;
        self.redo_stack.push(Snapshot {
            text: current_text,
            cursor_position: current_cursor,
        });
        Some((snapshot.text, snapshot.cursor_position))
    }

    /// Redo: returns the next (text, cursor_position) or None.
    pub fn redo(&mut self, current_text: String, current_cursor: usize) -> Option<(String, usize)> {
        let snapshot = self.redo_stack.pop()?;
        self.undo_stack.push(Snapshot {
            text: current_text,
            cursor_position: current_cursor,
        });
        Some((snapshot.text, snapshot.cursor_position))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}
