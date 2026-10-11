//! Application orchestration: one Shell/Workspace, storage jobs, and retained widgets.
mod commands;
pub mod global_config;
mod io;
mod navigation;
pub mod path_completion;
mod registered;
mod session;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use truff_core::commands::Action;
use truff_core::text::BufferId;
use truff_core::workspace::ViewId;
use truff_store::catalog::CatalogWorker;
use truff_store::index::BacklinkOccurrence;
use truff_store::session::SessionViewEntry;
use truff_store::{Store, VaultPaths};

use crate::shell::{Focus, Shell, ShellAction};

pub type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Application {
    pub shell: Shell,
    pub status: String,
    pub backlinks: Vec<BacklinkOccurrence>,
    pub backlink_selection: usize,
    pub quit_requested: bool,
    store: Option<Store>,
    catalog: Option<CatalogWorker>,
    paths: Option<VaultPaths>,
    pending_saves: HashSet<BufferId>,
    pending_loads: HashSet<BufferId>,
    queued_reloads: VecDeque<BufferId>,
    queued_recovery: VecDeque<BufferId>,
    close_after_save: HashMap<BufferId, ViewId>,
    restore_views: HashMap<PathBuf, SessionViewEntry>,
    pending_location: Option<(BufferId, navigation::Location)>,
    initial_note_pending: bool,
    file_operation_pending: bool,
    catalog_indexing: bool,
    pending_catalog: usize,
    pending_recovery: HashSet<truff_core::workspace::SnapshotKey>,
    recovery_deadline: Option<Instant>,
    projection: crate::editor::ProjectionMode,
    storage_waker: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    watched_generation: Option<u64>,
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

impl Application {
    pub fn new() -> Self {
        let mut shell = Shell::new();
        let config = global_config::GlobalConfig::load();
        shell.set_theme(config.resolve_theme());
        Self {
            shell,
            status: "Open a vault with :vault <path>".into(),
            backlinks: Vec::new(),
            backlink_selection: 0,
            quit_requested: false,
            store: None,
            catalog: None,
            paths: None,
            pending_saves: HashSet::new(),
            pending_loads: HashSet::new(),
            queued_reloads: VecDeque::new(),
            queued_recovery: VecDeque::new(),
            close_after_save: HashMap::new(),
            restore_views: HashMap::new(),
            pending_location: None,
            initial_note_pending: false,
            file_operation_pending: false,
            catalog_indexing: false,
            pending_catalog: 0,
            pending_recovery: HashSet::new(),
            recovery_deadline: None,
            projection: crate::editor::ProjectionMode::LivePreview,
            storage_waker: None,
            watched_generation: None,
        }
    }

    pub fn vault_path(&self) -> Option<&Path> {
        self.paths.as_ref().map(VaultPaths::root)
    }
    pub fn projection(&self) -> crate::editor::ProjectionMode {
        self.projection
    }
    pub fn needs_poll(&self) -> bool {
        self.catalog_indexing
            || self.pending_catalog > 0
            || !self.pending_recovery.is_empty()
            || !self.pending_saves.is_empty()
            || !self.pending_loads.is_empty()
            || !self.queued_reloads.is_empty()
            || !self.queued_recovery.is_empty()
            || self.recovery_deadline.is_some()
    }
    /// Storage wakeup for a native adapter. The callback is invoked for actual results only.
    pub fn set_storage_waker(&mut self, callback: std::sync::Arc<dyn Fn() + Send + Sync>) {
        self.storage_waker = Some(callback.clone());
        if let Some(store) = &mut self.store {
            store.set_event_waker(callback.clone());
        }
        if let Some(catalog) = &self.catalog {
            catalog.set_event_waker(callback);
        }
    }

    /// Deadline for a native timer; waking here submits an idle recovery checkpoint.
    pub fn recovery_deadline(&self) -> Option<Instant> {
        self.recovery_deadline
    }

    pub fn file_operation_pending(&self) -> bool {
        self.file_operation_pending
            || !self.pending_loads.is_empty()
            || !self.queued_reloads.is_empty()
    }

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

    pub fn active_path(&self) -> Option<&Path> {
        let id = self.shell.editor_view()?.buffer_id();
        self.shell.workspace().buffers().get(id)?.path()
    }

    pub fn active_relative_path(&self) -> Option<String> {
        self.paths
            .as_ref()?
            .relative_path(self.active_path()?)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    }

    pub fn handle_shell_action(&mut self, action: ShellAction) {
        let editor_edited = matches!(
            &action,
            ShellAction::EditorEdit
                | ShellAction::Core(Action::ApplyEdit { .. } | Action::Undo(_) | Action::Redo(_))
        );
        let result = match action {
            ShellAction::Core(action) => self.dispatch(action),
            ShellAction::Command(command) => self.execute_command(&command),
            ShellAction::RegisteredCommand { id, argument } => {
                self.registered_command(&id, &argument)
            }
            ShellAction::SidebarSelectFile(path) => self.open_note(&path),
            ShellAction::SidebarDeleteFile(path) => self.trash(&path),
            ShellAction::Error(error) => Err(error.into()),
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.status = error.to_string();
        }
        if editor_edited
            && self
                .shell
                .editor_buffer()
                .is_some_and(|buffer| buffer.is_dirty())
        {
            self.recovery_deadline = Some(Instant::now() + Duration::from_millis(750));
        } else if editor_edited {
            // Undo to a savepoint must clear a checkpoint before any later crash.
            self.recovery_deadline = Some(Instant::now());
        }
    }

    pub(crate) fn dispatch(&mut self, action: Action) -> AppResult<()> {
        let effects = self.shell.workspace_mut().dispatch(action)?;
        self.apply_effects(effects)
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown() {
            eprintln!("Could not persist Truff session: {error}");
        }
        // The catalog may still own accepted file operations; release its owner before the vault lock.
        self.catalog.take();
        self.store.take();
    }
}
