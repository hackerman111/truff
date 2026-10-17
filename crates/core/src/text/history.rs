use std::collections::VecDeque;
use std::mem::size_of;

use super::edit::{CursorMeta, SingleEdit, UndoGroup};

mod registers;
pub use registers::{Register, RegisterKind, Registers};

/// Default memory limit for undo/redo history per buffer: 8 MiB.
pub const DEFAULT_MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;

fn compact_edit_text(edit: &mut SingleEdit) {
    if edit.text.capacity() > edit.text.len().saturating_mul(2)
        && edit.text.capacity() - edit.text.len() >= 4096
    {
        edit.text.shrink_to_fit();
    }
}

/// A group of edits applied under the same `UndoGroup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryGroup {
    /// Identifier of this undo group.
    pub group_id: UndoGroup,
    /// Forward edits in execution order.
    pub forward_edits: Vec<SingleEdit>,
    /// Boundaries of transaction batches in `forward_edits`.
    pub forward_batch_ends: Vec<u32>,
    /// Inverse edits in execution order.
    pub inverse_edits: Vec<SingleEdit>,
    /// Boundaries of transaction batches in `inverse_edits`.
    pub inverse_batch_ends: Vec<u32>,
    /// Cursor metadata before edits were applied.
    pub cursor_before: Option<CursorMeta>,
    /// Cursor metadata after edits were applied.
    pub cursor_after: Option<CursorMeta>,
    /// Estimated memory usage in bytes.
    pub size_in_bytes: usize,
    before_state: u64,
    after_state: u64,
}

impl HistoryGroup {
    fn new(group_id: UndoGroup, before_state: u64, after_state: u64) -> Self {
        Self {
            group_id,
            forward_edits: Vec::new(),
            forward_batch_ends: Vec::new(),
            inverse_edits: Vec::new(),
            inverse_batch_ends: Vec::new(),
            cursor_before: None,
            cursor_after: None,
            size_in_bytes: size_of::<Self>(),
            before_state,
            after_state,
        }
    }

    fn heap_bytes(&self) -> usize {
        self.forward_edits.capacity() * size_of::<SingleEdit>()
            + self.inverse_edits.capacity() * size_of::<SingleEdit>()
            + self.forward_batch_ends.capacity() * size_of::<u32>()
            + self.inverse_batch_ends.capacity() * size_of::<u32>()
            + self
                .forward_edits
                .iter()
                .map(|e| e.text.capacity())
                .sum::<usize>()
            + self
                .inverse_edits
                .iter()
                .map(|e| e.text.capacity())
                .sum::<usize>()
    }

    pub fn push_batch(
        &mut self,
        mut forward: Vec<SingleEdit>,
        mut inverse: Vec<SingleEdit>,
        cursor: Option<CursorMeta>,
    ) {
        if self.cursor_before.is_none() {
            self.cursor_before = cursor;
        }
        self.cursor_after = cursor;

        // Fast path: coalescing sequential single-character insertions (Insert run)
        if forward.len() == 1 && inverse.len() == 1 {
            let f_new = &forward[0];
            let i_new = &inverse[0];
            if f_new.range.is_empty() && !f_new.text.is_empty() && i_new.text.is_empty() {
                let prev_batch_start = if self.forward_batch_ends.len() >= 2 {
                    self.forward_batch_ends[self.forward_batch_ends.len() - 2] as usize
                } else {
                    0
                };
                if self.forward_edits.len() == prev_batch_start + 1
                    && self.inverse_edits.len() == prev_batch_start + 1
                {
                    let last_f = &mut self.forward_edits[prev_batch_start];
                    let last_i = &mut self.inverse_edits[prev_batch_start];
                    if last_f.range.is_empty()
                        && last_i.text.is_empty()
                        && f_new.range.start == last_f.range.start + last_f.text.len()
                    {
                        let old_cap = last_f.text.capacity();
                        last_f.text.push_str(&f_new.text);
                        last_i.range.end += f_new.text.len();
                        self.size_in_bytes += last_f.text.capacity().saturating_sub(old_cap);
                        return;
                    }
                }
            }
        }

        for edit in &mut forward {
            compact_edit_text(edit);
        }
        for edit in &mut inverse {
            compact_edit_text(edit);
        }
        self.forward_edits.extend(forward);
        self.forward_batch_ends
            .push(self.forward_edits.len() as u32);
        self.inverse_edits.extend(inverse);
        self.inverse_batch_ends
            .push(self.inverse_edits.len() as u32);
        self.size_in_bytes = size_of::<Self>() + self.heap_bytes();
    }

    pub fn forward_batches(
        &self,
    ) -> impl DoubleEndedIterator<Item = &[SingleEdit]> + ExactSizeIterator + '_ {
        (0..self.forward_batch_ends.len()).map(move |i| {
            let start = if i == 0 {
                0
            } else {
                self.forward_batch_ends[i - 1] as usize
            };
            let end = self.forward_batch_ends[i] as usize;
            &self.forward_edits[start..end]
        })
    }

    pub fn inverse_batches(
        &self,
    ) -> impl DoubleEndedIterator<Item = &[SingleEdit]> + ExactSizeIterator + '_ {
        (0..self.inverse_batch_ends.len()).map(move |i| {
            let start = if i == 0 {
                0
            } else {
                self.inverse_batch_ends[i - 1] as usize
            };
            let end = self.inverse_batch_ends[i] as usize;
            &self.inverse_edits[start..end]
        })
    }
}

