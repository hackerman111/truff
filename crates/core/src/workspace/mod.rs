use std::collections::HashMap;
use std::path::PathBuf;

mod changes;
mod dispatch;
mod events;
mod protocol;
pub use changes::BufferChange;
pub use protocol::{StoreEvent, StoreJob};

pub mod buffers;
pub mod views;

pub use buffers::{BufferEntry, BufferRegistry, BufferState, SnapshotKey, VaultEpoch};
pub use views::{JumpHistory, View, ViewId};

use crate::commands::{AppError, Effect};
use crate::text::BufferId;

/// Default maximum number of open views in a workspace (§10.1).
pub const DEFAULT_MAX_VIEWS: usize = 32;

/// Central workspace managing buffer registry, active views, and vault epoch.
pub struct Workspace {
    epoch: VaultEpoch,
    buffers: BufferRegistry,
    views: HashMap<ViewId, View>,
    active_view: Option<ViewId>,
    max_views: usize,
    buffer_changes: Vec<BufferChange>,
}

impl Workspace {
    /// Creates a new workspace with epoch 0 and default limits.
    pub fn new() -> Self {
        Self::with_epoch(VaultEpoch::new(0))
    }

    /// Creates a workspace initialized with an explicit `VaultEpoch`.
    pub fn with_epoch(epoch: VaultEpoch) -> Self {
        Self {
            epoch,
            buffers: BufferRegistry::new(epoch),
            views: HashMap::new(),
            active_view: None,
            max_views: DEFAULT_MAX_VIEWS,
            buffer_changes: Vec::new(),
        }
    }

    /// Current vault epoch.
    pub fn epoch(&self) -> VaultEpoch {
        self.epoch
    }

    /// Advances to a new vault epoch, invalidating stale replies from the previous epoch.
    pub fn switch_vault(&mut self, epoch: VaultEpoch) {
        self.buffer_changes.clear();
        self.epoch = epoch;
        self.buffers.set_epoch(epoch);
    }

    /// Reference to the buffer registry.
    pub fn buffers(&self) -> &BufferRegistry {
        &self.buffers
    }

    /// Mutable reference to the buffer registry.
    pub fn buffers_mut(&mut self) -> &mut BufferRegistry {
        &mut self.buffers
    }

    /// Tracked views map.
    pub fn views(&self) -> &HashMap<ViewId, View> {
        &self.views
    }

    /// Returns a reference to a view by `ViewId`.
    pub fn get_view(&self, view_id: ViewId) -> Option<&View> {
        self.views.get(&view_id)
    }

    /// Returns a mutable reference to a view by `ViewId`.
    pub fn get_view_mut(&mut self, view_id: ViewId) -> Option<&mut View> {
        self.views.get_mut(&view_id)
    }

    /// Borrows the two independently owned parts of an editor input context.
    pub fn editor_parts_mut(
        &mut self,
        view_id: ViewId,
    ) -> Option<(&mut crate::text::Buffer, &mut View)> {
        let view = self.views.get_mut(&view_id)?;
        let buffer = self.buffers.get_buffer_mut(view.buffer_id())?;
        Some((buffer, view))
    }

    /// Synchronizes lifecycle state after Vim or an input adapter edits a resident buffer.
    pub fn refresh_buffer_state(&mut self, buffer_id: BufferId) {
        if let Some(entry) = self.buffers.get_mut(buffer_id)
            && entry.state() != BufferState::Conflict
            && let Some(buffer) = entry.buffer()
        {
            let state = if buffer.is_dirty() {
                BufferState::Dirty
            } else {
                BufferState::Clean
            };
            entry.set_state(state);
        }
    }

    /// Returns the active view ID, if any.
    pub fn active_view(&self) -> Option<ViewId> {
        self.active_view
    }

    /// Sets the active view.
    pub fn set_active_view(&mut self, view_id: Option<ViewId>) {
        self.active_view = view_id;
    }

    /// Opens a buffer populated from initial text.
    pub fn open_buffer_from_text(
        &mut self,
        path: PathBuf,
        text: &str,
    ) -> Result<BufferId, AppError> {
        self.buffers.open_from_text(Some(path), text)
    }

    /// Creates a new view attached to the given buffer ID.
    pub fn create_view(&mut self, buffer_id: BufferId) -> Result<ViewId, AppError> {
        if self.buffers.get(buffer_id).is_none() {
            return Err(AppError::BufferNotFound(buffer_id));
        }
        if self.views.len() >= self.max_views {
            return Err(AppError::MaxViewsExceeded(self.max_views));
        }

        let view_id = ViewId::next();
        let view = View::new(view_id, buffer_id);
        self.views.insert(view_id, view);
        self.active_view = Some(view_id);
        Ok(view_id)
    }

    /// Closes a view. Preserves any dirty buffer in the registry.
    pub fn close_view(&mut self, view_id: ViewId) -> Result<Vec<Effect>, AppError> {
        if self.views.remove(&view_id).is_none() {
            return Err(AppError::ViewNotFound(view_id));
        }

        if self.active_view == Some(view_id) {
            self.active_view = self.views.keys().next().copied();
        }

        Ok(vec![Effect::ViewClosed(view_id)])
    }

    /// Forcefully closes and removes a buffer from the registry.
    pub fn close_buffer_force(&mut self, buffer_id: BufferId) -> Result<(), AppError> {
        self.buffers.close_force(buffer_id)
    }

    /// Releases clean text and undo history while retaining its identity and disk metadata.
    /// The caller must first finish storage jobs and delete recovery snapshots for this buffer.
    pub fn unload_buffer(&mut self, buffer_id: BufferId) -> Result<bool, AppError> {
        if self
            .views
            .values()
            .any(|view| view.buffer_id() == buffer_id)
        {
            return Ok(false);
        }
        let unloaded = self.buffers.unload_clean(buffer_id)?;
        if unloaded {
            self.buffer_changes
                .retain(|change| change.buffer_id != buffer_id);
        }
        Ok(unloaded)
    }
}
impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

/// Pure source-level refactor plan describing an edit to the source buffer and creation of a target file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefactorPlan {
    pub source_transaction: crate::text::Transaction,
    pub target_path: PathBuf,
    pub extracted_content: String,
}

impl From<crate::markdown::link_selection::ExtractNotePlan> for RefactorPlan {
    fn from(plan: crate::markdown::link_selection::ExtractNotePlan) -> Self {
        Self {
            source_transaction: plan.source_transaction,
            target_path: plan.target_path,
            extracted_content: plan.extracted_content,
        }
    }
}
