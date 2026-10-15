use super::{AppResult, Application};
use crate::editor::ProjectionMode;
use std::collections::{HashMap, HashSet, VecDeque};
use truff_core::{Action, BufferId, BytePos, Effect, ViewId, workspace::StoreJob};

struct ClosedView {
    buffer: BufferId,
    anchor: usize,
    head: usize,
    top: usize,
    projection: ProjectionMode,
}

#[derive(Default)]
pub(super) struct ViewHistory {
    pinned: HashSet<ViewId>,
    closed: VecDeque<ClosedView>,
    restoring: HashMap<ViewId, ClosedView>,
}

impl Application {
    pub(crate) fn view_is_pinned(&self, id: ViewId) -> bool {
        self.view_history.pinned.contains(&id)
    }

    pub(crate) fn toggle_view_pin(&mut self, id: ViewId) {
        if self.shell.workspace().get_view(id).is_none() {
            return;
        }
        if !self.view_history.pinned.remove(&id) {
            self.view_history.pinned.insert(id);
        }
    }

    pub(crate) fn close_view(&mut self, id: ViewId) -> AppResult<()> {
        let view = self
            .shell
            .workspace()
            .get_view(id)
            .ok_or("View was closed")?;
        let saved = ClosedView {
            buffer: view.buffer_id(),
            anchor: view.anchor().get(),
            head: view.head().get(),
            top: view.scroll_top_line(),
            projection: self
                .view_projections
                .get(&id)
                .copied()
                .unwrap_or(ProjectionMode::LivePreview),
        };
        self.dispatch(Action::CloseView(id))?;
        self.quit_requested = false;
        self.view_history.pinned.remove(&id);
        self.view_history.restoring.remove(&id);
        self.view_projections.remove(&id);
        if self.view_history.closed.len() == 32 {
            self.view_history.closed.pop_front();
        }
        self.view_history.closed.push_back(saved);
        Ok(())
    }

    pub(crate) fn close_other_views(&mut self, keep: ViewId) -> AppResult<()> {
        if self.shell.workspace().get_view(keep).is_none() {
            return Err("View was closed".into());
        }
        let others: Vec<_> = self
            .shell
            .workspace()
            .views()
            .keys()
            .copied()
            .filter(|&id| id != keep && !self.view_is_pinned(id))
            .collect();
        for id in others {
            self.close_view(id)?;
        }
        self.shell.workspace_mut().set_active_view(Some(keep));
        Ok(())
    }

    pub(crate) fn reopen_view(&mut self) -> AppResult<ViewId> {
        let saved = self
            .view_history
            .closed
            .back()
            .ok_or("No recently closed view")?;
        let buffer_id = saved.buffer;
        let view_id = self.shell.workspace_mut().create_view(buffer_id)?;
        let entry = self
            .shell
            .workspace()
            .buffers()
            .get(buffer_id)
            .ok_or("Closed buffer unavailable")?;
        if entry.buffer().is_none() {
            let path = entry
                .path()
                .ok_or("Closed buffer has no path")?
                .to_path_buf();
            let key = self
                .shell
                .workspace()
                .buffers()
                .get_snapshot_key(buffer_id)
                .ok_or("Closed snapshot unavailable")?;
            if let Err(error) =
                self.apply_effects(vec![Effect::SubmitStoreJob(StoreJob::LoadFile {
                    key,
                    path,
                })])
            {
                let _ = self.shell.workspace_mut().close_view(view_id);
                return Err(error);
            }
        }
        let saved = self
            .view_history
            .closed
            .pop_back()
            .expect("history entry retained while reopening");
        self.view_projections.insert(view_id, saved.projection);
        self.view_history.restoring.insert(view_id, saved);
        self.restore_closed_views();
        self.quit_requested = false;
        self.shell.reset_editor_input();
        Ok(view_id)
    }

    pub(super) fn restore_closed_views(&mut self) {
        self.view_history.restoring.retain(|&id, saved| {
            let Some((buffer, view)) = self.shell.workspace_mut().editor_parts_mut(id) else {
                return self.shell.workspace().get_view(id).is_some();
            };
            view.set_cursor(
                buffer.snap_to_grapheme_boundary(BytePos::new(saved.anchor)),
                buffer.snap_to_grapheme_boundary(BytePos::new(saved.head)),
            );
            view.set_scroll_top_line(saved.top.min(buffer.line_len().saturating_sub(1)));
            false
        });
    }
}
