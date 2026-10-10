//! Persistence, file operations, and background worker facade for Truff v2.
//!
//! Owns filesystem operations, atomic saving, recovery snapshots, and worker backpressure.

use std::fmt;
use std::path::PathBuf;

pub use files::{
    FileError, atomic_write_file, compute_file_fingerprint, compute_fingerprint,
    read_file_with_limit,
};
pub use recovery::{
    DEFAULT_MAX_RECOVERY_STORAGE, RecoveredSnapshot, RecoveryError, RecoveryManager,
};
pub use vault::{VaultError, VaultLock, VaultPaths};
pub use worker::{DEFAULT_MAX_DURABLE_SLOTS, JobId, StoreWorker, SubmitError};

pub mod catalog;
pub mod files;
pub mod index;
pub mod operations;
pub mod recovery;
pub mod session;
pub mod vault;
pub mod worker;

use truff_core::workspace::{StoreEvent, StoreJob};

/// Errors occurring in the store subsystem.
#[derive(Debug)]
pub enum StoreError {
    Vault(VaultError),
    Submit(SubmitError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vault(e) => write!(f, "vault error: {e}"),
            Self::Submit(e) => write!(f, "submit error: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<VaultError> for StoreError {
    fn from(e: VaultError) -> Self {
        Self::Vault(e)
    }
}

impl From<SubmitError> for StoreError {
    fn from(e: SubmitError) -> Self {
        Self::Submit(e)
    }
}

/// Unified facade managing vault storage, atomic file persistence, and background jobs.
pub struct Store {
    paths: VaultPaths,
    _lock: Option<VaultLock>,
    worker: StoreWorker,
}

impl Store {
    /// Initializes a store instance for the given vault directory using default worker capacity.
    pub fn new(vault_dir: PathBuf) -> Result<Self, StoreError> {
        Self::new_with_capacity(vault_dir, DEFAULT_MAX_DURABLE_SLOTS)
    }

    /// Initializes a store instance with an explicit bounded durable worker capacity.
    pub fn new_with_capacity(vault_dir: PathBuf, capacity: usize) -> Result<Self, StoreError> {
        let paths = VaultPaths::new(vault_dir.clone());
        let lock = VaultLock::acquire(paths.root())?;
        let worker =
            StoreWorker::start(paths.root().to_path_buf(), capacity).map_err(VaultError::Io)?;

        Ok(Self {
            paths,
            _lock: Some(lock),
            worker,
        })
    }

    /// Access to vault paths helper.
    pub fn paths(&self) -> &VaultPaths {
        &self.paths
    }

    /// Submits a persistent or I/O job to the background worker with backpressure.
    pub fn try_submit(&mut self, job: StoreJob) -> Result<JobId, SubmitError> {
        self.worker.try_submit(job)
    }

    pub fn set_watch_targets(&mut self, targets: worker::WatchTargets) {
        self.worker.set_watch_targets(targets);
    }

    /// Registers an adapter wakeup; invoked only for a new storage/watcher result.
    pub fn set_event_waker(&mut self, callback: worker::EventWaker) {
        self.worker.set_event_waker(callback);
    }

    pub fn is_running(&self) -> bool {
        self.worker.is_running()
    }

    /// Drains all available asynchronous store events produced by completed jobs.
    pub fn poll_events(&mut self) -> Vec<StoreEvent> {
        self.worker.poll_events()
    }

    /// Shuts down the store worker, guaranteeing that all accepted durable jobs are completed.
    pub fn shutdown(mut self) -> Result<Vec<StoreEvent>, StoreError> {
        self.worker.shutdown();
        Ok(self.worker.poll_events())
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        // Struct field drop order must never unlock before accepted disk jobs drain.
        self.worker.shutdown();
        self._lock.take();
    }
}
