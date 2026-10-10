use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::files::{self, DEFAULT_MAX_FILE_BYTES, FileError};
use crate::recovery::{RecoveredSnapshot, RecoveryError, RecoveryManager};
use crate::vault::{VaultError, VaultPaths};

/// Maximum entries allowed in session buffers (§10.1).
pub const MAX_SESSION_BUFFERS: usize = 256;

/// Maximum entries allowed in session views (§10.1).
pub const MAX_SESSION_VIEWS: usize = 32;

/// Maximum entries allowed in session jump history (§10.1).
pub const MAX_SESSION_JUMP_ENTRIES: usize = 256;

/// Errors occurring during session saving or lazy restoration.
#[derive(Debug)]
pub enum SessionError {
    Io(io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    Vault(VaultError),
    Recovery(RecoveryError),
    File(FileError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "session I/O error: {e}"),
            Self::TomlDe(e) => write!(f, "session TOML deserialization error: {e}"),
            Self::TomlSer(e) => write!(f, "session TOML serialization error: {e}"),
            Self::Vault(e) => write!(f, "session vault error: {e}"),
            Self::Recovery(e) => write!(f, "session recovery error: {e}"),
            Self::File(e) => write!(f, "session file error: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<io::Error> for SessionError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for SessionError {
    fn from(e: toml::de::Error) -> Self {
        Self::TomlDe(e)
    }
}

impl From<toml::ser::Error> for SessionError {
    fn from(e: toml::ser::Error) -> Self {
        Self::TomlSer(e)
    }
}

impl From<VaultError> for SessionError {
    fn from(e: VaultError) -> Self {
        Self::Vault(e)
    }
}

impl From<RecoveryError> for SessionError {
    fn from(e: RecoveryError) -> Self {
        Self::Recovery(e)
    }
}

impl From<FileError> for SessionError {
    fn from(e: FileError) -> Self {
        Self::File(e)
    }
}

/// Metadata describing an open buffer in a saved workspace session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBufferEntry {
    pub path: PathBuf,
    pub is_dirty: bool,
    pub last_saved_fingerprint: Option<String>,
}

/// View state stored in a workspace session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionViewEntry {
    pub buffer_path: PathBuf,
    pub cursor_anchor: usize,
    pub cursor_head: usize,
    pub top_line: usize,
}

/// Persisted workspace session structure matching `session.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionData {
    pub version: u32,
    pub vault_path: PathBuf,
    pub active_buffer: Option<PathBuf>,
    pub buffers: Vec<SessionBufferEntry>,
    pub views: Vec<SessionViewEntry>,
    pub active_view_index: Option<usize>,
    pub jump_history: Vec<(usize, usize)>,
}

/// State of a buffer restored from a workspace session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferRestoreState {
    /// Inactive buffer whose text is deferred for lazy loading on demand.
    Lazy,
    /// Active or preloaded buffer whose text is resident in memory.
    Loaded(String),
    /// File was missing on disk at restoration time; represented as closeable.
    MissingOnDisk,
    /// Failed to read buffer text from disk.
    Error(String),
}

/// Fully restored buffer record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredBufferEntry {
    pub path: PathBuf,
    pub is_dirty: bool,
    pub last_saved_fingerprint: Option<String>,
    pub state: BufferRestoreState,
}

/// Workspace session restored with active buffer loaded first and inactive buffers lazy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoredSession {
    pub version: u32,
    pub vault_path: PathBuf,
    pub active_buffer_path: Option<PathBuf>,
    pub buffers: Vec<RestoredBufferEntry>,
    pub views: Vec<SessionViewEntry>,
    pub active_view_index: Option<usize>,
    pub jump_history: Vec<(usize, usize)>,
    pub recovered_dirty_buffers: Vec<RecoveredSnapshot>,
}

