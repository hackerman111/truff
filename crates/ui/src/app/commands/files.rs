use super::super::{AppResult, Application};
use crate::shell::Focus;
use std::path::{Path, PathBuf};
use truff_core::commands::Action;
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
        if !self.is_note_operation_target(from) {
            return Err("Folder operations are unavailable; select an indexed or open note".into());
        }
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

    pub(crate) fn trash(&mut self, path: &str) -> AppResult<()> {
        if !self.is_note_operation_target(path) {
            return Err("Folder operations are unavailable; select an indexed or open note".into());
        }
        self.start_file_operation(CatalogJob::Trash {
            path: path.to_owned(),
            dirty: self.dirty_paths(),
        })
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
