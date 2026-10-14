use super::Application;
use truff_core::workspace::{BufferState, StoreJob};

impl Application {
    pub(super) fn track_closed_buffers(&mut self) {
        let workspace = self.shell.workspace();
        for entry in workspace.buffers().entries() {
            if entry.path().is_some()
                && !workspace
                    .views()
                    .values()
                    .any(|view| view.buffer_id() == entry.id())
            {
                self.closed_buffers.entry(entry.id()).or_insert(None);
            }
        }
    }

    pub(super) fn release_closed_buffers(&mut self) -> bool {
        if self.closed_buffers.is_empty()
            || self.paths.is_none()
            || self.file_operation_pending
            || self.recovery_deadline.is_some()
            || self.store.as_ref().is_none_or(|store| !store.is_running())
        {
            return false;
        }
        let candidates: Vec<_> = self.closed_buffers.keys().copied().collect();
        let mut changed = false;
        for id in candidates {
            let workspace = self.shell.workspace();
            if workspace
                .views()
                .values()
                .any(|view| view.buffer_id() == id)
            {
                self.closed_buffers.remove(&id);
                continue;
            }
            if self.pending_saves.contains(&id)
                || self.pending_loads.contains(&id)
                || self.close_after_save.contains_key(&id)
                || self.queued_reloads.contains(&id)
                || self.queued_recovery.contains(&id)
                || self.pending_recovery.iter().any(|key| key.buffer_id == id)
            {
                continue;
            }
            let Some(entry) = workspace.buffers().get(id) else {
                self.closed_buffers.remove(&id);
                continue;
            };
            if entry.state() != BufferState::Clean
                || entry.fingerprint().is_none()
                || entry.buffer().is_none_or(|buffer| buffer.is_dirty())
            {
                self.closed_buffers.remove(&id);
                continue;
            }
            let Some(key) = workspace.buffers().get_snapshot_key(id) else {
                continue;
            };
            if self.closed_buffers.get(&id) == Some(&Some(key)) {
                changed |= self
                    .shell
                    .workspace_mut()
                    .unload_buffer(id)
                    .unwrap_or(false);
                self.closed_buffers.remove(&id);
                continue;
            }
            // A current-revision deletion acknowledgement is required even for
            // clean text: an older dirty checkpoint may still exist on disk.
            self.closed_buffers.insert(id, None);
            match self
                .store
                .as_mut()
                .expect("running store checked above")
                .try_submit(StoreJob::DeleteRecoverySnapshot { key })
            {
                Ok(_) => {
                    self.pending_recovery.insert(key);
                }
                Err(truff_store::SubmitError::QueueFull) => break,
                Err(error) => {
                    self.closed_buffers.remove(&id);
                    self.status = error.to_string();
                }
            }
        }
        if changed {
            self.refresh_watch_targets();
        }
        changed
    }
}
