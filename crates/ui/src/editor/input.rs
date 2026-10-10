//! Unified input adapter handling keyboard, mouse, clipboard, and IME.
//!
//! Strict guarantees:
//! - IME preedit and cancellation NEVER modify the persistent text buffer or Rope.
//! - IME commit runs through EXACTLY ONE atomic `Buffer::apply` transaction.
//! - Grapheme cluster preservation on all edits and motions.

use truff_core::Buffer;
use truff_core::text::{ChangeSet, EditError, Transaction};
use unicode_segmentation::UnicodeSegmentation;

use super::layout::snap_to_grapheme_boundary;

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
                let text = buffer.as_str();
                let snapped_anchor = snap_to_grapheme_boundary(&text, anchor.min(max_len));
                let snapped_head = snap_to_grapheme_boundary(&text, head.min(max_len));
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
        let tx = if let Some((start, end)) = selection.take() {
            *cursor = start + s.len();
            Transaction::replace(start..end, s)
        } else {
            let cur = *cursor;
            *cursor = cur + s.len();
            Transaction::insert(cur, s)
        };
        buffer.apply(tx)
    }

    fn delete_backward(
        &mut self,
        buffer: &mut Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
    ) -> Result<Option<ChangeSet>, EditError> {
        if let Some((start, end)) = selection.take() {
            *cursor = start;
            let tx = Transaction::delete(start..end);
            return buffer.apply(tx).map(Some);
        }

        let cur = *cursor;
        if cur == 0 {
            return Ok(None);
        }

        let text = buffer.as_str();
        let mut prev = 0;
        for (idx, cluster) in text.grapheme_indices(true) {
            let next = idx + cluster.len();
            if next >= cur {
                prev = idx;
                break;
            }
        }

        *cursor = prev;
        let tx = Transaction::delete(prev..cur);
        buffer.apply(tx).map(Some)
    }

    fn delete_forward(
        &mut self,
        buffer: &mut Buffer,
        cursor: &mut usize,
        selection: &mut Option<(usize, usize)>,
    ) -> Result<Option<ChangeSet>, EditError> {
        if let Some((start, end)) = selection.take() {
            *cursor = start;
            let tx = Transaction::delete(start..end);
            return buffer.apply(tx).map(Some);
        }

        let cur = *cursor;
        if cur >= buffer.byte_len() {
            return Ok(None);
        }

        let text = buffer.as_str();
        let mut next = cur;
        for (idx, cluster) in text.grapheme_indices(true) {
            if idx >= cur {
                next = idx + cluster.len();
                break;
            }
        }

        if next > cur {
            let tx = Transaction::delete(cur..next);
            buffer.apply(tx).map(Some)
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

        let text = buffer.as_str();
        let mut target = 0;
        for (idx, cluster) in text.grapheme_indices(true) {
            let end = idx + cluster.len();
            if end >= cur {
                target = idx;
                break;
            }
        }

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

        let text = buffer.as_str();
        let mut target = cur;
        for (idx, cluster) in text.grapheme_indices(true) {
            if idx >= cur {
                target = idx + cluster.len();
                break;
            }
        }

        *cursor = target;
        if extend_selection && let Some((anchor, _)) = *selection {
            *selection = Some((anchor.min(target), anchor.max(target)));
        }
    }
}
