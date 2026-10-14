use super::{AffectedFileRewrite, FileOpError, FileOpPlan};
use crate::files::compute_fingerprint;
use crate::index::{FileId, IndexDatabase, Resolution, Target};
use crate::vault::VaultPaths;
use rusqlite::{OptionalExtension, params};
use std::{
    fs,
    path::{Path, PathBuf},
};
use truff_core::markdown::facts::MarkdownFacts;

impl FileOpPlan {
    pub(super) fn compute_rewrites(
        &mut self,
        vault: &VaultPaths,
        from: &Path,
        to: &Path,
        dirty_buffers: &[PathBuf],
        db: &IndexDatabase,
    ) -> Result<(), FileOpError> {
        self.rewrites.clear();
        let from_id: Option<i64> = db
            .connection()
            .prepare_cached("SELECT id FROM files WHERE relative_path = ?1")?
            .query_row([from.to_string_lossy()], |r| r.get(0))
            .optional()?;
        let Some(from_id) = from_id.map(FileId) else {
            return Ok(());
        };
        let backlinks = db
            .get_backlinks(from_id, None, 0, usize::MAX)
            .map_err(|e| FileOpError::InvalidOperation(e.to_string()))?;
        // Backlinks are already ordered by source path and occurrence ordinal.
        for occurrence in &backlinks.occurrences {
            if dirty_buffers
                .iter()
                .any(|path| path == Path::new(&occurrence.source_path))
            {
                return Err(FileOpError::DirtyBufferBlocked(
                    occurrence.source_path.clone().into(),
                ));
            }
        }
        let new_stem = to
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| to.to_str().unwrap_or(""));
        for group in backlinks
            .occurrences
            .chunk_by(|a, b| a.source_file_id == b.source_file_id)
        {
            let first = &group[0];
            let affected = PathBuf::from(&first.source_path);
            let abs_path = vault.resolve_path(&affected)?;
            let content = match fs::read_to_string(&abs_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let fingerprint: String = db
                .connection()
                .prepare_cached("SELECT fingerprint FROM files WHERE id = ?1")?
                .query_row(params![first.source_file_id.0], |r| r.get(0))?;
            let mut replacements = Vec::with_capacity(group.len());
            if compute_fingerprint(content.as_bytes()) == fingerprint {
                for occurrence in group {
                    replacements.push((
                        occurrence.range.start,
                        occurrence.range.end,
                        replacement(
                            to,
                            new_stem,
                            occurrence.is_wiki,
                            occurrence.heading.as_deref(),
                            occurrence.label.as_deref(),
                        ),
                    ));
                }
            } else {
                // External edits can invalidate indexed byte ranges. Preserve the
                // existing reparse/resolve behavior for a stale source document.
                let Ok(facts) = MarkdownFacts::extract(&content) else {
                    continue;
                };
                let resolver = db.resolver();
                for link in facts.links {
                    let target = Target::Note {
                        path_or_name: link.target,
                        heading_or_block: link.heading.clone(),
                        alias: None,
                    };
                    if matches!(resolver.resolve(first.source_file_id, &target), Resolution::Found(id, _) if id == from_id)
                    {
                        replacements.push((
                            link.range.start,
                            link.range.end,
                            replacement(
                                to,
                                new_stem,
                                link.is_wiki,
                                link.heading.as_deref(),
                                link.label.as_deref(),
                            ),
                        ));
                    }
                }
            }
            if !replacements.is_empty() {
                self.rewrites.push(AffectedFileRewrite {
                    rel_path: affected,
                    replacements,
                });
            }
        }
        Ok(())
    }
}

fn replacement(
    to: &Path,
    new_stem: &str,
    is_wiki: bool,
    heading: Option<&str>,
    label: Option<&str>,
) -> String {
    if is_wiki {
        let mut inner = new_stem.to_string();
        if let Some(heading) = heading {
            inner.push('#');
            inner.push_str(heading);
        }
        if let Some(label) = label {
            inner.push('|');
            inner.push_str(label);
        }
        format!("[[{inner}]]")
    } else {
        let mut target = to.to_string_lossy().into_owned();
        if let Some(heading) = heading {
            target.push('#');
            target.push_str(heading);
        }
        format!("[{}]({target})", label.unwrap_or(""))
    }
}
