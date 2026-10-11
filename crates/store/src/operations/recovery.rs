use super::{FileOpError, FileOpKind, FileOpPlan};
use crate::files;
use crate::vault::VaultPaths;
use rusqlite::{Connection, params};
use std::fs;

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
