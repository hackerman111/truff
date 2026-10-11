use super::SnapshotKey;
use std::path::PathBuf;

/// A background I/O or persistence job dispatched from `core` to `store`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreJob {
    /// Request reading file content from disk.
    LoadFile { key: SnapshotKey, path: PathBuf },
    /// Request durable atomic saving of a buffer snapshot to disk.
    SaveFile {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
        base_fingerprint: Option<String>,
    },
    /// Explicitly approved reload, still invalidated by a newer local revision.
    ReloadFile { key: SnapshotKey, path: PathBuf },
    /// Explicit user approval to overwrite the external version.
    OverwriteFile {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
    },
    /// Durable creation of a distinct destination, preserving an existing file.
    SaveAsFile {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
    },
    /// Request writing an emergency crash recovery snapshot.
    WriteRecoverySnapshot {
        key: SnapshotKey,
        path: PathBuf,
        content: String,
        base_fingerprint: Option<String>,
    },
    /// Request deleting recovery snapshots once a buffer returns to its savepoint.
    DeleteRecoverySnapshot { key: SnapshotKey },
}

/// An asynchronous persistence event emitted by `store` to `core`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreEvent {
    /// File content successfully read from disk.
    FileLoaded {
        key: SnapshotKey,
        content: String,
        fingerprint: String,
    },
    FileReloaded {
        key: SnapshotKey,
        content: String,
        fingerprint: String,
    },
    SavedAs {
        key: SnapshotKey,
        path: PathBuf,
        fingerprint: String,
    },
    /// File snapshot successfully written and synced to disk.
    SaveAck {
        key: SnapshotKey,
        fingerprint: String,
    },
    /// File saving failed due to I/O error or permission failure.
    SaveFailed { key: SnapshotKey, error: String },
    /// Recovery snapshot was written or deleted on disk.
    RecoveryAck { key: SnapshotKey },
    /// External file modification was detected while buffer has unsaved changes.
    Conflict {
        key: SnapshotKey,
        disk_content: Option<String>,
        disk_fingerprint: String,
    },
    /// File was modified externally.
    ExternalChanged {
        key: SnapshotKey,
        path: PathBuf,
        new_fingerprint: String,
    },
}
