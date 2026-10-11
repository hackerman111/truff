use std::ops::Range;

use super::edit::{Change, ChangeSet, EditError, SingleEdit, Transaction, UndoGroup};
use super::history::History;
mod line_ending;
mod position;

pub use line_ending::LineEnding;
use line_ending::normalize_crlf;
pub use position::{BufferId, BytePos};

/// Default maximum buffer size: 16 MiB.
pub const DEFAULT_MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

/// The transactional text buffer owning rope, revision, dirty state, and undo history.
pub struct Buffer {
    id: BufferId,
    rope: crop::Rope,
    revision: u64,
    savepoint_revision: u64,
    max_file_bytes: usize,
    line_ending: LineEnding,
    history: History,
}

impl Buffer {
    /// Creates a new empty buffer.
    pub fn new(id: BufferId) -> Self {
        Self {
            id,
            rope: crop::Rope::new(),
            revision: 0,
            savepoint_revision: 0,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            line_ending: LineEnding::Lf,
            history: History::new(),
        }
    }

    /// Creates a buffer from a string slice with an automatically allocated ID.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self::from_text(BufferId::next(), text).unwrap_or_else(|_| Self::new(BufferId::next()))
    }

    /// Creates a buffer populated from initial text, checking the size limit.
    pub fn from_text(id: BufferId, text: &str) -> Result<Self, EditError> {
        let max_file_bytes = DEFAULT_MAX_FILE_BYTES;
        if text.len() > max_file_bytes {
            return Err(EditError::SizeLimitExceeded {
                current_len: 0,
                new_len: text.len(),
                limit: max_file_bytes,
            });
        }

        let line_ending = LineEnding::detect(text);
        let rope = crop::Rope::from(text);

        Ok(Self {
            id,
            rope,
            revision: 0,
            savepoint_revision: 0,
            max_file_bytes,
            line_ending,
            history: History::new(),
        })
    }

    /// Returns the unique buffer ID.
    pub fn id(&self) -> BufferId {
        self.id
    }

    /// Returns a reference to the underlying `Rope`.
    pub fn rope(&self) -> &crop::Rope {
        &self.rope
    }

    /// Converts the buffer content to an owned `String`.
    pub fn as_str(&self) -> String {
        self.rope.to_string()
    }

    /// Total byte length of the buffer.
    pub fn byte_len(&self) -> usize {
        self.rope.byte_len()
    }

    /// Returns `true` if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.rope.is_empty()
    }

    /// Replaces a clean resident document from disk, discarding undo but preserving revision order.
    pub(crate) fn reload_from_text(&mut self, text: &str) -> Result<(), EditError> {
        let mut replacement = Self::from_text(self.id, text)?;
        replacement.revision = self.revision + 1;
        replacement.set_savepoint();
        *self = replacement;
        Ok(())
    }

    /// Monotonically increasing revision number.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The revision at the last savepoint.
    pub fn savepoint_revision(&self) -> u64 {
        self.savepoint_revision
    }

    /// Returns `true` if the buffer has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        if self.revision == self.savepoint_revision {
            false
        } else {
            !self.history.is_at_savepoint()
        }
    }

    /// Sets the current revision and history state as the clean savepoint.
    pub fn set_savepoint(&mut self) {
        self.savepoint_revision = self.revision;
        self.history.mark_savepoint();
    }

    /// Configured maximum buffer size in bytes.
    pub fn max_file_bytes(&self) -> usize {
        self.max_file_bytes
    }

    /// Overrides the maximum buffer size limit in bytes.
    pub fn set_max_file_bytes(&mut self, limit: usize) {
        self.max_file_bytes = limit;
    }

    /// Detected or configured line ending.
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    /// Sets the line ending convention.
    pub fn set_line_ending(&mut self, le: LineEnding) {
        self.line_ending = le;
    }

    /// Accesses the undo/redo history.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Mutably accesses the undo/redo history.
    pub fn history_mut(&mut self) -> &mut History {
        &mut self.history
    }

    /// Returns a slice of the buffer.
    pub fn slice(&self, range: Range<usize>) -> crop::RopeSlice<'_> {
        self.rope.byte_slice(range)
    }

    /// Checks if a byte offset is a UTF-8 character boundary.
    pub fn is_char_boundary(&self, pos: usize) -> bool {
        self.rope.is_char_boundary(pos)
    }

    /// Returns the largest byte offset less than or equal to `pos` that is a character boundary.
    pub fn floor_char_boundary(&self, mut pos: usize) -> usize {
        pos = pos.min(self.rope.byte_len());
        while !self.rope.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    /// Number of lines in the buffer.
    pub fn line_len(&self) -> usize {
        self.rope.line_len()
    }

    /// Line index containing the given byte offset.
    /// Snaps to the nearest preceding character boundary if `pos` is not on a boundary.
    pub fn line_of_byte(&self, pos: usize) -> usize {
        let snapped = self.floor_char_boundary(pos);
        self.rope.line_of_byte(snapped)
    }

    /// Start byte offset of the given line.
    pub fn byte_of_line(&self, line: usize) -> usize {
        self.rope.byte_of_line(line)
    }

    /// Counts extended grapheme clusters across borrowed Rope chunks.
    pub fn grapheme_len(&self) -> usize {
        super::grapheme::count(&self.rope)
    }

    /// Advances to the next grapheme boundary without copying the logical line.
    /// Invalid UTF-8 byte offsets return `None`.
    pub fn grapheme_next(&self, pos: BytePos) -> Option<BytePos> {
        super::grapheme::next(&self.rope, pos.get()).map(BytePos::new)
    }

    /// Moves to the preceding grapheme boundary across Rope chunk boundaries.
    /// Positions beyond EOF are clamped; invalid UTF-8 byte offsets return `None`.
    pub fn grapheme_prev(&self, pos: BytePos) -> Option<BytePos> {
        super::grapheme::prev(&self.rope, pos.get()).map(BytePos::new)
    }

    /// Applies a transaction atomically to the buffer.
    pub fn apply(&mut self, tx: Transaction) -> Result<ChangeSet, EditError> {
        let mut normalized_edits = tx.edits;
        if self.line_ending == LineEnding::CrLf {
            for edit in &mut normalized_edits {
                if edit.text.contains('\n') {
                    edit.text = normalize_crlf(&edit.text);
                }
            }
        }

        Self::validate_edits(&self.rope, self.max_file_bytes, &normalized_edits)?;

        let (inverse_edits, changes) = self.build_inverse_and_changes(&normalized_edits);

        // Apply edits in reverse order (from largest offset to smallest)
        for edit in normalized_edits.iter().rev() {
            self.rope.replace(edit.range.clone(), &edit.text);
        }

        let undo_group = tx.undo_group.unwrap_or_else(UndoGroup::next);

        let forward_tx = Transaction {
            edits: normalized_edits,
            cursor: tx.cursor,
            undo_group: Some(undo_group),
        };

        let inverse_tx = Transaction {
            edits: inverse_edits,
            cursor: tx.cursor,
            undo_group: Some(undo_group),
        };

        self.history.record(undo_group, forward_tx, inverse_tx);
        self.revision += 1;

        Ok(ChangeSet::new(changes))
    }

    /// Reverts the most recent `UndoGroup` atomically.
    pub fn undo(&mut self) -> Result<Option<ChangeSet>, EditError> {
        let Some(history_group) = self.history.peek_undo() else {
            return Ok(None);
        };
        let count = history_group.inverse.len();
        let (rope, changes) = self.stage_history(history_group.inverse.iter().rev())?;
        let history_group = self.history.pop_undo().expect("peeked undo group exists");
        self.rope = rope;
        self.revision += count as u64;
        self.history.push_redo(history_group);
        Ok(Some(changes))
    }

    /// Reapplies the most recently undone `UndoGroup`.
    pub fn redo(&mut self) -> Result<Option<ChangeSet>, EditError> {
        let Some(history_group) = self.history.peek_redo() else {
            return Ok(None);
        };
        let count = history_group.forward.len();
        let (rope, changes) = self.stage_history(history_group.forward.iter())?;
        let history_group = self.history.pop_redo().expect("peeked redo group exists");
        self.rope = rope;
        self.revision += count as u64;
        self.history.push_undo(history_group);
        Ok(Some(changes))
    }

    fn validate_edits(
        rope: &crop::Rope,
        limit: usize,
        edits: &[SingleEdit],
    ) -> Result<(), EditError> {
        let current_len = rope.byte_len();
        let mut total_deleted: usize = 0;
        let mut total_inserted: usize = 0;

        for (i, edit) in edits.iter().enumerate() {
            if edit.range.start > edit.range.end {
                return Err(EditError::InvalidByteRange {
                    range: edit.range.clone(),
                    len: current_len,
                });
            }
            if edit.range.end > current_len {
                return Err(EditError::InvalidByteRange {
                    range: edit.range.clone(),
                    len: current_len,
                });
            }
            if !rope.is_char_boundary(edit.range.start) {
                return Err(EditError::NotCharBoundary {
                    pos: edit.range.start,
                });
            }
            if !rope.is_char_boundary(edit.range.end) {
                return Err(EditError::NotCharBoundary {
                    pos: edit.range.end,
                });
            }

            if i + 1 < edits.len() {
                let next = &edits[i + 1];
                if edit.range.start > next.range.start {
                    return Err(EditError::UnsortedEdits {
                        first: edit.range.clone(),
                        second: next.range.clone(),
                    });
                }
                if edit.range.end > next.range.start
                    || (edit.range == next.range && edit.range.is_empty())
                {
                    return Err(EditError::OverlappingEdits {
                        first: edit.range.clone(),
                        second: next.range.clone(),
                    });
                }
            }

            total_deleted = total_deleted.checked_add(edit.range.len()).ok_or(
                EditError::SizeLimitExceeded {
                    current_len,
                    new_len: usize::MAX,
                    limit,
                },
            )?;
            total_inserted = total_inserted.checked_add(edit.text.len()).ok_or(
                EditError::SizeLimitExceeded {
                    current_len,
                    new_len: usize::MAX,
                    limit,
                },
            )?;
        }

        let new_len = current_len
            .checked_sub(total_deleted)
            .and_then(|l| l.checked_add(total_inserted))
            .ok_or(EditError::SizeLimitExceeded {
                current_len,
                new_len: usize::MAX,
                limit,
            })?;

        if new_len > limit {
            return Err(EditError::SizeLimitExceeded {
                current_len,
                new_len,
                limit,
            });
        }

        Ok(())
    }

    fn build_inverse_and_changes(&self, edits: &[SingleEdit]) -> (Vec<SingleEdit>, Vec<Change>) {
        let mut inverse_edits: Vec<SingleEdit> = Vec::with_capacity(edits.len());
        let mut changes = Vec::with_capacity(edits.len());

        let mut shift: isize = 0;
        for edit in edits {
            let old_text = self.rope.byte_slice(edit.range.clone()).to_string();
            let new_start = (edit.range.start as isize + shift) as usize;
            let new_end = new_start + edit.text.len();
            let delta = (edit.text.len() as isize) - (edit.range.len() as isize);
            shift += delta;

            // Adjacent deletions collapse onto one insertion point in the inverse.
            if let Some(previous) = inverse_edits.last_mut()
                && previous.range == (new_start..new_end)
                && previous.range.is_empty()
            {
                previous.text.push_str(&old_text);
            } else {
                inverse_edits.push(SingleEdit {
                    range: new_start..new_end,
                    text: old_text,
                });
            }
            changes.push(Change::new(edit.range.clone(), edit.text.len()));
        }

        (inverse_edits, changes)
    }

    fn stage_history<'a>(
        &self,
        transactions: impl Iterator<Item = &'a Transaction>,
    ) -> Result<(crop::Rope, ChangeSet), EditError> {
        // Rope clones share unchanged storage; failed replay discards only edited paths.
        let mut rope = self.rope.clone();
        let mut changes = ChangeSet::default();
        for tx in transactions {
            Self::validate_edits(&rope, self.max_file_bytes, &tx.edits)?;
            changes.append_batch(ChangeSet::new(
                tx.edits
                    .iter()
                    .map(|edit| Change::new(edit.range.clone(), edit.text.len()))
                    .collect(),
            ));
            for edit in tx.edits.iter().rev() {
                rope.replace(edit.range.clone(), &edit.text);
            }
        }
        Ok((rope, changes))
    }
}
