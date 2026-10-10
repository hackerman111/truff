use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod buffers;
pub mod views;

pub use buffers::{BufferEntry, BufferRegistry, BufferState, SnapshotKey, VaultEpoch};
pub use views::{JumpHistory, View, ViewId};

use crate::commands::{Action, AppError, Effect};
use crate::text::{BufferId, ChangeSet};

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
    /// Explicitly approved reload, still invalidated by a newer local revision.
    ReloadFile { key: SnapshotKey, path: PathBuf },
    /// Explicit user approval to overwrite the external version.
    OverwriteFile {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
    },
    /// Durable creation of a distinct destination, preserving an existing file.
    SaveAsFile {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
    },
    /// Request writing an emergency crash recovery snapshot.
    WriteRecoverySnapshot {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
        base_fingerprint: Option<String>,
    },
    /// Request deleting recovery snapshots once a buffer returns to its savepoint.
    DeleteRecoverySnapshot { key: SnapshotKey },
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
    FileReloaded {
        key: SnapshotKey,
        content: String,
        fingerprint: String,
    },
    SavedAs {
        key: SnapshotKey,
        path: PathBuf,
        fingerprint: String,
    },
    /// File snapshot successfully written and synced to disk.
    SaveAck {
        key: SnapshotKey,
        fingerprint: String,
    },
    /// File saving failed due to I/O error or permission failure.
    SaveFailed { key: SnapshotKey, error: String },
    /// Recovery snapshot was written or deleted on disk.
    RecoveryAck { key: SnapshotKey },
    /// External file modification was detected while buffer has unsaved changes.
    Conflict {
        key: SnapshotKey,
        disk_content: Option<String>,
        disk_fingerprint: String,
    },
    /// File was modified externally.
    ExternalChanged {
        key: SnapshotKey,
        path: PathBuf,
        new_fingerprint: String,
    },
}

/// Bounded layout notification; ranges use source coordinates before/after this edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferChange {
    pub buffer_id: BufferId,
    pub revision: u64,
    pub old_range: std::ops::Range<usize>,
    pub new_range: std::ops::Range<usize>,
    /// Overflow/grouped history may require invalidation from this source offset onward.
    pub invalidate_suffix: bool,
}

