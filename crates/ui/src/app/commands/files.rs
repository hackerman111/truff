use super::super::{AppResult, Application};
use crate::shell::Focus;
use std::path::{Path, PathBuf};
use truff_core::commands::{Action, AppError};
use truff_core::workspace::BufferState;
use truff_store::catalog::CatalogJob;

impl Application {
    pub(super) fn rename_note(&mut self, argument: &str) -> AppResult<()> {
        let from = if self.shell.focus() == Focus::Sidebar {
            self.shell
                .sidebar_selected_path()
                .ok_or("Select a note first")?
        } else {
            self.active_relative_path().ok_or("Open a note first")?
        };
        self.rename_path(&from, argument)
    }

    pub(crate) fn rename_path(&mut self, from: &str, argument: &str) -> AppResult<()> {
        let is_note = self.is_note_operation_target(from);
        let is_folder = self.is_folder_operation_target(from);
        if !is_note && !is_folder {
            return Err(
                "Folder operations are unavailable; select an indexed note or existing folder"
                    .into(),
            );
        }
        if argument.is_empty() {
            return Err("Usage: :rename <new/path>".into());
        }
        let paths = self.paths.as_ref().ok_or("Open a vault first")?;
        let destination = paths.resolve_path(Path::new(argument))?;
        if is_note
            && self
                .shell
                .workspace()
                .buffers()
                .get_by_path(&destination)
                .is_some()
        {
            return Err("Destination is already tracked by the workspace".into());
        }
        if destination.exists() {
            return Err("Destination already exists".into());
        }
        self.start_file_operation(CatalogJob::Rename {
            from: from.to_owned(),
            to: argument.to_owned(),
            dirty: self.dirty_paths(),
        })
    }
    pub(crate) fn save(&mut self, close: bool) -> AppResult<()> {
        if self.file_operation_pending() {
            return Err("Wait for the file operation to finish".into());
        }
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let id = view.buffer_id();
        let view_id = view.id();
        if self.active_path().is_none() {
            return Err("Scratch has no path; open a vault and use :new name.md".into());
        }
        self.dispatch(Action::SaveBuffer(id))?;
        if close {
            self.close_after_save.insert(id, view_id);
        }
        Ok(())
    }

