use std::fmt;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
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
    vault_root: Option<PathBuf>,
    max_storage_bytes: usize,
}

impl RecoveryManager {
    /// Creates a new `RecoveryManager` for a vault root directory.
    pub fn new(vault_dir: PathBuf, max_storage_bytes: usize) -> Self {
        let vault_root = vault_dir.canonicalize().ok();
        let recovery_dir = vault_root
            .as_ref()
            .unwrap_or(&vault_dir)
            .join(".truff/recovery");
        Self {
            recovery_dir,
            vault_root,
            max_storage_bytes,
        }
    }

    /// Path to the recovery directory.
    pub fn recovery_dir(&self) -> &Path {
        &self.recovery_dir
    }

    fn validate_internal_directory(&self) -> io::Result<()> {
        for path in self.recovery_dir.ancestors() {
            if fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "symlink recovery directory",
                ));
            }
            if self.vault_root.as_deref() == Some(path) {
                break;
            }
        }
        Ok(())
    }

    /// Computes total bytes currently occupied by all snapshots in the recovery directory.
    pub fn current_storage_bytes(&self) -> io::Result<u64> {
        let mut total = 0u64;
        self.validate_internal_directory()?;
        let entries = match fs::read_dir(&self.recovery_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        for entry in entries {
            total = total.saturating_add(entry?.metadata()?.len());
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
        self.validate_internal_directory()?;
        fs::create_dir_all(&self.recovery_dir)?;
        #[cfg(unix)]
        {
            // Existing paths can follow a failed creation sync. Retrying the
            // parents preserves durability without caching directory identities.
            let internal = self
                .recovery_dir
                .parent()
                .ok_or_else(|| io::Error::other("missing recovery parent"))?;
            File::open(internal)?.sync_all()?;
            if let Some(root) = internal.parent() {
                File::open(root)?.sync_all()?;
            }
        }

        if rel_path.contains(['\n', '\r']) || content.len() > crate::files::DEFAULT_MAX_FILE_BYTES {
            return Err(RecoveryError::CorruptedSnapshot(
                "invalid path or oversized content".into(),
            ));
        }
        let checksum = crate::files::compute_fingerprint(content.as_bytes());
        let header = format!(
            "TRUFF_SNAP_V2\nepoch:{}\nbuffer_id:{}\nrevision:{}\nrel_path:{}\nbase_fp:{}\ncontent_len:{}\nchecksum:{}\n---DATA---\n",
            key.epoch.0,
            key.buffer_id.0,
            key.revision,
            rel_path,
            base_fingerprint.unwrap_or(""),
            content.len(),
            checksum
        );
        if header.len() > 16 * 1024 {
            return Err(RecoveryError::CorruptedSnapshot(
                "snapshot header exceeds size limit".into(),
            ));
        }
        let target_name = format!(
            "{}_r{:020}_e{:020}.snap",
            key.buffer_id.0, key.revision, key.epoch.0
        );
        let target_path = self.recovery_dir.join(&target_name);
        if let Ok(existing) = self.try_read_snapshot(&target_path)
            && existing.key == key
            && existing.rel_path == rel_path
            && existing.content == content
            && existing.base_fingerprint.as_deref() == base_fingerprint
        {
            File::open(&target_path)?.sync_all()?;
            #[cfg(unix)]
            File::open(&self.recovery_dir)?.sync_all()?;
            return Ok(target_path);
        }
        let size = header.len() + content.len();
        // Never evict the sole valid unsaved copy to admit a new checkpoint.
        // With two durable copies it is safe to retire the older one first.
        let mut valid = self.valid_snapshots(key.buffer_id)?;
        valid.sort_by_key(|(_, key)| (key.epoch, key.revision));
        let mut storage = self.current_storage_bytes()?;
        while storage.saturating_add(size as u64) > self.max_storage_bytes as u64 && valid.len() > 1
        {
            let (oldest, _) = valid.remove(0);
            let old_size = fs::metadata(&oldest)?.len();
            fs::remove_file(oldest)?;
            storage = storage.saturating_sub(old_size);
        }
        if storage.saturating_add(size as u64) > self.max_storage_bytes as u64 {
            return Err(RecoveryError::StorageLimitExceeded(self.max_storage_bytes));
        }

        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let tmp_path = self
            .recovery_dir
            .join(format!(".{target_name}.{pid}_{id}.tmp"));

        let write_res = (|| -> Result<(), io::Error> {
            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(&tmp_path)?;
            #[cfg(test)]
            crate::files::fault(crate::files::WriteStage::Write)?;
            file.write_all(header.as_bytes())?;
            file.write_all(content.as_bytes())?;
            file.flush()?;
            #[cfg(test)]
            crate::files::fault(crate::files::WriteStage::Sync)?;
            file.sync_all()?;
            drop(file);

            #[cfg(test)]
            crate::files::fault(crate::files::WriteStage::Rename)?;
            fs::rename(&tmp_path, &target_path)?;

            #[cfg(unix)]
            {
                #[cfg(test)]
                crate::files::fault(crate::files::WriteStage::DirectorySync)?;
                File::open(&self.recovery_dir)?.sync_all()?;
            }

            Ok(())
        })();

        if let Err(e) = write_res {
            let _ = fs::remove_file(&tmp_path);
            return Err(RecoveryError::Io(e));
        }

        valid.retain(|(path, _)| path != &target_path);
        valid.push((target_path.clone(), key));
        valid.sort_by_key(|(_, key)| (key.epoch, key.revision));
        let mut removed = false;
        while valid.len() > 2 {
            fs::remove_file(valid.remove(0).0)?;
            removed = true;
        }
        // Invalid remnants cannot serve as the previous copy and must not consume the budget.
        let prefix = format!("{}_r", key.buffer_id.0);
        for entry in fs::read_dir(&self.recovery_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix)
                && name.ends_with(".snap")
                && !valid.iter().any(|(path, _)| *path == entry.path())
            {
                fs::remove_file(entry.path())?;
                removed = true;
            }
        }
        #[cfg(unix)]
        if removed {
            // The rename is already durable. A second directory sync is needed
            // only when cleanup subsequently changed directory entries.
            File::open(&self.recovery_dir)?.sync_all()?;
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
        self.valid_snapshots(buffer_id)?
            .into_iter()
            .max_by_key(|(_, key)| (key.epoch, key.revision))
            .map(|(path, _)| self.try_read_snapshot(&path))
            .unwrap_or(Err(RecoveryError::NoValidSnapshot))
    }

    fn valid_snapshots(
        &self,
        buffer_id: BufferId,
    ) -> Result<Vec<(PathBuf, SnapshotKey)>, RecoveryError> {
        self.validate_internal_directory()?;
        let mut valid = Vec::new();
        let prefix = format!("{}_r", buffer_id.0);
        for entry in fs::read_dir(&self.recovery_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix)
                && name.ends_with(".snap")
                && entry.file_type()?.is_file()
                && let Ok(snapshot) = self.try_read_snapshot(&entry.path())
                && snapshot.key.buffer_id == buffer_id
            {
                valid.push((entry.path(), snapshot.key));
            }
        }
        Ok(valid)
    }

    fn try_read_snapshot(&self, path: &Path) -> Result<RecoveredSnapshot, RecoveryError> {
        let invalid =
            || RecoveryError::CorruptedSnapshot("invalid snapshot header or checksum".into());
        let limit = self
            .max_storage_bytes
            .min(crate::files::DEFAULT_MAX_FILE_BYTES + 16 * 1024);
        let file = File::open(path)?;
        if file.metadata()?.len() > limit as u64 {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err(invalid());
        }
        let delimiter = b"---DATA---\n";
        let header_end = bytes
            .windows(delimiter.len())
            .position(|window| window == delimiter)
            .ok_or_else(invalid)?;
        if header_end > 16 * 1024 {
            return Err(invalid());
        }
        let header = std::str::from_utf8(&bytes[..header_end]).map_err(|_| invalid())?;
        let mut lines = header.lines();
        let magic = lines.next().ok_or_else(invalid)?;
        if !matches!(magic, "TRUFF_SNAP_V1" | "TRUFF_SNAP_V2") {
            return Err(invalid());
        }
        let mut epoch = None;
        let mut buffer_id = None;
        let mut revision = None;
        let mut rel_path = None;
        let mut base_fp = None;
        let mut length = None;
        let mut checksum = None;
        for line in lines {
            let (name, value) = line.split_once(':').ok_or_else(invalid)?;
            match name {
                "epoch" => epoch = Some(value.parse::<u64>().map_err(|_| invalid())?),
                "buffer_id" => buffer_id = Some(value.parse::<u64>().map_err(|_| invalid())?),
                "revision" => revision = Some(value.parse::<u64>().map_err(|_| invalid())?),
                "rel_path" => rel_path = Some(value.to_owned()),
                "base_fp" if !value.is_empty() => base_fp = Some(value.to_owned()),
                "base_fp" => {}
                "content_len" => length = Some(value.parse::<usize>().map_err(|_| invalid())?),
                "checksum" => checksum = Some(value),
                _ => return Err(invalid()),
            }
        }
        let content_bytes = &bytes[header_end + delimiter.len()..];
        if Some(content_bytes.len()) != length {
            return Err(invalid());
        }
        let checksum = checksum.ok_or_else(invalid)?;
        if magic == "TRUFF_SNAP_V2" {
            if crate::files::compute_fingerprint(content_bytes) != checksum {
                return Err(invalid());
            }
        } else {
            let mut hasher = std::hash::DefaultHasher::new();
            content_bytes.hash(&mut hasher);
            if u64::from_str_radix(checksum, 16).ok() != Some(hasher.finish()) {
                return Err(invalid());
            }
        }
        let content = String::from_utf8(content_bytes.to_vec()).map_err(|_| invalid())?;
        Ok(RecoveredSnapshot {
            key: SnapshotKey::new(
                VaultEpoch::new(epoch.ok_or_else(invalid)?),
                BufferId::new(buffer_id.ok_or_else(invalid)?),
                revision.ok_or_else(invalid)?,
            ),
            rel_path: rel_path.ok_or_else(invalid)?,
            content,
            base_fingerprint: base_fp,
        })
    }

    /// Cleans up snapshots for a given buffer ID after successful save.
    pub fn delete_snapshots(&self, buffer_id: BufferId) -> Result<(), RecoveryError> {
        self.validate_internal_directory()?;
        let entries = match fs::read_dir(&self.recovery_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let prefix = format!("{}_r", buffer_id.0);
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(&prefix) && name_str.ends_with(".snap") {
                fs::remove_file(entry.path())?;
            }
        }
        #[cfg(unix)]
        File::open(&self.recovery_dir)?.sync_all()?;
        Ok(())
    }

    /// Recovers all latest valid snapshots across all buffers in the recovery directory.
    pub fn recover_all_latest_snapshots(&self) -> Result<Vec<RecoveredSnapshot>, RecoveryError> {
        self.validate_internal_directory()?;
        let entries = match fs::read_dir(&self.recovery_dir) {
            Ok(e) => e,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(RecoveryError::Io(err)),
        };

        let mut buffer_files: std::collections::HashMap<u64, Vec<PathBuf>> =
            std::collections::HashMap::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.ends_with(".snap")
                && let Some((buf_str, _)) = name_str.split_once("_r")
                && let Ok(buf_id) = buf_str.parse::<u64>()
            {
                buffer_files.entry(buf_id).or_default().push(entry.path());
            }
        }

        let mut recovered = Vec::new();
        for (_buf_id, mut files) in buffer_files {
            if let Some(snapshot) = files
                .drain(..)
                .filter_map(|path| self.try_read_snapshot(&path).ok())
                .max_by_key(|snapshot| (snapshot.key.epoch, snapshot.key.revision))
            {
                recovered.push(snapshot);
            }
        }

        Ok(recovered)
    }
}
