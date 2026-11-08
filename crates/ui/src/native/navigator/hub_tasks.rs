use super::super::NativeApp;

impl NativeApp {
    fn vault_index_path(&self) -> Option<std::path::PathBuf> {
        self.application
            .vault_path()
            .map(|root| root.join("index.md"))
    }

    fn project_index_path(&self) -> Option<std::path::PathBuf> {
        if let Some(active) = self.application.active_path()
            && let Some(parent) = active.parent()
        {
            return Some(parent.join("index.md"));
        }
        self.vault_index_path()
    }

    fn read_index_markdown(&self, path: &std::path::Path) -> String {
        if let Some(buf_id) = self
            .application
            .shell
            .workspace()
            .buffers()
            .get_by_path(path)
            && let Some(buffer) = self
                .application
                .shell
                .workspace()
                .buffers()
                .get_buffer(buf_id)
        {
            return buffer.as_str();
        }
        std::fs::read_to_string(path).unwrap_or_default()
    }

    fn mark_buffer_clean(&mut self, path: &std::path::Path, updated: &str) {
        if let Some(buf_id) = self
            .application
            .shell
            .workspace()
            .buffers()
            .get_by_path(path)
            && let Some(entry) = self
                .application
                .shell
                .workspace_mut()
                .buffers_mut()
                .get_mut(buf_id)
        {
            if let Some(buf) = entry.buffer_mut() {
                buf.set_savepoint();
            }
            entry.set_fingerprint(Some(truff_store::compute_fingerprint(updated.as_bytes())));
            entry.set_state(truff_core::workspace::BufferState::Clean);
        }
    }

    pub(in crate::native) fn sync_hub_tasks(&mut self) {
        use crate::navigator::tasks::{parse_fenced_file_tasks, parse_index_tasks};
        let vault_tasks = self
            .vault_index_path()
            .map(|path| parse_index_tasks(&self.read_index_markdown(&path)))
            .unwrap_or_default();
        let file_tasks = self
            .application
            .shell
            .editor_buffer()
            .filter(|b| !b.is_empty())
            .map(|buffer| buffer.as_str())
            .or_else(|| {
                self.application
                    .active_path()
                    .and_then(|p| std::fs::read_to_string(p).ok())
            })
            .map(|text| parse_fenced_file_tasks(&text))
            .unwrap_or_default();
        let project_tasks = self
            .project_index_path()
            .map(|path| parse_index_tasks(&self.read_index_markdown(&path)))
            .unwrap_or_default();
        let saved_cat = self.navigator.hub_tasks.active_category;
        let saved_selected = self.navigator.hub_tasks.selected;
        self.navigator
            .hub_tasks
            .set_lists(vault_tasks, file_tasks, project_tasks);
        self.navigator.hub_tasks.active_category = saved_cat;
        self.navigator.hub_tasks.selected = saved_selected;
        self.navigator.hub_tasks.clamp_selection();
    }

    pub(super) fn apply_hub_task_op(&mut self, op: crate::navigator::HubTaskOp) {
        use crate::navigator::{
            HubTaskCategory, HubTaskOp,
            tasks::{apply_task_op_to_file_text, apply_task_op_to_index_text},
        };
        let category = match &op {
            HubTaskOp::Add { category, .. }
            | HubTaskOp::Edit { category, .. }
            | HubTaskOp::SetStatus { category, .. }
            | HubTaskOp::Delete { category, .. } => *category,
        };
        match category {
            HubTaskCategory::File => {
                let Some(current) = self.application.shell.editor_buffer().map(|b| b.as_str())
                else {
                    return;
                };
                let updated = apply_task_op_to_file_text(&current, &op);
                let byte_len = self
                    .application
                    .shell
                    .editor_buffer()
                    .map_or(0, |b| b.byte_len());
                if let Err(error) = self.application.shell.apply_transaction(
                    truff_core::text::Transaction::replace(0..byte_len, updated.clone()),
                ) {
                    self.application.status = error.to_string();
                } else {
                    self.application
                        .handle_shell_action(crate::shell::ShellAction::EditorEdit);
                    if let Some(active_path) = self
                        .application
                        .active_path()
                        .map(std::path::Path::to_path_buf)
                    {
                        if let Err(error) = std::fs::write(&active_path, &updated) {
                            self.application.status = error.to_string();
                        } else {
                            self.mark_buffer_clean(&active_path, &updated);
                        }
                    }
                }
            }
            HubTaskCategory::Vault | HubTaskCategory::Project => {
                let target_path = if category == HubTaskCategory::Vault {
                    self.vault_index_path()
                } else {
                    self.project_index_path()
                };
                if let Some(path) = target_path {
                    let current = self.read_index_markdown(&path);
                    let updated = apply_task_op_to_index_text(&current, &op);
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let wrote_ok = match std::fs::write(&path, &updated) {
                        Ok(()) => true,
                        Err(error) => {
                            self.application.status = error.to_string();
                            false
                        }
                    };
                    if self.application.active_path() == Some(path.as_path()) {
                        let byte_len = self
                            .application
                            .shell
                            .editor_buffer()
                            .map_or(0, |b| b.byte_len());
                        let _ = self.application.shell.apply_transaction(
                            truff_core::text::Transaction::replace(0..byte_len, updated.clone()),
                        );
                        self.application
                            .handle_shell_action(crate::shell::ShellAction::EditorEdit);
                    } else if let Some(buf_id) = self
                        .application
                        .shell
                        .workspace()
                        .buffers()
                        .get_by_path(&path)
                        && let Some(buffer) = self
                            .application
                            .shell
                            .workspace_mut()
                            .buffers_mut()
                            .get_buffer_mut(buf_id)
                    {
                        let byte_len = buffer.byte_len();
                        let _ = buffer.apply(truff_core::text::Transaction::replace(
                            0..byte_len,
                            updated.clone(),
                        ));
                    }
                    if wrote_ok {
                        self.mark_buffer_clean(&path, &updated);
                    }
                } else {
                    // No vault path open (e.g., headless scratch state): keep in-memory state
                    return;
                }
            }
        }
        self.sync_hub_tasks();
        if let crate::navigator::HubTaskOp::Add { category, .. } = op {
            let cat = category.index();
            self.navigator.hub_tasks.selected[cat] =
                self.navigator.hub_tasks.lists[cat].len().saturating_sub(1);
        }
        self.refresh_navigator();
    }
}
