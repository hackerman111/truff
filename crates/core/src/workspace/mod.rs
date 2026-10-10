use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod buffers;
pub mod views;

pub use buffers::{BufferEntry, BufferRegistry, BufferState, SnapshotKey, VaultEpoch};
pub use views::{JumpHistory, View, ViewId};

use crate::commands::{Action, AppError, Effect};
use crate::text::BufferId;

/// Default maximum number of open views in a workspace (§10.1).
pub const DEFAULT_MAX_VIEWS: usize = 32;

/// A background I/O or persistence job dispatched from `core` to `store`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreJob {
    /// Request reading file content from disk.
    LoadFile { key: SnapshotKey, path: PathBuf },
    /// Request durable atomic saving of a buffer snapshot to disk.
    SaveFile {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
        base_fingerprint: Option<String>,
    },
    /// Request writing an emergency crash recovery snapshot.
    WriteRecoverySnapshot {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
        base_fingerprint: Option<String>,
    },
    /// Request deleting a recovery snapshot after successful save.
    DeleteRecoverySnapshot { buffer_id: BufferId },
}

/// An asynchronous persistence event emitted by `store` to `core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreEvent {
    /// File content successfully read from disk.
    FileLoaded {
        key: SnapshotKey,
        content: String,
        fingerprint: String,
    },
    /// File snapshot successfully written and synced to disk.
    SaveAck {
        key: SnapshotKey,
        fingerprint: String,
    },
    /// File saving failed due to I/O error or permission failure.
    SaveFailed { key: SnapshotKey, error: String },
    /// Recovery snapshot was written to disk.
    RecoveryAck { key: SnapshotKey },
    /// External file modification was detected while buffer has unsaved changes.
    Conflict {
        key: SnapshotKey,
        disk_content: Option<String>,
        disk_fingerprint: String,
    },
    /// File was modified externally.
    ExternalChanged {
        buffer_id: BufferId,
        path: PathBuf,
        new_fingerprint: String,
    },
}

/// Central workspace managing buffer registry, active views, and vault epoch.
pub struct Workspace {
    epoch: VaultEpoch,
    buffers: BufferRegistry,
    views: HashMap<ViewId, View>,
    active_view: Option<ViewId>,
    max_views: usize,
}

impl Workspace {
    /// Creates a new workspace with epoch 0 and default limits.
    pub fn new() -> Self {
        Self::with_epoch(VaultEpoch::new(0))
    }

    /// Creates a workspace initialized with an explicit `VaultEpoch`.
    pub fn with_epoch(epoch: VaultEpoch) -> Self {
        Self {
            epoch,
            buffers: BufferRegistry::new(epoch),
            views: HashMap::new(),
            active_view: None,
            max_views: DEFAULT_MAX_VIEWS,
        }
    }

    /// Current vault epoch.
    pub fn epoch(&self) -> VaultEpoch {
        self.epoch
    }

    /// Advances to a new vault epoch, invalidating stale replies from the previous epoch.
    pub fn switch_vault(&mut self, epoch: VaultEpoch) {
        self.epoch = epoch;
        self.buffers.set_epoch(epoch);
    }

    /// Reference to the buffer registry.
    pub fn buffers(&self) -> &BufferRegistry {
        &self.buffers
    }

    /// Mutable reference to the buffer registry.
    pub fn buffers_mut(&mut self) -> &mut BufferRegistry {
        &mut self.buffers
    }

    /// Tracked views map.
    pub fn views(&self) -> &HashMap<ViewId, View> {
        &self.views
    }

    /// Returns a reference to a view by `ViewId`.
    pub fn get_view(&self, view_id: ViewId) -> Option<&View> {
        self.views.get(&view_id)
    }

    /// Returns a mutable reference to a view by `ViewId`.
    pub fn get_view_mut(&mut self, view_id: ViewId) -> Option<&mut View> {
        self.views.get_mut(&view_id)
    }

    /// Returns the active view ID, if any.
    pub fn active_view(&self) -> Option<ViewId> {
        self.active_view
    }

    /// Sets the active view.
    pub fn set_active_view(&mut self, view_id: Option<ViewId>) {
        self.active_view = view_id;
    }

