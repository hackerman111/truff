use super::{AppResult, Application, global_config, path_completion};
use crate::shell::{Focus, Shell};
use std::path::Path;
use truff_core::commands::Action;
use truff_store::{Store, catalog::CatalogWorker};

impl Application {
    pub fn open_vault(&mut self, path: &Path) -> AppResult<()> {
        let path = path.canonicalize()?;
        if !path.is_dir() {
            return Err("Vault must be a directory".into());
        }
        if self.paths.is_some() {
            self.shutdown()?;
        }
        let mut store = Store::new(path.clone())?;
        if let Some(callback) = &self.storage_waker {
            store.set_event_waker(callback.clone());
        }
        self.watched_generation = None;
        let paths = store.paths().clone();
        let catalog = CatalogWorker::start(paths.clone())?;
        if let Some(callback) = &self.storage_waker {
            catalog.set_event_waker(callback.clone());
        }
        self.shell = Shell::new();
        self.shell.set_sidebar_available(true);
        self.shell.set_focus(Focus::Sidebar);
        self.store = Some(store);
        self.catalog = Some(catalog);
        self.paths = Some(paths);
        self.pending_saves.clear();
        self.pending_loads.clear();
        self.queued_reloads.clear();
        self.queued_recovery.clear();
        self.close_after_save.clear();
        self.backlinks.clear();
        self.restore_views.clear();
        self.initial_note_pending = true;
        self.catalog_indexing = true;
        self.pending_catalog = 0;
        self.pending_recovery.clear();
        self.recovery_deadline = None;
        self.quit_requested = false;
        self.restore_session()?;
        self.refresh_watch_targets();
        let mut config = global_config::GlobalConfig::load();
        config.record_vault(&path);
        Ok(())
    }

    pub fn try_auto_open_saved_vault(&mut self) -> AppResult<bool> {
        let config = global_config::GlobalConfig::load();
        let Some(last_vault) = config.last_vault else {
            return Ok(false);
        };
        if !last_vault.is_dir() {
            return Ok(false);
        }
        match self.open_vault(&last_vault) {
            Ok(()) => Ok(true),
            Err(e) => {
                self.status = format!("Could not open saved vault: {e}");
                Ok(false)
            }
        }
    }

    pub fn create_and_open_vault(
        &mut self,
        path: &Path,
        initial_note: Option<&str>,
    ) -> AppResult<()> {
        let expanded = path_completion::expand_path(&path.to_string_lossy());
        std::fs::create_dir_all(&expanded)?;
        let canonical = expanded.canonicalize()?;
        if let Some(note_name) = initial_note {
            let note_filename = if note_name.ends_with(".md") {
                note_name.to_string()
            } else {
                format!("{note_name}.md")
            };
            let note_path = canonical.join(&note_filename);
            if !note_path.exists() {
                let note_title = note_filename.trim_end_matches(".md");
                let starter = format!(
                    "# {note_title}\n\nДобро пожаловать в ваше новое хранилище Truff!\n\n- Нажмите `i`, чтобы перейти в режим вставки (Insert) и редактировать текст.\n- Нажмите `Esc`, чтобы вернуться в нормальный режим (Normal).\n- Создавайте новые заметки командой `:new <имя.md>`.\n"
                );
                std::fs::write(&note_path, starter)?;
            }
        }
        self.open_vault(&canonical)?;
        Ok(())
    }

    pub fn open_note(&mut self, path: &str) -> AppResult<()> {
        let paths = self.paths.as_ref().ok_or("Open a vault first")?;
        let absolute = paths.resolve_path(Path::new(path))?;
        // Reads must obey the same symlink boundary as file operations.
        let canonical = absolute.canonicalize()?;
        if !canonical.starts_with(paths.root()) {
            return Err("Note resolves outside the vault".into());
        }
        let effects = self
            .shell
            .workspace_mut()
            .dispatch(Action::OpenPath(absolute))?;
        self.shell.reset_editor_input();
        self.shell.set_focus(Focus::Editor);
        self.backlinks.clear();
        self.backlink_selection = 0;
        self.shell.set_right_panel_available(false);
        self.pending_location = None;
        self.apply_effects(effects)?;
        if let Some(path) = self.active_relative_path() {
            self.shell.select_sidebar_file(&path);
        }
        Ok(())
    }

    pub fn create_note(&mut self, path: &str) -> AppResult<()> {
        let paths = self.paths.as_ref().ok_or("Open a vault first")?;
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("File path cannot be empty".into());
        }
        let rel = if trimmed.ends_with(".md") {
            trimmed.to_string()
        } else {
            format!("{trimmed}.md")
        };
        let absolute = paths.resolve_path(Path::new(&rel))?;
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if !absolute.exists() {
            let title = absolute
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Note");
            let initial = format!("# {title}\n\n");
            std::fs::write(&absolute, initial)?;
        }
        let mut files = self
            .shell
            .sidebar_model()
            .all_items()
            .iter()
            .filter_map(|r| r.data.clone())
            .collect::<Vec<_>>();
        if !files.contains(&rel) {
            files.push(rel.clone());
            files.sort();
            self.shell.set_sidebar_files(files);
        }
        self.open_note(&rel)?;
        Ok(())
    }

    pub fn create_folder(&mut self, path: &str) -> AppResult<()> {
        let paths = self.paths.as_ref().ok_or("Open a vault first")?;
        let trimmed = path.trim().trim_matches(['/', '\\']);
        if trimmed.is_empty() {
            return Err("Folder path cannot be empty".into());
        }
        let absolute = paths.resolve_path(Path::new(trimmed))?;
        std::fs::create_dir_all(&absolute)?;
        Ok(())
    }
}
