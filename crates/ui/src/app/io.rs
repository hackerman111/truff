//! Dispatch tagged storage jobs and apply their acknowledgements to the canonical workspace.
use super::{AppResult, Application};
use crate::shell::Focus;
use std::path::Path;
use std::time::Instant;
use truff_core::commands::{Action, Effect};
use truff_core::text::{BufferId, BytePos};
use truff_core::workspace::{StoreEvent, StoreJob};
use truff_store::Store;
use truff_store::catalog::{CatalogEvent, CatalogWorker};

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

    fn accept_store(&mut self, event: StoreEvent) {
        if matches!(&event, StoreEvent::ExternalChanged { .. }) {
            if let Ok(effects) = self.shell.workspace_mut().accept(event)
                && let Err(error) = self.apply_effects(effects)
            {
                self.status = error.to_string();
            }
            return;
        }
        let key = match &event {
            StoreEvent::FileLoaded { key, .. }
            | StoreEvent::SaveAck { key, .. }
            | StoreEvent::SaveFailed { key, .. }
            | StoreEvent::Conflict { key, .. }
            | StoreEvent::RecoveryAck { key }
            | StoreEvent::FileReloaded { key, .. }
            | StoreEvent::SavedAs { key, .. } => *key,
            _ => return,
        };
        let loaded = matches!(
            event,
            StoreEvent::FileLoaded { .. } | StoreEvent::FileReloaded { .. }
        );
        let reloaded = matches!(event, StoreEvent::FileReloaded { .. });
        let saved = matches!(
            event,
            StoreEvent::SaveAck { .. } | StoreEvent::SavedAs { .. }
        );
        let failed = matches!(
            event,
            StoreEvent::SaveFailed { .. } | StoreEvent::Conflict { .. }
        );
        self.pending_recovery.remove(&key);
        if loaded || failed {
            self.pending_loads.remove(&key.buffer_id);
        }
        if saved || failed {
            self.pending_saves.remove(&key.buffer_id);
        }
        match self.shell.workspace_mut().accept(event) {
            Ok(effects) => {
                if let Err(error) = self.apply_effects(effects) {
                    self.status = error.to_string();
                }
            }
            Err(error) => {
                self.status = error.to_string();
                return;
            }
        }
        if loaded {
            if reloaded {
                self.recovery_deadline = Some(Instant::now());
            }
            self.watched_generation = None;
            self.refresh_watch_targets();
            if let Some(path) = self
                .shell
                .workspace()
                .buffers()
                .get(key.buffer_id)
                .and_then(|entry| entry.path())
                .map(Path::to_path_buf)
                && let Some(saved_view) = self.restore_views.remove(&path)
                && let Some((buffer, view)) = self
                    .shell
                    .workspace_mut()
                    .active_view()
                    .and_then(|id| self.shell.workspace_mut().editor_parts_mut(id))
                && buffer.id() == key.buffer_id
            {
                let text = buffer.as_str();
                let anchor =
                    crate::editor::snap_to_grapheme_boundary(&text, saved_view.cursor_anchor);
                let head = crate::editor::snap_to_grapheme_boundary(&text, saved_view.cursor_head);
                view.set_cursor(BytePos::new(anchor), BytePos::new(head));
                view.set_scroll_top_line(saved_view.top_line);
            }
            if let Err(error) = self.apply_pending_location() {
                self.status = error.to_string();
            } else {
                self.status = "Note loaded".into();
            }
        }
        if saved {
            if self
                .shell
                .workspace()
                .buffers()
                .get_buffer(key.buffer_id)
                .is_some_and(|buffer| !buffer.is_dirty())
            {
                // A checkpoint accepted before SaveAck may execute after the save's own cleanup.
                self.recovery_deadline = Some(Instant::now());
            }
            if let Some(path) = self
                .shell
                .workspace()
                .buffers()
                .get(key.buffer_id)
                .and_then(|entry| entry.path())
                .and_then(|path| self.paths.as_ref()?.relative_path(path).ok())
                .map(|path| path.to_string_lossy().into_owned())
                && let Err(error) =
                    self.submit_catalog(truff_store::catalog::CatalogJob::RefreshNote { path })
            {
                self.status = error.to_string();
            }
            if let Some(view) = self.close_after_save.remove(&key.buffer_id) {
                let still_dirty = self
                    .shell
                    .workspace()
                    .buffers()
                    .get_buffer(key.buffer_id)
                    .is_some_and(|buffer| buffer.is_dirty());
                if !still_dirty {
                    let _ = self.dispatch(Action::CloseView(view));
                }
            }
        } else if failed {
            self.close_after_save.remove(&key.buffer_id);
        }
    }

    fn accept_catalog(&mut self, event: CatalogEvent) -> AppResult<()> {
        match event {
            CatalogEvent::Files(files) => {
                let first = files.first().cloned();
                self.shell.set_sidebar_files(files);
                if let Some(path) = self.active_relative_path() {
                    self.shell.select_sidebar_file(&path);
                }
                if self.initial_note_pending {
                    self.initial_note_pending = false;
                    if let Some(path) = first {
                        self.open_note(&path)?;
                    } else {
                        self.status = "Vault is empty. Create a note with :new <name.md>".into();
                    }
                }
            }
            CatalogEvent::Ready => {
                self.catalog_indexing = false;
                if self.status == "Reading vault…" {
                    self.status = "Vault ready".into();
                }
            }
            CatalogEvent::JobDone => {
                self.pending_catalog = self.pending_catalog.saturating_sub(1);
            }
            CatalogEvent::Backlinks { path, occurrences }
                if Some(path.as_str()) == self.active_relative_path().as_deref() =>
            {
                self.backlinks = occurrences;
                self.backlink_selection = 0;
                self.shell.set_focus(Focus::RightPanel);
            }
            CatalogEvent::Backlinks { .. } => {}
            CatalogEvent::Follow { path, anchor } => {
                self.open_location(&path, anchor.map(super::navigation::Location::Anchor))?
            }
            CatalogEvent::Renamed {
                from,
                to,
                rewritten,
            } => {
                self.file_operation_pending = false;
                let paths = self.paths.as_ref().ok_or("No vault")?;
                let from_abs = paths.resolve_path(Path::new(&from))?;
                let to_abs = paths.resolve_path(Path::new(&to))?;
                self.shell
                    .workspace_mut()
                    .buffers_mut()
                    .rename_path(&from_abs, to_abs)?;
                for relative in rewritten {
                    let absolute = paths.resolve_path(&relative)?;
                    if let Some(id) = self.shell.workspace().buffers().get_by_path(&absolute) {
                        self.queued_reloads.push_back(id);
                    }
                }
                self.status = format!("Renamed {from} → {to}");
            }
            CatalogEvent::Trashed(path) => {
                self.file_operation_pending = false;
                if let Some(paths) = &self.paths {
                    let absolute = paths.resolve_path(Path::new(&path))?;
                    if let Some(id) = self.shell.workspace().buffers().get_by_path(&absolute) {
                        let views: Vec<_> = self
                            .shell
                            .workspace()
                            .views()
                            .values()
                            .filter(|view| view.buffer_id() == id)
                            .map(|view| view.id())
                            .collect();
                        for view in views {
                            self.shell.workspace_mut().close_view(view)?;
                        }
                        self.shell.workspace_mut().close_buffer_force(id)?;
                    }
                }
                self.status = format!("Moved {path} to .truff/trash");
            }
            CatalogEvent::Created(path) => {
                self.file_operation_pending = false;
                self.open_note(&path)?;
            }
            CatalogEvent::Error(error) => {
                self.file_operation_pending = false;
                self.status = error;
            }
        }
        Ok(())
    }
}
