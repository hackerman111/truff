use super::{ViewId, Workspace};
use crate::commands::AppError;
use crate::text::{BufferId, ChangeSet};

/// Bounded layout notification; ranges use source coordinates before/after this edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferChange {
    pub buffer_id: BufferId,
    pub revision: u64,
    pub old_range: std::ops::Range<usize>,
    pub new_range: std::ops::Range<usize>,
    /// Overflow/grouped history may require invalidation from this source offset onward.
    pub invalidate_suffix: bool,
}

impl Workspace {
    /// Drains at most the registry buffer limit of change summaries. No text snapshots are stored.
    pub fn take_buffer_changes(&mut self) -> Vec<BufferChange> {
        std::mem::take(&mut self.buffer_changes)
    }

    pub(super) fn notify_buffer_change(&mut self, buffer_id: BufferId, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        self.buffer_changes
            .retain(|notice| self.buffers.get_buffer(notice.buffer_id).is_some());
        let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
            return;
        };
        let start = changes
            .changes()
            .iter()
            .map(|c| c.old_range.start)
            .min()
            .unwrap_or(0);
        let old_end = changes
            .changes()
            .iter()
            .map(|c| c.old_range.end)
            .max()
            .unwrap_or(start);
        let new_end = changes.map_pos(old_end).max(start).min(buffer.byte_len());
        let grouped = changes.batches().count() > 1;
        let notice = BufferChange {
            buffer_id,
            revision: buffer.revision(),
            old_range: start..old_end,
            new_range: start.min(new_end)..new_end,
            invalidate_suffix: grouped,
        };
        // More than one pending edit has changing coordinate spaces: conservatively
        // invalidate the suffix from the earliest affected byte, still without scans.
        if let Some(previous) = self
            .buffer_changes
            .iter_mut()
            .find(|n| n.buffer_id == buffer_id)
        {
            let first = previous.old_range.start.min(start);
            previous.revision = notice.revision;
            previous.old_range.start = first;
            previous.old_range.end = previous.old_range.end.max(old_end);
            previous.new_range = first.min(buffer.byte_len())
                ..previous.new_range.end.max(new_end).min(buffer.byte_len());
            previous.invalidate_suffix = true;
        } else {
            self.buffer_changes.push(notice);
        }
    }

    /// Publishes already-applied editor changes. The initiating view (including its
    /// jump anchors) has already been rebased by the canonical Vim/Input executor.
    pub fn publish_editor_changes(
        &mut self,
        view_id: ViewId,
        changes: &[ChangeSet],
    ) -> Result<(), AppError> {
        let buffer_id = self
            .views
            .get(&view_id)
            .ok_or(AppError::ViewNotFound(view_id))?
            .buffer_id();
        for change in changes {
            for view in self.views.values_mut() {
                if view.buffer_id() == buffer_id && view.id() != view_id {
                    view.apply_change_set(change);
                }
            }
            self.notify_buffer_change(buffer_id, change);
        }
        self.refresh_buffer_state(buffer_id);
        Ok(())
    }
}
