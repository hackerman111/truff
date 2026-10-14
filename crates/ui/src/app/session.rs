use std::path::{Path, PathBuf};

use truff_core::text::{BufferId, BytePos, Transaction};
use truff_core::workspace::{SnapshotKey, StoreJob};
use truff_store::session::{
    BufferRestoreState, SessionBufferEntry, SessionData, SessionViewEntry, restore_session,
    save_session,
};
use truff_store::{DEFAULT_MAX_RECOVERY_STORAGE, RecoveryManager};

use super::{AppResult, Application};

impl Application {
    pub(super) fn restore_session(&mut self) -> AppResult<()> {
        let paths = self.paths.as_ref().ok_or("No vault")?.clone();
        let recovery =
            RecoveryManager::new(paths.root().to_path_buf(), DEFAULT_MAX_RECOVERY_STORAGE);
        let session = restore_session(&paths, &recovery)?;
        if let Some(last_id) = session
            .recovered_dirty_buffers
            .iter()
            .map(|snapshot| snapshot.key.buffer_id)
            .max()
        {
            // New snapshots must not overwrite old IDs awaiting migration/deletion.
            last_id
                .get()
                .checked_add(
                    session.buffers.len() as u64 + session.recovered_dirty_buffers.len() as u64 + 2,
                )
                .ok_or("Recovery buffer identifiers exhaust the identifier space")?;
            BufferId::reserve_through(last_id).ok_or("Recovery buffer identifier overflow")?;
        }
        for saved_view in session.views {
            if let Ok(path) = paths.resolve_path(&saved_view.buffer_path) {
                self.restore_views.insert(path, saved_view);
            }
        }
        for entry in session.buffers {
            let path = paths.resolve_path(&entry.path)?;
            match entry.state {
                BufferRestoreState::Loaded(text) => {
                    let id = self
                        .shell
                        .workspace_mut()
                        .open_buffer_from_text(path, &text)?;
                    if let Some(buffer) = self.shell.workspace_mut().buffers_mut().get_mut(id) {
                        buffer.set_fingerprint(entry.last_saved_fingerprint);
                    }
                }
                BufferRestoreState::Lazy => {
                    self.shell
                        .workspace_mut()
                        .buffers_mut()
                        .register_unloaded(path)?;
                }
                _ => {}
            }
        }
        for snapshot in session.recovered_dirty_buffers {
            let path = paths.resolve_path(Path::new(&snapshot.rel_path))?;
            let id = if let Some(id) = self.shell.workspace().buffers().get_by_path(&path) {
                id
            } else {
                self.shell.workspace_mut().open_buffer_from_text(path, "")?
            };
            let entry = self
                .shell
                .workspace_mut()
                .buffers_mut()
                .get_mut(id)
                .ok_or("Missing recovery destination")?;
            if entry.buffer().is_none() {
                entry.set_buffer(Some(truff_core::Buffer::from_text(id, "")?));
            }
            let buffer = entry.buffer_mut().ok_or("Missing recovery buffer")?;
            buffer.apply(Transaction::replace(
                0..buffer.byte_len(),
                &snapshot.content,
            ))?;
            let key = SnapshotKey::new(
                self.shell.workspace().epoch(),
                id,
                self.shell
                    .workspace()
                    .buffers()
                    .get_buffer(id)
                    .unwrap()
                    .revision(),
            );
            recovery.write_snapshot(
                key,
                &snapshot.rel_path,
                &snapshot.content,
                snapshot.base_fingerprint.as_deref(),
            )?;
            if snapshot.key.buffer_id != id {
                recovery.delete_snapshots(snapshot.key.buffer_id)?;
            }
            let entry = self
                .shell
                .workspace_mut()
                .buffers_mut()
                .get_mut(id)
                .unwrap();
            entry.set_fingerprint(snapshot.base_fingerprint);
            self.shell.workspace_mut().refresh_buffer_state(id);
            self.status = "Recovered unsaved changes".into();
        }
        if let Some(path) = session.active_buffer_path {
            let absolute = paths.resolve_path(&path)?;
            if self
                .shell
                .workspace()
                .buffers()
                .get_by_path(&absolute)
                .is_some()
            {
                self.initial_note_pending = false;
                self.open_note(&path.to_string_lossy())?;
                if let Some(saved_view) = self.restore_views.remove(&absolute)
                    && let Some((buffer, view)) = self
                        .shell
                        .workspace_mut()
                        .active_view()
                        .and_then(|id| self.shell.workspace_mut().editor_parts_mut(id))
                {
                    let text = buffer.as_str();
                    view.set_cursor(
                        BytePos::new(crate::editor::snap_to_grapheme_boundary(
                            &text,
                            saved_view.cursor_anchor,
                        )),
                        BytePos::new(crate::editor::snap_to_grapheme_boundary(
                            &text,
                            saved_view.cursor_head,
                        )),
                    );
                    view.set_scroll_top_line(saved_view.top_line);
                }
            }
        }
        Ok(())
    }

    pub(super) fn queue_recovery(&mut self) {
        if self.paths.is_none() {
            return;
        }
        self.queued_recovery.clear();
        for entry in self.shell.workspace().buffers().entries() {
            if entry.buffer().is_some() && entry.path().is_some() {
                self.queued_recovery.push_back(entry.id());
            }
        }
    }

