use super::super::{AppResult, Application};
use crate::shell::Focus;
use std::path::Path;
use truff_store::catalog::CatalogEvent;

impl Application {
    pub(super) fn accept_catalog(&mut self, event: CatalogEvent) -> AppResult<()> {
        match event {
            CatalogEvent::Files(files) => {
                let first = files.first().cloned();
                self.shell.set_sidebar_files(files);
                if let Some(path) = self.active_relative_path() {
                    self.shell.select_sidebar_file(&path);
                }
                if self.initial_note_pending {
                    self.initial_note_pending = false;
                    if let Some(path) = first {
                        self.open_note(&path)?;
                    } else {
                        self.status = "Vault is empty. Create a note with :new <name.md>".into();
                    }
                }
            }
            CatalogEvent::Ready => {
                self.catalog_indexing = false;
                if self.status == "Reading vault…" {
                    self.status = "Vault ready".into();
                }
            }
            CatalogEvent::JobDone => {
                self.pending_catalog = self.pending_catalog.saturating_sub(1);
            }
            CatalogEvent::Backlinks { path, occurrences }
                if Some(path.as_str()) == self.active_relative_path().as_deref() =>
            {
                if occurrences.is_empty() {
                    self.backlinks = Vec::new();
                    self.backlink_selection = 0;
                    self.status = "No backlinks found".into();
                    self.shell.set_right_panel_available(false);
                    self.shell.set_focus(Focus::Editor);
                } else {
                    self.backlinks = occurrences;
                    self.backlink_selection = 0;
                    self.shell.set_right_panel_available(true);
                    self.shell.set_focus(Focus::RightPanel);
                }
            }
            CatalogEvent::Backlinks { .. } => {}
            CatalogEvent::Follow { path, anchor } => self.open_location(
                &path,
                anchor.map(super::super::navigation::Location::Anchor),
            )?,
            CatalogEvent::Renamed {
                from,
                to,
                rewritten,
            } => {
                self.file_operation_pending = false;
                let paths = self.paths.as_ref().ok_or("No vault")?;
                let from_abs = paths.resolve_path(Path::new(&from))?;
                let to_abs = paths.resolve_path(Path::new(&to))?;
                self.shell
                    .workspace_mut()
                    .buffers_mut()
                    .rename_path(&from_abs, to_abs)?;
                for relative in rewritten {
                    let absolute = paths.resolve_path(&relative)?;
                    if let Some(id) = self.shell.workspace().buffers().get_by_path(&absolute) {
                        self.queued_reloads.push_back(id);
                    }
                }
                self.status = format!("Renamed {from} → {to}");
            }
            CatalogEvent::Trashed(path) => {
                self.file_operation_pending = false;
                if let Some(paths) = &self.paths {
                    let absolute = paths.resolve_path(Path::new(&path))?;
                    if let Some(id) = self.shell.workspace().buffers().get_by_path(&absolute) {
                        let views: Vec<_> = self
                            .shell
                            .workspace()
                            .views()
                            .values()
                            .filter(|view| view.buffer_id() == id)
                            .map(|view| view.id())
                            .collect();
                        for view in views {
                            self.shell.workspace_mut().close_view(view)?;
                        }
                        self.shell.workspace_mut().close_buffer_force(id)?;
                    }
                }
                self.status = format!("Moved {path} to .truff/trash");
            }
            CatalogEvent::Created(path) => {
                self.file_operation_pending = false;
                self.open_note(&path)?;
            }
            CatalogEvent::Error(error) => {
                self.file_operation_pending = false;
                self.status = error;
            }
        }
        Ok(())
    }
}
