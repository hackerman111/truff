//! Unified input adapter handling keyboard, mouse, clipboard, and IME.
//!
//! Strict guarantees:
//! - IME preedit and cancellation NEVER modify the persistent text buffer or Rope.
//! - IME commit runs through EXACTLY ONE atomic `Buffer::apply` transaction.
//! - Grapheme cluster preservation on all edits and motions.

use truff_core::Buffer;
use truff_core::BytePos;
use truff_core::text::{ChangeSet, EditError, Transaction};

use truff_core::vim::motion::snap_grapheme;

/// Actions that can be performed on the editor buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorInput {
    /// Active IME preedit text. Should NOT modify the persistent rope buffer.
    ImePreedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    /// Cancel active IME preedit composition without inserting anything.
    ImeCancel,
    /// IME text commit. Must be inserted into the buffer exactly once.
    ImeCommit(String),
    ImeDeleteSurrounding {
        before_chars: usize,
        after_chars: usize,
    },
    /// Plain text insertion (e.g. keyboard typing).
    InsertText(String),
    /// Backspace (delete previous grapheme or selection).
    DeleteBackward,
    /// Delete (delete next grapheme or selection).
    DeleteForward,
    /// Move cursor left by one grapheme cluster.
    MoveCursorLeft { extend_selection: bool },
    /// Move cursor right by one grapheme cluster.
    MoveCursorRight { extend_selection: bool },
    /// Set explicit selection range (anchor, head).
    SetSelection { anchor: usize, head: usize },
    /// Select the entire text.
    SelectAll,
    /// Paste text from clipboard.
    Paste(String),
}

/// Unified input adapter maintaining transient IME composition state and executing edits.
#[derive(Debug, Clone, Default)]
pub struct InputAdapter {
    ime_preedit: Option<String>,
    ime_cursor: Option<(usize, usize)>,
}

impl InputAdapter {
    /// Creates a new `InputAdapter`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the active IME preedit string if in composition mode.
    pub fn ime_preedit(&self) -> Option<&str> {
        self.ime_preedit.as_deref()
    }

    /// Returns the active IME cursor position within preedit string.
    pub fn ime_cursor(&self) -> Option<(usize, usize)> {
        self.ime_cursor
    }