    pub fn save_all_dirty(&mut self) -> AppResult<()> {
        let dirty_buffers: Vec<_> = self
            .shell
            .workspace()
            .buffers()
            .entries()
            .filter(|entry| {
                entry.path().is_some()
                    && (entry.state() == BufferState::Conflict
                        || entry.buffer().is_some_and(|b| b.is_dirty()))
            })
            .map(|entry| (entry.id(), entry.state()))
            .collect();
        for (id, state) in dirty_buffers {
            if state == BufferState::Conflict {
                self.dispatch(Action::OverwriteBuffer(id))?;
            } else if let Err(err) = self.dispatch(Action::SaveBuffer(id)) {
                if err
                    .downcast_ref::<AppError>()
                    .is_some_and(|e| matches!(e, AppError::BufferConflict { .. }))
                    || err.to_string().contains("conflict")
                {
                    self.dispatch(Action::OverwriteBuffer(id))?;
                } else {
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn trash(&mut self, path: &str) -> AppResult<()> {
        if !self.is_note_operation_target(path) && !self.is_folder_operation_target(path) {
            return Err(
                "Folder operations are unavailable; select an indexed note or existing folder"
                    .into(),
            );
        }
        self.start_file_operation(CatalogJob::Trash {
            path: path.to_owned(),
            dirty: self.dirty_paths(),
        })
    }

    pub fn empty_trash(&mut self) -> AppResult<usize> {
        let Some(paths) = &self.paths else {
            return Err("Vault is not open".into());
        };
        let trash_dir = paths.trash_dir();
        if !trash_dir.exists() {
            self.status = "Корзина пуста".to_string();
            return Ok(0);
        }
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(trash_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if std::fs::remove_dir_all(&p).is_ok() {
                        count += 1;
                    }
                } else if std::fs::remove_file(&p).is_ok() {
                    count += 1;
                }
            }
        }
        self.status = format!("Корзина очищена (удалено объектов: {count})");
        Ok(count)
    }

    pub(crate) fn enqueue_file_operations(&mut self, mut jobs: Vec<CatalogJob>) -> AppResult<()> {
        if jobs.is_empty() {
            return Ok(());
        }
        if self.file_operation_pending() {
            for job in jobs {
                self.pending_file_operations.push_back(job);
            }
            return Ok(());
        }
        let first = jobs.remove(0);
        for job in jobs {
            self.pending_file_operations.push_back(job);
        }
        self.start_file_operation(first)
    }

    pub(crate) fn advance_file_operations(&mut self) -> AppResult<()> {
        if let Some(next) = self.pending_file_operations.pop_front() {
            self.start_file_operation(next)?;
        }
        Ok(())
    }

    pub(crate) fn trash_files(&mut self, paths: &[String]) -> AppResult<()> {
        let dirty = self.dirty_paths();
        let jobs: Vec<_> = paths
            .iter()
            .map(|p| CatalogJob::Trash {
                path: p.clone(),
                dirty: dirty.clone(),
            })
            .collect();
        self.enqueue_file_operations(jobs)
    }

    pub(crate) fn batch_rename(&mut self, renames: Vec<(String, String)>) -> AppResult<()> {
        let dirty = self.dirty_paths();
        let jobs: Vec<_> = renames
            .into_iter()
            .map(|(from, to)| CatalogJob::Rename {
                from,
                to,
                dirty: dirty.clone(),
            })
            .collect();
        self.enqueue_file_operations(jobs)
    }

    pub(crate) fn paste_files(
        &mut self,
        target_dir: &str,
        op: crate::navigator::ClipboardOp,
        paths: &[String],
    ) -> AppResult<()> {
        let vault_paths = self.paths.as_ref().ok_or("Open a vault first")?;
        let dirty = self.dirty_paths();
        let mut jobs = Vec::new();

        for src in paths {
            let src_path = Path::new(src);
            let file_name = src_path
                .file_name()
                .ok_or_else(|| format!("Invalid file path: {src}"))?;

            let mut dest_rel = if target_dir.is_empty() {
                PathBuf::from(file_name)
            } else {
                Path::new(target_dir).join(file_name)
            };

            // Prevent cyclic move: cannot move a directory into itself or its child
            if op == crate::navigator::ClipboardOp::Move
                && (src == dest_rel.to_str().unwrap_or("") || dest_rel.starts_with(src_path))
            {
                return Err(format!("Cannot move '{src}' into itself or its subfolder").into());
            }

            // For copy, if destination exists, auto-append copy suffix
            if op == crate::navigator::ClipboardOp::Copy {
                let mut counter = 1;
                while vault_paths
                    .resolve_path(&dest_rel)
                    .is_ok_and(|p| p.exists())
                {
                    let stem = src_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file");
                    let ext = src_path.extension().and_then(|s| s.to_str()).unwrap_or("");
                    let new_name = if ext.is_empty() {
                        if counter == 1 {
                            format!("{stem} (copy)")
                        } else {
                            format!("{stem} (copy {counter})")
                        }
                    } else if counter == 1 {
                        format!("{stem} (copy).{ext}")
                    } else {
                        format!("{stem} (copy {counter}).{ext}")
                    };
                    dest_rel = if target_dir.is_empty() {
                        PathBuf::from(new_name)
                    } else {
                        Path::new(target_dir).join(new_name)
                    };
                    counter += 1;
                }
            }

            let dest_str = dest_rel.to_string_lossy().into_owned();
            match op {
                crate::navigator::ClipboardOp::Move => {
                    jobs.push(CatalogJob::Rename {
                        from: src.clone(),
                        to: dest_str,
                        dirty: dirty.clone(),
                    });
                }
                crate::navigator::ClipboardOp::Copy => {
                    jobs.push(CatalogJob::Copy {
                        from: src.clone(),
                        to: dest_str,
                    });
                }
            }
        }

        self.enqueue_file_operations(jobs)
    }

    pub(crate) fn is_folder_operation_target(&self, path: &str) -> bool {
        let Some(paths) = &self.paths else {
            return false;
        };
        let Ok(full_path) = paths.resolve_path(Path::new(path)) else {
            return false;
        };
        full_path.is_dir()
    }

    pub(crate) fn is_note_operation_target(&self, path: &str) -> bool {
        if self.shell.sidebar_file_paths().any(|note| note == path) {
            return true;
        }
        let Some(paths) = &self.paths else {
            return false;
        };
        self.shell.workspace().buffers().entries().any(|entry| {
            entry.buffer().is_some()
                && entry
                    .path()
                    .and_then(|absolute| paths.relative_path(absolute).ok())
                    .is_some_and(|relative| relative == Path::new(path))
        })
    }

    pub(in crate::app) fn start_file_operation(&mut self, job: CatalogJob) -> AppResult<()> {
        if self.file_operation_pending()
            || !self.pending_saves.is_empty()
            || !self.pending_loads.is_empty()
        {
            return Err("Wait for pending file I/O to finish".into());
        }
        self.submit_catalog(job)?;
        // Freeze edits until apply + affected-buffer reload completes, avoiding dirty overwrite races.
        self.file_operation_pending = true;
        self.status = "Applying file operation…".into();
        Ok(())
    }

    pub(in crate::app) fn submit_catalog(&mut self, job: CatalogJob) -> AppResult<()> {
        self.catalog
            .as_ref()
            .ok_or("Open a vault first")?
            .try_submit(job)?;
        self.pending_catalog += 1;
        Ok(())
    }

    pub(super) fn dirty_paths(&self) -> Vec<PathBuf> {
        let Some(paths) = &self.paths else {
            return Vec::new();
        };
        self.shell
            .workspace()
            .buffers()
            .entries()
            .filter(|entry| entry.buffer().is_some_and(|buffer| buffer.is_dirty()))
            .filter_map(|entry| {
                paths
                    .relative_path(entry.path()?)
                    .ok()
                    .map(Path::to_path_buf)
            })
            .collect()
    }

    pub(super) fn dirty_texts(&self) -> Vec<(String, String)> {
        let Some(paths) = &self.paths else {
            return Vec::new();
        };
        self.shell
            .workspace()
            .buffers()
            .entries()
            .filter_map(|entry| {
                let buffer = entry.buffer()?;
                if !buffer.is_dirty() {
                    return None;
                }
                Some((
                    paths
                        .relative_path(entry.path()?)
                        .ok()?
                        .to_string_lossy()
                        .into_owned(),
                    buffer.as_str(),
                ))
            })
            .collect()
    }
}
