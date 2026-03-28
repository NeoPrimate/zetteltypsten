use ropey::Rope;

/// A rope-based text buffer for editing Typst source files.
#[derive(Clone)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Insert text at a byte offset.
    pub fn insert(&mut self, byte_offset: usize, text: &str) {
        let char_idx = self.rope.byte_to_char(byte_offset);
        self.rope.insert(char_idx, text);
    }

    /// Delete a range of bytes.
    pub fn delete(&mut self, byte_range: std::ops::Range<usize>) {
        let start = self.rope.byte_to_char(byte_range.start);
        let end = self.rope.byte_to_char(byte_range.end);
        self.rope.remove(start..end);
    }

    /// Replace a byte range with new text.
    pub fn replace(&mut self, byte_range: std::ops::Range<usize>, text: &str) {
        self.delete(byte_range.clone());
        self.insert(byte_range.start, text);
    }

    /// Get the line number (0-indexed) for a byte offset.
    pub fn line_of_byte(&self, byte_offset: usize) -> usize {
        let char_idx = self.rope.byte_to_char(byte_offset.min(self.len_bytes()));
        self.rope.char_to_line(char_idx)
    }

    /// Get the byte offset of the start of a line.
    pub fn line_start_byte(&self, line: usize) -> usize {
        let char_idx = self.rope.line_to_char(line);
        self.rope.char_to_byte(char_idx)
    }

    /// Get the text of a specific line (including newline if present).
    pub fn line(&self, line_idx: usize) -> String {
        self.rope.line(line_idx).to_string()
    }

    /// Get the column (byte offset within line) for a byte offset.
    pub fn col_of_byte(&self, byte_offset: usize) -> usize {
        let line = self.line_of_byte(byte_offset);
        byte_offset - self.line_start_byte(line)
    }

    /// Convert a char index to a byte offset.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx)
    }

    /// Convert a byte offset to a char index.
    pub fn byte_to_char(&self, byte_offset: usize) -> usize {
        self.rope.byte_to_char(byte_offset)
    }

    /// Get a slice of text as bytes.
    pub fn slice_bytes(&self, byte_range: std::ops::Range<usize>) -> String {
        let start = self.rope.byte_to_char(byte_range.start);
        let end = self.rope.byte_to_char(byte_range.end);
        self.rope.slice(start..end).to_string()
    }

    /// Get a slice of text by byte range (alias for slice_bytes).
    pub fn slice(&self, byte_range: std::ops::Range<usize>) -> String {
        self.slice_bytes(byte_range)
    }

    /// Swap two lines. Returns the byte offset of the start of the second line after swap.
    pub fn swap_lines(&mut self, line_a: usize, line_b: usize) -> usize {
        if line_a == line_b || line_a >= self.len_lines() || line_b >= self.len_lines() {
            return self.line_start_byte(line_a);
        }
        let (first, second) = if line_a < line_b {
            (line_a, line_b)
        } else {
            (line_b, line_a)
        };

        let first_text = self.line(first);
        let second_text = self.line(second);

        // Ensure both have newlines for consistent replacement
        let first_clean = first_text.trim_end_matches('\n');
        let second_clean = second_text.trim_end_matches('\n');

        let first_range = self.line_byte_range(self.line_start_byte(first));
        let second_range = self.line_byte_range(self.line_start_byte(second));

        // Replace second line first (higher offset) to avoid invalidating first's range
        if second_text.ends_with('\n') {
            self.replace(second_range.clone(), &format!("{first_clean}\n"));
        } else {
            self.replace(second_range.clone(), first_clean);
        }

        if first_text.ends_with('\n') {
            self.replace(first_range.clone(), &format!("{second_clean}\n"));
        } else {
            self.replace(first_range.clone(), second_clean);
        }

        self.line_start_byte(first)
    }

    /// Duplicate the line at the given byte offset, inserting a copy below.
    /// Returns the byte offset of the start of the new (duplicated) line.
    pub fn duplicate_line(&mut self, byte_offset: usize) -> usize {
        let line_idx = self.line_of_byte(byte_offset);
        let line_start = self.line_start_byte(line_idx);
        let line_text = self.line(line_idx);
        let has_newline = line_text.ends_with('\n');
        let insert_pos = line_start + line_text.len();

        if has_newline {
            // Line already has newline, insert copy after it
            self.insert(insert_pos, &line_text);
            insert_pos
        } else {
            // Last line — add newline then copy
            let to_insert = format!("\n{line_text}");
            self.insert(insert_pos, &to_insert);
            insert_pos + 1 // skip the newline
        }
    }

    /// Get the byte range of the full line at the given byte offset (including newline).
    pub fn line_byte_range(&self, byte_offset: usize) -> std::ops::Range<usize> {
        let line_idx = self.line_of_byte(byte_offset);
        let start = self.line_start_byte(line_idx);
        let line_text = self.line(line_idx);
        start..start + line_text.len()
    }

    /// Delete the word before the cursor (Option+Backspace).
    pub fn delete_word_backward(&mut self, byte_offset: usize) -> usize {
        if byte_offset == 0 {
            return 0;
        }
        let text = self.text();
        let bytes = text.as_bytes();
        let mut pos = byte_offset;

        // Skip whitespace going left
        while pos > 0 && bytes[pos - 1].is_ascii_whitespace() && bytes[pos - 1] != b'\n' {
            pos -= 1;
        }
        // Skip word chars going left
        while pos > 0 && (bytes[pos - 1].is_ascii_alphanumeric() || bytes[pos - 1] == b'_') {
            pos -= 1;
        }
        // If we didn't move, delete one char
        if pos == byte_offset && pos > 0 {
            pos -= 1;
        }

        self.delete(pos..byte_offset);
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_read() {
        let mut buf = Buffer::new("hello world");
        buf.insert(5, " beautiful");
        assert_eq!(buf.text(), "hello beautiful world");
    }

    #[test]
    fn delete_range() {
        let mut buf = Buffer::new("hello beautiful world");
        buf.delete(5..15);
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn line_operations() {
        let buf = Buffer::new("line one\nline two\nline three");
        assert_eq!(buf.len_lines(), 3);
        assert_eq!(buf.line_of_byte(0), 0);
        assert_eq!(buf.line_of_byte(9), 1);
        assert_eq!(buf.line(1), "line two\n");
    }
}
