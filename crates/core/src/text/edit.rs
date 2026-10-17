use std::fmt;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

use super::buffer::BytePos;

/// Monotonically allocated identifier for grouping multiple edits into an atomic undo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UndoGroup(pub u64);

impl UndoGroup {
    /// Creates an `UndoGroup` with an explicit ID.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Allocates the next unique `UndoGroup` ID.
    pub fn next() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric ID.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A single atomic text replacement within a byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleEdit {
    /// Target byte range to replace.
    pub range: Range<usize>,
    /// Replacement text.
    pub text: String,
}

impl SingleEdit {
    /// Creates a replacement edit.
    pub fn new(range: Range<usize>, text: impl Into<String>) -> Self {
        Self {
            range,
            text: text.into(),
        }
    }

    /// Creates an insertion edit at a specific byte offset.
    pub fn insert(pos: usize, text: impl Into<String>) -> Self {
        Self::new(pos..pos, text)
    }

    /// Creates a deletion edit over a byte range.
    pub fn delete(range: Range<usize>) -> Self {
        Self::new(range, String::new())
    }

    /// Creates a replacement edit over a byte range.
    pub fn replace(range: Range<usize>, text: impl Into<String>) -> Self {
        Self::new(range, text)
    }
}

/// Optional cursor or selection metadata associated with a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorMeta {
    /// Selection anchor position.
    pub anchor: BytePos,
    /// Selection head (caret) position.
    pub head: BytePos,
}

/// A transaction containing a batch of non-overlapping text edits, optional cursor state, and an undo group.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transaction {
    /// Disjoint edits, sorted in ascending order of their start offsets.
    pub edits: Vec<SingleEdit>,
    /// Associated cursor or selection metadata.
    pub cursor: Option<CursorMeta>,
    /// Undo group grouping this transaction with preceding/succeeding transactions.
    pub undo_group: Option<UndoGroup>,
}

impl Transaction {
    /// Creates an empty transaction.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an edit to this transaction.
    pub fn with_edit(mut self, edit: SingleEdit) -> Self {
        self.edits.push(edit);
        self
    }

    /// Helper creating a single insertion transaction.
    pub fn insert(pos: usize, text: impl Into<String>) -> Self {
        Self::new().with_edit(SingleEdit::insert(pos, text))
    }

    /// Helper creating a single deletion transaction.
    pub fn delete(range: Range<usize>) -> Self {
        Self::new().with_edit(SingleEdit::delete(range))
    }

    /// Helper creating a single replacement transaction.
    pub fn replace(range: Range<usize>, text: impl Into<String>) -> Self {
        Self::new().with_edit(SingleEdit::replace(range, text))
    }

    /// Sets the cursor metadata.
    pub fn with_cursor(mut self, anchor: BytePos, head: BytePos) -> Self {
        self.cursor = Some(CursorMeta { anchor, head });
        self
    }

    /// Sets the undo group.
    pub fn with_undo_group(mut self, group: UndoGroup) -> Self {
        self.undo_group = Some(group);
        self
    }
}

/// An individual delta produced by an applied edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Old byte range that was replaced.
    pub old_range: Range<usize>,
    /// Length of the new replacement text in bytes.
    pub new_len: usize,
    /// Cumulative shift before this edit in a sorted, disjoint batch.
    pub prefix_shift: isize,
}

impl Change {
    /// Constructs a new `Change`.
    pub fn new(old_range: Range<usize>, new_len: usize) -> Self {
        Self {
            old_range,
            new_len,
            prefix_shift: 0,
        }
    }

    /// Net difference in byte length (`new_len as isize - old_len as isize`).
    #[inline(always)]
    pub const fn delta(&self) -> isize {
        self.new_len as isize - (self.old_range.end - self.old_range.start) as isize
    }
}

/// Collection of applied changes, used for rebasing cursors, syntax highlighting, and layout invalidation.
///
/// Transaction boundaries and change storage cannot be mutated independently.
///
/// ```compile_fail
/// use truff_core::ChangeSet;
/// let mut changes = ChangeSet::default();
/// changes.changes.clear();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeSet {
    /// Applied changes in execution order. Grouped operations contain successive
    /// coordinate spaces; use `batches()` or `map_pos()` to consume them.
    changes: Vec<Change>,
    batch_ends: Vec<usize>,
    // When true, all batches are disjoint and sorted, allowing O(log K) binary search.
    is_indexed: bool,
}

impl ChangeSet {
    /// Creates a `ChangeSet` from a list of changes.
    pub fn new(mut changes: Vec<Change>) -> Self {
        if changes.is_empty() {
            return Self::default();
        }
        let mut shift = 0isize;
        let mut is_indexed = true;
        let mut prev_end = 0usize;
        for (i, change) in changes.iter_mut().enumerate() {
            if change.old_range.start > change.old_range.end {
                is_indexed = false;
                break;
            }
            if i > 0 && change.old_range.start < prev_end {
                is_indexed = false;
            }
            change.prefix_shift = shift;
            shift += change.delta();
            prev_end = change.old_range.end;
        }
        Self {
            changes,
            batch_ends: Vec::new(),
            is_indexed,
        }
    }

