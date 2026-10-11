use super::{BufferState, SnapshotKey, StoreJob, View, Workspace};
use crate::commands::{Action, AppError, Effect};
use std::path::{Path, PathBuf};

impl Workspace {
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
}