/// Central workspace managing buffer registry, active views, and vault epoch.
pub struct Workspace {
    epoch: VaultEpoch,
    buffers: BufferRegistry,
    views: HashMap<ViewId, View>,
    active_view: Option<ViewId>,
    max_views: usize,
    buffer_changes: Vec<BufferChange>,
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
            buffer_changes: Vec::new(),
        }
    }

    /// Drains at most the registry buffer limit of change summaries. No text snapshots are stored.
    pub fn take_buffer_changes(&mut self) -> Vec<BufferChange> {
        std::mem::take(&mut self.buffer_changes)
    }

    fn notify_buffer_change(&mut self, buffer_id: BufferId, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        self.buffer_changes
            .retain(|notice| self.buffers.get_buffer(notice.buffer_id).is_some());
        let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
            return;
        };
        let start = changes
            .changes()
            .iter()
            .map(|c| c.old_range.start)
            .min()
            .unwrap_or(0);
        let old_end = changes
            .changes()
            .iter()
            .map(|c| c.old_range.end)
            .max()
            .unwrap_or(start);
        let new_end = changes.map_pos(old_end).max(start).min(buffer.byte_len());
        let grouped = changes.batches().count() > 1;
        let notice = BufferChange {
            buffer_id,
            revision: buffer.revision(),
            old_range: start..old_end,
            new_range: start.min(new_end)..new_end,
            invalidate_suffix: grouped,
        };
        // More than one pending edit has changing coordinate spaces: conservatively
        // invalidate the suffix from the earliest affected byte, still without scans.
        if let Some(previous) = self
            .buffer_changes
            .iter_mut()
            .find(|n| n.buffer_id == buffer_id)
        {
            let first = previous.old_range.start.min(start);
            previous.revision = notice.revision;
            previous.old_range.start = first;
            previous.old_range.end = previous.old_range.end.max(old_end);
            previous.new_range = first.min(buffer.byte_len())
                ..previous.new_range.end.max(new_end).min(buffer.byte_len());
            previous.invalidate_suffix = true;
        } else {
            self.buffer_changes.push(notice);
        }
    }

    /// Publishes already-applied editor changes. The initiating view (including its
    /// jump anchors) has already been rebased by the canonical Vim/Input executor.
    pub fn publish_editor_changes(
        &mut self,
        view_id: ViewId,
        changes: &[ChangeSet],
    ) -> Result<(), AppError> {
        let buffer_id = self
            .views
            .get(&view_id)
            .ok_or(AppError::ViewNotFound(view_id))?
            .buffer_id();
        for change in changes {
            for view in self.views.values_mut() {
                if view.buffer_id() == buffer_id && view.id() != view_id {
                    view.apply_change_set(change);
                }
            }
            self.notify_buffer_change(buffer_id, change);
        }
        self.refresh_buffer_state(buffer_id);
        Ok(())
    }

    /// Current vault epoch.
    pub fn epoch(&self) -> VaultEpoch {
        self.epoch
    }

    /// Advances to a new vault epoch, invalidating stale replies from the previous epoch.
    pub fn switch_vault(&mut self, epoch: VaultEpoch) {
        self.buffer_changes.clear();
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

    /// Borrows the two independently owned parts of an editor input context.
    pub fn editor_parts_mut(
        &mut self,
        view_id: ViewId,
    ) -> Option<(&mut crate::text::Buffer, &mut View)> {
        let view = self.views.get_mut(&view_id)?;
        let buffer = self.buffers.get_buffer_mut(view.buffer_id())?;
        Some((buffer, view))
    }

    /// Synchronizes lifecycle state after Vim or an input adapter edits a resident buffer.
    pub fn refresh_buffer_state(&mut self, buffer_id: BufferId) {
        if let Some(entry) = self.buffers.get_mut(buffer_id)
            && entry.state() != BufferState::Conflict
            && let Some(buffer) = entry.buffer()
        {
            let state = if buffer.is_dirty() {
                BufferState::Dirty
            } else {
                BufferState::Clean
            };
            entry.set_state(state);
        }
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
                let buf_id = self.buffers.register_unloaded(path.clone())?;
                let existing_view = self
                    .views
                    .values()
                    .find(|view| view.buffer_id() == buf_id)
                    .map(View::id);
                let view_id = if let Some(id) = existing_view {
                    self.active_view = Some(id);
                    id
                } else {
                    self.create_view(buf_id)?
                };
                effects.push(Effect::InvalidateView(view_id));
                if self.buffers.get_buffer(buf_id).is_none() {
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

                if let Some(entry) = self.buffers.get_mut(buf_id)
                    && entry.state() != BufferState::Conflict
                {
                    entry.set_state(if is_dirty {
                        BufferState::Dirty
                    } else {
                        BufferState::Clean
                    });
                }

                // Rebase views looking at this buffer
                for v in self.views.values_mut() {
                    if v.buffer_id() == buf_id {
                        v.apply_change_set(&changes);
                        if v.id() == view_id
                            && let Some(c) = explicit_cursor
                        {
                            v.set_cursor(c.anchor, c.head);
                        }
                        effects.push(Effect::InvalidateView(v.id()));
                    }
                }

                self.notify_buffer_change(buf_id, &changes);
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
                if entry.state() == BufferState::Conflict {
                    return Err(AppError::BufferConflict { buffer_id });
                }
                if entry.buffer().is_none() {
                    return Err(AppError::BufferNotFound(buffer_id));
                }
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
            Action::OverwriteBuffer(buffer_id) | Action::SaveBufferAs { buffer_id, .. } => {
                let entry = self
                    .buffers
                    .get(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                let buffer = entry.buffer().ok_or(AppError::BufferNotFound(buffer_id))?;
                let key = SnapshotKey::new(self.epoch, buffer_id, buffer.revision());
                let content = buffer.as_str();
                let job = match action {
                    Action::SaveBufferAs { path, .. } => {
                        if self.buffers.get_by_path(&path).is_some() {
                            return Err(AppError::BufferAlreadyOpen(path));
                        }
                        StoreJob::SaveAsFile { key, path, content }
                    }
                    _ => StoreJob::OverwriteFile {
                        key,
                        path: entry
                            .path()
                            .unwrap_or(Path::new("untitled.md"))
                            .to_path_buf(),
                        content,
                    },
                };
                effects.push(Effect::SubmitStoreJob(job));
            }
            Action::ReloadDiscard(buffer_id) => {
                let entry = self
                    .buffers
                    .get(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                let key = self
                    .buffers
                    .get_snapshot_key(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                let path = entry
                    .path()
                    .unwrap_or(Path::new("untitled.md"))
                    .to_path_buf();
                effects.push(Effect::SubmitStoreJob(StoreJob::ReloadFile { key, path }));
            }
            Action::ReloadBuffer(buffer_id) => {
                let entry = self
                    .buffers
                    .get(buffer_id)
                    .ok_or(AppError::BufferNotFound(buffer_id))?;
                if entry.buffer().is_some_and(|buffer| buffer.is_dirty()) {
                    return Err(AppError::BufferDirty { buffer_id });
                }
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
                    self.notify_buffer_change(buf_id, &changes);
                    self.refresh_buffer_state(buf_id);
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
                    self.notify_buffer_change(buf_id, &changes);
                    self.refresh_buffer_state(buf_id);
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
            | StoreEvent::Conflict { key, .. }
            | StoreEvent::FileReloaded { key, .. }
            | StoreEvent::SavedAs { key, .. }
            | StoreEvent::ExternalChanged { key, .. } => *key,
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

        let discard = matches!(&event, StoreEvent::FileReloaded { .. });
        match event {
            StoreEvent::SavedAs {
                key,
                path,
                fingerprint,
            } => {
                self.buffers.assign_path(key.buffer_id, path)?;
                return self.accept(StoreEvent::SaveAck { key, fingerprint });
            }
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
                    entry.set_fingerprint(Some(fingerprint));
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
                if let Some(entry) = self.buffers.get_mut(key.buffer_id)
                    && entry.state() != BufferState::Conflict
                    && entry.buffer().is_some_and(|buffer| buffer.is_dirty())
                {
                    entry.set_state(BufferState::Dirty);
                }
                effects.push(Effect::StatusMessage(format!("Save failed: {error}")));
            }
            StoreEvent::Conflict { key, .. } => {
                if let Some(entry) = self.buffers.get_mut(key.buffer_id) {
                    entry.set_state(BufferState::Conflict);
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
            }
            | StoreEvent::FileReloaded {
                key,
                content,
                fingerprint,
            } => {
                let entry = self
                    .buffers
                    .get_mut(key.buffer_id)
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                if let Some(current) = entry.buffer() {
                    if current.revision() != key.revision {
                        return Err(AppError::StaleSnapshotKey {
                            expected: SnapshotKey::new(
                                self.epoch,
                                key.buffer_id,
                                current.revision(),
                            ),
                            actual: key,
                        });
                    }
                    if current.is_dirty() && !discard {
                        return Err(AppError::BufferDirty {
                            buffer_id: key.buffer_id,
                        });
                    }
                }
                if let Some(buffer) = entry.buffer_mut() {
                    buffer.reload_from_text(&content)?;
                } else {
                    entry.set_buffer(Some(crate::text::Buffer::from_text(
                        key.buffer_id,
                        &content,
                    )?));
                }
                entry.set_state(BufferState::Clean);
                entry.set_fingerprint(Some(fingerprint));
                effects.push(Effect::BufferStateChanged {
                    buffer_id: key.buffer_id,
                    state: BufferState::Clean,
                });
                for v in self.views.values_mut() {
                    if v.buffer_id() == key.buffer_id {
                        let snap = |pos: crate::text::BytePos| {
                            use unicode_segmentation::UnicodeSegmentation;
                            let offset = pos.get().min(content.len());
                            crate::text::BytePos::new(if offset == content.len() {
                                offset
                            } else {
                                content
                                    .grapheme_indices(true)
                                    .take_while(|(index, _)| *index <= offset)
                                    .last()
                                    .map_or(0, |(index, _)| index)
                            })
                        };
                        v.set_cursor(snap(v.anchor()), snap(v.head()));
                        effects.push(Effect::InvalidateView(v.id()));
                    }
                }
            }
            StoreEvent::ExternalChanged {
                key,
                path,
                new_fingerprint,
            } => {
                let entry = self
                    .buffers
                    .get_mut(key.buffer_id)
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                // A queued watcher event cannot follow a rename or save-as to another path.
                if entry.path() != Some(path.as_path())
                    || entry.fingerprint() == Some(new_fingerprint.as_str())
                {
                    return Ok(effects);
                }
                if entry.state() == BufferState::Conflict
                    || entry.buffer().is_some_and(|buffer| buffer.is_dirty())
                    || new_fingerprint.is_empty()
                {
                    entry.set_state(BufferState::Conflict);
                    effects.push(Effect::BufferStateChanged {
                        buffer_id: key.buffer_id,
                        state: BufferState::Conflict,
                    });
                    effects.push(Effect::StatusMessage(
                        "External change: use :reload!, :saveas <path>, or :w!".into(),
                    ));
                } else {
                    let current = self
                        .buffers
                        .get_snapshot_key(key.buffer_id)
                        .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                    effects.push(Effect::SubmitStoreJob(StoreJob::LoadFile {
                        key: current,
                        path,
                    }));
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
