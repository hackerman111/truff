//! Text buffers, transactions, edit history, and grapheme-aware navigation.

pub mod buffer;
pub mod edit;
pub mod history;

pub use buffer::{Buffer, BufferId, BytePos, DEFAULT_MAX_FILE_BYTES, LineEnding};
pub use edit::{Change, ChangeSet, CursorMeta, EditError, SingleEdit, Transaction, UndoGroup};
pub use history::{DEFAULT_MAX_HISTORY_BYTES, History, Register, RegisterKind, Registers};
