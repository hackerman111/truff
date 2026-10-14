use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use truff_core::markdown::facts::MarkdownFacts;

use crate::VaultPaths;
use crate::files::{DEFAULT_MAX_FILE_BYTES, compute_fingerprint, read_file_with_limit};
use crate::index::{FileId, IndexDatabase, update};

use super::{CatalogEvent, CatalogReplies};

fn enumerate(root: &Path) -> io::Result<Vec<String>> {
    let mut directories = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() || entry.file_name() == ".truff" {
                continue;
            }
            let path = entry.path();
            if kind.is_dir() {
                directories.push(path);
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                files.push(
                    path.strip_prefix(root)
                        .expect("enumerated path is under root")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn affected_files(
    paths: &VaultPaths,
    path: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let absolute = paths.resolve_path(Path::new(path))?;
    if absolute.is_dir() {
        Ok(enumerate(&absolute)?
            .into_iter()
            .map(|suffix| Path::new(path).join(suffix).to_string_lossy().into_owned())
            .collect())
    } else {
        Ok(vec![path.to_owned()])
    }
}

pub(super) fn rebuild(
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    events: &CatalogReplies,
) -> Result<HashMap<String, FileId>, Box<dyn std::error::Error>> {
    let files = enumerate(paths.root())?;
    let _ = events.send(CatalogEvent::Files(files.clone()));
    let existing = indexed_paths(db)?;
    let removed: Vec<_> = existing
        .iter()
        .filter(|(path, _)| files.binary_search(path).is_err())
        .map(|(_, &id)| id)
        .collect();
    refresh(paths, db, events, &files, &removed)?;
    Ok(indexed_paths(db)?)
}

pub(super) fn indexed_paths(db: &IndexDatabase) -> rusqlite::Result<HashMap<String, FileId>> {
    db.connection()
        .prepare_cached("SELECT relative_path, id FROM files")?
        .query_map([], |row| Ok((row.get(0)?, FileId(row.get(1)?))))?
        .collect()
}

/// Read only the selected files. A startup scan selects every path; file operations
/// select their own affected paths. All SQL mutations belong to one transaction.
pub(super) fn refresh(
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    events: &CatalogReplies,
    changed: &[String],
    removed: &[FileId],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut changed: Vec<_> = changed
        .iter()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .collect();
    changed.sort_unstable();
    changed.dedup();
    db.clear_dirty_overlays();
    let tx = db.connection_mut().transaction()?;
    let mut next_id: i64 =
        tx.query_row("SELECT coalesce(max(id), 0) FROM files", [], |r| r.get(0))?;
    for &id in removed {
        update::remove_document_in(&tx, id)?;
    }
    let mut updated = !removed.is_empty();
    // Register all targets before indexing any facts, preserving IDs of existing files.
    for &path in &changed {
        let exists: bool = tx
            .prepare_cached("SELECT EXISTS(SELECT 1 FROM files WHERE relative_path=?1)")?
            .query_row([path], |r| r.get(0))?;
        if !exists {
            next_id = next_id.checked_add(1).ok_or("File ID limit reached")?;
            update::register_document(&tx, FileId(next_id), path)?;
            updated = true;
        }
    }
    for &path in &changed {
        match read_file_with_limit(
            &paths.resolve_path(Path::new(path))?,
            DEFAULT_MAX_FILE_BYTES,
        ) {
            Ok(text) => {
                let (id, fingerprint, generation): (i64, String, i64) = tx.prepare_cached(
                    "SELECT id, fingerprint, index_generation FROM files WHERE relative_path=?1")?
                    .query_row([path], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                let actual = compute_fingerprint(text.as_bytes());
                if actual == fingerprint {
                    continue;
                }
                let generation = generation
                    .checked_add(1)
                    .ok_or("Index generation limit reached")?;
                update::index_document_in(
                    &tx,
                    FileId(id),
                    path,
                    path,
                    &actual,
                    generation as u64,
                    &MarkdownFacts::extract(&text)?,
                    &text,
                )?;
                updated = true;
            }
            Err(error) => {
                let _ = events.send(CatalogEvent::Error(format!("{path}: {error}")));
            }
        }
    }
    if updated {
        update::refresh_resolutions(&tx)?;
    }
    tx.commit()?;
    Ok(())
}