/// Atomically persists workspace session to `.truff/session.toml`.
pub fn save_session(vault: &VaultPaths, session: &SessionData) -> Result<(), SessionError> {
    let truff_dir = vault.truff_dir();
    fs::create_dir_all(truff_dir)?;

    let mut bounded_session = session.clone();
    bounded_session.buffers.truncate(MAX_SESSION_BUFFERS);
    bounded_session.views.truncate(MAX_SESSION_VIEWS);
    bounded_session
        .jump_history
        .truncate(MAX_SESSION_JUMP_ENTRIES);

    let toml_str = toml::to_string_pretty(&bounded_session)?;

    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let target_path = truff_dir.join("session.toml");
    let tmp_path = truff_dir.join(format!(".session.toml.{pid}_{id}.tmp"));

    let write_res = (|| -> io::Result<()> {
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        file.write_all(toml_str.as_bytes())?;
        file.sync_data()?;
        drop(file);

        fs::rename(&tmp_path, &target_path)?;

        #[cfg(unix)]
        {
            if let Ok(dir) = File::open(truff_dir) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(SessionError::Io(e));
    }

    Ok(())
}

/// Restores workspace session: active buffer is loaded first, inactive buffers lazily.
pub fn restore_session(
    vault: &VaultPaths,
    recovery: &RecoveryManager,
) -> Result<RestoredSession, SessionError> {
    let session_path = vault.truff_dir().join("session.toml");

    let session: SessionData = if session_path.exists() {
        let content = fs::read_to_string(&session_path)?;
        toml::from_str(&content)?
    } else {
        SessionData {
            version: 1,
            vault_path: vault.root().to_path_buf(),
            active_buffer: None,
            buffers: Vec::new(),
            views: Vec::new(),
            active_view_index: None,
            jump_history: Vec::new(),
        }
    };

    let mut restored_buffers = Vec::new();

    // 1. Restore buffers: active buffer first
    for entry in &session.buffers {
        let is_active = session.active_buffer.as_ref() == Some(&entry.path);
        let abs_path = match vault.resolve_path(&entry.path) {
            Ok(p) => p,
            Err(_) => {
                restored_buffers.push(RestoredBufferEntry {
                    path: entry.path.clone(),
                    is_dirty: entry.is_dirty,
                    last_saved_fingerprint: entry.last_saved_fingerprint.clone(),
                    state: BufferRestoreState::MissingOnDisk,
                });
                continue;
            }
        };

        if !abs_path.exists() {
            restored_buffers.push(RestoredBufferEntry {
                path: entry.path.clone(),
                is_dirty: entry.is_dirty,
                last_saved_fingerprint: entry.last_saved_fingerprint.clone(),
                state: BufferRestoreState::MissingOnDisk,
            });
            continue;
        }

        if is_active {
            // Active buffer is loaded immediately into memory
            let state = match files::read_file_with_limit(&abs_path, DEFAULT_MAX_FILE_BYTES) {
                Ok(text) => BufferRestoreState::Loaded(text),
                Err(e) => BufferRestoreState::Error(e.to_string()),
            };
            restored_buffers.push(RestoredBufferEntry {
                path: entry.path.clone(),
                is_dirty: entry.is_dirty,
                last_saved_fingerprint: entry.last_saved_fingerprint.clone(),
                state,
            });
        } else {
            // Inactive buffer text remains lazy
            restored_buffers.push(RestoredBufferEntry {
                path: entry.path.clone(),
                is_dirty: entry.is_dirty,
                last_saved_fingerprint: entry.last_saved_fingerprint.clone(),
                state: BufferRestoreState::Lazy,
            });
        }
    }

    // 2. Scan emergency recovery directory for dirty buffers (even if views were closed)
    let recovered_snapshots = recovery.recover_all_latest_snapshots()?;

    Ok(RestoredSession {
        version: session.version,
        vault_path: session.vault_path,
        active_buffer_path: session.active_buffer,
        buffers: restored_buffers,
        views: session.views,
        active_view_index: session.active_view_index,
        jump_history: session.jump_history,
        recovered_dirty_buffers: recovered_snapshots,
    })
}
