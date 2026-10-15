//! Application orchestration: one Shell/Workspace, storage jobs, and retained widgets.
mod commands;
pub(crate) use commands::command_parts;
mod completion;
mod document;
pub mod global_config;
mod io;
pub(crate) mod navigation;
pub mod path_completion;
mod registered;
mod retention;
mod selection;
mod session;
pub(crate) use selection::SelectionTarget;
mod vault;
mod views;

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

use crate::shell::{Shell, ShellAction};

pub type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

pub struct Application {
    pub shell: Shell,
    pub status: String,
    pub backlinks: Vec<BacklinkOccurrence>,
    pub quit_requested: bool,
    pub settings_requested: bool,
    pub focus_mode: bool,
    pub toc_requested: Option<String>,
    pub wiki_requested: Option<String>,
    pub stream_requested: Option<String>,
    pub link_completion: crate::overlays::LinkCompletionOverlay,
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
    closed_buffers: HashMap<BufferId, Option<truff_core::workspace::SnapshotKey>>,
    recovery_deadline: Option<Instant>,
    view_projections: HashMap<ViewId, crate::editor::ProjectionMode>,
    storage_waker: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    watched_generation: Option<u64>,
    view_history: views::ViewHistory,
    catalog_revision: u64,
    recent_vault_paths: Vec<PathBuf>,
    document_facts: Option<document::DocumentFacts>,
    recent_notes: VecDeque<String>,
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
        shell.set_typography(config.typography.clone());
        Self {
            shell,
            status: "Open a vault with :vault <path>".into(),
            backlinks: Vec::new(),
            quit_requested: false,
            settings_requested: false,
            focus_mode: false,
            toc_requested: None,
            wiki_requested: None,
            stream_requested: None,
            link_completion: crate::overlays::LinkCompletionOverlay::default(),
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
            closed_buffers: HashMap::new(),
            recovery_deadline: None,
            view_projections: HashMap::new(),
            storage_waker: None,
            watched_generation: None,
            view_history: views::ViewHistory::default(),
            catalog_revision: 0,
            recent_vault_paths: config.recent_vaults.clone(),
            document_facts: None,
            recent_notes: VecDeque::new(),
        }
    }

    pub fn vault_path(&self) -> Option<&Path> {
        self.paths.as_ref().map(VaultPaths::root)
    }
    pub fn projection(&self) -> crate::editor::ProjectionMode {
        self.shell
            .workspace()
            .active_view()
            .and_then(|id| self.view_projections.get(&id).copied())
            .unwrap_or(crate::editor::ProjectionMode::LivePreview)
    }
    pub(super) fn set_projection(&mut self, mode: crate::editor::ProjectionMode) {
        self.view_projections
            .retain(|id, _| self.shell.workspace().get_view(*id).is_some());
        if let Some(id) = self.shell.workspace().active_view() {
            self.view_projections.insert(id, mode);
        }
    }
    pub(crate) fn recent_notes(&self) -> impl Iterator<Item = &str> {
        self.recent_notes.iter().map(String::as_str)
    }

    pub(crate) fn record_recent_note(&mut self) {
        let Some(path) = self.active_relative_path() else {
            return;
        };
        if self.recent_notes.front() == Some(&path) {
            return;
        }
        self.recent_notes.retain(|existing| existing != &path);
        self.recent_notes.push_front(path);
        self.recent_notes.truncate(128);
    }

    pub(crate) fn focus_view(&mut self, id: ViewId) -> AppResult<()> {
        self.dispatch(Action::FocusView(id))?;
        self.shell.reset_editor_input();
        self.record_recent_note();
        Ok(())
    }

    pub(crate) fn catalog_revision(&self) -> u64 {
        self.catalog_revision
    }

    pub(crate) fn search_index_path(&self) -> Option<PathBuf> {
        if self.catalog_indexing {
            return None;
        }
        Some(self.paths.as_ref()?.truff_dir().join("index.db"))
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

    pub fn is_path_dirty(&self, rel_path: &str) -> bool {
        let Some(paths) = &self.paths else {
            return false;
        };
        let abs = path_completion::normalize_path(&paths.root().join(rel_path));
        if !abs.starts_with(paths.root()) {
            return false;
        }
        self.shell
            .workspace()
            .buffers()
            .get_by_path(&abs)
            .and_then(|id| self.shell.workspace().buffers().get_buffer(id))
            .is_some_and(|buffer| buffer.is_dirty())
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
