//! Bounded transport for the Store search API; the GUI never opens SQLite or runs a query.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
};
use truff_store::index::{IndexDatabase, IndexError, SearchPage, SearchQuery};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct SearchKey {
    pub index: PathBuf,
    pub epoch: truff_core::workspace::VaultEpoch,
    pub mode: crate::navigator::NavigatorMode,
    pub text: String,
    pub page: usize,
    pub catalog_revision: u64,
}

struct Request {
    key: SearchKey,
    generation: u64,
}

pub(super) struct Reply {
    pub key: SearchKey,
    pub generation: u64,
    pub result: Result<SearchPage, IndexError>,
}

pub(super) struct SearchTransport {
    sender: Option<mpsc::SyncSender<Request>>,
    reply: Arc<Mutex<Option<Reply>>>,
    thread: Option<JoinHandle<()>>,
    pending: Option<Request>,
    current: Option<SearchKey>,
    generation: u64,
    pub busy: bool,
}

impl SearchTransport {
    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        let reply = Arc::new(Mutex::new(None));
        let output = reply.clone();
        let thread = std::thread::Builder::new()
            .name("truff-search".into())
            .spawn(move || {
                let mut database: Option<(PathBuf, IndexDatabase)> = None;
                while let Ok(mut request) = receiver.recv() {
                    // Coalesce waiting keystrokes before starting a Store query.
                    while let Ok(newer) = receiver.try_recv() {
                        request = newer;
                    }
                    let result = (|| {
                        if database
                            .as_ref()
                            .is_none_or(|(path, _)| *path != request.key.index)
                        {
                            database = None;
                            let db = IndexDatabase::open(&request.key.index)?;
                            database = Some((request.key.index.clone(), db));
                        }
                        let (_, db) = database
                            .as_ref()
                            .expect("successful open initialized the search reader");
                        let mut page = db.search(&SearchQuery {
                            id: request.generation,
                            text: request.key.text.clone(),
                            page: request.key.page,
                            page_size: 100,
                        })?;
                        for result in &mut page.results {
                            bound_text(&mut result.title, 256);
                            bound_text(&mut result.snippet, 4096);
                        }
                        Ok(page)
                    })();
                    let previous = output.lock().ok().and_then(|mut slot| {
                        slot.replace(Reply {
                            key: request.key,
                            generation: request.generation,
                            result,
                        })
                    });
                    drop(previous);
                    wake();
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            reply,
            thread: Some(thread),
            pending: None,
            current: None,
            generation: 0,
            busy: false,
        })
    }

    pub fn request(&mut self, key: SearchKey) -> bool {
        if self.current.as_ref() == Some(&key) {
            self.flush();
            return false;
        }
        self.generation += 1;
        self.current = Some(key.clone());
        self.pending = Some(Request {
            key,
            generation: self.generation,
        });
        self.busy = true;
        self.flush();
        true
    }

    fn flush(&mut self) {
        let Some(request) = self.pending.take() else {
            return;
        };
        let Some(sender) = &self.sender else {
            self.busy = false;
            return;
        };
        match sender.try_send(request) {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(request)) => self.pending = Some(request),
            Err(mpsc::TrySendError::Disconnected(_)) => self.busy = false,
        }
    }

    pub fn cancel(&mut self) {
        if self.current.is_none() && self.pending.is_none() && !self.busy {
            return;
        }
        self.generation += 1;
        self.current = None;
        self.pending = None;
        self.busy = false;
    }

    pub fn poll(&mut self) -> Option<Reply> {
        self.flush();
        let reply = self.reply.lock().ok()?.take()?;
        // Reject old queries, closed/reopened overlays and prior vault epochs.
        if reply.generation != self.generation || self.current.as_ref() != Some(&reply.key) {
            return None;
        }
        self.busy = false;
        Some(reply)
    }
}

// Presentation limits run on the worker, before large Store fields reach the GUI.
fn bound_text(text: &mut String, bytes: usize) {
    if text.len() <= bytes {
        return;
    }
    let mut end = bytes.saturating_sub(3);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str("...");
    text.shrink_to_fit();
}

impl Drop for SearchTransport {
    fn drop(&mut self) {
        self.pending = None;
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
