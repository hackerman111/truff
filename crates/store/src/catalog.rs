//! Background vault enumeration, index queries, and recoverable file operations.
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread::{self, JoinHandle};

use truff_core::markdown::facts::{DocFacts, MarkdownFacts};

use crate::VaultPaths;
use crate::files::{DEFAULT_MAX_FILE_BYTES, compute_fingerprint, read_file_with_limit};
use crate::index::{BacklinkOccurrence, FileId, IndexDatabase, Resolution, Target};
use crate::operations::{FileOpPlan, recover_pending_operations};

#[derive(Debug)]
pub enum CatalogJob {
    RefreshNote {
        path: String,
    },
    Backlinks {
        path: String,
        dirty: Vec<(String, String)>,
    },
    Follow {
        source: String,
        target: String,
    },
    Rename {
        from: String,
        to: String,
        dirty: Vec<PathBuf>,
    },
    Trash {
        path: String,
        dirty: Vec<PathBuf>,
    },
    Create {
        path: String,
    },
}

#[derive(Debug)]
pub enum CatalogEvent {
    Files(Vec<String>),
    Ready,
    Backlinks {
        path: String,
        occurrences: Vec<BacklinkOccurrence>,
    },
    JobDone,
    Follow {
        path: String,
        anchor: Option<String>,
    },
    Renamed {
        from: String,
        to: String,
        rewritten: Vec<PathBuf>,
    },
    Trashed(String),
    Created(String),
    Error(String),
}

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
            let callback = self.wake.callback.lock().unwrap_or_else(|poison| poison.into_inner()).clone();
            if let Some(callback) = callback { callback(); }
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
        let events_tx = CatalogReplies { sender: events_tx, wake: wake.clone() };
        let handle = thread::Builder::new()
            .name("truff-catalog".into())
            .spawn(move || {
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let mut db = IndexDatabase::open(paths.truff_dir().join("index.db"))?;
                    recover_pending_operations(&paths, db.connection_mut())?;
                    let mut ids = rebuild(&paths, &mut db, &events_tx)?;
                    let _ = events_tx.send(CatalogEvent::Ready);
                    while let Ok(job) = jobs.recv() {
                        let event = execute(job, &paths, &mut db, &ids);
                        let refresh = matches!(
                            event,
                            Ok(CatalogEvent::Renamed { .. }
                                | CatalogEvent::Trashed(_)
                                | CatalogEvent::Created(_))
                        );
                        if events_tx
                            .send(
                                event
                                    .unwrap_or_else(|error| CatalogEvent::Error(error.to_string())),
                            )
                            .is_err()
                        {
                            break;
                        }
                        if refresh {
                            ids = rebuild(&paths, &mut db, &events_tx)?;
                            let _ = events_tx.send(CatalogEvent::Ready);
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
        *self.wake.callback.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(callback.clone());
        if self.wake.pending.load(Ordering::Acquire) { callback(); }
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

fn enumerate(root: &Path) -> io::Result<Vec<String>> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() || entry.file_name() == ".truff" {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                directories.push(path);
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                files.push(
                    path.strip_prefix(root)
                        .expect("enumerated path is under root")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

fn rebuild(
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    events: &CatalogReplies,
) -> Result<HashMap<String, FileId>, Box<dyn std::error::Error>> {
    let files = enumerate(paths.root())?;
    let _ = events.send(CatalogEvent::Files(files.clone()));
    let old_ids: Vec<FileId> = db
        .connection()
        .prepare("SELECT id FROM files")?
        .query_map([], |row| Ok(FileId(row.get(0)?)))?
        .collect::<Result<_, _>>()?;
    for id in old_ids {
        db.remove_file(id)?;
    }
    let mut ids = HashMap::with_capacity(files.len());
    // Register every target first so forward references resolve regardless of filename order.
    for (index, path) in files.iter().enumerate() {
        let id = FileId(index as i64 + 1);
        db.index_file(id, path, path, "", 0, &DocFacts::default(), "")?;
        ids.insert(path.clone(), id);
    }
    for path in &files {
        match read_file_with_limit(
            &paths.resolve_path(Path::new(path))?,
            DEFAULT_MAX_FILE_BYTES,
        ) {
            Ok(text) => {
                let facts = MarkdownFacts::extract(&text)?;
                db.index_file(
                    ids[path],
                    path,
                    path,
                    &compute_fingerprint(text.as_bytes()),
                    1,
                    &facts,
                    &text,
                )?;
            }
            Err(error) => {
                let _ = events.send(CatalogEvent::Error(format!("{}: {error}", path)));
            }
        }
    }
    Ok(ids)
}

fn execute(
    job: CatalogJob,
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    ids: &HashMap<String, FileId>,
) -> Result<CatalogEvent, Box<dyn std::error::Error>> {
    match job {
        CatalogJob::RefreshNote { path } => {
            let id = *ids.get(&path).ok_or("Note is not indexed")?;
            let text = read_file_with_limit(
                &paths.resolve_path(Path::new(&path))?,
                DEFAULT_MAX_FILE_BYTES,
            )?;
            db.clear_dirty_overlay(id);
            db.index_file(
                id,
                &path,
                &path,
                &compute_fingerprint(text.as_bytes()),
                1,
                &MarkdownFacts::extract(&text)?,
                &text,
            )?;
            Ok(CatalogEvent::Ready)
        }
        CatalogJob::Backlinks { path, dirty } => {
            for &id in ids.values() {
                db.clear_dirty_overlay(id);
            }
            let backlinks = (|| -> Result<_, Box<dyn std::error::Error>> {
                for (path, text) in dirty {
                    if let Some(&id) = ids.get(&path) {
                        db.set_dirty_overlay(id, path, MarkdownFacts::extract(&text)?);
                    }
                }
                let id = *ids.get(&path).ok_or("Note is not indexed")?;
                Ok(db.get_backlinks(id, None, 0, 100)?.occurrences)
            })();
            // Unsaved facts belong to this query; file manifests must validate against disk.
            for &id in ids.values() {
                db.clear_dirty_overlay(id);
            }
            Ok(CatalogEvent::Backlinks {
                path,
                occurrences: backlinks?,
            })
        }
        CatalogJob::Follow { source, target } => {
            let id = *ids.get(&source).ok_or("Source note is not indexed")?;
            match db.resolver().resolve(id, &Target::parse(&target)) {
                Resolution::Found(id, anchor) => {
                    let path = ids
                        .iter()
                        .find(|(_, file_id)| **file_id == id)
                        .map(|(path, _)| path.clone())
                        .ok_or("Resolved file is unavailable")?;
                    Ok(CatalogEvent::Follow { path, anchor })
                }
                Resolution::Missing => {
                    Err("Link target is missing; use :new <path.md> to create it".into())
                }
                _ => Err("Link target is ambiguous".into()),
            }
        }
        CatalogJob::Rename { from, to, dirty } => {
            let mut plan = FileOpPlan::rename(paths, &from, &to)?;
            plan.validate(paths, &dirty, db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Renamed {
                from,
                to,
                rewritten: plan
                    .rewrites
                    .iter()
                    .map(|rewrite| rewrite.rel_path.clone())
                    .collect(),
            })
        }
        CatalogJob::Trash { path, dirty } => {
            let mut plan = FileOpPlan::trash(paths, &path)?;
            plan.validate(paths, &dirty, db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Trashed(path))
        }
        CatalogJob::Create { path } => {
            let mut plan = FileOpPlan::create_file(paths, &path, String::new())?;
            plan.validate(paths, &[], db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Created(path))
        }
    }
}
