use crate::buffer::Buffer;

/// Cursor position and optional selection in a buffer.
///
/// All positions are byte offsets into the buffer.
#[derive(Clone, Debug)]
pub struct Cursor {
    /// The primary cursor position (byte offset).
    pub position: usize,
    /// If set, defines the anchor of a selection. The selection
    /// spans from `anchor` to `position`.
    pub anchor: Option<usize>,
}

impl Cursor {
    pub fn new(position: usize) -> Self {
        Self {
            position,
            anchor: None,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.anchor.is_some_and(|a| a != self.position)
    }

    pub fn selection_range(&self) -> Option<std::ops::Range<usize>> {
        let anchor = self.anchor?;
        if anchor == self.position {
            return None;
        }
        let start = anchor.min(self.position);
        let end = anchor.max(self.position);
        Some(start..end)
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    pub fn start_selection(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.position);
        }
    }

    // --- Character movement ---

    pub fn move_right(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            if let Some(range) = self.selection_range() {
                self.position = range.end;
                self.clear_selection();
                return;
            }
            self.clear_selection();
        }
        if self.position < buf.len_bytes() {
            let char_idx = buf.byte_to_char(self.position);
            if char_idx < buf.len_chars() {
                self.position = buf.char_to_byte(char_idx + 1);
            }
        }
    }

    pub fn move_left(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            if let Some(range) = self.selection_range() {
                self.position = range.start;
                self.clear_selection();
                return;
            }
            self.clear_selection();
        }
        if self.position > 0 {
            let char_idx = buf.byte_to_char(self.position);
            if char_idx > 0 {
                self.position = buf.char_to_byte(char_idx - 1);
            }
        }
    }

    pub fn move_up(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        let line = buf.line_of_byte(self.position);
        if line > 0 {
            let col = buf.col_of_byte(self.position);
            let prev_line_start = buf.line_start_byte(line - 1);
            let prev_line_len = buf.line(line - 1).trim_end_matches('\n').len();
            self.position = prev_line_start + col.min(prev_line_len);
        }
    }

    pub fn move_down(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        let line = buf.line_of_byte(self.position);
        if line + 1 < buf.len_lines() {
            let col = buf.col_of_byte(self.position);
            let next_line_start = buf.line_start_byte(line + 1);
            let next_line_len = buf.line(line + 1).trim_end_matches('\n').len();
            self.position = next_line_start + col.min(next_line_len);
        }
    }

    // --- Line start/end (Home/End, Cmd+Left/Right) ---

    pub fn move_to_line_start(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        let line = buf.line_of_byte(self.position);
        self.position = buf.line_start_byte(line);
    }

    pub fn move_to_line_end(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        let line = buf.line_of_byte(self.position);
        let line_text = buf.line(line);
        let line_start = buf.line_start_byte(line);
        self.position = line_start + line_text.trim_end_matches('\n').len();
    }

    // --- Buffer start/end (Cmd+Up/Down) ---

    pub fn move_to_start(&mut self, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        self.position = 0;
    }

    pub fn move_to_end(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            self.clear_selection();
        }
        self.position = buf.len_bytes();
    }

    // --- Word movement (Option+Left/Right) ---

    pub fn move_word_left(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            if let Some(range) = self.selection_range() {
                self.position = range.start;
                self.clear_selection();
                return;
            }
            self.clear_selection();
        }
        self.position = find_word_boundary_left(buf, self.position);
    }

    pub fn move_word_right(&mut self, buf: &Buffer, selecting: bool) {
        if selecting {
            self.start_selection();
        } else {
            if let Some(range) = self.selection_range() {
                self.position = range.end;
                self.clear_selection();
                return;
            }
            self.clear_selection();
        }
        self.position = find_word_boundary_right(buf, self.position);
    }

    pub fn clamp(&mut self, buf: &Buffer) {
        self.position = self.position.min(buf.len_bytes());
        if let Some(ref mut anchor) = self.anchor {
            *anchor = (*anchor).min(buf.len_bytes());
        }
    }
}

/// Find the byte offset of the previous word boundary.
fn find_word_boundary_left(buf: &Buffer, from: usize) -> usize {
    if from == 0 {
        return 0;
    }
    let text = buf.text();
    let bytes = text.as_bytes();
    let mut pos = from;

    // Skip whitespace/punctuation going left
    while pos > 0 && !bytes[pos - 1].is_ascii_alphanumeric() && bytes[pos - 1] != b'_' {
        pos -= 1;
    }
    // Skip word chars going left
    while pos > 0 && (bytes[pos - 1].is_ascii_alphanumeric() || bytes[pos - 1] == b'_') {
        pos -= 1;
    }
    pos
}

/// Find the byte offset of the next word boundary.
fn find_word_boundary_right(buf: &Buffer, from: usize) -> usize {
    let text = buf.text();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut pos = from;

    // Skip word chars going right
    while pos < len && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
        pos += 1;
    }
    // Skip whitespace/punctuation going right
    while pos < len && !bytes[pos].is_ascii_alphanumeric() && bytes[pos] != b'_' {
        pos += 1;
    }
    pos
}
