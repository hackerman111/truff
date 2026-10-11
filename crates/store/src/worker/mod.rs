mod jobs;
mod runtime;
#[cfg(test)]
mod tests;
mod watch;

use runtime::WorkerRuntime;
use watch::WatchState;
pub use watch::WatchTargets;

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use truff_core::workspace::{StoreEvent, StoreJob};

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
        let (watcher, watch) = WatchState::start(&vault_dir, &job_tx)?;
        let worker_handle = thread::Builder::new()
            .name("truff-store-worker".into())
            .spawn(move || {
                WorkerRuntime {
                    paths: crate::VaultPaths::new(vault_dir.clone()),
                    recovery: RecoveryManager::new(
                        vault_dir,
                        crate::recovery::DEFAULT_MAX_RECOVERY_STORAGE,
                    ),
                    watcher,
                    watch,
                    job_rx,
                    event_tx,
                    watch_tx,
                    waker: worker_waker,
                    wake_pending: worker_pending,
                    targets: worker_targets,
                }
                .run();
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

impl Drop for StoreWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
