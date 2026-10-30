use super::NativeApp;
mod providers;

impl NativeApp {
    pub fn open_navigator(&mut self, mode: crate::navigator::NavigatorMode) {
        let opening = !self.navigator.is_open();
        if opening {
            self.navigator_origin = self.application.shell.focus();
            self.navigator_selection = self.application.capture_selection().ok();
        }
        if let Some(search) = &mut self.search {
            search.cancel();
        }
        if opening {
            self.application
                .shell
                .push_focus(crate::focus::Focus::Navigator);
        } else {
            self.application
                .shell
                .set_focus(crate::focus::Focus::Navigator);
        }
        if mode == crate::navigator::NavigatorMode::Links
            && self.application.active_relative_path().is_some()
            && let Err(error) = self.application.execute_command("backlinks")
        {
            self.application.status = error.to_string();
        }
        let (context_title, items) = self.navigator_items(mode);
        self.navigator.vault_root = self
            .application
            .vault_path()
            .map(std::path::Path::to_path_buf);
        self.navigator.open_mode(mode, context_title, items);
        if matches!(
            mode,
            crate::navigator::NavigatorMode::Outline | crate::navigator::NavigatorMode::Links
        ) {
            self.sync_hub_tasks();
        }
        if matches!(
            mode,
            crate::navigator::NavigatorMode::Files | crate::navigator::NavigatorMode::Folders
        ) && let Some(active_path) = self.application.active_relative_path()
            && let Some(pos) = self.navigator.filtered_indices.iter().position(|&idx| {
                matches!(
                    &self.navigator.items.get(idx).map(|item| &item.payload),
                    Some(crate::navigator::NavigatorItemPayload::File { path, .. })
                        if path == &active_path
                )
            })
        {
            self.navigator.selected_index = pos;
        }
        self.navigator_revision = self.application.catalog_revision();
    }

    pub(super) fn refresh_navigator(&mut self) {
        if !self.navigator.is_open() {
            return;
        }
        let selected = self
            .navigator
            .selected_item()
            .map(|item| item.payload.clone());
        let old = std::mem::take(&mut self.navigator.items);
        let folds: std::collections::HashMap<_, _> = old
            .iter()
            .filter_map(|item| match &item.payload {
                crate::navigator::NavigatorItemPayload::File { path, is_dir: true } => {
                    Some((path.as_str(), item.is_collapsed))
                }
                _ => None,
            })
            .collect();
        let (title, mut items) = self.navigator_items(self.navigator.mode);
        for item in &mut items {
            if let crate::navigator::NavigatorItemPayload::File { path, is_dir: true } =
                &item.payload
                && let Some(&collapsed) = folds.get(path.as_str())
            {
                item.is_collapsed = collapsed;
            }
        }
        self.navigator.vault_root = self
            .application
            .vault_path()
            .map(std::path::Path::to_path_buf);
        self.navigator.cached_preview = None;
        self.navigator.context_title = title;
        self.navigator.items = items;
        self.navigator.recompute_filter();
        if matches!(
            self.navigator.mode,
            crate::navigator::NavigatorMode::Outline | crate::navigator::NavigatorMode::Links
        ) {
            self.sync_hub_tasks();
        }
        if let Some(index) = self
            .navigator
            .filtered_indices
            .iter()
            .position(|&index| Some(&self.navigator.items[index].payload) == selected.as_ref())
        {
            self.navigator.selected_index = index;
        }
        self.navigator_revision = self.application.catalog_revision();
        self.application
            .shell
            .set_focus(crate::focus::Focus::Navigator);
    }

