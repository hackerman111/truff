use super::{ViewId, Workspace};
use crate::commands::AppError;
use crate::text::{BufferId, ChangeSet};

/// Bounded layout notification; ranges use source coordinates before/after this edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferChange {
    pub buffer_id: BufferId,
    pub revision: u64,
    /// Nonempty edit batches represented here. A revision gap requires a full rebuild.
    pub edit_batches: u64,
    pub old_range: std::ops::Range<usize>,
    pub new_range: std::ops::Range<usize>,
    /// Forces a full layout rebuild, such as after a disk reload.
    pub invalidate_suffix: bool,
}

impl BufferChange {
    fn merge(&mut self, next: Self) {
        let start = self.old_range.start;
        let old_end = self.old_range.end;
        let new_end = self.new_range.end;
        let inverse_start = if next.old_range.start <= start {
            next.old_range.start
        } else if next.old_range.start >= new_end {
            next.old_range.start - new_end + old_end
        } else {
            start
        };
        let inverse_end = if next.old_range.end < start {
            next.old_range.end
        } else if next.old_range.end >= new_end {
            next.old_range.end - new_end + old_end
        } else {
            old_end
        };
        let mapped_end = if new_end < next.old_range.start {
            new_end
        } else if new_end <= next.old_range.end {
            next.new_range.end
        } else {
            new_end - next.old_range.end + next.new_range.end
        };
        // The whole changed interval is invalid, but its unaffected suffix still
        // has an exact displacement across successive coordinate spaces.
        self.old_range = start.min(inverse_start)..old_end.max(inverse_end);
        self.new_range =
            self.new_range.start.min(next.new_range.start)..mapped_end.max(next.new_range.end);
        self.revision = next.revision;
        self.edit_batches = self.edit_batches.saturating_add(next.edit_batches);
        self.invalidate_suffix |= next.invalidate_suffix;
    }
}

impl Workspace {
    pub(super) fn notify_buffer_reload(&mut self, buffer_id: BufferId, old_len: usize) {
        let Some(buffer) = self.buffers.get_buffer(buffer_id) else {
            return;
        };
        let notice = BufferChange {
            buffer_id,
            revision: buffer.revision(),
            edit_batches: 1,
            old_range: 0..old_len,
            new_range: 0..buffer.byte_len(),
            invalidate_suffix: true,
        };
        if let Some(previous) = self
            .buffer_changes
            .iter_mut()
            .find(|n| n.buffer_id == buffer_id)
        {
            previous.merge(notice);
        } else {
            self.buffer_changes.push(notice);
        }
    }

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
        let revision = buffer.revision();
        for batch in changes.batches().filter(|batch| !batch.is_empty()) {
            let start = batch.iter().map(|c| c.old_range.start).min().unwrap_or(0);
            let old_end = batch.iter().map(|c| c.old_range.end).max().unwrap_or(start);
            let delta: isize = batch.iter().map(|c| c.delta()).sum();
            let notice = BufferChange {
                buffer_id,
                revision,
                edit_batches: 1,
                old_range: start..old_end,
                new_range: start..(old_end as isize + delta) as usize,
                invalidate_suffix: false,
            };
            if let Some(previous) = self
                .buffer_changes
                .iter_mut()
                .find(|n| n.buffer_id == buffer_id)
            {
                previous.merge(notice);
            } else {
                self.buffer_changes.push(notice);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_edits_preserve_unchanged_prefix_suffix_and_length_delta() {
        let before = "abcd";
        for a in 0..=before.len() {
            for b in a..=before.len() {
                for first in ["", "X", "YZ"] {
                    let mut middle = before.to_owned();
                    middle.replace_range(a..b, first);
                    for c in 0..=middle.len() {
                        for d in c..=middle.len() {
                            for second in ["", "P", "QR"] {
                                let mut after = middle.clone();
                                after.replace_range(c..d, second);
                                let make = |start, end, len| BufferChange {
                                    buffer_id: BufferId::new(1),
                                    revision: 1,
                                    edit_batches: 1,
                                    old_range: start..end,
                                    new_range: start..start + len,
                                    invalidate_suffix: false,
                                };
                                let mut notice = make(a, b, first.len());
                                notice.merge(make(c, d, second.len()));
                                assert_eq!(notice.edit_batches, 2);
                                assert_eq!(
                                    &before[..notice.old_range.start],
                                    &after[..notice.new_range.start]
                                );
                                assert_eq!(
                                    &before[notice.old_range.end..],
                                    &after[notice.new_range.end..]
                                );
                                assert_eq!(
                                    notice.new_range.len() as isize
                                        - notice.old_range.len() as isize,
                                    after.len() as isize - before.len() as isize
                                );
                                assert!(!notice.invalidate_suffix);
                            }
                        }
                    }
                }
            }
        }
    }
}
