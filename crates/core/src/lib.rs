//! Core abstractions and data structures for Truff v2.
//!
//! Owns text buffers, transactions, command registry, and workspace model.
//! Completely independent of UI and SQL persistence.

pub use crop::Rope;
pub use text::{
    Buffer, BufferId, BytePos, Change, ChangeSet, CursorMeta, EditError, History, LineEnding,
    Register, RegisterKind, Registers, SingleEdit, Transaction, UndoGroup,
};

pub mod text;
