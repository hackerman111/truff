use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use truff_core::markdown::facts::MarkdownFacts;

use crate::VaultPaths;
use crate::files::{DEFAULT_MAX_FILE_BYTES, compute_fingerprint, read_file_with_limit};
use crate::index::{FileId, IndexDatabase, resolve, update};

use super::{CatalogEvent, CatalogReplies};

#[derive(Debug, Clone)]
pub(super) struct ScannedFile {
    pub relative_path: String,
    pub mtime_ns: i64,
    pub file_size: i64,
}

fn enumerate(root: &Path) -> io::Result<Vec<ScannedFile>> {
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
                let metadata = entry.metadata().ok();
                let file_size = metadata.as_ref().map(|m| m.len() as i64).unwrap_or(-1);
                let mtime_ns = metadata
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
                    .unwrap_or(-1);

                files.push(ScannedFile {
                    relative_path: path
                        .strip_prefix(root)
                        .expect("enumerated path is under root")
                        .to_string_lossy()
                        .into_owned(),
                    mtime_ns,
                    file_size,
                });
            }
        }
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
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
            .map(|f| {
                Path::new(path)
                    .join(f.relative_path)
                    .to_string_lossy()
                    .into_owned()
            })
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
    let scanned_files = enumerate(paths.root())?;
    let paths_list: Vec<String> = scanned_files
        .iter()
        .map(|f| f.relative_path.clone())
        .collect();
    let _ = events.send(CatalogEvent::Files(paths_list.clone()));
    let existing = indexed_paths(db)?;
    let removed: Vec<_> = existing
        .iter()
        .filter(|(path, _)| paths_list.binary_search(path).is_err())
        .map(|(_, &id)| id)
        .collect();
    refresh_scanned(paths, db, events, &scanned_files, &removed)?;
    Ok(indexed_paths(db)?)
}

pub(super) fn indexed_paths(db: &IndexDatabase) -> rusqlite::Result<HashMap<String, FileId>> {
    db.connection()
        .prepare_cached("SELECT relative_path, id FROM files")?
        .query_map([], |row| Ok((row.get(0)?, FileId(row.get(1)?))))?
        .collect()
}

struct StoredFileMeta {
    id: i64,
    fingerprint: String,
    generation: i64,
    mtime_ns: i64,
    file_size: i64,
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
    let mut scanned = Vec::with_capacity(changed.len());
    for path in changed {
        if !Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        {
            continue;
        }
        let (mtime_ns, file_size) = match paths.resolve_path(Path::new(path)) {
            Ok(p) => match fs::metadata(&p) {
                Ok(m) => {
                    let size = m.len() as i64;
                    let mtime = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
                        .unwrap_or(-1);
                    (mtime, size)
                }
                Err(_) => (-1, -1),
            },
            Err(_) => (-1, -1),
        };
        scanned.push(ScannedFile {
            relative_path: path.clone(),
            mtime_ns,
            file_size,
        });
    }
    refresh_scanned(paths, db, events, &scanned, removed)
}

