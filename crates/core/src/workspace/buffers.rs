use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::commands::AppError;
use crate::text::Buffer;
pub use crate::text::BufferId;

/// Default maximum number of open buffer records in the registry (§10.1).
pub const DEFAULT_MAX_BUFFERS: usize = 256;

/// Monotonically increasing epoch identifier for a vault session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VaultEpoch(pub u64);

impl VaultEpoch {
    /// Creates a `VaultEpoch` with an explicit number.
    pub const fn new(epoch: u64) -> Self {
        Self(epoch)
    }

    /// Returns the raw epoch number.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next monotonic epoch.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl fmt::Display for VaultEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Generation key uniquely identifying an immutable buffer snapshot: `(VaultEpoch, BufferId, Revision)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotKey {
    /// The vault epoch when this snapshot was captured.
    pub epoch: VaultEpoch,
    /// The buffer's unique identifier.
    pub buffer_id: BufferId,
    /// The buffer's monotonic revision number.
    pub revision: u64,
}

impl SnapshotKey {
    /// Creates a new `SnapshotKey`.
    pub const fn new(epoch: VaultEpoch, buffer_id: BufferId, revision: u64) -> Self {
        Self {
            epoch,
            buffer_id,
            revision,
        }
    }
}

impl fmt::Display for SnapshotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(epoch:{}, buf:{}, rev:{})",
            self.epoch.0, self.buffer_id.0, self.revision
        )
    }
}

/// Lifecycle state of a buffer within the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BufferState {
    /// Buffer metadata is tracked, but text Rope is not resident in memory.
    #[default]
    Unloaded,
    /// Buffer text matches the disk state; no unsaved edits.
    Clean,
    /// Buffer has in-memory modifications not yet persisted to disk.
    Dirty,
    /// Buffer has in-memory edits, and disk content was modified externally.
    Conflict,
}

/// An entry in the buffer registry containing buffer metadata and optional resident text.
pub struct BufferEntry {
    id: BufferId,
    path: Option<PathBuf>,
    state: BufferState,
    buffer: Option<Buffer>,
    fingerprint: Option<String>,
}

impl BufferEntry {
    /// Creates a new `BufferEntry`.
    pub fn new(id: BufferId, path: Option<PathBuf>, buffer: Option<Buffer>) -> Self {
        let state = if buffer.is_some() {
            BufferState::Clean
        } else {
            BufferState::Unloaded
        };
        Self {
            id,
            path,
            state,
            buffer,
            fingerprint: None,
        }
    }

    /// Returns the buffer ID.
    pub fn id(&self) -> BufferId {
        self.id
    }

    /// Returns the buffer's associated file path, if any.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Returns the current lifecycle state.
    pub fn state(&self) -> BufferState {
        self.state
    }

    /// Updates the buffer state.
    pub fn set_state(&mut self, state: BufferState) {
        self.state = state;
    }

    /// Reference to the underlying text buffer, if loaded.
    pub fn buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    /// Mutable reference to the underlying text buffer, if loaded.
    pub fn buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.buffer.as_mut()
    }

    /// Sets the underlying text buffer.
    pub fn set_buffer(&mut self, buffer: Option<Buffer>) {
        self.buffer = buffer;
    }

    /// Associated disk fingerprint.
    pub fn fingerprint(&self) -> Option<&str> {
        self.fingerprint.as_deref()
    }

    /// Updates the disk fingerprint.
    pub fn set_fingerprint(&mut self, fp: Option<String>) {
        self.fingerprint = fp;
    }
}

/// Registry managing all open/tracked buffers in a workspace.
pub struct BufferRegistry {
    buffers: HashMap<BufferId, BufferEntry>,
    path_to_id: HashMap<PathBuf, BufferId>,
    closed_paths: HashMap<PathBuf, BufferId>,
    replaced_by: HashMap<BufferId, BufferId>,
    epoch: VaultEpoch,
    max_buffers: usize,
}

impl BufferRegistry {
    /// Creates an empty registry for the given vault epoch.
    pub fn new(epoch: VaultEpoch) -> Self {
        Self {
            buffers: HashMap::new(),
            path_to_id: HashMap::new(),
            closed_paths: HashMap::new(),
            replaced_by: HashMap::new(),
            epoch,
            max_buffers: DEFAULT_MAX_BUFFERS,
        }
    }

