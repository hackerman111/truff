use super::super::Application;
use std::path::Path;
use std::time::Instant;
use truff_core::commands::Action;
use truff_core::text::BytePos;
use truff_core::workspace::StoreEvent;

impl Application {
    pub(super) fn accept_store(&mut self, event: StoreEvent) {
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
        let recovery_ack = matches!(event, StoreEvent::RecoveryAck { .. });
        let failed = matches!(
            event,
            StoreEvent::SaveFailed { .. } | StoreEvent::Conflict { .. }
        );
        if recovery_ack || matches!(event, StoreEvent::SaveFailed { .. }) {
            self.pending_recovery.remove(&key);
        }
        if failed {
            // Keep the resident copy if recovery cleanup could not be confirmed.
            self.closed_buffers.remove(&key.buffer_id);
        }
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
        if recovery_ack && let Some(ready) = self.closed_buffers.get_mut(&key.buffer_id) {
            *ready = Some(key);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_ack_does_not_release_a_pending_recovery_key() {
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
        app.closed_buffers.insert(id, None);

        app.accept_store(StoreEvent::SaveAck {
            key,
            fingerprint: "saved-fingerprint".into(),
        });
        assert!(app.pending_recovery.contains(&key));
        assert_eq!(app.closed_buffers.get(&id), Some(&None));
        assert!(app.recovery_deadline.is_some());

        app.accept_store(StoreEvent::RecoveryAck { key });
        assert!(!app.pending_recovery.contains(&key));
        assert_eq!(app.closed_buffers.get(&id), Some(&Some(key)));
    }
}
