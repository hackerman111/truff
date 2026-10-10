//! Application orchestration: one Shell/Workspace, storage jobs, and retained widgets.
mod commands;
mod io;
mod navigation;
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
        Self {
            shell: Shell::new(),
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
        if let Some(catalog) = &self.catalog { catalog.set_event_waker(callback); }
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
        if let Some(callback) = &self.storage_waker { catalog.set_event_waker(callback.clone()); }
        self.shell = Shell::new();
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
        self.status = "Reading vault…".into();
        self.restore_session()?;
        self.refresh_watch_targets();
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
        self.pending_location = None;
        self.apply_effects(effects)?;
        if let Some(path) = self.active_relative_path() {
            self.shell.select_sidebar_file(&path);
        }
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
            ShellAction::RegisteredCommand { id, argument } => self.registered_command(&id, &argument),
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