    /// Opens a buffer populated from initial text.
    pub fn open_buffer_from_text(
        &mut self,
        path: PathBuf,
        text: &str,
    ) -> Result<BufferId, AppError> {
        self.buffers.open_from_text(Some(path), text)
    }

    /// Creates a new view attached to the given buffer ID.
    pub fn create_view(&mut self, buffer_id: BufferId) -> Result<ViewId, AppError> {
        if self.buffers.get(buffer_id).is_none() {
            return Err(AppError::BufferNotFound(buffer_id));
        }
        if self.views.len() >= self.max_views {
            return Err(AppError::MaxViewsExceeded(self.max_views));
        }

        let view_id = ViewId::next();
        let view = View::new(view_id, buffer_id);
        self.views.insert(view_id, view);
        self.active_view = Some(view_id);
        Ok(view_id)
    }

    /// Closes a view. Preserves any dirty buffer in the registry.
    pub fn close_view(&mut self, view_id: ViewId) -> Result<Vec<Effect>, AppError> {
        if self.views.remove(&view_id).is_none() {
            return Err(AppError::ViewNotFound(view_id));
        }

        if self.active_view == Some(view_id) {
            self.active_view = self.views.keys().next().copied();
        }

        Ok(vec![Effect::ViewClosed(view_id)])
    }

    /// Forcefully closes and removes a buffer from the registry.
    pub fn close_buffer_force(&mut self, buffer_id: BufferId) -> Result<(), AppError> {
        self.buffers.close_force(buffer_id)
    }

