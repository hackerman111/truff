use std::collections::HashMap;
use std::path::Path;

use truff_core::markdown::facts::MarkdownFacts;

use crate::VaultPaths;
use crate::index::{FileId, IndexDatabase, Resolution, Target};
use crate::operations::FileOpPlan;

use super::{CatalogEvent, CatalogJob, CatalogReplies, scan};

pub(super) fn execute(
    job: CatalogJob,
    paths: &VaultPaths,
    db: &mut IndexDatabase,
    ids: &HashMap<String, FileId>,
    events: &CatalogReplies,
) -> Result<CatalogEvent, Box<dyn std::error::Error>> {
    match job {
        CatalogJob::RefreshNote { path } => {
            ids.get(&path).ok_or("Note is not indexed")?;
            scan::refresh(paths, db, events, &[path], &[])?;
            Ok(CatalogEvent::Ready)
        }
        CatalogJob::Backlinks { path, dirty } => {
            db.clear_dirty_overlays();
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
            db.clear_dirty_overlays();
            Ok(CatalogEvent::Backlinks {
                path,
                occurrences: backlinks?,
            })
        }
        CatalogJob::Follow { source, target } => {
            let id = *ids.get(&source).ok_or("Source note is not indexed")?;
            match db.resolver().resolve(id, &Target::parse(&target)) {
                Resolution::Found(id, anchor) => {
                    let path = db.file_path(id)?.ok_or("Resolved file is unavailable")?;
                    Ok(CatalogEvent::Follow { path, anchor })
                }
                Resolution::Missing => {
                    Err("Link target is missing; use :new <path.md> to create it".into())
                }
                _ => Err("Link target is ambiguous".into()),
            }
        }
        CatalogJob::Rename { from, to, dirty } => {
            let from = relative_path(paths, &from)?;
            let to = relative_path(paths, &to)?;
            let mut plan = FileOpPlan::rename(paths, &from, &to)?;
            plan.validate(paths, &dirty, db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Renamed {
                rewritten: plan
                    .rewrites
                    .iter()
                    .map(|rewrite| {
                        if rewrite.rel_path == Path::new(&from) {
                            to.clone().into()
                        } else {
                            rewrite.rel_path.clone()
                        }
                    })
                    .collect(),
                from,
                to,
            })
        }
        CatalogJob::Trash { path, dirty } => {
            let path = relative_path(paths, &path)?;
            let mut plan = FileOpPlan::trash(paths, &path)?;
            plan.validate(paths, &dirty, db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Trashed(path))
        }
        CatalogJob::Create { path } => {
            let path = relative_path(paths, &path)?;
            let mut plan = FileOpPlan::create_file(paths, &path, String::new())?;
            plan.validate(paths, &[], db)?;
            plan.stage(paths, db.connection_mut())?;
            plan.apply(paths, db.connection_mut())?;
            Ok(CatalogEvent::Created(path))
        }
    }
}

fn relative_path(paths: &VaultPaths, path: &str) -> Result<String, crate::vault::VaultError> {
    let absolute = paths.resolve_path(Path::new(path))?;
    Ok(paths
        .relative_path(&absolute)?
        .to_string_lossy()
        .into_owned())
}