    pub fn handle_navigator_action(&mut self, action: crate::navigator::NavigatorAction) {
        use crate::navigator::NavigatorAction;
        match action {
            NavigatorAction::None => {}
            NavigatorAction::RevealFolder { path } => {
                if self.navigator.mode != crate::navigator::NavigatorMode::Files {
                    self.open_navigator(crate::navigator::NavigatorMode::Files);
                }
                self.navigator.reveal_path(&path);
                self.navigator.expand_or_child();
            }
            NavigatorAction::SearchPage { forward } => {
                let last = self.navigator.search_total.saturating_sub(1) / 100;
                self.navigator.search_page = if forward {
                    (self.navigator.search_page + 1).min(last)
                } else {
                    self.navigator.search_page.saturating_sub(1)
                };
            }
            NavigatorAction::SwitchProvider { mode } => self.open_navigator(mode),
            NavigatorAction::SwitchView { view_id } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(error) = self.application.focus_view(view_id) {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::OpenVault { path } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(error) = self.application.open_vault(&path) {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::OpenLocation { path, byte } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(error) = self
                    .application
                    .open_location(&path, Some(crate::app::navigation::Location::Byte(byte)))
                {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::ApplyColor { hex } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                let result = self
                    .navigator_selection
                    .take()
                    .ok_or_else(|| -> Box<dyn std::error::Error> {
                        "Select text with v or V first".into()
                    })
                    .and_then(|target| self.application.color_target(target, hex));
                if let Err(error) = result {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::ApplyHighlight { hex } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                let result = self
                    .navigator_selection
                    .take()
                    .ok_or_else(|| -> Box<dyn std::error::Error> {
                        "Select text with v or V first".into()
                    })
                    .and_then(|target| self.application.highlight_target(target, hex));
                if let Err(error) = result {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::ApplyCallout { kind } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                let result = self
                    .navigator_selection
                    .take()
                    .ok_or_else(|| -> Box<dyn std::error::Error> {
                        "Select text with v or V first".into()
                    })
                    .and_then(|target| self.application.callout_target(target, kind));
                if let Err(error) = result {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::RemoveStyle => {
                self.navigator.close();
                self.application.shell.pop_focus();
                let result = self
                    .navigator_selection
                    .take()
                    .ok_or_else(|| -> Box<dyn std::error::Error> {
                        "Select text with v or V first".into()
                    })
                    .and_then(|target| self.application.remove_style_target(target));
                if let Err(error) = result {
                    self.application.status = error.to_string();
                }
            }
            NavigatorAction::CloseView { view_id } => {
                if let Err(error) = self.application.close_view(view_id) {
                    self.application.status = error.to_string();
                }
                self.refresh_navigator();
            }
            NavigatorAction::PinView { view_id } => {
                self.application.toggle_view_pin(view_id);
                self.refresh_navigator();
            }
            NavigatorAction::CloseOtherViews { view_id } => {
                if let Err(error) = self.application.close_other_views(view_id) {
                    self.application.status = error.to_string();
                }
                self.refresh_navigator();
            }
            NavigatorAction::ReopenView => {
                if let Err(error) = self.application.reopen_view() {
                    self.application.status = error.to_string();
                }
                self.refresh_navigator();
            }

            NavigatorAction::Close => {
                self.navigator.close();
                self.application.shell.pop_focus();
            }
            NavigatorAction::CopyPath { path } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.application.status = format!("Скопирован путь: {path}");
            }
            NavigatorAction::OpenFile { path } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(e) = self.application.open_note(&path) {
                    self.application.status = e.to_string();
                }
            }
            NavigatorAction::CreateFile { parent_dir } => {
                self.dialogs.file.open = true;
                self.dialogs.file.path = parent_dir.map_or(String::new(), |d| format!("{d}/"));
                self.dialogs.file.error = None;
                self.dialogs.file.needs_cursor_end = true;
            }
            NavigatorAction::CreateFolder { parent_dir } => {
                self.dialogs.folder.open = true;
                self.dialogs.folder.path = parent_dir.map_or(String::new(), |d| format!("{d}/"));
                self.dialogs.folder.error = None;
                self.dialogs.folder.needs_cursor_end = true;
            }
            NavigatorAction::RenameFile { path } => {
                self.dialogs.operation.rename(path);
            }
            NavigatorAction::DeleteFile { path } => {
                self.dialogs.operation.trash(path);
            }
            NavigatorAction::SwitchBuffer { buffer_id } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Some(view) = self
                    .application
                    .shell
                    .workspace()
                    .views()
                    .values()
                    .find(|v| v.buffer_id() == buffer_id)
                {
                    let view_id = view.id();
                    self.application
                        .shell
                        .workspace_mut()
                        .set_active_view(Some(view_id));
                    self.application.shell.reset_editor_input();
                }
            }
            NavigatorAction::CloseBuffer { buffer_id } => {
                if let Some(view) = self
                    .application
                    .shell
                    .workspace()
                    .views()
                    .values()
                    .find(|v| v.buffer_id() == buffer_id)
                {
                    let view_id = view.id();
                    if let Err(error) = self.application.close_view(view_id) {
                        self.application.status = error.to_string();
                    }
                    if self.navigator.is_open()
                        && self.navigator.mode == crate::navigator::NavigatorMode::Sessions
                    {
                        self.refresh_navigator();
                    }
                }
            }
            NavigatorAction::ExecuteCommand { command_id } => {
                let selection = self.navigator_selection.take();
                self.navigator.close();
                self.application.shell.pop_focus();
                if command_id.as_str() == "note.link_selection" {
                    if let Some(target) = selection {
                        self.dialogs.operation.link(target);
                    } else {
                        self.application.status = "Выделите текст с помощью v или V".into();
                    }
                    return;
                }
                let color_picker = command_id.as_str() == "style.text_color";
                let action = self
                    .application
                    .shell
                    .execute_registered(command_id, String::new());
                self.action(action);
                if color_picker {
                    self.navigator_selection = selection;
                }
            }
            NavigatorAction::JumpHeading { line } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Some(view_id) = self.application.shell.workspace().active_view()
                    && let Some((buffer, view)) = self
                        .application
                        .shell
                        .workspace_mut()
                        .editor_parts_mut(view_id)
                {
                    let line_idx = line.saturating_sub(1);
                    if line_idx < buffer.line_len() {
                        let byte_offset = buffer.byte_of_line(line_idx);
                        let byte_pos = truff_core::text::BytePos::new(byte_offset);
                        view.set_cursor_with_jump(byte_pos, byte_pos);
                    }
                }
            }
            NavigatorAction::OpenLink { target } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                if let Err(e) = self
                    .application
                    .execute_command(&format!("follow {target}"))
                {
                    self.application.status = e.to_string();
                }
            }
            NavigatorAction::SelectCustom { id, data } => {
                self.navigator.close();
                self.application.shell.pop_focus();
                self.application.status = format!("{id}: {data}");
            }
            NavigatorAction::HubTask(op) => {
                self.apply_hub_task_op(op);
            }
        }
    }

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

    pub(super) fn sync_hub_tasks(&mut self) {
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
        self.navigator
            .hub_tasks
            .set_lists(vault_tasks, file_tasks, project_tasks);
    }

    fn apply_hub_task_op(&mut self, op: crate::navigator::HubTaskOp) {
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
                        } else if let Some(buf_id) = self
                            .application
                            .shell
                            .workspace()
                            .buffers()
                            .get_by_path(&active_path)
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
                            entry.set_fingerprint(Some(truff_store::compute_fingerprint(
                                updated.as_bytes(),
                            )));
                            entry.set_state(truff_core::workspace::BufferState::Clean);
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
                    if wrote_ok
                        && let Some(buf_id) = self
                            .application
                            .shell
                            .workspace()
                            .buffers()
                            .get_by_path(&path)
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
                        entry.set_fingerprint(Some(truff_store::compute_fingerprint(
                            updated.as_bytes(),
                        )));
                        entry.set_state(truff_core::workspace::BufferState::Clean);
                    }
                } else {
                    // No vault path open (e.g., headless scratch state): keep in-memory state
                    return;
                }
            }
        }
        self.refresh_navigator();
    }
}