    pub(super) fn submit_next_recovery(&mut self) -> AppResult<()> {
        let Some(&id) = self.queued_recovery.front() else {
            return Ok(());
        };
        let Some(entry) = self.shell.workspace().buffers().get(id) else {
            self.queued_recovery.pop_front();
            return Ok(());
        };
        let (Some(buffer), Some(path)) = (entry.buffer(), entry.path()) else {
            self.queued_recovery.pop_front();
            return Ok(());
        };
        let key = SnapshotKey::new(self.shell.workspace().epoch(), id, buffer.revision());
        if self.pending_recovery.contains(&key) {
            // A queued cleanup may share the revision of an earlier checkpoint.
            // Keep it queued until that acknowledgement has released the key.
            return Ok(());
        }
        let job = if buffer.is_dirty() {
            StoreJob::WriteRecoverySnapshot {
                key,
                path: self
                    .paths
                    .as_ref()
                    .ok_or("No vault")?
                    .relative_path(path)?
                    .to_path_buf(),
                content: buffer.rope().clone(),
                base_fingerprint: entry.fingerprint().map(str::to_owned),
            }
        } else {
            StoreJob::DeleteRecoverySnapshot { key }
        };
        match self.store.as_mut().ok_or("No store")?.try_submit(job) {
            Ok(_) => {
                self.queued_recovery.pop_front();
                self.pending_recovery.insert(key);
            }
            Err(truff_store::SubmitError::QueueFull) => {}
            Err(error) => {
                self.queued_recovery.clear();
                return Err(error.into());
            }
        }
        Ok(())
    }

    /// Finish accepted writes, persist dirty recovery and session, then release the vault lock.
    pub fn shutdown(&mut self) -> AppResult<()> {
        let Some(paths) = self.paths.clone() else {
            return Ok(());
        };
        // File operations own their snapshots until their acknowledgement has been processed.
        self.recovery_deadline = None;
        while self.file_operation_pending
            || !self.queued_reloads.is_empty()
            || !self.queued_recovery.is_empty()
            || !self.pending_saves.is_empty()
            || !self.pending_loads.is_empty()
            || !self.pending_recovery.is_empty()
        {
            self.poll();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let recovery =
            RecoveryManager::new(paths.root().to_path_buf(), DEFAULT_MAX_RECOVERY_STORAGE);
        let mut buffers = Vec::new();
        for entry in self.shell.workspace().buffers().entries() {
            let Some(path) = entry.path() else {
                continue;
            };
            let relative = paths.relative_path(path)?.to_path_buf();
            let dirty = entry.buffer().is_some_and(|buffer| buffer.is_dirty());
            if let Some(buffer) = entry.buffer().filter(|_| dirty) {
                recovery.write_snapshot(
                    SnapshotKey::new(
                        self.shell.workspace().epoch(),
                        entry.id(),
                        buffer.revision(),
                    ),
                    &relative.to_string_lossy(),
                    &buffer.as_str(),
                    entry.fingerprint(),
                )?;
            } else if entry.buffer().is_some() {
                // Undo can return to the savepoint after a dirty checkpoint was written.
                // Lazy buffers may still have recovery data, so only clear resident clean buffers.
                recovery.delete_snapshots(entry.id())?;
            }
            buffers.push(SessionBufferEntry {
                path: relative,
                is_dirty: dirty,
                last_saved_fingerprint: entry.fingerprint().map(str::to_owned),
            });
        }
        buffers.sort_by(|a, b| a.path.cmp(&b.path));
        let active_view = self.shell.workspace().active_view();
        let mut views: Vec<_> = self
            .shell
            .workspace()
            .views()
            .values()
            .filter_map(|view| {
                let entry = self.shell.workspace().buffers().get(view.buffer_id())?;
                let path = paths.relative_path(entry.path()?).ok()?.to_path_buf();
                Some((
                    view.id(),
                    SessionViewEntry {
                        buffer_path: path,
                        cursor_anchor: view.anchor().get(),
                        cursor_head: view.head().get(),
                        top_line: view.scroll_top_line(),
                    },
                ))
            })
            .collect();
        views.sort_by_key(|(id, _)| *id);
        let active_view_index = views.iter().position(|(id, _)| Some(*id) == active_view);
        let active_buffer: Option<PathBuf> = self
            .active_path()
            .and_then(|path| paths.relative_path(path).ok().map(Path::to_path_buf));
        save_session(
            &paths,
            &SessionData {
                version: 1,
                vault_path: paths.root().to_path_buf(),
                active_buffer,
                buffers,
                views: views.into_iter().map(|(_, view)| view).collect(),
                active_view_index,
                jump_history: Vec::new(),
            },
        )?;
        self.catalog.take();
        self.store.take();
        self.paths = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_cleanup_waits_for_an_inflight_checkpoint_at_the_same_revision() {
        let mut app = Application::new();
        let id = app
            .shell
            .workspace_mut()
            .open_buffer_from_text("note.md".into(), "saved text")
            .unwrap();
        let key = app
            .shell
            .workspace()
            .buffers()
            .get_snapshot_key(id)
            .unwrap();
        app.pending_recovery.insert(key);
        app.queued_recovery.push_back(id);

        app.submit_next_recovery().unwrap();
        assert_eq!(app.queued_recovery.front(), Some(&id));
        assert_eq!(app.pending_recovery.len(), 1);
        assert!(app.pending_recovery.contains(&key));
    }
}
