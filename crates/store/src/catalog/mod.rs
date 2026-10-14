//! Background vault enumeration, index queries, and recoverable file operations.
mod jobs;
mod protocol;
mod scan;

pub use protocol::{CatalogEvent, CatalogJob};

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::VaultPaths;
use crate::index::IndexDatabase;
use crate::operations::recover_pending_operations;

#[derive(Default)]
struct CatalogWake {
    pending: AtomicBool,
    callback: Mutex<Option<crate::worker::EventWaker>>,
}

struct CatalogReplies {
    sender: SyncSender<CatalogEvent>,
    wake: Arc<CatalogWake>,
}

impl CatalogReplies {
    fn send(&self, event: CatalogEvent) -> Result<(), mpsc::SendError<CatalogEvent>> {
        self.sender.send(event)?;
        if !self.wake.pending.swap(true, Ordering::AcqRel) {
            let callback = self
                .wake
                .callback
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone();
            if let Some(callback) = callback {
                callback();
            }
        }
        Ok(())
    }
}

/// Single owner of SQLite. Neither indexing nor file manifests execute in a GUI callback.
pub struct CatalogWorker {
    sender: Option<SyncSender<CatalogJob>>,
    events: Receiver<CatalogEvent>,
    handle: Option<JoinHandle<()>>,
    wake: Arc<CatalogWake>,
}

impl CatalogWorker {
    pub fn start(paths: VaultPaths) -> io::Result<Self> {
        let (sender, jobs) = mpsc::sync_channel(8);
        let (events_tx, events) = mpsc::sync_channel(16);
        let wake = Arc::new(CatalogWake::default());
        let events_tx = CatalogReplies {
            sender: events_tx,
            wake: wake.clone(),
        };
        let handle = thread::Builder::new()
            .name("truff-catalog".into())
            .spawn(move || {
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let mut db = IndexDatabase::open(paths.truff_dir().join("index.db"))?;
                    recover_pending_operations(&paths, db.connection_mut())?;
                    let mut ids = scan::rebuild(&paths, &mut db, &events_tx)?;
                    let _ = events_tx.send(CatalogEvent::Ready);
                    while let Ok(job) = jobs.recv() {
                        let event = jobs::execute(job, &paths, &mut db, &ids, &events_tx);
                        let (changed, removed) = match &event {
                            Ok(CatalogEvent::Created(path)) => (vec![path.clone()], Vec::new()),
                            Ok(CatalogEvent::Trashed(path)) => {
                                let removed = ids
                                    .iter()
                                    .filter(|(candidate, _)| {
                                        std::path::Path::new(candidate).starts_with(path)
                                    })
                                    .map(|(_, &id)| id)
                                    .collect();
                                (Vec::new(), removed)
                            }
                            Ok(CatalogEvent::Renamed {
                                from,
                                to,
                                rewritten,
                            }) => {
                                let mut changed = scan::affected_files(&paths, to)?;
                                let mut removed = Vec::new();
                                for (path, &id) in &ids {
                                    if std::path::Path::new(path).starts_with(from) {
                                        removed.push(id);
                                    }
                                }
                                changed.extend(
                                    rewritten.iter().map(|p| p.to_string_lossy().into_owned()),
                                );
                                (changed, removed)
                            }
                            _ => (Vec::new(), Vec::new()),
                        };
                        if !changed.is_empty() || !removed.is_empty() {
                            scan::refresh(&paths, &mut db, &events_tx, &changed, &removed)?;
                            ids = scan::indexed_paths(&db)?;
                            let mut files: Vec<_> = ids.keys().cloned().collect();
                            files.sort();
                            let _ = events_tx.send(CatalogEvent::Files(files));
                        }
                        if events_tx
                            .send(
                                event
                                    .unwrap_or_else(|error| CatalogEvent::Error(error.to_string())),
                            )
                            .is_err()
                        {
                            break;
                        }
                        if events_tx.send(CatalogEvent::JobDone).is_err() {
                            break;
                        }
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    let _ = events_tx.send(CatalogEvent::Error(error.to_string()));
                    let _ = events_tx.send(CatalogEvent::Ready);
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            events,
            handle: Some(handle),
            wake,
        })
    }

    pub fn try_submit(&self, job: CatalogJob) -> Result<(), crate::SubmitError> {
        match self
            .sender
            .as_ref()
            .ok_or(crate::SubmitError::ShuttingDown)?
            .try_send(job)
        {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(crate::SubmitError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(crate::SubmitError::WorkerDied),
        }
    }

    pub fn is_running(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn poll_events(&self) -> Vec<CatalogEvent> {
        self.wake.pending.store(false, Ordering::Release);
        self.events.try_iter().collect()
    }

    pub fn set_event_waker(&self, callback: crate::worker::EventWaker) {
        *self
            .wake
            .callback
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(callback.clone());
        if self.wake.pending.load(Ordering::Acquire) {
            callback();
        }
    }

    pub fn shutdown(&mut self) {
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            // Drain bounded replies while the worker drains accepted jobs; joining first can deadlock.
            while self.events.recv().is_ok() {}
            let _ = handle.join();
        }
    }
}

impl Drop for CatalogWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}
