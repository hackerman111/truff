use crate::files::FileError;
use crate::vault::VaultError;
use std::path::PathBuf;
use std::{fmt, io};

/// Errors occurring during file operation planning, validation, staging, or execution.
#[derive(Debug)]
pub enum FileOpError {
    Io(io::Error),
    Vault(VaultError),
    File(FileError),
    Sqlite(rusqlite::Error),
    Serialization(serde_json::Error),
    SourceNotFound(PathBuf),
    DestinationAlreadyExists(PathBuf),
    SymlinkForbidden(PathBuf),
    DirtyBufferBlocked(PathBuf),
    InvalidOperation(String),
}

impl fmt::Display for FileOpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "file operation I/O error: {e}"),
            Self::Vault(e) => write!(f, "vault path error: {e}"),
            Self::File(e) => write!(f, "file error: {e}"),
            Self::Sqlite(e) => write!(f, "sqlite error: {e}"),
            Self::Serialization(e) => write!(f, "serialization error: {e}"),
            Self::SourceNotFound(p) => write!(f, "source file not found: {}", p.display()),
            Self::DestinationAlreadyExists(p) => {
                write!(f, "destination already exists: {}", p.display())
            }
            Self::SymlinkForbidden(p) => {
                write!(
                    f,
                    "symlinks are forbidden in vault operations: {}",
                    p.display()
                )
            }
            Self::DirtyBufferBlocked(p) => {
                write!(
                    f,
                    "affected file has unsaved changes in editor: {}",
                    p.display()
                )
            }
            Self::InvalidOperation(msg) => write!(f, "invalid file operation: {msg}"),
        }
    }
}

impl std::error::Error for FileOpError {}

impl From<io::Error> for FileOpError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<VaultError> for FileOpError {
    fn from(e: VaultError) -> Self {
        Self::Vault(e)
    }
}

impl From<FileError> for FileOpError {
    fn from(e: FileError) -> Self {
        Self::File(e)
    }
}

impl From<rusqlite::Error> for FileOpError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e)
    }
}

impl From<serde_json::Error> for FileOpError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e)
    }
}