    /// Creates a `ChangeSet` from a slice of edits without cloning strings.
    pub fn from_edits(edits: &[SingleEdit]) -> Self {
        let mut changes: Vec<Change> = edits
            .iter()
            .map(|e| Change {
                old_range: e.range.clone(),
                new_len: e.text.len(),
                prefix_shift: 0,
            })
            .collect();
        changes.sort_unstable_by_key(|c| c.old_range.start);
        let mut cumulative: isize = 0;
        let mut disjoint = true;
        let mut prev_end = 0usize;
        for (i, c) in changes.iter_mut().enumerate() {
            if i > 0 && c.old_range.start < prev_end {
                disjoint = false;
            }
            c.prefix_shift = cumulative;
            cumulative += c.delta();
            prev_end = c.old_range.end;
        }
        Self {
            changes,
            batch_ends: Vec::new(),
            is_indexed: disjoint,
        }
    }

    /// Returns changes in execution order. For grouped operations, `batches()`
    /// retains the boundaries between successive coordinate spaces.
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Visits transaction batches in execution order, each using the source
    /// coordinates immediately before that transaction.
    pub fn batches(&self) -> impl Iterator<Item = &[Change]> {
        let mut start = 0;
        self.batch_ends
            .iter()
            .copied()
            .chain(std::iter::once(self.changes.len()))
            .map(move |end| {
                let batch = &self.changes[start..end];
                start = end;
                batch
            })
    }

    pub(crate) fn append_batch(&mut self, batch: Self) {
        if self.changes.is_empty() {
            *self = batch;
            return;
        }
        if batch.changes.is_empty() {
            return;
        }
        let offset = self.changes.len();
        self.batch_ends.push(offset);
        self.batch_ends
            .extend(batch.batch_ends.into_iter().map(|end| offset + end));
        self.changes.extend(batch.changes);
        self.is_indexed = self.is_indexed && batch.is_indexed;
    }

    /// Returns `true` if no edits were applied.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns the number of applied changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Re-maps a byte offset from the pre-edit coordinate space to the post-edit coordinate space.
    pub fn map_pos(&self, pos: usize) -> usize {
        self.batches().fold(pos, |pos, batch| {
            if batch.is_empty() {
                return pos;
            }
            if self.is_indexed {
                let end = batch.partition_point(|change| change.old_range.start <= pos);
                if end == 0 {
                    return pos;
                }
                let change = &batch[end - 1];
                let shift = change.prefix_shift;
                if pos <= change.old_range.end {
                    (change.old_range.start as isize + shift) as usize + change.new_len
                } else {
                    (pos as isize + shift + change.delta()) as usize
                }
            } else {
                let mut shift = 0isize;
                let mut mapped = None;
                for change in batch {
                    if pos < change.old_range.start {
                        break;
                    }
                    if pos <= change.old_range.end {
                        mapped = Some(
                            ((change.old_range.start as isize + shift) as usize) + change.new_len,
                        );
                    }
                    shift += change.delta();
                }
                mapped.unwrap_or((pos as isize + shift) as usize)
            }
        })
    }
}

/// Errors occurring during transaction validation or application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// Byte range is out of buffer bounds or start > end.
    InvalidByteRange { range: Range<usize>, len: usize },
    /// Specified position does not lie on a UTF-8 character boundary.
    NotCharBoundary { pos: usize },
    /// Edits within a transaction overlap.
    OverlappingEdits {
        first: Range<usize>,
        second: Range<usize>,
    },
    /// Edits within a transaction are not sorted ascending by start position.
    UnsortedEdits {
        first: Range<usize>,
        second: Range<usize>,
    },
    /// The resulting buffer size would exceed the allowed limit.
    SizeLimitExceeded {
        current_len: usize,
        new_len: usize,
        limit: usize,
    },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidByteRange { range, len } => {
                write!(
                    f,
                    "invalid byte range {:?} for buffer of length {}",
                    range, len
                )
            }
            Self::NotCharBoundary { pos } => {
                write!(
                    f,
                    "byte position {} is not a valid UTF-8 character boundary",
                    pos
                )
            }
            Self::OverlappingEdits { first, second } => {
                write!(
                    f,
                    "overlapping edits in transaction: {:?} and {:?}",
                    first, second
                )
            }
            Self::UnsortedEdits { first, second } => {
                write!(
                    f,
                    "unsorted edits in transaction: {:?} must come before {:?}",
                    second, first
                )
            }
            Self::SizeLimitExceeded {
                current_len,
                new_len,
                limit,
            } => {
                write!(
                    f,
                    "buffer size limit exceeded: new size {} bytes (current {}) exceeds limit of {} bytes",
                    new_len, current_len, limit
                )
            }
        }
    }
}

impl std::error::Error for EditError {}
