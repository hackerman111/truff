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
                    let target = if rewrite.rel_path == *from {
                        to
                    } else {
                        &rewrite.rel_path
                    };
                    let abs_target = vault.resolve_path(target)?;
                    if !abs_target.exists() {
                        continue;
                    }
                    let content = fs::read_to_string(&abs_target)?;
                    let updated = apply_replacements(&content, &rewrite.replacements)?;

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

fn apply_replacements(
    content: &str,
    replacements: &[(usize, usize, String)],
) -> Result<String, FileOpError> {
    let mut sorted: Vec<_> = replacements.iter().collect();
    sorted.sort_by_key(|replacement| replacement.0);
    // Repeated insertions at one offset retain the order of reverse application.
    for group in sorted.chunk_by_mut(|a, b| a.0 == b.0) {
        group.reverse();
    }
    let invalid = || FileOpError::InvalidOperation("invalid or overlapping rewrite ranges".into());
    let mut capacity = content.len();
    let mut previous_end = 0;
    for &&(start, end, ref text) in &sorted {
        if start < previous_end
            || start > end
            || !content.is_char_boundary(start)
            || !content.is_char_boundary(end)
        {
            return Err(invalid());
        }
        capacity = capacity
            .checked_sub(end - start)
            .and_then(|n| n.checked_add(text.len()))
            .ok_or_else(invalid)?;
        previous_end = end;
    }
    let mut updated = String::with_capacity(capacity);
    let mut copied = 0;
    for (start, end, text) in sorted {
        updated.push_str(&content[copied..*start]);
        updated.push_str(text);
        copied = *end;
    }
    updated.push_str(&content[copied..]);
    Ok(updated)
}

#[cfg(test)]
mod tests {
    use super::apply_replacements;

    #[test]
    fn replacements_use_original_byte_ranges_and_validate_unicode_and_overlap() {
        let changes = vec![(8, 9, "long".into()), (0, 2, "αβ".into())];
        assert_eq!(
            apply_replacements("é text x!", &changes).unwrap(),
            "αβ text long!"
        );
        assert_eq!(
            apply_replacements("ab", &[(1, 1, "x".into()), (1, 1, "y".into())]).unwrap(),
            "ayxb"
        );
        for changes in [
            vec![(1, 2, "x".into())],
            vec![(0, 4, "x".into()), (3, 4, "y".into())],
            vec![(0, 99, "x".into())],
        ] {
            assert!(apply_replacements("é text x!", &changes).is_err());
        }
    }
}