pub(super) fn refresh_scanned(
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    events: &CatalogReplies,
    scanned_files: &[ScannedFile],
    removed: &[FileId],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut scanned_files: Vec<_> = scanned_files
        .iter()
        .filter(|f| {
            Path::new(&f.relative_path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .cloned()
        .collect();
    scanned_files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    scanned_files.dedup_by(|a, b| a.relative_path == b.relative_path);

    db.clear_dirty_overlays();
    let tx = db.connection_mut().transaction()?;
    let mut next_id: i64 =
        tx.query_row("SELECT coalesce(max(id), 0) FROM files", [], |r| r.get(0))?;

    for &id in removed {
        update::remove_document_in(&tx, id)?;
    }

    // Preload all indexed files metadata in ONE single batch query
    let mut stored_files: HashMap<String, StoredFileMeta> = {
        let mut stmt = tx.prepare_cached(
            "SELECT relative_path, id, fingerprint, index_generation, mtime_ns, file_size FROM files",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                StoredFileMeta {
                    id: row.get(1)?,
                    fingerprint: row.get(2)?,
                    generation: row.get(3)?,
                    mtime_ns: row.get(4)?,
                    file_size: row.get(5)?,
                },
            ))
        })?;
        let mut map = HashMap::new();
        for r in rows {
            let (path, meta) = r?;
            map.insert(path, meta);
        }
        map
    };

    let mut topology_changed = !removed.is_empty();
    let mut modified_files = Vec::new();

    // Register all new targets before indexing any facts, preserving IDs of existing files
    for file in &scanned_files {
        if !stored_files.contains_key(&file.relative_path) {
            next_id = next_id.checked_add(1).ok_or("File ID limit reached")?;
            update::register_document(
                &tx,
                FileId(next_id),
                &file.relative_path,
                file.mtime_ns,
                file.file_size,
            )?;
            stored_files.insert(
                file.relative_path.clone(),
                StoredFileMeta {
                    id: next_id,
                    fingerprint: String::new(),
                    generation: 0,
                    mtime_ns: file.mtime_ns,
                    file_size: file.file_size,
                },
            );
            topology_changed = true;
        }
    }

    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0);
    // 10ms racy-clean window
    const RACY_CLEAN_WINDOW_NS: i64 = 10_000_000;

    // Load batch resolver cache if large batch (>= 16 changed files)
    let batch_cache = if scanned_files.len() >= 16 {
        resolve::BatchResolverCache::load(&tx).ok()
    } else {
        None
    };
    let resolver = match &batch_cache {
        Some(c) => resolve::Resolver::with_cache(&tx, c),
        None => resolve::Resolver::new(&tx),
    };

    for file in &scanned_files {
        let stored = match stored_files.get(&file.relative_path) {
            Some(s) => s,
            None => continue,
        };

        // Fast path: if mtime and file_size match and are outside racy-clean window, skip file read & SHA-256!
        if file.mtime_ns > 0
            && stored.mtime_ns == file.mtime_ns
            && stored.file_size == file.file_size
            && !stored.fingerprint.is_empty()
            && now_ns.saturating_sub(file.mtime_ns) > RACY_CLEAN_WINDOW_NS
        {
            continue;
        }

        let resolved = match paths.resolve_path(Path::new(&file.relative_path)) {
            Ok(p) => p,
            Err(error) => {
                let _ = events.send(CatalogEvent::Error(format!(
                    "{}: {error}",
                    file.relative_path
                )));
                continue;
            }
        };

        match read_file_with_limit(&resolved, DEFAULT_MAX_FILE_BYTES) {
            Ok(text) => {
                let actual = compute_fingerprint(text.as_bytes());
                if !stored.fingerprint.is_empty() && actual == stored.fingerprint {
                    // Fingerprint matched (touch or racy clean): just update mtime/size
                    if stored.mtime_ns != file.mtime_ns || stored.file_size != file.file_size {
                        let _ = update::update_file_meta(
                            &tx,
                            FileId(stored.id),
                            file.mtime_ns,
                            file.file_size,
                        );
                    }
                    continue;
                }
                let generation = stored
                    .generation
                    .checked_add(1)
                    .ok_or("Index generation limit reached")?;
                update::index_document_meta_with_resolver_in(
                    &tx,
                    FileId(stored.id),
                    &file.relative_path,
                    &file.relative_path,
                    &actual,
                    generation as u64,
                    file.mtime_ns,
                    file.file_size,
                    &MarkdownFacts::extract(&text)?,
                    &text,
                    Some(&resolver),
                )?;
                modified_files.push(FileId(stored.id));
            }
            Err(error) => {
                let _ = events.send(CatalogEvent::Error(format!(
                    "{}: {error}",
                    file.relative_path
                )));
            }
        }
    }

    if topology_changed {
        update::refresh_unresolved_links(&tx, &resolver)?;
    } else if !modified_files.is_empty() {
        for file_id in modified_files {
            update::refresh_resolutions_for_content(&tx, file_id)?;
        }
    }

    tx.commit()?;
    Ok(())
}
