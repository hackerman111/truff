use super::{FileOpError, FileOpKind, FileOpPlan};
use crate::files;
use crate::vault::VaultPaths;
use rusqlite::{Connection, params};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

impl FileOpPlan {
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
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent)?;
                }
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