    /// Returns the current vault epoch.
    pub fn epoch(&self) -> VaultEpoch {
        self.epoch
    }

    /// Updates the current vault epoch.
    pub fn set_epoch(&mut self, epoch: VaultEpoch) {
        self.epoch = epoch;
    }

    /// Number of tracked buffers.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Returns `true` if no buffers are registered.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Configured maximum buffers admission cap.
    pub fn max_buffers(&self) -> usize {
        self.max_buffers
    }

    /// Returns an entry by `BufferId`.
    pub fn get(&self, id: BufferId) -> Option<&BufferEntry> {
        self.buffers.get(&id)
    }

    /// Returns a mutable entry by `BufferId`.
    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut BufferEntry> {
        self.buffers.get_mut(&id)
    }

    /// Returns reference to the text `Buffer` if loaded.
    pub fn get_buffer(&self, id: BufferId) -> Option<&Buffer> {
        self.buffers.get(&id)?.buffer()
    }

    /// Returns mutable reference to the text `Buffer` if loaded.
    pub fn get_buffer_mut(&mut self, id: BufferId) -> Option<&mut Buffer> {
        self.buffers.get_mut(&id)?.buffer_mut()
    }

    /// Returns the state of a buffer.
    pub fn get_state(&self, id: BufferId) -> Option<BufferState> {
        self.buffers.get(&id).map(|e| e.state())
    }

    /// Looks up `BufferId` for a given path.
    pub fn get_by_path(&self, path: &Path) -> Option<BufferId> {
        self.path_to_id.get(path).copied()
    }

    /// Returns the snapshot key for an existing loaded buffer.
    pub fn get_snapshot_key(&self, id: BufferId) -> Option<SnapshotKey> {
        let entry = self.buffers.get(&id)?;
        let rev = entry.buffer().map(|b| b.revision()).unwrap_or(0);
        Some(SnapshotKey::new(self.epoch, id, rev))
    }

    /// Returns the replacement `BufferId` if a closed buffer was superseded by reopening.
    pub fn get_replacement_id(&self, old_id: BufferId) -> Option<BufferId> {
        self.replaced_by.get(&old_id).copied()
    }

    /// Opens or registers a buffer populated with initial text.
    pub fn open_from_text(
        &mut self,
        path: Option<PathBuf>,
        text: &str,
    ) -> Result<BufferId, AppError> {
        if self.buffers.len() >= self.max_buffers {
            return Err(AppError::MaxBuffersExceeded(self.max_buffers));
        }

        let id = BufferId::next();
        let buf = Buffer::from_text(id, text)?;
        let mut entry = BufferEntry::new(id, path.clone(), Some(buf));
        entry.set_state(BufferState::Clean);

        if let Some(ref p) = path {
            if let Some(old_id) = self.path_to_id.insert(p.clone(), id) {
                self.replaced_by.insert(old_id, id);
            } else if let Some(old_id) = self.closed_paths.remove(p) {
                self.replaced_by.insert(old_id, id);
            }
        }

        self.buffers.insert(id, entry);
        Ok(id)
    }

    /// Forcefully closes and removes a buffer from the registry, recording tombstone if path exists.
    pub fn close_force(&mut self, id: BufferId) -> Result<(), AppError> {
        let entry = self
            .buffers
            .remove(&id)
            .ok_or(AppError::BufferNotFound(id))?;
        if let Some(p) = entry.path() {
            self.closed_paths.insert(p.to_path_buf(), id);
            if self.path_to_id.get(p) == Some(&id) {
                self.path_to_id.remove(p);
            }
        }
        Ok(())
    }

    /// Closes a buffer only if it is not Dirty or Conflict.
    ///
    /// Returns `Ok(true)` if closed, `Ok(false)` if buffer is dirty/conflict and kept.
    pub fn close(&mut self, id: BufferId) -> Result<bool, AppError> {
        let entry = self.buffers.get(&id).ok_or(AppError::BufferNotFound(id))?;
        if entry.state() == BufferState::Dirty || entry.state() == BufferState::Conflict {
            return Ok(false);
        }
        self.close_force(id)?;
        Ok(true)
    }

    /// Records that an old buffer was replaced by a newly opened buffer.
    pub fn record_replacement(&mut self, old_id: BufferId, new_id: BufferId) {
        self.replaced_by.insert(old_id, new_id);
    }
}