    /// Dispatches an `EditorInput` against a transactional `Buffer` and cursor/selection state.
    ///
    /// GUARANTEES:
    /// - `ImePreedit` and `ImeCancel` do NOT call `Buffer::apply` and do not alter `buffer.revision()`.
    /// - `ImeCommit` executes EXACTLY ONE atomic `Buffer::apply`.
    pub fn handle_input(
        &mut self,
        buffer: &mut Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
        input: EditorInput,
    ) -> Result<Option<ChangeSet>, EditError> {
        if !matches!(
            input,
            EditorInput::ImePreedit { .. } | EditorInput::ImeCancel
        ) {
            for position in
                std::iter::once(*cursor).chain(selection.iter().flat_map(|&(a, b)| [a, b]))
            {
                if position > buffer.byte_len() {
                    return Err(EditError::InvalidByteRange {
                        range: position..position,
                        len: buffer.byte_len(),
                    });
                }
                if !buffer.is_char_boundary(position) {
                    return Err(EditError::NotCharBoundary { pos: position });
                }
            }
        }
        match input {
            EditorInput::ImePreedit { text, cursor: cur } => {
                if text.is_empty() {
                    self.ime_preedit = None;
                    self.ime_cursor = None;
                } else {
                    self.ime_preedit = Some(text);
                    self.ime_cursor = cur;
                }
                Ok(None)
            }
            EditorInput::ImeCancel => {
                self.ime_preedit = None;
                self.ime_cursor = None;
                Ok(None)
            }
            EditorInput::ImeCommit(commit_text) => {
                self.ime_preedit = None;
                self.ime_cursor = None;
                if commit_text.is_empty() {
                    return Ok(None);
                }
                let changes = self.insert_str(buffer, cursor, selection, &commit_text)?;
                Ok(Some(changes))
            }
            EditorInput::ImeDeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                let range = truff_core::vim::execute::surrounding_char_range(
                    buffer,
                    *cursor,
                    before_chars,
                    after_chars,
                );
                if range.is_empty() {
                    return Ok(None);
                }
                let start = range.start;
                let changes = buffer.apply(Transaction::delete(range))?;
                *cursor = start;
                *selection = None;
                Ok(Some(changes))
            }
            EditorInput::InsertText(text) => {
                if text.is_empty() {
                    return Ok(None);
                }
                let changes = self.insert_str(buffer, cursor, selection, &text)?;
                Ok(Some(changes))
            }
            EditorInput::Paste(text) => {
                if text.is_empty() {
                    return Ok(None);
                }
                let changes = self.insert_str(buffer, cursor, selection, &text)?;
                Ok(Some(changes))
            }
            EditorInput::DeleteBackward => self.delete_backward(buffer, cursor, selection),
            EditorInput::DeleteForward => self.delete_forward(buffer, cursor, selection),
            EditorInput::MoveCursorLeft { extend_selection } => {
                self.move_left(buffer, cursor, selection, extend_selection);
                Ok(None)
            }
            EditorInput::MoveCursorRight { extend_selection } => {
                self.move_right(buffer, cursor, selection, extend_selection);
                Ok(None)
            }
            EditorInput::SetSelection { anchor, head } => {
                let max_len = buffer.byte_len();
                let snapped_anchor = snap_grapheme(buffer, anchor.min(max_len));
                let snapped_head = snap_grapheme(buffer, head.min(max_len));
                *cursor = snapped_head;
                *selection = Some((
                    snapped_anchor.min(snapped_head),
                    snapped_anchor.max(snapped_head),
                ));
                Ok(None)
            }
            EditorInput::SelectAll => {
                *selection = Some((0, buffer.byte_len()));
                *cursor = buffer.byte_len();
                Ok(None)
            }
        }
    }

    fn insert_str(
        &mut self,
        buffer: &mut Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
        s: &str,
    ) -> Result<ChangeSet, EditError> {
        let (start, end) = selection.unwrap_or((*cursor, *cursor));
        let tx = Transaction::replace(start..end, s);
        let changes = buffer.apply(tx)?;
        *cursor = start + s.len();
        *selection = None;
        Ok(changes)
    }

    fn delete_backward(
        &mut self,
        buffer: &mut Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
    ) -> Result<Option<ChangeSet>, EditError> {
        if let Some((start, end)) = *selection {
            let changes = buffer.apply(Transaction::delete(start..end))?;
            *cursor = start;
            *selection = None;
            return Ok(Some(changes));
        }

        let cur = *cursor;
        if cur == 0 {
            return Ok(None);
        }

        let prev = buffer
            .grapheme_prev(BytePos::new(cur))
            .map_or(0, BytePos::get);
        let changes = buffer.apply(Transaction::delete(prev..cur))?;
        *cursor = prev;
        Ok(Some(changes))
    }

    fn delete_forward(
        &mut self,
        buffer: &mut Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
    ) -> Result<Option<ChangeSet>, EditError> {
        if let Some((start, end)) = *selection {
            let changes = buffer.apply(Transaction::delete(start..end))?;
            *cursor = start;
            *selection = None;
            return Ok(Some(changes));
        }

        let cur = *cursor;
        if cur >= buffer.byte_len() {
            return Ok(None);
        }

        let next = buffer
            .grapheme_next(BytePos::new(cur))
            .map_or(cur, BytePos::get);
        if next > cur {
            buffer.apply(Transaction::delete(cur..next)).map(Some)
        } else {
            Ok(None)
        }
    }

    fn move_left(
        &mut self,
        buffer: &Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
        extend_selection: bool,
    ) {
        if !extend_selection {
            if let Some((start, _)) = selection.take() {
                *cursor = start;
                return;
            }
        } else if selection.is_none() {
            *selection = Some((*cursor, *cursor));
        }

        let cur = *cursor;
        if cur == 0 {
            return;
        }

        let target = buffer
            .grapheme_prev(BytePos::new(cur))
            .map_or(0, BytePos::get);

        *cursor = target;
        if extend_selection && let Some((anchor, _)) = *selection {
            *selection = Some((anchor.min(target), anchor.max(target)));
        }
    }

    fn move_right(
        &mut self,
        buffer: &Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
        extend_selection: bool,
    ) {
        if !extend_selection {
            if let Some((_, end)) = selection.take() {
                *cursor = end;
                return;
            }
        } else if selection.is_none() {
            *selection = Some((*cursor, *cursor));
        }

        let cur = *cursor;
        if cur >= buffer.byte_len() {
            return;
        }

        let target = buffer
            .grapheme_next(BytePos::new(cur))
            .map_or(cur, BytePos::get);

        *cursor = target;
        if extend_selection && let Some((anchor, _)) = *selection {
            *selection = Some((anchor.min(target), anchor.max(target)));
        }
    }
}
