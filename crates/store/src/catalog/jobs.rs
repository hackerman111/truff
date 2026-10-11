use std::collections::HashMap;
use std::path::Path;

use truff_core::markdown::facts::MarkdownFacts;

use crate::VaultPaths;
use crate::files::{DEFAULT_MAX_FILE_BYTES, compute_fingerprint, read_file_with_limit};
use crate::index::{FileId, IndexDatabase, Resolution, Target};
use crate::operations::FileOpPlan;

use super::{CatalogEvent, CatalogJob};

pub(super) fn execute(
    job: CatalogJob,
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    ids: &HashMap<String, FileId>,
) -> Result<CatalogEvent, Box<dyn std::error::Error>> {
    match job {
        CatalogJob::RefreshNote { path } => {
            let id = *ids.get(&path).ok_or("Note is not indexed")?;
            let text = read_file_with_limit(
                &paths.resolve_path(Path::new(&path))?,
                DEFAULT_MAX_FILE_BYTES,
            )?;
            db.clear_dirty_overlay(id);
            db.index_file(
                id,
                &path,
                &path,
                &compute_fingerprint(text.as_bytes()),
                1,
                &MarkdownFacts::extract(&text)?,
                &text,
            )?;
            Ok(CatalogEvent::Ready)
        }
        CatalogJob::Backlinks { path, dirty } => {
            for &id in ids.values() {
                db.clear_dirty_overlay(id);
            }
            let backlinks = (|| -> Result<_, Box<dyn std::error::Error>> {
                for (path, text) in dirty {
                    if let Some(&id) = ids.get(&path) {
                        db.set_dirty_overlay(id, path, MarkdownFacts::extract(&text)?);
                    }
                }
                let id = *ids.get(&path).ok_or("Note is not indexed")?;
                Ok(db.get_backlinks(id, None, 0, 100)?.occurrences)
            })();
            // Unsaved facts belong to this query; file manifests must validate against disk.
            for &id in ids.values() {
                db.clear_dirty_overlay(id);
            }
            Ok(CatalogEvent::Backlinks {
                path,
                occurrences: backlinks?,
            })
        }
        CatalogJob::Follow { source, target } => {
            let id = *ids.get(&source).ok_or("Source note is not indexed")?;
            match db.resolver().resolve(id, &Target::parse(&target)) {
                Resolution::Found(id, anchor) => {
                    let path = ids
                        .iter()
                        .find(|(_, file_id)| **file_id == id)
                        .map(|(path, _)| path.clone())
                        .ok_or("Resolved file is unavailable")?;
                    Ok(CatalogEvent::Follow { path, anchor })
                }
                Resolution::Missing => {
                    Err("Link target is missing; use :new <path.md> to create it".into())
                }
                _ => Err("Link target is ambiguous".into()),
            }
        }
        CatalogJob::Rename { from, to, dirty } => {
            let mut plan = FileOpPlan::rename(paths, &from, &to)?;
            plan.validate(paths, &dirty, db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Renamed {
                from,
                to,
                rewritten: plan
                    .rewrites
                    .iter()
                    .map(|rewrite| rewrite.rel_path.clone())
                    .collect(),
            })
        }
        CatalogJob::Trash { path, dirty } => {
            let mut plan = FileOpPlan::trash(paths, &path)?;
            plan.validate(paths, &dirty, db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Trashed(path))
        }
        CatalogJob::Create { path } => {
            let mut plan = FileOpPlan::create_file(paths, &path, String::new())?;
            plan.validate(paths, &[], db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Created(path))
        }
    }
}