    /// Dispatches an action and returns the resulting effects.
    pub fn dispatch(&mut self, action: Action) -> Result<Vec<Effect>, AppError> {
        let mut effects = Vec::new();

        match action {
            Action::OpenPath(path) => {
                if let Some(buf_id) = self.buffers.get_by_path(&path) {
                    let view_id = self.create_view(buf_id)?;
                    effects.push(Effect::InvalidateView(view_id));
                } else {
                    let buf_id = BufferId::next();
                    let key = SnapshotKey::new(self.epoch, buf_id, 0);
                    effects.push(Effect::SubmitStoreJob(StoreJob::LoadFile {
                        key,
                        path: path.clone(),
                    }));
                }
            }
            Action::CreateView(buffer_id) => {
                let view_id = self.create_view(buffer_id)?;
                effects.push(Effect::InvalidateView(view_id));
            }
            Action::CloseView(view_id) => {
                let effs = self.close_view(view_id)?;
                effects.extend(effs);
            }
            Action::FocusView(view_id) => {
                if !self.views.contains_key(&view_id) {
                    return Err(AppError::ViewNotFound(view_id));
                }
                self.active_view = Some(view_id);
                effects.push(Effect::InvalidateView(view_id));
            }
            Action::MoveCursor {
                view_id,
                anchor,
                head,
            } => {
                let view = self
                    .views
                    .get_mut(&view_id)
                    .ok_or(AppError::ViewNotFound(view_id))?;
                view.set_cursor(anchor, head);
                effects.push(Effect::InvalidateView(view_id));
            }
            Action::Scroll { view_id, top_line } => {
                let view = self
                    .views
                    .get_mut(&view_id)
                    .ok_or(AppError::ViewNotFound(view_id))?;
                view.set_scroll_top_line(top_line);
                effects.push(Effect::InvalidateView(view_id));
            }
            Action::ApplyEdit {
                view_id,
                transaction,
            } => {
                let (buf_id, explicit_cursor) = {
                    let view = self
                        .views
                        .get(&view_id)
                        .ok_or(AppError::ViewNotFound(view_id))?;
                    (view.buffer_id(), transaction.cursor)
                };

                let buf = self
                    .buffers
                    .get_buffer_mut(buf_id)
                    .ok_or(AppError::BufferNotFound(buf_id))?;
                let changes = buf.apply(transaction)?;
                let is_dirty = buf.is_dirty();

                if is_dirty && let Some(entry) = self.buffers.get_mut(buf_id) {
                    entry.set_state(BufferState::Dirty);
                }

                // Rebase views looking at this buffer
                for v in self.views.values_mut() {
                    if v.buffer_id() == buf_id {
                        if v.id() == view_id
                            && let Some(c) = explicit_cursor
                        {
                            v.set_cursor(c.anchor, c.head);
                        } else {
                            v.apply_change_set(&changes);
                        }
                        effects.push(Effect::InvalidateView(v.id()));
                    }
                }

                if is_dirty {
                    effects.push(Effect::BufferStateChanged {
                        buffer_id: buf_id,
                        state: BufferState::Dirty,
                    });
                }
            }
            Action::SaveBuffer(buffer_id) => {
                let entry = self
                    .buffers
                    .get(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                let path = entry
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("untitled.md"));
                let content = entry
                    .buffer()
                    .map(|b| b.rope().to_string())
                    .unwrap_or_default();
                let rev = entry.buffer().map(|b| b.revision()).unwrap_or(0);
                let key = SnapshotKey::new(self.epoch, buffer_id, rev);
                let base_fingerprint = entry.fingerprint().map(str::to_string);

                effects.push(Effect::SubmitStoreJob(StoreJob::SaveFile {
                    key,
                    path,
                    content,
                    base_fingerprint,
                }));
            }
            Action::ReloadBuffer(buffer_id) => {
                let entry = self
                    .buffers
                    .get(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                let path = entry
                    .path()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("untitled.md"));
                let rev = entry.buffer().map(|b| b.revision()).unwrap_or(0);
                let key = SnapshotKey::new(self.epoch, buffer_id, rev);
                effects.push(Effect::SubmitStoreJob(StoreJob::LoadFile { key, path }));
            }
            Action::Undo(view_id) => {
                let buf_id = self
                    .views
                    .get(&view_id)
                    .ok_or(AppError::ViewNotFound(view_id))?
                    .buffer_id();
                let buf = self
                    .buffers
                    .get_buffer_mut(buf_id)
                    .ok_or(AppError::BufferNotFound(buf_id))?;
                if let Some(changes) = buf.undo()? {
                    for v in self.views.values_mut() {
                        if v.buffer_id() == buf_id {
                            v.apply_change_set(&changes);
                            effects.push(Effect::InvalidateView(v.id()));
                        }
                    }
                }
            }
            Action::Redo(view_id) => {
                let buf_id = self
                    .views
                    .get(&view_id)
                    .ok_or(AppError::ViewNotFound(view_id))?
                    .buffer_id();
                let buf = self
                    .buffers
                    .get_buffer_mut(buf_id)
                    .ok_or(AppError::BufferNotFound(buf_id))?;
                if let Some(changes) = buf.redo()? {
                    for v in self.views.values_mut() {
                        if v.buffer_id() == buf_id {
                            v.apply_change_set(&changes);
                            effects.push(Effect::InvalidateView(v.id()));
                        }
                    }
                }
            }
            Action::SwitchVault(epoch) => {
                self.switch_vault(epoch);
                effects.push(Effect::StatusMessage(format!(
                    "Switched to vault epoch {epoch}"
                )));
            }
            Action::CreateFile(path) => {
                effects.push(Effect::StatusMessage(format!(
                    "Create file: {}",
                    path.display()
                )));
            }
            Action::CreateFolder(path) => {
                effects.push(Effect::StatusMessage(format!(
                    "Create folder: {}",
                    path.display()
                )));
            }
            Action::RenamePath { from, to } => {
                effects.push(Effect::StatusMessage(format!(
                    "Rename {} to {}",
                    from.display(),
                    to.display()
                )));
            }
            Action::TrashPath(path) => {
                effects.push(Effect::StatusMessage(format!(
                    "Move to trash: {}",
                    path.display()
                )));
            }
        }

        Ok(effects)
    }

