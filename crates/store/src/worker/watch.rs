use super::{EventWaker, WorkerMessage, wake};
use crate::{VaultPaths, files};
use notify::Watcher;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use truff_core::workspace::StoreEvent;

#[derive(Default)]
struct PendingChanges {
    paths: std::collections::HashSet<PathBuf>,
    all: bool,
}

pub type WatchTargets = Vec<(truff_core::workspace::SnapshotKey, PathBuf, Option<String>)>;

pub(super) struct WatchedFile {
    pub(super) key: truff_core::workspace::SnapshotKey,
    pub(super) path: PathBuf,
    pub(super) fingerprint: String,
}

pub(super) struct WatchState {
    watched: std::collections::HashMap<truff_core::text::BufferId, WatchedFile>,
    dirty: Arc<Mutex<PendingChanges>>,
}

impl WatchState {
    pub(super) fn start(
        vault_dir: &Path,
        sender: &SyncSender<WorkerMessage>,
    ) -> std::io::Result<(notify::RecommendedWatcher, Self)> {
        let watch_dirty = Arc::new(std::sync::Mutex::new(PendingChanges::default()));
        let signal = watch_dirty.clone();
        let watcher_sender = sender.clone();
        let mut watcher =
            notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
                let Ok(mut pending) = signal.lock() else {
                    return;
                };
                let mut changed = false;
                match result {
                    Ok(event) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                        for path in event.paths {
                            if path.components().any(|c| c.as_os_str() == ".truff") {
                                continue;
                            }
                            changed = true;
                            if pending.paths.len()
                                < truff_core::workspace::buffers::DEFAULT_MAX_BUFFERS
                            {
                                pending.paths.insert(path);
                            } else {
                                pending.all = true;
                            }
                        }
                    }
                    Err(_) => {
                        pending.all = true;
                        changed = true;
                    }
                    _ => {}
                }
                if changed {
                    let _ = watcher_sender.try_send(WorkerMessage::Reconcile);
                }
            })
            .map_err(std::io::Error::other)?;
        watcher
            .watch(vault_dir, notify::RecursiveMode::Recursive)
            .map_err(std::io::Error::other)?;

        Ok((
            watcher,
            Self {
                watched: std::collections::HashMap::new(),
                dirty: watch_dirty,
            },
        ))
    }
    pub(super) fn track(&mut self, file: WatchedFile) {
        if self.watched.len() < truff_core::workspace::buffers::DEFAULT_MAX_BUFFERS
            || self.watched.contains_key(&file.key.buffer_id)
        {
            self.watched.insert(file.key.buffer_id, file);
        }
    }
    pub(super) fn refresh_targets(
        &mut self,
        paths: &VaultPaths,
        targets: &Mutex<Option<WatchTargets>>,
    ) {
        let targets = targets.lock().ok().and_then(|mut slot| slot.take());
        if let Some(targets) = targets {
            let mut next = std::collections::HashMap::with_capacity(targets.len());
            for (key, path, base) in targets {
                let Ok(path) = paths.resolve_path(&path) else {
                    continue;
                };
                let previous = self
                    .watched
                    .remove(&key.buffer_id)
                    .filter(|file| file.path == path && file.key.epoch == key.epoch);
                let fingerprint = base
                    .or_else(|| previous.map(|file| file.fingerprint))
                    .unwrap_or_else(|| files::compute_file_fingerprint(&path).unwrap_or_default());
                next.insert(
                    key.buffer_id,
                    WatchedFile {
                        key,
                        path,
                        fingerprint,
                    },
                );
            }
            self.watched = next;
            if let Ok(mut pending) = self.dirty.lock() {
                pending.all = true;
            }
        }
    }
    pub(super) fn reconcile(
        &mut self,
        paths: &VaultPaths,
        watch_tx: &SyncSender<StoreEvent>,
        waker: &Mutex<Option<EventWaker>>,
        wake_pending: &AtomicBool,
    ) {
        let pending = self
            .dirty
            .lock()
            .ok()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default();
        if pending.all || !pending.paths.is_empty() {
            for file in self.watched.values_mut() {
                if !pending.all
                    && !pending
                        .paths
                        .iter()
                        .any(|path| file.path.starts_with(path) || path == &file.path)
                {
                    continue;
                }
                let fingerprint = paths
                    .resolve_path(&file.path)
                    .ok()
                    .and_then(|path| files::compute_file_fingerprint(&path).ok())
                    .unwrap_or_default();
                if fingerprint == file.fingerprint {
                    continue;
                }
                let event = StoreEvent::ExternalChanged {
                    key: file.key,
                    path: file.path.clone(),
                    new_fingerprint: fingerprint.clone(),
                };
                match watch_tx.try_send(event) {
                    Ok(()) => {
                        file.fingerprint = fingerprint;
                        wake(waker, wake_pending);
                    }
                    Err(TrySendError::Full(_)) => {
                        if let Ok(mut pending) = self.dirty.lock() {
                            pending.all = true;
                        }
                        break;
                    }
                    Err(TrySendError::Disconnected(_)) => break,
                }
            }
        }
    }
}
