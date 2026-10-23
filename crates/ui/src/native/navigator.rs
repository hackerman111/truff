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
            }
            NavigatorAction::CreateFolder { parent_dir } => {
                self.dialogs.folder.open = true;
                self.dialogs.folder.path = parent_dir.map_or(String::new(), |d| format!("{d}/"));
                self.dialogs.folder.error = None;
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
        }
    }
}
