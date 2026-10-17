use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub use truff_core::text::DEFAULT_MAX_FILE_BYTES;

/// Errors occurring during file read or write operations.
#[derive(Debug)]
pub enum FileError {
    Io(io::Error),
    FileTooLarge {
        size: u64,
        limit: usize,
    },
    InvalidParentDir,
    Conflict {
        current_fingerprint: String,
        expected_fingerprint: String,
    },
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::FileTooLarge { size, limit } => {
                write!(f, "file size {size} exceeds limit of {limit} bytes")
            }
            Self::InvalidParentDir => {
                write!(f, "file parent directory does not exist or is invalid")
            }
            Self::Conflict {
                current_fingerprint,
                expected_fingerprint,
            } => write!(
                f,
                "conflict: current disk fingerprint {current_fingerprint} != expected {expected_fingerprint}"
            ),
        }
    }
}

impl std::error::Error for FileError {}

impl From<io::Error> for FileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// Computes a deterministic fingerprint string for the given byte slice.
pub fn compute_fingerprint(bytes: &[u8]) -> String {
    format!("{:x}:{}", Sha256::digest(bytes), bytes.len())
}

/// Computes a fingerprint string for a file existing on disk.
pub fn compute_file_fingerprint(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut size = 0usize;
    let mut chunk = [0u8; 8192];
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        size += read;
        if size > DEFAULT_MAX_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "fingerprint file exceeds size limit",
            ));
        }
        hasher.update(&chunk[..read]);
    }
    Ok(format!("{:x}:{size}", hasher.finalize()))
}

/// Reads a file into a `String`, strictly checking maximum file size limit.
pub fn read_file_with_limit(path: &Path, max_bytes: usize) -> Result<String, FileError> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    if size > max_bytes as u64 {
        return Err(FileError::FileTooLarge {
            size,
            limit: max_bytes,
        });
    }

    let mut bytes = Vec::with_capacity(size as usize);
    File::open(path)?
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(FileError::FileTooLarge {
            size: bytes.len() as u64,
            limit: max_bytes,
        });
    }
    let text = String::from_utf8(bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.utf8_error().to_string()))?;
    Ok(text)
}

/// Atomically writes content to a file.
///
/// Steps (§9.2):
/// 1. Write to temporary file in the same parent directory.
/// 2. Flush and sync file data and metadata to disk (`sync_all`).
/// 3. Atomic rename replacing the target file.
/// 4. Synchronize parent directory to persist directory entry on Unix.
///
/// Returns the computed fingerprint of the written content.
/// Atomically writes content from a rope to a file without full string allocation.
pub fn write_rope_atomically(
    path: &Path,
    rope: &truff_core::Rope,
    create_only: bool,
) -> Result<String, FileError> {
    write_chunks(path, rope.chunks(), create_only)
}

pub fn atomic_write_file(path: &Path, content: &str) -> Result<String, FileError> {
    write_chunks(path, [content], false)
}

/// Publishes a new file without replacing a racing existing destination.
pub fn atomic_create_file(path: &Path, content: &str) -> Result<String, FileError> {
    write_chunks(path, [content], true)
}

#[allow(dead_code)]
pub(crate) fn write_file_atomically(
    path: &Path,
    content: &str,
    create_only: bool,
) -> Result<String, FileError> {
    write_chunks(path, [content], create_only)
}

fn write_chunks<'a>(
    path: &Path,
    chunks: impl IntoIterator<Item = &'a str>,
    create_only: bool,
) -> Result<String, FileError> {
    let parent = path.parent().ok_or(FileError::InvalidParentDir)?;
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(FileError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "symlink path",
            )));
        }
    }
    if !parent.is_dir() {
        return Err(FileError::InvalidParentDir);
    }

    static TMP_COUNTER: AtomicU64 = AtomicU64::new(1);
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.md");

    let tmp_path = parent.join(format!(".{file_name}.{pid}_{counter}.tmp"));

    let mut hasher = Sha256::new();
    let mut total_len = 0usize;
    let write_res = (|| -> Result<(), io::Error> {
        let file = File::options()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;

        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.file_type().is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "symlink target",
                ));
            }
            file.set_permissions(meta.permissions())?;
        }
        #[cfg(test)]
        fault(WriteStage::Write)?;

        let mut writer = io::BufWriter::with_capacity(64 * 1024, file);
        for chunk in chunks {
            writer.write_all(chunk.as_bytes())?;
            hasher.update(chunk.as_bytes());
            total_len += chunk.len();
        }
        writer.flush()?;
        let file = writer.into_inner().map_err(|e| e.into_error())?;

        #[cfg(test)]
        fault(WriteStage::Sync)?;
        file.sync_all()?;
        drop(file);

        #[cfg(test)]
        fault(WriteStage::Rename)?;
        if create_only {
            fs::hard_link(&tmp_path, path)?;
            fs::remove_file(&tmp_path)?;
        } else {
            fs::rename(&tmp_path, path)?;
        }

        #[cfg(unix)]
        {
            #[cfg(test)]
            fault(WriteStage::DirectorySync)?;
            File::open(parent)?.sync_all()?;
        }

        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(FileError::Io(e));
    }

    Ok(format!("{:x}:{total_len}", hasher.finalize()))
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteStage {
    Write,
    Sync,
    Rename,
    DirectorySync,
}

#[cfg(test)]
thread_local! { static WRITE_FAULT: std::cell::Cell<Option<WriteStage>> = const { std::cell::Cell::new(None) }; }

#[cfg(test)]
pub(crate) fn inject_write_failure(stage: Option<WriteStage>) {
    WRITE_FAULT.set(stage);
}

#[cfg(test)]
pub(crate) fn fault(stage: WriteStage) -> io::Result<()> {
    if WRITE_FAULT.get() == Some(stage) {
        Err(io::Error::other("injected write failure"))
    } else {
        Ok(())
    }
}
