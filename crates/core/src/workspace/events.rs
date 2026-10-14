use super::{BufferState, SnapshotKey, StoreEvent, StoreJob, Workspace};
use crate::commands::{AppError, Effect};

impl Workspace {
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
        if self
            .buffers
            .get(key.buffer_id)
            .is_some_and(|entry| key.revision < entry.revision_floor())
        {
            return Err(AppError::StaleSnapshotKey {
                expected: self
                    .buffers
                    .get_snapshot_key(key.buffer_id)
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?,
                actual: key,
            });
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
                if entry.revision() != key.revision {
                    return Err(AppError::StaleSnapshotKey {
                        expected: SnapshotKey::new(self.epoch, key.buffer_id, entry.revision()),
                        actual: key,
                    });
                }
                if let Some(current) = entry.buffer()
                    && current.is_dirty()
                    && !discard
                {
                    return Err(AppError::BufferDirty {
                        buffer_id: key.buffer_id,
                    });
                }
                let old_len = entry.buffer().map_or(0, crate::text::Buffer::byte_len);
                if let Some(buffer) = entry.buffer_mut() {
                    buffer.reload_from_text(&content)?;
                } else {
                    entry.set_buffer(Some(crate::text::Buffer::from_text_at_revision(
                        key.buffer_id,
                        &content,
                        key.revision,
                    )?));
                }
                entry.set_state(BufferState::Clean);
                entry.set_fingerprint(Some(fingerprint));
                effects.push(Effect::BufferStateChanged {
                    buffer_id: key.buffer_id,
                    state: BufferState::Clean,
                });
                let buffer = entry
                    .buffer()
                    .ok_or(AppError::BufferNotFound(key.buffer_id))?;
                for v in self.views.values_mut() {
                    if v.buffer_id() == key.buffer_id {
                        let snap = |pos: crate::text::BytePos| {
                            crate::text::BytePos::new(crate::vim::motion::snap_grapheme(
                                buffer,
                                pos.get(),
                            ))
                        };
                        v.set_cursor(snap(v.anchor()), snap(v.head()));
                        effects.push(Effect::InvalidateView(v.id()));
                    }
                }
                self.notify_buffer_reload(key.buffer_id, old_len);
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
                if entry.buffer().is_none()
                    || entry.path() != Some(path.as_path())
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
