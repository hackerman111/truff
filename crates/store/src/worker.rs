use std::fmt;
use std::fs;
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

/// Background worker managing disk I/O and recovery persistence.
pub struct StoreWorker {
    job_sender: Option<SyncSender<(JobId, StoreJob)>>,
    event_receiver: Receiver<StoreEvent>,
    worker_handle: Option<JoinHandle<()>>,
    is_shutting_down: Arc<AtomicBool>,
}

impl StoreWorker {
    /// Starts a background store worker with the specified bounded capacity.
    pub fn start(vault_dir: PathBuf, capacity: usize) -> Self {
        let (job_tx, job_rx) = mpsc::sync_channel::<(JobId, StoreJob)>(capacity);
        let (event_tx, event_rx) = mpsc::channel::<StoreEvent>();
        let is_shutting_down = Arc::new(AtomicBool::new(false));

        let worker_handle = thread::Builder::new()
            .name("truff-store-worker".to_string())
            .spawn(move || {
                let recovery =
                    RecoveryManager::new(vault_dir, crate::recovery::DEFAULT_MAX_RECOVERY_STORAGE);

                while let Ok((_job_id, job)) = job_rx.recv() {
                    match job {
                        StoreJob::SaveFile {
                            key,
                            path,
                            content,
                            base_fingerprint,
                        } => {
                            if path.exists()
                                && let Ok(actual_fp) = files::compute_file_fingerprint(&path)
                                && let Some(ref base_fp) = base_fingerprint
                                && &actual_fp != base_fp
                            {
                                let disk_content = fs::read_to_string(&path).ok();
                                let _ = event_tx.send(StoreEvent::Conflict {
                                    key,
                                    disk_content,
                                    disk_fingerprint: actual_fp,
                                });
                                continue;
                            }

                            match files::atomic_write_file(&path, &content) {
                                Ok(fp) => {
                                    let _ = recovery.delete_snapshots(key.buffer_id);
                                    let _ = event_tx.send(StoreEvent::SaveAck {
                                        key,
                                        fingerprint: fp,
                                    });
                                }
                                Err(e) => {
                                    let _ = event_tx.send(StoreEvent::SaveFailed {
                                        key,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        }
                        StoreJob::LoadFile { key, path } => {
                            match files::read_file_with_limit(&path, files::DEFAULT_MAX_FILE_BYTES)
                            {
                                Ok(content) => {
                                    let fp = files::compute_fingerprint(content.as_bytes());
                                    let _ = event_tx.send(StoreEvent::FileLoaded {
                                        key,
                                        content,
                                        fingerprint: fp,
                                    });
                                }
                                Err(e) => {
                                    let _ = event_tx.send(StoreEvent::SaveFailed {
                                        key,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        }
                        StoreJob::WriteRecoverySnapshot {
                            key,
                            path,
                            content,
                            base_fingerprint,
                        } => {
                            let rel = path.to_string_lossy();
                            match recovery.write_snapshot(
                                key,
                                &rel,
                                &content,
                                base_fingerprint.as_deref(),
                            ) {
                                Ok(_) => {
                                    let _ = event_tx.send(StoreEvent::RecoveryAck { key });
                                }
                                Err(e) => {
                                    let _ = event_tx.send(StoreEvent::SaveFailed {
                                        key,
                                        error: e.to_string(),
                                    });
                                }
                            }
                        }
                        StoreJob::DeleteRecoverySnapshot { buffer_id } => {
                            let _ = recovery.delete_snapshots(buffer_id);
                        }
                    }
                }
            })
            .expect("failed to spawn truff store worker thread");

        Self {
            job_sender: Some(job_tx),
            event_receiver: event_rx,
            worker_handle: Some(worker_handle),
            is_shutting_down,
        }
    }

    /// Attempts to submit a job to the worker queue with backpressure.
    pub fn try_submit(&mut self, job: StoreJob) -> Result<JobId, SubmitError> {
        if self.is_shutting_down.load(Ordering::Relaxed) {
            return Err(SubmitError::ShuttingDown);
        }

        let sender = self.job_sender.as_ref().ok_or(SubmitError::ShuttingDown)?;
        let id = JobId::next();

        match sender.try_send((id, job)) {
            Ok(()) => Ok(id),
            Err(TrySendError::Full(_)) => Err(SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(SubmitError::WorkerDied),
        }
    }

    /// Drains all available store events non-blockingly.
    pub fn poll_events(&mut self) -> Vec<StoreEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_receiver.try_recv() {
            events.push(event);
        }
        events
    }

    /// Gracefully shuts down the store worker, ensuring all already-accepted jobs complete.
    pub fn shutdown(&mut self) {
        self.is_shutting_down.store(true, Ordering::Relaxed);
        // Dropping the sender causes rx.recv() to finish all queued jobs and then terminate
        self.job_sender.take();

        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for StoreWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
