use super::super::{AppResult, Application};
use crate::shell::Focus;
use std::path::{Path, PathBuf};
use truff_core::commands::Action;
use truff_store::catalog::CatalogJob;

impl Application {
    pub(super) fn rename_note(&mut self, argument: &str) -> AppResult<()> {
        let from = if self.shell.focus() == Focus::Sidebar {
            self.shell
                .sidebar_model()
                .selected_row()
                .and_then(|row| row.data.clone())
                .ok_or("Select a note first")?
        } else {
            self.active_relative_path().ok_or("Open a note first")?
        };
        if argument.is_empty() {
            return Err("Usage: :rename <new/path.md>".into());
        }
        let paths = self.paths.as_ref().ok_or("Open a vault first")?;
        let destination = paths.resolve_path(Path::new(argument))?;
        if self
            .shell
            .workspace()
            .buffers()
            .get_by_path(&destination)
            .is_some()
        {
            return Err("Destination is already tracked by the workspace".into());
        }
        self.start_file_operation(CatalogJob::Rename {
            from,
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

    pub(crate) fn trash(&mut self, path: &str) -> AppResult<()> {
        self.start_file_operation(CatalogJob::Trash {
            path: path.to_owned(),
            dirty: self.dirty_paths(),
        })
    }

    pub(super) fn start_file_operation(&mut self, job: CatalogJob) -> AppResult<()> {
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
