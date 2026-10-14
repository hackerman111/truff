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
    revision_floor: u64,
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
            revision_floor: 0,
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

    pub(crate) fn revision(&self) -> u64 {
        self.buffer().map_or(self.revision_floor, Buffer::revision)
    }

    pub(crate) fn revision_floor(&self) -> u64 {
        self.revision_floor
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
    watch_generation: u64,
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
            watch_generation: 0,
        }
    }

    /// Returns the current vault epoch.
    pub fn epoch(&self) -> VaultEpoch {
        self.epoch
    }

    /// Updates the current vault epoch.
    pub fn set_epoch(&mut self, epoch: VaultEpoch) {
        if self.epoch != epoch {
            self.closed_paths.clear();
            self.replaced_by.clear();
        }
        self.epoch = epoch;
        self.watch_generation = self.watch_generation.wrapping_add(1);
    }

    /// Changes only when watched identities, paths or vault epoch change.
    pub fn watch_generation(&self) -> u64 {
        self.watch_generation
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

    pub fn entries(&self) -> impl Iterator<Item = &BufferEntry> {
        self.buffers.values()
    }

    /// Registers the identity before issuing a load so replies have a live destination.
    pub fn register_unloaded(&mut self, path: PathBuf) -> Result<BufferId, AppError> {
        if let Some(id) = self.get_by_path(&path) {
            return Ok(id);
        }
        if self.buffers.len() >= self.max_buffers {
            return Err(AppError::MaxBuffersExceeded(self.max_buffers));
        }
        self.watch_generation = self.watch_generation.wrapping_add(1);
        let id = BufferId::next();
        if let Some(old_id) = self.closed_paths.remove(&path) {
            self.record_replacement(old_id, id);
        }
        self.path_to_id.insert(path.clone(), id);
        self.buffers
            .insert(id, BufferEntry::new(id, Some(path), None));
        Ok(id)
    }

    pub fn rename_path(&mut self, from: &Path, to: PathBuf) -> Result<(), AppError> {
        if self.path_to_id.contains_key(&to) {
            return Err(AppError::BufferAlreadyOpen(to));
        }
        if let Some(id) = self.path_to_id.remove(from) {
            self.watch_generation = self.watch_generation.wrapping_add(1);
            self.buffers
                .get_mut(&id)
                .expect("registered path has a buffer")
                .path = Some(to.clone());
            self.path_to_id.insert(to, id);
        }
        Ok(())
    }

    /// Rebinds an existing identity only after its save-as destination is durable.
    pub fn assign_path(&mut self, id: BufferId, path: PathBuf) -> Result<(), AppError> {
        if self
            .path_to_id
            .get(&path)
            .is_some_and(|existing| *existing != id)
        {
            return Err(AppError::BufferAlreadyOpen(path));
        }
        let entry = self
            .buffers
            .get_mut(&id)
            .ok_or(AppError::BufferNotFound(id))?;
        self.watch_generation = self.watch_generation.wrapping_add(1);
        if let Some(old) = entry.path.replace(path.clone()) {
            self.path_to_id.remove(&old);
        }
        self.path_to_id.insert(path, id);
        Ok(())
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

    /// Returns the snapshot key, including the reserved revision of an unloaded buffer.
    pub fn get_snapshot_key(&self, id: BufferId) -> Option<SnapshotKey> {
        let entry = self.buffers.get(&id)?;
        let rev = entry.revision();
        Some(SnapshotKey::new(self.epoch, id, rev))
    }

    pub(crate) fn unload_clean(&mut self, id: BufferId) -> Result<bool, AppError> {
        let entry = self
            .buffers
            .get_mut(&id)
            .ok_or(AppError::BufferNotFound(id))?;
        let Some(buffer) = entry.buffer() else {
            return Ok(false);
        };
        if entry.state != BufferState::Clean || buffer.is_dirty() || entry.path.is_none() {
            return Ok(false);
        }
        let Some(revision) = buffer.revision().checked_add(1) else {
            return Ok(false);
        };
        entry.revision_floor = revision;
        entry.buffer = None;
        entry.state = BufferState::Unloaded;
        self.watch_generation = self.watch_generation.wrapping_add(1);
        Ok(true)
    }

    /// Returns the replacement `BufferId` if a closed buffer was superseded by reopening.
    pub fn get_replacement_id(&self, old_id: BufferId) -> Option<BufferId> {
        let mut id = *self.replaced_by.get(&old_id)?;
        for _ in 0..=self.replaced_by.len() {
            if let Some(next) = self.replaced_by.get(&id) {
                id = *next;
            } else {
                return self.buffers.contains_key(&id).then_some(id);
            }
        }
        None
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

        self.watch_generation = self.watch_generation.wrapping_add(1);
        let id = BufferId::next();
        let buf = Buffer::from_text(id, text)?;
        let mut entry = BufferEntry::new(id, path.clone(), Some(buf));
        entry.set_state(BufferState::Clean);

        if let Some(ref p) = path {
            if let Some(old_id) = self.path_to_id.insert(p.clone(), id) {
                self.record_replacement(old_id, id);
            } else if let Some(old_id) = self.closed_paths.remove(p) {
                self.record_replacement(old_id, id);
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
        self.watch_generation = self.watch_generation.wrapping_add(1);
        if let Some(p) = entry.path()
            && self.path_to_id.get(p) == Some(&id)
        {
            self.closed_paths.insert(p.to_path_buf(), id);
            self.path_to_id.remove(p);
            if self.closed_paths.len() > self.max_buffers {
                let oldest = self
                    .closed_paths
                    .iter()
                    .min_by_key(|(_, id)| **id)
                    .map(|(path, _)| path.clone());
                if let Some(path) = oldest {
                    self.closed_paths.remove(&path);
                }
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
        if old_id == new_id || self.get_replacement_id(new_id) == Some(old_id) {
            return;
        }
        // Flatten retained aliases while their common destination changes.
        for target in self.replaced_by.values_mut() {
            if *target == old_id {
                *target = new_id;
            }
        }
        self.replaced_by.insert(old_id, new_id);
        if self.replaced_by.len() > self.max_buffers
            && let Some(oldest) = self.replaced_by.keys().min().copied()
        {
            self.replaced_by.remove(&oldest);
        }
    }
}
