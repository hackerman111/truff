use std::collections::{HashMap, VecDeque};
use std::mem::size_of;

use super::edit::{SingleEdit, Transaction, UndoGroup};

/// Default memory limit for undo/redo history per buffer: 8 MiB.
pub const DEFAULT_MAX_HISTORY_BYTES: usize = 8 * 1024 * 1024;

/// Kind of text selection stored in a register (Vim-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegisterKind {
    /// Characterwise text.
    #[default]
    Characterwise,
    /// Linewise text (includes terminating newline).
    Linewise,
    /// Blockwise rectangular text.
    Blockwise,
}

/// A stored register entry containing text and selection shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Register {
    /// Text content.
    pub text: String,
    /// Selection shape.
    pub kind: RegisterKind,
}

impl Register {
    /// Creates a new register entry.
    pub fn new(text: impl Into<String>, kind: RegisterKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    /// Returns estimated memory consumption in bytes.
    pub fn size_in_bytes(&self) -> usize {
        size_of::<Self>() + self.text.capacity()
    }
}

/// Collection of named, unnamed, and clipboard registers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Registers {
    unnamed: Option<Register>,
    named: HashMap<char, Register>,
}

impl Registers {
    /// Creates an empty register collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves a register by name (`None` or `Some('"')` accesses the unnamed register).
    pub fn get(&self, name: Option<char>) -> Option<&Register> {
        match name {
            None | Some('"') => self.unnamed.as_ref(),
            Some(c) => {
                let key = c.to_ascii_lowercase();
                self.named.get(&key)
            }
        }
    }

    /// Sets the value of a register.
    ///
    /// If `name` is uppercase (`'A'..='Z'`), appends to the corresponding lowercase register.
    /// In all cases, also updates the unnamed register.
    pub fn set(&mut self, name: Option<char>, value: Register) {
        match name {
            None | Some('"') => {
                self.unnamed = Some(value);
            }
            Some(c) if c.is_ascii_uppercase() => {
                let key = c.to_ascii_lowercase();
                if let Some(existing) = self.named.get_mut(&key) {
                    existing.text.push_str(&value.text);
                    existing.kind = value.kind;
                    self.unnamed = Some(existing.clone());
                } else {
                    self.named.insert(key, value.clone());
                    self.unnamed = Some(value);
                }
            }
            Some(c) => {
                let key = c.to_ascii_lowercase();
                self.named.insert(key, value.clone());
                self.unnamed = Some(value);
            }
        }
    }

    /// Clears all stored registers.
    pub fn clear(&mut self) {
        self.unnamed = None;
        self.named.clear();
    }
}

fn tx_heap_bytes(tx: &Transaction) -> usize {
    tx.edits.capacity() * size_of::<SingleEdit>()
        + tx.edits
            .iter()
            .map(|edit| edit.text.capacity())
            .sum::<usize>()
}

/// A group of transactions applied under the same `UndoGroup`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryGroup {
    /// Identifier of this undo group.
    pub group_id: UndoGroup,
    /// Forward transactions in chronological order.
    pub forward: Vec<Transaction>,
    /// Inverse transactions in chronological order; replay backwards for undo.
    pub inverse: Vec<Transaction>,
    /// Estimated memory usage in bytes.
    pub size_in_bytes: usize,
    before_state: u64,
    after_state: u64,
}

impl HistoryGroup {
    fn new(group_id: UndoGroup, before_state: u64, after_state: u64) -> Self {
        Self {
            group_id,
            forward: Vec::new(),
            inverse: Vec::new(),
            size_in_bytes: size_of::<Self>(),
            before_state,
            after_state,
        }
    }

    fn add(&mut self, forward_tx: Transaction, inverse_tx: Transaction) {
        let payload = tx_heap_bytes(&forward_tx) + tx_heap_bytes(&inverse_tx);
        let old_capacity = self.forward.capacity() + self.inverse.capacity();
        self.forward.push(forward_tx);
        self.inverse.push(inverse_tx);
        self.size_in_bytes += payload
            + (self.forward.capacity() + self.inverse.capacity() - old_capacity)
                * size_of::<Transaction>();
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

    /// Records a forward transaction and its inverse under a given `UndoGroup`.
    pub fn record(&mut self, group_id: UndoGroup, forward: Transaction, inverse: Transaction) {
        // Redo stack is invalidated on any new user edit
        for group in self.redo_stack.drain(..) {
            self.total_bytes = self.total_bytes.saturating_sub(group.size_in_bytes);
        }
        self.redo_stack.shrink_to_fit();
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
            top.add(forward, inverse);
            top.after_state = self.current_state;
            let delta = top.size_in_bytes - old_size;
            self.total_bytes += delta;
            self.enforce_memory_limit();
            return;
        }

        let mut group = HistoryGroup::new(group_id, before_state, self.current_state);
        group.add(forward, inverse);
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

    pub(crate) fn peek_undo(&self) -> Option<&HistoryGroup> {
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

    pub(crate) fn peek_redo(&self) -> Option<&HistoryGroup> {
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
            self.undo_stack.shrink_to_fit();
            self.redo_stack.shrink_to_fit();
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
