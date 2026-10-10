use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::text::{BufferId, BytePos, ChangeSet};

/// Default maximum entries stored in a view's jump history (§10.1).
pub const DEFAULT_MAX_JUMP_ENTRIES: usize = 256;

/// Strongly-typed unique identifier for a workspace view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViewId(pub u64);

impl ViewId {
    /// Creates a `ViewId` with an explicit number.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Allocates the next unique `ViewId`.
    pub fn next() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the raw numeric ID.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ViewId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Bounded jump list maintaining cursor locations for `<C-o>` / `<C-i>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpHistory {
    entries: Vec<(BytePos, BytePos)>,
    index: usize,
    max_entries: usize,
}

impl JumpHistory {
    /// Creates a new empty jump history with default capacity.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: 0,
            max_entries: DEFAULT_MAX_JUMP_ENTRIES,
        }
    }

    /// Records a new jump point (anchor, head).
    pub fn record_jump(&mut self, anchor: BytePos, head: BytePos) {
        // If we jumped back and now make a new jump, truncate future redo entries
        if self.index < self.entries.len() {
            self.entries.truncate(self.index);
        }

        // Avoid duplicate consecutive jumps
        if let Some(&(last_anchor, last_head)) = self.entries.last()
            && last_anchor == anchor
            && last_head == head
        {
            return;
        }

        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        } else {
            self.index += 1;
        }

        self.entries.push((anchor, head));
        self.index = self.entries.len();
    }

    /// Jumps back in history (`<C-o>`).
    pub fn jump_back(&mut self) -> Option<(BytePos, BytePos)> {
        if self.index > 0 {
            self.index -= 1;
            self.entries.get(self.index).copied()
        } else {
            None
        }
    }

    /// Jumps forward in history (`<C-i>`).
    pub fn jump_forward(&mut self) -> Option<(BytePos, BytePos)> {
        if self.index + 1 < self.entries.len() {
            self.index += 1;
            self.entries.get(self.index).copied()
        } else {
            None
        }
    }

    /// Rebases all recorded jump points through a `ChangeSet`.
    pub fn apply_change_set(&mut self, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        for (anchor, head) in &mut self.entries {
            *anchor = BytePos::new(changes.map_pos(anchor.get()));
            *head = BytePos::new(changes.map_pos(head.get()));
        }
    }
}

impl Default for JumpHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// An independent view into a buffer with its own cursor, selection, and viewport scroll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    id: ViewId,
    buffer_id: BufferId,
    anchor: BytePos,
    head: BytePos,
    scroll_top_line: usize,
    scroll_anchor: Option<(usize, usize)>,
    jump_history: JumpHistory,
}

impl View {
    /// Creates a new view attached to the given buffer ID.
    pub fn new(id: ViewId, buffer_id: BufferId) -> Self {
        Self {
            id,
            buffer_id,
            anchor: BytePos::ZERO,
            head: BytePos::ZERO,
            scroll_top_line: 0,
            scroll_anchor: None,
            jump_history: JumpHistory::new(),
        }
    }

    /// The unique view identifier.
    pub fn id(&self) -> ViewId {
        self.id
    }

    /// The attached buffer identifier.
    pub fn buffer_id(&self) -> BufferId {
        self.buffer_id
    }

    /// Selection anchor position.
    pub fn anchor(&self) -> BytePos {
        self.anchor
    }

    /// Cursor / caret head position.
    pub fn head(&self) -> BytePos {
        self.head
    }

    /// Updates cursor and selection anchor without adding to jump history.
    pub fn set_cursor(&mut self, anchor: BytePos, head: BytePos) {
        self.anchor = anchor;
        self.head = head;
    }

    /// Updates cursor and records the previous position in jump history.
    pub fn set_cursor_with_jump(&mut self, anchor: BytePos, head: BytePos) {
        self.jump_history.record_jump(self.anchor, self.head);
        self.anchor = anchor;
        self.head = head;
    }

    /// Top line index visible in the scroll viewport.
    pub fn scroll_top_line(&self) -> usize {
        self.scroll_top_line
    }

    /// Sets the top line index visible in the scroll viewport.
    pub fn set_scroll_top_line(&mut self, line: usize) {
        self.scroll_top_line = line;
    }

    /// Scroll anchor `(line, byte_offset)`.
    pub fn scroll_anchor(&self) -> Option<(usize, usize)> {
        self.scroll_anchor
    }

    /// Sets the scroll anchor.
    pub fn set_scroll_anchor(&mut self, anchor: Option<(usize, usize)>) {
        self.scroll_anchor = anchor;
    }

    /// Reference to this view's jump history.
    pub fn jump_history(&self) -> &JumpHistory {
        &self.jump_history
    }

    /// Mutable reference to this view's jump history.
    pub fn jump_history_mut(&mut self) -> &mut JumpHistory {
        &mut self.jump_history
    }

    /// Rebases cursor, selection, and jump history via applied changes.
    pub fn apply_change_set(&mut self, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        self.anchor = BytePos::new(changes.map_pos(self.anchor.get()));
        self.head = BytePos::new(changes.map_pos(self.head.get()));
        self.jump_history.apply_change_set(changes);
    }
}
