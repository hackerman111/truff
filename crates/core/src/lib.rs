//! Core abstractions and data structures for Truff v2.
//!
//! Owns text buffers, transactions, command registry, and workspace model.
//! Completely independent of UI and SQL persistence.

pub use crop::{Rope, RopeSlice};
pub use text::{
    Buffer, BufferId, BytePos, Change, ChangeSet, CursorMeta, EditError, History, LineEnding,
    Register, RegisterKind, Registers, SingleEdit, Transaction, UndoGroup,
};

pub use commands::{Action, AppError, CommandContext, CommandId, CommandSpec, Effect};
pub use workspace::buffers::{
    BufferEntry, BufferRegistry, BufferState, DEFAULT_MAX_BUFFERS, SnapshotKey, VaultEpoch,
};
pub use workspace::views::{DEFAULT_MAX_JUMP_ENTRIES, JumpHistory, View, ViewId};
pub use workspace::{DEFAULT_MAX_VIEWS, RefactorPlan, StoreEvent, StoreJob, Workspace};

pub mod commands;
pub mod config;
pub mod markdown;
pub mod navigation;
pub mod text;
pub mod vim;
pub mod workspace;
