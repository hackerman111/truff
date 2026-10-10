use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

use crate::text::{BytePos, EditError, Transaction};
use crate::workspace::StoreJob;
use crate::workspace::buffers::{BufferId, BufferState, SnapshotKey, VaultEpoch};
use crate::workspace::views::ViewId;

/// Strongly-typed identifier for a command.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct CommandId(pub Cow<'static, str>);

impl CommandId {
    /// Constructs a `CommandId` from a static string slice.
    pub const fn from_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// Constructs a `CommandId` from an owned string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    /// Returns the command identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Actions dispatched to the `Workspace` from UI, shortcuts, commands, or tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open a buffer by path.
    OpenPath(PathBuf),
    /// Create a new view for an existing buffer.
    CreateView(BufferId),
    /// Close an existing view.
    CloseView(ViewId),
    /// Set focus to a specific view.
    FocusView(ViewId),
    /// Move cursor and selection within a view.
    MoveCursor {
        view_id: ViewId,
        anchor: BytePos,
        head: BytePos,
    },
    /// Scroll a view to a top line.
    Scroll { view_id: ViewId, top_line: usize },
    /// Apply an editing transaction to a view's buffer.
    ApplyEdit {
        view_id: ViewId,
        transaction: Transaction,
    },
    /// Request saving a buffer to disk.
    SaveBuffer(BufferId),
    /// Force reload a buffer from disk.
    ReloadBuffer(BufferId),
    /// Undo the last transaction in the view's buffer.
    Undo(ViewId),
    /// Redo the last undone transaction in the view's buffer.
    Redo(ViewId),
    /// Switch active vault epoch.
    SwitchVault(VaultEpoch),
    /// Create a new file on disk.
    CreateFile(PathBuf),
    /// Create a new folder on disk.
    CreateFolder(PathBuf),
    /// Rename or move a path.
    RenamePath { from: PathBuf, to: PathBuf },
    /// Move a path to trash / recovery.
    TrashPath(PathBuf),
}

/// Effects produced by `Workspace::dispatch` or `Workspace::accept`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Submit an asynchronous I/O job to the store worker.
    SubmitStoreJob(StoreJob),
    /// Notification that a view was closed.
    ViewClosed(ViewId),
    /// Notification that a view's cursor/scroll or text changed and requires redraw.
    InvalidateView(ViewId),
    /// Notification that buffer state changed (Clean, Dirty, Conflict).
    BufferStateChanged {
        buffer_id: BufferId,
        state: BufferState,
    },
    /// Status message to be displayed to the user.
    StatusMessage(String),
}

/// Specification for a command in the command registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: CommandId,
    pub title: Cow<'static, str>,
    pub description: Cow<'static, str>,
}

/// Errors occurring during workspace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    BufferNotFound(BufferId),
    ViewNotFound(ViewId),
    EditFailed(EditError),
    StaleSnapshotKey {
        expected: SnapshotKey,
        actual: SnapshotKey,
    },
    StaleEpoch {
        current: VaultEpoch,
        received: VaultEpoch,
    },
    BufferConflict {
        buffer_id: BufferId,
    },
    BufferAlreadyOpen(PathBuf),
    MaxViewsExceeded(usize),
    MaxBuffersExceeded(usize),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id}"),
            Self::ViewNotFound(id) => write!(f, "view not found: {id}"),
            Self::EditFailed(err) => write!(f, "edit failed: {err}"),
            Self::StaleSnapshotKey { expected, actual } => {
                write!(
                    f,
                    "stale snapshot key: received {actual:?}, expected {expected:?}"
                )
            }
            Self::StaleEpoch { current, received } => {
                write!(
                    f,
                    "stale vault epoch: received {received:?}, current {current:?}"
                )
            }
            Self::BufferConflict { buffer_id } => write!(f, "buffer conflict for {buffer_id}"),
            Self::BufferAlreadyOpen(path) => write!(f, "buffer already open: {}", path.display()),
            Self::MaxViewsExceeded(max) => write!(f, "maximum views exceeded: {max}"),
            Self::MaxBuffersExceeded(max) => write!(f, "maximum buffers exceeded: {max}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<EditError> for AppError {
    fn from(e: EditError) -> Self {
        Self::EditFailed(e)
    }
}
