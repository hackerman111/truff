use super::FileOpError;
use crate::index::IndexDatabase;
use crate::vault::VaultPaths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Concrete kind of file operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileOpKind {
    CreateFile { path: PathBuf, content: String },
    CreateFolder { path: PathBuf },
    Rename { from: PathBuf, to: PathBuf },
    Move { from: PathBuf, to: PathBuf },
    Trash { path: PathBuf },
}

/// A planned rewrite for an affected note containing references to a renamed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedFileRewrite {
    pub rel_path: PathBuf,
    pub replacements: Vec<(usize, usize, String)>,
}

/// Multi-stage transactional plan for filesystem operations (Create, Rename, Move, Trash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpPlan {
    pub kind: FileOpKind,
    pub rewrites: Vec<AffectedFileRewrite>,
    pub id: Option<i64>,
}

impl FileOpPlan {
    /// Constructs a rename operation plan.
    pub fn rename(
        vault: &VaultPaths,
        from: impl Into<PathBuf>,
        to: impl Into<PathBuf>,
    ) -> Result<Self, FileOpError> {
        let from = from.into();
        let to = to.into();
        let _ = vault.resolve_path(&from)?;
        let _ = vault.resolve_path(&to)?;

        Ok(Self {
            kind: FileOpKind::Rename { from, to },
            rewrites: Vec::new(),
            id: None,
        })
    }

    /// Constructs a move operation plan.
    pub fn move_path(
        vault: &VaultPaths,
        from: impl Into<PathBuf>,
        to: impl Into<PathBuf>,
    ) -> Result<Self, FileOpError> {
        let from = from.into();
        let to = to.into();
        let _ = vault.resolve_path(&from)?;
        let _ = vault.resolve_path(&to)?;

        Ok(Self {
            kind: FileOpKind::Move { from, to },
            rewrites: Vec::new(),
            id: None,
        })
    }

    /// Constructs a trash operation plan.
    pub fn trash(vault: &VaultPaths, path: impl Into<PathBuf>) -> Result<Self, FileOpError> {
        let path = path.into();
        let _ = vault.resolve_path(&path)?;

        Ok(Self {
            kind: FileOpKind::Trash { path },
            rewrites: Vec::new(),
            id: None,
        })
    }

    /// Constructs a file creation plan.
    pub fn create_file(
        vault: &VaultPaths,
        path: impl Into<PathBuf>,
        content: String,
    ) -> Result<Self, FileOpError> {
        let path = path.into();
        let _ = vault.resolve_path(&path)?;

        Ok(Self {
            kind: FileOpKind::CreateFile { path, content },
            rewrites: Vec::new(),
            id: None,
        })
    }

    /// Constructs a folder creation plan.
    pub fn create_folder(
        vault: &VaultPaths,
        path: impl Into<PathBuf>,
    ) -> Result<Self, FileOpError> {
        let path = path.into();
        let _ = vault.resolve_path(&path)?;

        Ok(Self {
            kind: FileOpKind::CreateFolder { path },
            rewrites: Vec::new(),
            id: None,
        })
    }

    /// Validates the plan against filesystem state, symlink rules, and dirty buffer conflicts.
    pub fn validate(
        &mut self,
        vault: &VaultPaths,
        dirty_buffers: &[PathBuf],
        db: &IndexDatabase,
    ) -> Result<(), FileOpError> {
        match &self.kind {
            FileOpKind::CreateFile { path, .. } | FileOpKind::CreateFolder { path } => {
                let abs = vault.resolve_path(path)?;
                check_symlink(&abs)?;
                if abs.exists() {
                    return Err(FileOpError::DestinationAlreadyExists(path.clone()));
                }
            }
            FileOpKind::Trash { path } => {
                let abs = vault.resolve_path(path)?;
                check_symlink(&abs)?;
                if !abs.exists() {
                    return Err(FileOpError::SourceNotFound(path.clone()));
                }
                if dirty_buffers.iter().any(|d| d == path) {
                    return Err(FileOpError::DirtyBufferBlocked(path.clone()));
                }
            }
            FileOpKind::Rename { from, to } | FileOpKind::Move { from, to } => {
                let from_path = from.clone();
                let to_path = to.clone();
                let abs_from = vault.resolve_path(&from_path)?;
                let abs_to = vault.resolve_path(&to_path)?;
                check_symlink(&abs_from)?;
                check_symlink(&abs_to)?;

                if !abs_from.exists() {
                    return Err(FileOpError::SourceNotFound(from_path));
                }
                if abs_to.exists() {
                    return Err(FileOpError::DestinationAlreadyExists(to_path));
                }

                // If source itself has unsaved edits in memory, block the rename
                if dirty_buffers.iter().any(|d| d == &from_path) {
                    return Err(FileOpError::DirtyBufferBlocked(from_path));
                }

                // Determine affected occurrences via index
                self.compute_rewrites(vault, &from_path, &to_path, dirty_buffers, db)?;
            }
        }
        Ok(())
    }
}

/// Checks if a path or any of its symlink attributes violates security rules.
fn check_symlink(path: &Path) -> Result<(), FileOpError> {
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.is_symlink()
    {
        return Err(FileOpError::SymlinkForbidden(path.to_path_buf()));
    }
    Ok(())
}
