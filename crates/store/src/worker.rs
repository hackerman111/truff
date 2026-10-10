use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use truff_core::workspace::{StoreEvent, StoreJob};

use crate::files;
use crate::recovery::RecoveryManager;

/// Default maximum durable background slots (§10.1).
pub const DEFAULT_MAX_DURABLE_SLOTS: usize = 32;

/// Strongly-typed identifier for an asynchronous store job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JobId(pub u64);

impl JobId {
    /// Creates a `JobId` with explicit numeric ID.
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    /// Allocates next monotonic `JobId`.
    pub fn next() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Raw numeric value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors occurring when submitting jobs to the store worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitError {
    /// Durable job queue has reached its capacity limit (backpressure).
    QueueFull,
    /// Store is currently shutting down and refuses new submissions.
    ShuttingDown,
    /// The background worker thread has unexpectedly terminated.
    WorkerDied,
}

impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueueFull => write!(f, "store durable job queue is full"),
            Self::ShuttingDown => write!(f, "store is shutting down"),
            Self::WorkerDied => write!(f, "store worker thread died"),
        }
    }
}

impl std::error::Error for SubmitError {}

/// Wake callback invoked after an actual new disk result; adapters can request a repaint.
pub type EventWaker = Arc<dyn Fn() + Send + Sync>;

enum WorkerMessage {
    Job(JobId, StoreJob),
    Reconcile,
    Shutdown,
}

/// Background worker with admission accounting for queued work AND unread outcomes.
pub struct StoreWorker {
    job_sender: Option<SyncSender<WorkerMessage>>,
    event_receiver: Receiver<StoreEvent>,
    watch_receiver: Receiver<StoreEvent>,
    worker_handle: Option<JoinHandle<()>>,
    outstanding: usize,
    capacity: usize,
    waker: Arc<std::sync::Mutex<Option<EventWaker>>>,
    watch_targets: Arc<std::sync::Mutex<Option<WatchTargets>>>,
    wake_pending: Arc<AtomicBool>,
}

#[derive(Default)]
struct PendingChanges {
    paths: std::collections::HashSet<PathBuf>,
    all: bool,
}

pub type WatchTargets = Vec<(truff_core::workspace::SnapshotKey, PathBuf, Option<String>)>;

struct WatchedFile {
    key: truff_core::workspace::SnapshotKey,
    path: PathBuf,
    fingerprint: String,
}

fn wake(waker: &std::sync::Mutex<Option<EventWaker>>, pending: &AtomicBool) {
    if pending.swap(true, Ordering::AcqRel) {
        return;
    }
    let callback = waker.lock().ok().and_then(|guard| guard.clone());
    if let Some(callback) = callback {
        callback();
    }
}

