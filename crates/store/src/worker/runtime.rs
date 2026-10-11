use super::jobs::process_job;
use super::watch::WatchState;
use super::{EventWaker, WatchTargets, WorkerMessage, wake};
use crate::VaultPaths;
use crate::recovery::RecoveryManager;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use truff_core::workspace::StoreEvent;

pub(super) struct WorkerRuntime {
    pub(super) paths: VaultPaths,
    pub(super) recovery: RecoveryManager,
    pub(super) watcher: notify::RecommendedWatcher,
    pub(super) watch: WatchState,
    pub(super) job_rx: Receiver<WorkerMessage>,
    pub(super) event_tx: SyncSender<StoreEvent>,
    pub(super) watch_tx: SyncSender<StoreEvent>,
    pub(super) waker: Arc<Mutex<Option<EventWaker>>>,
    pub(super) wake_pending: Arc<AtomicBool>,
    pub(super) targets: Arc<Mutex<Option<WatchTargets>>>,
}

impl WorkerRuntime {
    pub(super) fn run(mut self) {
        loop {
            match self.job_rx.recv() {
                Ok(WorkerMessage::Job(_id, job)) => {
                    let (event, tracked) = process_job(&self.paths, &self.recovery, job);
                    if let Some(file) = tracked {
                        self.watch.track(file);
                    }
                    // Every accepted job reserved a reply slot before submission.
                    // Shutdown never needs to drain this channel to permit the join.
                    if self.event_tx.send(event).is_err() {
                        break;
                    }
                    wake(&self.waker, &self.wake_pending);
                }
                Ok(WorkerMessage::Reconcile) => {}
                Ok(WorkerMessage::Shutdown) | Err(_) => break,
            }
            self.watch.refresh_targets(&self.paths, &self.targets);
            self.watch
                .reconcile(&self.paths, &self.watch_tx, &self.waker, &self.wake_pending);
        }
        // notify is owned here and drops only after accepted jobs have drained.
        drop(self.watcher);
    }
}
