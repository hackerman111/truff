use std::path::PathBuf;

use crate::index::BacklinkOccurrence;

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
