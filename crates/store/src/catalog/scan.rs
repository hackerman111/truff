use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use truff_core::markdown::facts::{DocFacts, MarkdownFacts};

use crate::VaultPaths;
use crate::files::{DEFAULT_MAX_FILE_BYTES, compute_fingerprint, read_file_with_limit};
use crate::index::{FileId, IndexDatabase};

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

pub(super) fn rebuild(
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    events: &CatalogReplies,
) -> Result<HashMap<String, FileId>, Box<dyn std::error::Error>> {
    let files = enumerate(paths.root())?;
    let _ = events.send(CatalogEvent::Files(files.clone()));
    let old_ids: Vec<FileId> = db
        .connection()
        .prepare("SELECT id FROM files")?
        .query_map([], |row| Ok(FileId(row.get(0)?)))?
        .collect::<Result<_, _>>()?;
    for id in old_ids {
        db.remove_file(id)?;
    }
    let mut ids = HashMap::with_capacity(files.len());
    // Register every target first so forward references resolve regardless of filename order.
    for (index, path) in files.iter().enumerate() {
        let id = FileId(index as i64 + 1);
        db.index_file(id, path, path, "", 0, &DocFacts::default(), "")?;
        ids.insert(path.clone(), id);
    }
    for path in &files {
        match read_file_with_limit(
            &paths.resolve_path(Path::new(path))?,
            DEFAULT_MAX_FILE_BYTES,
        ) {
            Ok(text) => {
                let facts = MarkdownFacts::extract(&text)?;
                db.index_file(
                    ids[path],
                    path,
                    path,
                    &compute_fingerprint(text.as_bytes()),
                    1,
                    &facts,
                    &text,
                )?;
            }
            Err(error) => {
                let _ = events.send(CatalogEvent::Error(format!("{}: {error}", path)));
            }
        }
    }
    Ok(ids)
}
