use std::fmt;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use truff_core::markdown::facts::MarkdownFacts;
use truff_core::text::Transaction;

use crate::files::{self, FileError};
use crate::index::{FileId, IndexDatabase, Resolution, Target};
use crate::vault::{VaultError, VaultPaths};

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

/// Helper for constructing link creation transactions from editor selections.
pub struct LinkSelection;

impl LinkSelection {
    /// Creates a transaction replacing `range` with `[[target]]` or `[[target|label]]`.
    pub fn wikilink(range: Range<usize>, target: &str, label: Option<&str>) -> Transaction {
        let text = match label {
            Some(lbl) => format!("[[{target}|{lbl}]]"),
            None => format!("[[{target}]]"),
        };
        Transaction::replace(range, text)
    }

    /// Creates a transaction replacing `range` with `[label](target)`.
    pub fn markdown_link(range: Range<usize>, target: &str, label: &str) -> Transaction {
        let text = format!("[{label}]({target})");
        Transaction::replace(range, text)
    }
}

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

    fn compute_rewrites(
        &mut self,
        vault: &VaultPaths,
        from: &Path,
        to: &Path,
        dirty_buffers: &[PathBuf],
        db: &IndexDatabase,
    ) -> Result<(), FileOpError> {
        self.rewrites.clear();

        // 1. Find from_id in files table
        let from_str = from.to_string_lossy();
        let from_id_opt: Option<i64> = db
            .connection()
            .query_row(
                "SELECT id FROM files WHERE relative_path = ?1",
                params![from_str],
                |row| row.get(0),
            )
            .ok();

        let Some(from_id) = from_id_opt.map(FileId) else {
            return Ok(());
        };

        // 2. Query all files that might contain links to from_id
        let backlinks = db
            .get_backlinks(from_id, None, 0, usize::MAX)
            .map_err(|e| FileOpError::InvalidOperation(e.to_string()))?;

        // Group backlinks by source file path
        let mut affected_files: Vec<PathBuf> = Vec::new();
        for occ in &backlinks.occurrences {
            let path = PathBuf::from(&occ.source_path);
            if !affected_files.contains(&path) {
                affected_files.push(path);
            }
        }

        // 3. Check dirty buffers: if ANY affected file has unsaved edits, block!
        for affected in &affected_files {
            if dirty_buffers.iter().any(|d| d == affected) {
                return Err(FileOpError::DirtyBufferBlocked(affected.clone()));
            }
        }

        // 4. Compute replacement text for each affected file
        let new_stem = to
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| to.to_str().unwrap_or(""));

        for affected in affected_files {
            let abs_path = vault.resolve_path(&affected)?;
            if !abs_path.exists() {
                continue;
            }
            let content = match fs::read_to_string(&abs_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let facts = match MarkdownFacts::extract(&content) {
                Ok(f) => f,
                Err(_) => continue,
            };

            let resolver = db.resolver();
            let mut file_replacements = Vec::new();

            // Find source FileId for resolver context
            let source_id_opt: Option<i64> = db
                .connection()
                .query_row(
                    "SELECT id FROM files WHERE relative_path = ?1",
                    params![affected.to_string_lossy()],
                    |row| row.get(0),
                )
                .ok();
            let source_id = source_id_opt.map(FileId).unwrap_or(FileId(0));

            for link in &facts.links {
                let target = Target::Note {
                    path_or_name: link.target.clone(),
                    heading_or_block: link.heading.clone(),
                    alias: link.label.clone(),
                };

                // Check if target resolves to the renamed file
                match resolver.resolve(source_id, &target) {
                    Resolution::Found(target_id, _) if target_id == from_id => {
                        // Reconstruct link with new target name, preserving alias/heading and wiki format
                        let replacement_text = if link.is_wiki {
                            let mut inner = new_stem.to_string();
                            if let Some(ref h) = link.heading {
                                inner.push('#');
                                inner.push_str(h);
                            }
                            if let Some(ref lbl) = link.label {
                                inner.push('|');
                                inner.push_str(lbl);
                            }
                            format!("[[{inner}]]")
                        } else {
                            let label = link.label.as_deref().unwrap_or("");
                            let mut target_url = to.to_string_lossy().to_string();
                            if let Some(ref h) = link.heading {
                                target_url.push('#');
                                target_url.push_str(h);
                            }
                            format!("[{label}]({target_url})")
                        };

                        file_replacements.push((
                            link.range.start,
                            link.range.end,
                            replacement_text,
                        ));
                    }
                    _ => {
                        // Ambiguous or other targets are untouched
                    }
                }
            }

            if !file_replacements.is_empty() {
                self.rewrites.push(AffectedFileRewrite {
                    rel_path: affected,
                    replacements: file_replacements,
                });
            }
        }

        Ok(())
    }

    /// Stages the operation manifest in SQLite `operation_log`.
    pub fn stage(
        &mut self,
        _vault: &VaultPaths,
        conn: &mut Connection,
    ) -> Result<i64, FileOpError> {
        let op_type = match &self.kind {
            FileOpKind::CreateFile { .. } => "create_file",
            FileOpKind::CreateFolder { .. } => "create_folder",
            FileOpKind::Rename { .. } => "rename",
            FileOpKind::Move { .. } => "move",
            FileOpKind::Trash { .. } => "trash",
        };

        let payload = serde_json::to_string(&self)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT INTO operation_log (op_type, status, payload, created_at)
             VALUES (?1, 'staged', ?2, ?3)",
            params![op_type, payload, now],
        )?;

        let id = conn.last_insert_rowid();
        self.id = Some(id);
        Ok(id)
    }

    /// Executes the validated and staged filesystem operation atomically.
    pub fn apply(&self, vault: &VaultPaths, conn: &mut Connection) -> Result<(), FileOpError> {
        match &self.kind {
            FileOpKind::CreateFile { path, content } => {
                let abs = vault.resolve_path(path)?;
                files::atomic_write_file(&abs, content)?;
            }
            FileOpKind::CreateFolder { path } => {
                let abs = vault.resolve_path(path)?;
                fs::create_dir_all(&abs)?;
            }
            FileOpKind::Trash { path } => {
                let abs_src = vault.resolve_path(path)?;
                let trash_dest = vault.trash_dir().join(path);
                if let Some(parent) = trash_dest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&abs_src, &trash_dest)?;
            }
            FileOpKind::Rename { from, to } | FileOpKind::Move { from, to } => {
                let abs_from = vault.resolve_path(from)?;
                let abs_to = vault.resolve_path(to)?;
                if let Some(parent) = abs_to.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&abs_from, &abs_to)?;

                // Rewrite affected files on disk
                for rewrite in &self.rewrites {
                    let abs_target = vault.resolve_path(&rewrite.rel_path)?;
                    if !abs_target.exists() {
                        continue;
                    }
                    let content = fs::read_to_string(&abs_target)?;
                    let mut updated = content;

                    // Sort replacements in reverse order of byte start to avoid index drift
                    let mut sorted = rewrite.replacements.clone();
                    sorted.sort_by_key(|b| std::cmp::Reverse(b.0));

                    for (start, end, repl) in sorted {
                        if start <= updated.len() && end <= updated.len() && start <= end {
                            updated.replace_range(start..end, &repl);
                        }
                    }

                    files::atomic_write_file(&abs_target, &updated)?;
                }
            }
        }

        // Mark operation completed in operation_log
        if let Some(id) = self.id {
            conn.execute(
                "UPDATE operation_log SET status = 'completed' WHERE id = ?1",
                params![id],
            )?;
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

/// Recovers uncompleted staged operations from `operation_log` after a crash or interruption.
pub fn recover_pending_operations(
    vault: &VaultPaths,
    conn: &mut Connection,
) -> Result<usize, FileOpError> {
    let mut stmt = conn.prepare(
        "SELECT id, op_type, payload FROM operation_log WHERE status = 'staged' ORDER BY id ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let op_type: String = row.get(1)?;
        let payload: String = row.get(2)?;
        Ok((id, op_type, payload))
    })?;

    let mut pending = Vec::new();
    for item in rows {
        pending.push(item?);
    }

    let mut recovered_count = 0;
    for (id, _op_type, payload) in pending {
        let plan: FileOpPlan = match serde_json::from_str(&payload) {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Complete the operation based on current filesystem state
        match &plan.kind {
            FileOpKind::Trash { path } => {
                if let Ok(abs_src) = vault.resolve_path(path)
                    && abs_src.exists()
                {
                    let trash_dest = vault.trash_dir().join(path);
                    if let Some(parent) = trash_dest.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::rename(&abs_src, &trash_dest);
                }
            }
            FileOpKind::Rename { from, to } | FileOpKind::Move { from, to } => {
                if let Ok(abs_from) = vault.resolve_path(from)
                    && let Ok(abs_to) = vault.resolve_path(to)
                    && abs_from.exists()
                    && !abs_to.exists()
                {
                    if let Some(parent) = abs_to.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::rename(&abs_from, &abs_to);
                }
            }
            FileOpKind::CreateFile { path, content } => {
                if let Ok(abs) = vault.resolve_path(path)
                    && !abs.exists()
                {
                    let _ = files::atomic_write_file(&abs, content);
                }
            }
            FileOpKind::CreateFolder { path } => {
                if let Ok(abs) = vault.resolve_path(path) {
                    let _ = fs::create_dir_all(&abs);
                }
            }
        }

        conn.execute(
            "UPDATE operation_log SET status = 'completed' WHERE id = ?1",
            params![id],
        )?;
        recovered_count += 1;
    }

    Ok(recovered_count)
}