/// History tracking bounded undo/redo stacks and editing registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    undo_stack: VecDeque<HistoryGroup>,
    redo_stack: VecDeque<HistoryGroup>,
    total_bytes: usize,
    max_bytes: usize,
    current_state: u64,
    next_state: u64,
    savepoint_state: u64,
    /// An oversized group stays unavailable until a new group or savepoint.
    discarded_group: Option<UndoGroup>,
    registers: Registers,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            total_bytes: 0,
            max_bytes: DEFAULT_MAX_HISTORY_BYTES,
            current_state: 0,
            next_state: 1,
            savepoint_state: 0,
            discarded_group: None,
            registers: Registers::new(),
        }
    }
}

impl History {
    /// Creates a new empty `History`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Retained undo/redo storage, including vector/deque capacity and text capacity.
    /// Excludes fixed `History` storage, registers, and allocator bookkeeping.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
            + (self.undo_stack.capacity() - self.undo_stack.len() + self.redo_stack.capacity()
                - self.redo_stack.len())
                * size_of::<HistoryGroup>()
    }

    /// Returns the configured memory limit in bytes.
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Updates the limit, evicting farthest undo then redo entries as needed.
    pub fn set_max_bytes(&mut self, limit: usize) {
        self.max_bytes = limit;
        self.enforce_memory_limit();
    }

    /// Accesses registers.
    pub fn registers(&self) -> &Registers {
        &self.registers
    }

    /// Mutably accesses registers.
    pub fn registers_mut(&mut self) -> &mut Registers {
        &mut self.registers
    }

    /// Returns `true` if undo is available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Records forward edits and their inverse under a given `UndoGroup`.
    pub fn record(
        &mut self,
        group_id: UndoGroup,
        forward: Vec<SingleEdit>,
        inverse: Vec<SingleEdit>,
        cursor: Option<CursorMeta>,
    ) {
        // Redo stack is invalidated on any new user edit
        for group in self.redo_stack.drain(..) {
            self.total_bytes = self.total_bytes.saturating_sub(group.size_in_bytes);
        }
        let before_state = self.current_state;
        self.current_state = self.next_state;
        self.next_state += 1;

        if self.discarded_group == Some(group_id) && before_state != self.savepoint_state {
            return;
        }
        self.discarded_group = None;

        // Saving splits a group so undo can return to the exact saved text.
        if let Some(top) = self.undo_stack.back_mut()
            && top.group_id == group_id
            && before_state != self.savepoint_state
        {
            let old_size = top.size_in_bytes;
            top.push_batch(forward, inverse, cursor);
            top.after_state = self.current_state;
            let delta = top.size_in_bytes.saturating_sub(old_size);
            self.total_bytes += delta;
            self.enforce_memory_limit();
            return;
        }

        let mut group = HistoryGroup::new(group_id, before_state, self.current_state);
        group.push_batch(forward, inverse, cursor);
        self.total_bytes += group.size_in_bytes;
        self.undo_stack.push_back(group);

        self.enforce_memory_limit();
    }

    /// Pops the top undo group from the undo stack.
    pub fn pop_undo(&mut self) -> Option<HistoryGroup> {
        let group = self.undo_stack.pop_back()?;
        self.total_bytes = self.total_bytes.saturating_sub(group.size_in_bytes);
        self.current_state = group.before_state;
        self.discarded_group = None;
        Some(group)
    }

    /// Peeks at the top undo group without popping.
    pub fn peek_undo(&self) -> Option<&HistoryGroup> {
        self.undo_stack.back()
    }

    /// Pushes a group onto the redo stack.
    pub fn push_redo(&mut self, group: HistoryGroup) {
        self.current_state = group.before_state;
        self.total_bytes += group.size_in_bytes;
        self.redo_stack.push_back(group);
        self.enforce_memory_limit();
    }

    /// Pops the top redo group from the redo stack.
    pub fn pop_redo(&mut self) -> Option<HistoryGroup> {
        let group = self.redo_stack.pop_back()?;
        self.total_bytes = self.total_bytes.saturating_sub(group.size_in_bytes);
        self.current_state = group.after_state;
        self.discarded_group = None;
        Some(group)
    }

    /// Peeks at the top redo group without popping.
    pub fn peek_redo(&self) -> Option<&HistoryGroup> {
        self.redo_stack.back()
    }

    /// Pushes a group back onto the undo stack (used during redo).
    pub fn push_undo(&mut self, group: HistoryGroup) {
        self.current_state = group.after_state;
        self.total_bytes += group.size_in_bytes;
        self.undo_stack.push_back(group);
        self.enforce_memory_limit();
    }

    /// Marks the current undo position as the savepoint.
    pub fn mark_savepoint(&mut self) {
        self.savepoint_state = self.current_state;
        self.discarded_group = None;
    }

    /// Checks if the current undo position matches the savepoint.
    pub fn is_at_savepoint(&self) -> bool {
        self.current_state == self.savepoint_state
    }

    fn enforce_memory_limit(&mut self) {
        while self.total_bytes() > self.max_bytes {
            // Keep useful deque capacity between edits. Compact only after a
            // substantial reduction, avoiding a realloc/evict cycle at the limit.
            if self.undo_stack.capacity() > self.undo_stack.len().saturating_mul(2) {
                self.undo_stack.shrink_to_fit();
            }
            if self.redo_stack.capacity() > self.redo_stack.len().saturating_mul(2) {
                self.redo_stack.shrink_to_fit();
            }
            if self.total_bytes() <= self.max_bytes {
                break;
            }
            let evicted = if let Some(group) = self.undo_stack.pop_front() {
                if group.after_state == self.current_state {
                    self.discarded_group = Some(group.group_id);
                }
                group
            } else if let Some(group) = self.redo_stack.pop_front() {
                group
            } else {
                break;
            };
            self.total_bytes -= evicted.size_in_bytes;
        }
    }
}
