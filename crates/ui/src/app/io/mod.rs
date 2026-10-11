//! Dispatch tagged storage jobs and apply their acknowledgements to the canonical workspace.
mod catalog_events;
mod store_events;

use super::{AppResult, Application};
use std::time::Instant;
use truff_core::commands::{Action, Effect};
use truff_core::text::BufferId;
use truff_core::workspace::StoreJob;
use truff_store::Store;
use truff_store::catalog::CatalogWorker;

impl Application {
    pub(super) fn apply_effects(&mut self, effects: Vec<Effect>) -> AppResult<()> {
        for effect in effects {
            match effect {
                Effect::SubmitStoreJob(job) => {
                    let (save, load, id) = match &job {
                        StoreJob::SaveFile { key, .. }
                        | StoreJob::OverwriteFile { key, .. }
                        | StoreJob::SaveAsFile { key, .. } => (true, false, key.buffer_id),
                        StoreJob::LoadFile { key, .. } | StoreJob::ReloadFile { key, .. } => {
                            (false, true, key.buffer_id)
                        }
                        _ => (false, false, BufferId::new(0)),
                    };
                    if save && self.pending_saves.contains(&id) {
                        return Err(
                            "A save is already pending; wait for its acknowledgement".into()
                        );
                    }
                    if load && self.pending_loads.contains(&id) {
                        continue;
                    }
                    self.store
                        .as_mut()
                        .ok_or("Open a vault first")?
                        .try_submit(job)?;
                    if save {
                        self.pending_saves.insert(id);
                        self.status = "Saving…".into();
                    }
                    if load {
                        self.pending_loads.insert(id);
                        self.status = "Loading…".into();
                    }
                }
                Effect::StatusMessage(message) => self.status = message,
                Effect::ViewClosed(_) if self.shell.workspace().active_view().is_none() => {
                    self.quit_requested = true
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Polls tagged storage replies without doing disk I/O on the event thread.
    pub fn poll(&mut self) -> bool {
        self.refresh_watch_targets();
        let mut changed = false;
        let events = self
            .store
            .as_mut()
            .map(Store::poll_events)
            .unwrap_or_default();
        for event in events {
            changed = true;
            self.accept_store(event);
        }
        let events = self
            .catalog
            .as_ref()
            .map(CatalogWorker::poll_events)
            .unwrap_or_default();
        for event in events {
            changed = true;
            if let Err(error) = self.accept_catalog(event) {
                self.file_operation_pending = false;
                self.status = error.to_string();
            }
        }
        if self
            .catalog
            .as_ref()
            .is_some_and(|worker| !worker.is_running())
            && (self.catalog_indexing || self.pending_catalog > 0)
        {
            self.catalog_indexing = false;
            self.pending_catalog = 0;
            self.file_operation_pending = false;
            self.status = "Catalog worker stopped; reopen the vault".into();
            changed = true;
        }
        if self.store.as_ref().is_some_and(|store| !store.is_running())
            && (!self.pending_saves.is_empty()
                || !self.pending_loads.is_empty()
                || !self.pending_recovery.is_empty()
                || !self.queued_reloads.is_empty()
                || !self.queued_recovery.is_empty())
        {
            self.pending_saves.clear();
            self.pending_loads.clear();
            self.pending_recovery.clear();
            self.queued_reloads.clear();
            self.queued_recovery.clear();
            self.close_after_save.clear();
            self.status = "Storage worker stopped; unsaved text remains in memory".into();
            changed = true;
        }
        if let Some(&id) = self.queued_reloads.front() {
            match self.dispatch(Action::ReloadBuffer(id)) {
                Ok(()) => {
                    self.queued_reloads.pop_front();
                }
                Err(error)
                    if matches!(
                        error.downcast_ref::<truff_store::SubmitError>(),
                        Some(truff_store::SubmitError::QueueFull)
                    ) => {}
                Err(error) => {
                    self.queued_reloads.pop_front();
                    self.status = error.to_string();
                }
            }
            changed = true;
        }
        if self
            .recovery_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.recovery_deadline = None;
            self.queue_recovery();
        }
        if let Err(error) = self.submit_next_recovery() {
            self.status = error.to_string();
            changed = true;
        }
        changed
    }

    pub(super) fn refresh_watch_targets(&mut self) {
        let registry = self.shell.workspace().buffers();
        let generation = registry.watch_generation();
        if self.watched_generation == Some(generation) {
            return;
        }
        let Some(store) = &mut self.store else {
            return;
        };
        let targets = registry
            .entries()
            .filter_map(|entry| {
                Some((
                    registry.get_snapshot_key(entry.id())?,
                    entry.path()?.to_path_buf(),
                    entry.fingerprint().map(str::to_owned),
                ))
            })
            .collect();
        store.set_watch_targets(targets);
        self.watched_generation = Some(generation);
    }
}