    /// Accepts an asynchronous store event, validates generations, and updates state.
    pub fn accept(&mut self, event: StoreEvent) -> Result<Vec<Effect>, AppError> {
        let mut effects = Vec::new();

        // Check vault epoch generation
        let key = match &event {
            StoreEvent::FileLoaded { key, .. }
            | StoreEvent::SaveAck { key, .. }
            | StoreEvent::SaveFailed { key, .. }
            | StoreEvent::RecoveryAck { key }
            | StoreEvent::Conflict { key, .. } => *key,
            StoreEvent::ExternalChanged { buffer_id, .. } => {
                let rev = self
                    .buffers
                    .get(*buffer_id)
                    .and_then(|e| e.buffer().map(|b| b.revision()))
                    .unwrap_or(0);
                SnapshotKey::new(self.epoch, *buffer_id, rev)
            }
        };

        if key.epoch != self.epoch {
            return Err(AppError::StaleEpoch {
                current: self.epoch,
                received: key.epoch,
            });
        }

        // Check if buffer is tracked, or if it was superseded by reopening
        if self.buffers.get(key.buffer_id).is_none() {
            if let Some(new_id) = self.buffers.get_replacement_id(key.buffer_id) {
                let expected = self
                    .buffers
                    .get_snapshot_key(new_id)
                    .unwrap_or_else(|| SnapshotKey::new(self.epoch, new_id, 0));
                return Err(AppError::StaleSnapshotKey {
                    expected,
                    actual: key,
                });
            }
            return Err(AppError::BufferNotFound(key.buffer_id));
        }

        match event {
            StoreEvent::SaveAck { key, fingerprint } => {
                let entry = self
                    .buffers
                    .get_mut(key.buffer_id)
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                let buf = entry
                    .buffer_mut()
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                let current_rev = buf.revision();

                if current_rev == key.revision {
                    buf.set_savepoint();
                    entry.set_state(BufferState::Clean);
                    entry.set_fingerprint(Some(fingerprint));
                    effects.push(Effect::BufferStateChanged {
                        buffer_id: key.buffer_id,
                        state: BufferState::Clean,
                    });
                    effects.push(Effect::StatusMessage("Saved".to_string()));
                } else if current_rev > key.revision {
                    // Late save ack: user made newer edits while saving!
                    // Buffer remains dirty. Do NOT clear dirty state.
                    entry.set_state(BufferState::Dirty);
                    effects.push(Effect::StatusMessage(format!(
                        "Saved rev {}, but buffer has newer edits (rev {})",
                        key.revision, current_rev
                    )));
                } else {
                    let expected = SnapshotKey::new(self.epoch, key.buffer_id, current_rev);
                    return Err(AppError::StaleSnapshotKey {
                        expected,
                        actual: key,
                    });
                }
            }
            StoreEvent::SaveFailed { key, error } => {
                if let Some(entry) = self.buffers.get_mut(key.buffer_id) {
                    entry.set_state(BufferState::Dirty);
                }
                effects.push(Effect::StatusMessage(format!("Save failed: {error}")));
            }
            StoreEvent::Conflict {
                key,
                disk_fingerprint,
                ..
            } => {
                if let Some(entry) = self.buffers.get_mut(key.buffer_id) {
                    entry.set_state(BufferState::Conflict);
                    entry.set_fingerprint(Some(disk_fingerprint));
                }
                effects.push(Effect::BufferStateChanged {
                    buffer_id: key.buffer_id,
                    state: BufferState::Conflict,
                });
                effects.push(Effect::StatusMessage(
                    "Conflict detected: file was modified externally".to_string(),
                ));
            }
            StoreEvent::RecoveryAck { .. } => {
                effects.push(Effect::StatusMessage(
                    "Recovery snapshot synced".to_string(),
                ));
            }
            StoreEvent::FileLoaded {
                key,
                content,
                fingerprint,
            } => {
                let entry = self
                    .buffers
                    .get_mut(key.buffer_id)
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                let buf = crate::text::Buffer::from_text(key.buffer_id, &content)?;
                entry.set_buffer(Some(buf));
                entry.set_state(BufferState::Clean);
                entry.set_fingerprint(Some(fingerprint));
                effects.push(Effect::BufferStateChanged {
                    buffer_id: key.buffer_id,
                    state: BufferState::Clean,
                });
                for v in self.views.values() {
                    if v.buffer_id() == key.buffer_id {
                        effects.push(Effect::InvalidateView(v.id()));
                    }
                }
            }
            StoreEvent::ExternalChanged {
                buffer_id,
                new_fingerprint,
                ..
            } => {
                let entry = self
                    .buffers
                    .get_mut(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                if entry.state() == BufferState::Dirty {
                    entry.set_state(BufferState::Conflict);
                    entry.set_fingerprint(Some(new_fingerprint));
                    effects.push(Effect::BufferStateChanged {
                        buffer_id,
                        state: BufferState::Conflict,
                    });
                } else {
                    entry.set_fingerprint(Some(new_fingerprint));
                }
            }
        }

        Ok(effects)
    }
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}
