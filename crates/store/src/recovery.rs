use std::fmt;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use truff_core::text::BufferId;
use truff_core::workspace::buffers::{SnapshotKey, VaultEpoch};

/// Default maximum disk storage allowed for recovery snapshots: 256 MiB (§10.1).
pub const DEFAULT_MAX_RECOVERY_STORAGE: usize = 256 * 1024 * 1024;

/// Error occurring during recovery snapshot write or recovery.
#[derive(Debug)]
pub enum RecoveryError {
    Io(io::Error),
    StorageLimitExceeded(usize),
    CorruptedSnapshot(String),
    NoValidSnapshot,
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "recovery I/O error: {e}"),
            Self::StorageLimitExceeded(limit) => {
                write!(f, "recovery storage limit of {limit} bytes exceeded")
            }
            Self::CorruptedSnapshot(msg) => write!(f, "corrupted snapshot: {msg}"),
            Self::NoValidSnapshot => write!(f, "no valid snapshot found"),
        }
    }
}

impl std::error::Error for RecoveryError {}

impl From<io::Error> for RecoveryError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// A recovered buffer snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredSnapshot {
    pub key: SnapshotKey,
    pub rel_path: String,
    pub content: String,
    pub base_fingerprint: Option<String>,
}

/// Manages crash-recovery snapshots stored under `.truff/recovery/`.
pub struct RecoveryManager {
    recovery_dir: PathBuf,
    max_storage_bytes: usize,
}

impl RecoveryManager {
    /// Creates a new `RecoveryManager` for a vault root directory.
    pub fn new(vault_dir: PathBuf, max_storage_bytes: usize) -> Self {
        let recovery_dir = vault_dir.join(".truff/recovery");
        let _ = fs::create_dir_all(&recovery_dir);
        Self {
            recovery_dir,
            max_storage_bytes,
        }
    }

    /// Path to the recovery directory.
    pub fn recovery_dir(&self) -> &Path {
        &self.recovery_dir
    }