impl StoreWorker {
    pub fn start(vault_dir: PathBuf, capacity: usize) -> std::io::Result<Self> {
        use notify::Watcher;
        let capacity = capacity.max(1);
        let (job_tx, job_rx) = mpsc::sync_channel::<WorkerMessage>(capacity);
        let (event_tx, event_rx) = mpsc::sync_channel::<StoreEvent>(capacity);
        let (watch_tx, watch_rx) =
            mpsc::sync_channel::<StoreEvent>(truff_core::workspace::buffers::DEFAULT_MAX_BUFFERS);
        let outstanding = 0;
        let waker = Arc::new(std::sync::Mutex::new(None));
        let worker_waker = waker.clone();
        let wake_pending = Arc::new(AtomicBool::new(false));
        let worker_pending = wake_pending.clone();
        let watch_targets = Arc::new(std::sync::Mutex::new(None::<WatchTargets>));
        let worker_targets = watch_targets.clone();
        let watch_dirty = Arc::new(std::sync::Mutex::new(PendingChanges::default()));
        let signal = watch_dirty.clone();
        let watcher_sender = job_tx.clone();
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
            .watch(&vault_dir, notify::RecursiveMode::Recursive)
            .map_err(std::io::Error::other)?;
        let worker_handle = thread::Builder::new()
            .name("truff-store-worker".into())
            .spawn(move || {
                let paths = crate::VaultPaths::new(vault_dir.clone());
                let recovery =
                    RecoveryManager::new(vault_dir, crate::recovery::DEFAULT_MAX_RECOVERY_STORAGE);
                let mut watched: std::collections::HashMap<
                    truff_core::text::BufferId,
                    WatchedFile,
                > = std::collections::HashMap::new();
                loop {
                    match job_rx.recv() {
                        Ok(WorkerMessage::Job(_id, job)) => {
                            let (event, tracked) = process_job(&paths, &recovery, job);
                            if let Some(file) = tracked
                                && (watched.len()
                                    < truff_core::workspace::buffers::DEFAULT_MAX_BUFFERS
                                    || watched.contains_key(&file.key.buffer_id))
                            {
                                watched.insert(file.key.buffer_id, file);
                            }
                            // Every accepted job reserved a reply slot before submission.
                            // Shutdown never needs to drain this channel to permit the join.
                            if event_tx.send(event).is_err() {
                                break;
                            }
                            wake(&worker_waker, &worker_pending);
                        }
                        Ok(WorkerMessage::Reconcile) => {}
                        Ok(WorkerMessage::Shutdown) | Err(_) => break,
                    }
                    let targets = worker_targets.lock().ok().and_then(|mut slot| slot.take());
                    if let Some(targets) = targets {
                        let mut next = std::collections::HashMap::with_capacity(targets.len());
                        for (key, path, base) in targets {
                            let Ok(path) = paths.resolve_path(&path) else {
                                continue;
                            };
                            let previous = watched
                                .remove(&key.buffer_id)
                                .filter(|file| file.path == path && file.key.epoch == key.epoch);
                            let fingerprint = base
                                .or_else(|| previous.map(|file| file.fingerprint))
                                .unwrap_or_else(|| {
                                    files::compute_file_fingerprint(&path).unwrap_or_default()
                                });
                            next.insert(
                                key.buffer_id,
                                WatchedFile {
                                    key,
                                    path,
                                    fingerprint,
                                },
                            );
                        }
                        watched = next;
                        if let Ok(mut pending) = watch_dirty.lock() {
                            pending.all = true;
                        }
                    }
                    let pending = watch_dirty
                        .lock()
                        .ok()
                        .map(|mut pending| std::mem::take(&mut *pending))
                        .unwrap_or_default();
                    if pending.all || !pending.paths.is_empty() {
                        for file in watched.values_mut() {
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
                                    wake(&worker_waker, &worker_pending);
                                }
                                Err(TrySendError::Full(_)) => {
                                    if let Ok(mut pending) = watch_dirty.lock() {
                                        pending.all = true;
                                    }
                                    break;
                                }
                                Err(TrySendError::Disconnected(_)) => break,
                            }
                        }
                    }
                }
                // notify is owned here and drops only after accepted jobs have drained.
                drop(watcher);
            })?;
        Ok(Self {
            job_sender: Some(job_tx),
            event_receiver: event_rx,
            watch_receiver: watch_rx,
            worker_handle: Some(worker_handle),
            outstanding,
            capacity,
            waker,
            watch_targets,
            wake_pending,
        })
    }

    /// Publishes only the latest complete watch set; no control queue can accumulate.
    pub fn set_watch_targets(&mut self, mut targets: WatchTargets) {
        targets.truncate(truff_core::workspace::buffers::DEFAULT_MAX_BUFFERS);
        if let Ok(mut slot) = self.watch_targets.lock() {
            *slot = Some(targets);
        }
        if let Some(sender) = &self.job_sender {
            let _ = sender.try_send(WorkerMessage::Reconcile);
        }
    }

    pub fn set_event_waker(&mut self, callback: EventWaker) {
        if let Ok(mut slot) = self.waker.lock() {
            *slot = Some(callback.clone());
        }
        if self.wake_pending.load(Ordering::Acquire) {
            callback();
        }
    }

    pub fn try_submit(&mut self, job: StoreJob) -> Result<JobId, SubmitError> {
        let sender = self.job_sender.as_ref().ok_or(SubmitError::ShuttingDown)?;
        if self.outstanding >= self.capacity {
            return Err(SubmitError::QueueFull);
        }
        let id = JobId::next();
        self.outstanding += 1;
        match sender.try_send(WorkerMessage::Job(id, job)) {
            Ok(()) => Ok(id),
            Err(error) => {
                self.outstanding -= 1;
                match error {
                    TrySendError::Full(_) => Err(SubmitError::QueueFull),
                    TrySendError::Disconnected(_) => Err(SubmitError::WorkerDied),
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.worker_handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn poll_events(&mut self) -> Vec<StoreEvent> {
        // Clear before draining so a result arriving afterwards can always wake the adapter.
        self.wake_pending.store(false, Ordering::Release);
        let mut events = Vec::new();
        while let Ok(event) = self.event_receiver.try_recv() {
            self.outstanding -= 1;
            events.push(event);
        }
        let completed_jobs = events.len();
        events.extend(self.watch_receiver.try_iter());
        if events.len() > completed_jobs
            && let Some(sender) = &self.job_sender
        {
            let _ = sender.try_send(WorkerMessage::Reconcile);
        }
        events
    }

    pub fn shutdown(&mut self) {
        if let Some(sender) = self.job_sender.take() {
            // Watcher owns a sender clone: an explicit FIFO stop follows every accepted job.
            let _ = sender.send(WorkerMessage::Shutdown);
        }
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

fn process_job(
    paths: &crate::VaultPaths,
    recovery: &RecoveryManager,
    job: StoreJob,
) -> (StoreEvent, Option<WatchedFile>) {
    use truff_core::workspace::SnapshotKey;
    let key: SnapshotKey = match &job {
        StoreJob::SaveFile { key, .. }
        | StoreJob::LoadFile { key, .. }
        | StoreJob::ReloadFile { key, .. }
        | StoreJob::OverwriteFile { key, .. }
        | StoreJob::SaveAsFile { key, .. }
        | StoreJob::WriteRecoverySnapshot { key, .. }
        | StoreJob::DeleteRecoverySnapshot { key } => *key,
    };
    let failure = |error: String| (StoreEvent::SaveFailed { key, error }, None);
    let save_as = matches!(&job, StoreJob::SaveAsFile { .. });
    let explicit_reload = matches!(&job, StoreJob::ReloadFile { .. });
    let result = (|| -> Result<(StoreEvent, Option<WatchedFile>), Box<dyn std::error::Error>> {
        match job {
            StoreJob::SaveFile {
                path,
                content,
                base_fingerprint,
                ..
            } => {
                let path = paths.resolve_path(&path)?;
                if let Some(base) = &base_fingerprint {
                    let current = files::compute_file_fingerprint(&path).unwrap_or_default();
                    if &current != base {
                        let disk_content =
                            files::read_file_with_limit(&path, files::DEFAULT_MAX_FILE_BYTES).ok();
                        return Ok((
                            StoreEvent::Conflict {
                                key,
                                disk_content,
                                disk_fingerprint: current,
                            },
                            None,
                        ));
                    }
                }
                let fingerprint = if base_fingerprint.is_some() {
                    files::atomic_write_file(&path, &content)?
                } else {
                    files::atomic_create_file(&path, &content)?
                };
                recovery.delete_snapshots(key.buffer_id)?;
                Ok((
                    StoreEvent::SaveAck {
                        key,
                        fingerprint: fingerprint.clone(),
                    },
                    Some(WatchedFile {
                        key,
                        path,
                        fingerprint,
                    }),
                ))
            }
            StoreJob::OverwriteFile { path, content, .. }
            | StoreJob::SaveAsFile { path, content, .. } => {
                // Distinguish the explicit replacement command from create-only save-as.
                let path = paths.resolve_path(&path)?;
                let fingerprint = if save_as {
                    files::atomic_create_file(&path, &content)?
                } else {
                    files::atomic_write_file(&path, &content)?
                };
                recovery.delete_snapshots(key.buffer_id)?;
                let event = if save_as {
                    StoreEvent::SavedAs {
                        key,
                        path: path.clone(),
                        fingerprint: fingerprint.clone(),
                    }
                } else {
                    StoreEvent::SaveAck {
                        key,
                        fingerprint: fingerprint.clone(),
                    }
                };
                Ok((
                    event,
                    Some(WatchedFile {
                        key,
                        path,
                        fingerprint,
                    }),
                ))
            }
            StoreJob::LoadFile { path, .. } | StoreJob::ReloadFile { path, .. } => {
                let explicit = explicit_reload;
                let path = paths.resolve_path(&path)?;
                let content = files::read_file_with_limit(&path, files::DEFAULT_MAX_FILE_BYTES)?;
                let fingerprint = files::compute_fingerprint(content.as_bytes());
                let event = if explicit {
                    StoreEvent::FileReloaded {
                        key,
                        content,
                        fingerprint: fingerprint.clone(),
                    }
                } else {
                    StoreEvent::FileLoaded {
                        key,
                        content,
                        fingerprint: fingerprint.clone(),
                    }
                };
                Ok((
                    event,
                    Some(WatchedFile {
                        key,
                        path,
                        fingerprint,
                    }),
                ))
            }
            StoreJob::WriteRecoverySnapshot {
                path,
                content,
                base_fingerprint,
                ..
            } => {
                let resolved = paths.resolve_path(&path)?;
                let relative = paths.relative_path(&resolved)?;
                recovery.write_snapshot(
                    key,
                    &relative.to_string_lossy(),
                    &content,
                    base_fingerprint.as_deref(),
                )?;
                Ok((StoreEvent::RecoveryAck { key }, None))
            }
            StoreJob::DeleteRecoverySnapshot { .. } => {
                recovery.delete_snapshots(key.buffer_id)?;
                Ok((StoreEvent::RecoveryAck { key }, None))
            }
        }
    })();
    result.unwrap_or_else(|error| failure(error.to_string()))
}

impl Drop for StoreWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{WriteStage, inject_write_failure};

    #[test]
    fn failed_write_stages_never_ack_or_remove_recovery() {
        let vault = std::env::temp_dir().join(format!("truff-fault-stages-{}", std::process::id()));
        std::fs::create_dir_all(&vault).unwrap();
        let paths = crate::VaultPaths::new(vault.clone());
        let recovery = RecoveryManager::new(vault.clone(), 4096);
        let key = truff_core::workspace::SnapshotKey::new(
            truff_core::workspace::VaultEpoch::new(0),
            truff_core::text::BufferId::new(919),
            1,
        );
        for stage in [
            WriteStage::Write,
            WriteStage::Sync,
            WriteStage::Rename,
            WriteStage::DirectorySync,
        ] {
            let path = vault.join("note.md");
            std::fs::write(&path, "base").unwrap();
            recovery
                .write_snapshot(key, "note.md", "draft", None)
                .unwrap();
            inject_write_failure(Some(stage));
            let (event, _) = process_job(
                &paths,
                &recovery,
                StoreJob::OverwriteFile {
                    key,
                    path: path.clone(),
                    content: "draft".into(),
                },
            );
            inject_write_failure(None);
            assert!(matches!(event, StoreEvent::SaveFailed { .. }));
            assert_eq!(
                recovery
                    .recover_latest_valid_snapshot(key.buffer_id)
                    .unwrap()
                    .content,
                "draft"
            );
            if stage != WriteStage::DirectorySync {
                assert_eq!(std::fs::read_to_string(path).unwrap(), "base");
            }
        }
        std::fs::remove_dir_all(vault).unwrap();
    }

    #[test]
    fn recovery_faults_never_ack_and_preserve_the_previous_copy() {
        let vault =
            std::env::temp_dir().join(format!("truff-recovery-faults-{}", std::process::id()));
        std::fs::create_dir_all(&vault).unwrap();
        let paths = crate::VaultPaths::new(vault.clone());
        let recovery = RecoveryManager::new(vault.clone(), 4096);
        let old = truff_core::workspace::SnapshotKey::new(
            truff_core::workspace::VaultEpoch::new(0),
            truff_core::text::BufferId::new(920),
            1,
        );
        let previous = recovery
            .write_snapshot(old, "note.md", "previous", None)
            .unwrap();
        for (index, stage) in [
            WriteStage::Write,
            WriteStage::Sync,
            WriteStage::Rename,
            WriteStage::DirectorySync,
        ]
        .into_iter()
        .enumerate()
        {
            let key = truff_core::workspace::SnapshotKey {
                revision: index as u64 + 2,
                ..old
            };
            inject_write_failure(Some(stage));
            let (event, _) = process_job(
                &paths,
                &recovery,
                StoreJob::WriteRecoverySnapshot {
                    key,
                    path: PathBuf::from("note.md"),
                    content: "new draft".into(),
                    base_fingerprint: None,
                },
            );
            inject_write_failure(None);
            assert!(matches!(event, StoreEvent::SaveFailed { .. }));
            assert!(previous.exists());
        }
        std::fs::remove_dir_all(vault).unwrap();
    }
}
