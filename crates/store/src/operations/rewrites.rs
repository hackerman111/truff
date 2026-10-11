use super::{AffectedFileRewrite, FileOpError, FileOpPlan};
use crate::index::{FileId, IndexDatabase, Resolution, Target};
use crate::vault::VaultPaths;
use rusqlite::params;
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
}