    /// Computes total bytes currently occupied by all snapshots in the recovery directory.
    pub fn current_storage_bytes(&self) -> io::Result<u64> {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(&self.recovery_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            }
        }
        Ok(total)
    }

    /// Atomically writes a durable recovery snapshot.
    pub fn write_snapshot(
        &self,
        key: SnapshotKey,
        rel_path: &str,
        content: &str,
        base_fingerprint: Option<&str>,
    ) -> Result<PathBuf, RecoveryError> {
        fs::create_dir_all(&self.recovery_dir)?;

        let current_storage = self.current_storage_bytes().unwrap_or(0);
        let estimated_size = content.len() as u64 + 512;
        if current_storage + estimated_size > self.max_storage_bytes as u64 {
            return Err(RecoveryError::StorageLimitExceeded(self.max_storage_bytes));
        }

        let mut hasher = std::hash::DefaultHasher::new();
        content.as_bytes().hash(&mut hasher);
        let checksum = hasher.finish();

        let header = format!(
            "TRUFF_SNAP_V1\nepoch:{}\nbuffer_id:{}\nrevision:{}\nrel_path:{}\nbase_fp:{}\ncontent_len:{}\nchecksum:{:016x}\n---DATA---\n",
            key.epoch.0,
            key.buffer_id.0,
            key.revision,
            rel_path,
            base_fingerprint.unwrap_or(""),
            content.len(),
            checksum
        );

        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let target_name = format!("{}_r{:08}.snap", key.buffer_id.0, key.revision);
        let target_path = self.recovery_dir.join(&target_name);
        let tmp_path = self
            .recovery_dir
            .join(format!(".{target_name}.{pid}_{id}.tmp"));

        let write_res = (|| -> Result<(), io::Error> {
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            file.write_all(header.as_bytes())?;
            file.write_all(content.as_bytes())?;
            file.sync_data()?;
            drop(file);

            fs::rename(&tmp_path, &target_path)?;

            #[cfg(unix)]
            {
                if let Ok(dir) = File::open(&self.recovery_dir) {
                    let _ = dir.sync_all();
                }
            }

            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(RecoveryError::Io(e));
        }

        Ok(target_path)
    }

    /// Recovers the latest valid snapshot for the given `BufferId`.
    ///
    /// If the latest snapshot is corrupted or truncated, skips it and attempts earlier snapshots.
    pub fn recover_latest_valid_snapshot(
        &self,
        buffer_id: BufferId,
    ) -> Result<RecoveredSnapshot, RecoveryError> {
        let entries = fs::read_dir(&self.recovery_dir)?;
        let prefix = format!("{}_r", buffer_id.0);

        let mut snapshot_files = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".snap") {
                snapshot_files.push(entry.path());
            }
        }

        // Sort descending so newest revision comes first
        snapshot_files.sort_by(|a, b| b.cmp(a));

        for snap_path in snapshot_files {
            if let Ok(snap) = self.try_read_snapshot(&snap_path) {
                return Ok(snap);
            }
        }

        Err(RecoveryError::NoValidSnapshot)
    }

    fn try_read_snapshot(&self, path: &Path) -> Result<RecoveredSnapshot, RecoveryError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim() != "TRUFF_SNAP_V1" {
            return Err(RecoveryError::CorruptedSnapshot("bad magic".to_string()));
        }

        let mut epoch = 0;
        let mut buffer_id = 0;
        let mut revision = 0;
        let mut rel_path = String::new();
        let mut base_fp = None;
        let mut content_len = 0;
        let mut checksum = 0;

        loop {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                return Err(RecoveryError::CorruptedSnapshot(
                    "unexpected EOF".to_string(),
                ));
            }
            let trimmed = line.trim();
            if trimmed == "---DATA---" {
                break;
            }

            let Some((k, v)) = trimmed.split_once(':') else {
                continue;
            };
            match k {
                "epoch" => epoch = v.parse().unwrap_or(0),
                "buffer_id" => buffer_id = v.parse().unwrap_or(0),
                "revision" => revision = v.parse().unwrap_or(0),
                "rel_path" => rel_path = v.to_string(),
                "base_fp" if !v.is_empty() => {
                    base_fp = Some(v.to_string());
                }
                "content_len" => content_len = v.parse().unwrap_or(0),
                "checksum" => checksum = u64::from_str_radix(v, 16).unwrap_or(0),
                _ => {}
            }
        }

        let mut content_bytes = Vec::with_capacity(content_len);
        reader.read_to_end(&mut content_bytes)?;

        if content_bytes.len() != content_len {
            return Err(RecoveryError::CorruptedSnapshot(format!(
                "length mismatch: expected {content_len}, got {}",
                content_bytes.len()
            )));
        }

        let mut hasher = std::hash::DefaultHasher::new();
        content_bytes.hash(&mut hasher);
        let actual_checksum = hasher.finish();

        if actual_checksum != checksum {
            return Err(RecoveryError::CorruptedSnapshot(format!(
                "checksum mismatch: expected {checksum:016x}, got {actual_checksum:016x}"
            )));
        }

        let content = String::from_utf8(content_bytes).map_err(|_| {
            RecoveryError::CorruptedSnapshot("content is not valid utf-8".to_string())
        })?;

        Ok(RecoveredSnapshot {
            key: SnapshotKey::new(VaultEpoch::new(epoch), BufferId::new(buffer_id), revision),
            rel_path,
            content,
            base_fingerprint: base_fp,
        })
    }

    /// Cleans up snapshots for a given buffer ID after successful save.
    pub fn delete_snapshots(&self, buffer_id: BufferId) -> Result<(), RecoveryError> {
        let entries = fs::read_dir(&self.recovery_dir)?;
        let prefix = format!("{}_r", buffer_id.0);
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".snap") {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }
}
