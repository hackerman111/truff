use std::fmt;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
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
    let mut hasher = std::hash::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}:{}", hasher.finish(), bytes.len())
}

/// Computes a fingerprint string for a file existing on disk.
pub fn compute_file_fingerprint(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(compute_fingerprint(&bytes))
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

    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.utf8_error().to_string()))?;
    Ok(text)
}

/// Atomically writes content to a file.
///
/// Steps (§9.2):
/// 1. Write to temporary file in the same parent directory.
/// 2. Flush and sync file data to disk (`sync_data`).
/// 3. Atomic rename replacing the target file.
/// 4. Synchronize parent directory to persist directory entry on Unix.
///
/// Returns the computed fingerprint of the written content.
pub fn atomic_write_file(path: &Path, content: &str) -> Result<String, FileError> {
    let parent = path.parent().ok_or(FileError::InvalidParentDir)?;
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

    let write_res = (|| -> Result<(), io::Error> {
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;

        file.write_all(content.as_bytes())?;
        file.sync_data()?;
        drop(file);

        fs::rename(&tmp_path, path)?;

        #[cfg(unix)]
        {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }

        Ok(())
    })();

    if let Err(e) = write_res {
        let _ = fs::remove_file(&tmp_path);
        return Err(FileError::Io(e));
    }

    Ok(compute_fingerprint(content.as_bytes()))
}
