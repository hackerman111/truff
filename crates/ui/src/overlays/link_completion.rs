use truff_core::Buffer;

/// A candidate suggestion for Wiki-link completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCandidate {
    pub display: String,
    pub target: String,
    pub description: String,
}

/// Floating link completion overlay state for editor `[[` triggers (§5, §8).
#[derive(Debug, Clone, Default)]
pub struct LinkCompletionOverlay {
    pub active: bool,
    pub trigger_byte: usize,
    pub query: String,
    pub selected: usize,
    pub candidates: Vec<LinkCandidate>,
}

impl LinkCompletionOverlay {
    /// Detects if `cursor` inside `buffer` is positioned after an unclosed `[[` on the current line.
    /// Returns `Some((trigger_byte_pos, query_string))`.
    pub fn detect_trigger(buffer: &Buffer, cursor: usize) -> Option<(usize, String)> {
        let cursor = cursor.min(buffer.byte_len());
        let line_idx = buffer.line_of_byte(cursor);
        let line_start = buffer.byte_of_line(line_idx);
        if cursor < line_start {
            return None;
        }
        let line_to_cursor = buffer.slice(line_start..cursor).to_string();
        let pos = line_to_cursor.rfind("[[")?;
        let after_brackets = &line_to_cursor[pos + 2..];
        if after_brackets.contains("]]") {
            return None;
        }
        Some((line_start + pos, after_brackets.to_string()))
    }

    /// Selects the next candidate item cyclically.
    pub fn next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = (self.selected + 1) % self.candidates.len();
        }
    }

    /// Selects the previous candidate item cyclically.
    pub fn prev(&mut self) {
        if !self.candidates.is_empty() {
            if self.selected == 0 {
                self.selected = self.candidates.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Returns the currently selected candidate, if any.
    pub fn selected_candidate(&self) -> Option<&LinkCandidate> {
        self.candidates.get(self.selected)
    }

    /// Dismisses the completion popup.
    pub fn dismiss(&mut self) {
        self.active = false;
        self.candidates.clear();
        self.query.clear();
        self.selected = 0;
    }
}
