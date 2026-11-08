use super::super::NativeApp;
use crate::editor::EditorInput;
use crate::shell::{Focus, ShellAction};

impl NativeApp {
    pub(in crate::native) fn action(&mut self, action: ShellAction) {
        match action {
            ShellAction::FocusChanged(focus) => {
                if self.application.focus_mode {
                    self.application.shell.set_focus(Focus::Editor);
                } else if focus == Focus::Sidebar
                    && let Some(path) = self.application.active_relative_path()
                {
                    self.application.shell.select_sidebar_file(&path);
                }
            }
            ShellAction::ScrollCursor(pos) => {
                self.editor.scroll_cursor(pos);
            }
            ShellAction::Fold(cmd) => self.handle_fold_command(cmd),
            ShellAction::EditorEdit if self.application.shell.focus() == Focus::RightPanel => {}
            ShellAction::VisualRow { down, count } => {
                let target = self
                    .application
                    .shell
                    .editor_buffer()
                    .zip(self.application.shell.editor_view())
                    .and_then(|(buffer, view)| {
                        self.editor.visual_target(buffer, view, down, count)
                    });
                if let Some(target) = target
                    && let Err(error) = self.application.shell.apply_visual_row(target)
                {
                    self.application.status = error.to_string();
                }
            }
            ShellAction::RegisteredCommand { id, argument: _ }
                if id.as_str().starts_with("flash.") =>
            {
                use crate::overlays::FlashKind;
                if id.as_str() == "flash.jump_word" {
                    if let Err(error) = self
                        .application
                        .shell
                        .start_flash_search_ranges(&self.editor.visible_source_windows())
                    {
                        self.application.status = error.to_string();
                    }
                } else {
                    let kind = match id.as_str() {
                        "flash.select_sentence" => FlashKind::Sentence,
                        "flash.select_paragraph" => FlashKind::Paragraph,
                        _ => FlashKind::SyntaxObject,
                    };
                    if let Err(error) = self
                        .application
                        .shell
                        .start_flash_ranges(&self.editor.visible_source_windows(), kind)
                    {
                        self.application.status = error.to_string();
                    }
                }
            }
            ShellAction::RegisteredCommand { id, argument }
                if id.as_str() == "note.link_selection" && argument.is_empty() =>
            {
                match self.application.capture_selection() {
                    Ok(target) => self.dialogs.operation.link(target),
                    Err(error) => self.application.status = error.to_string(),
                }
            }
            ShellAction::SidebarDeleteFile(path) => self.dialogs.operation.trash(path),
            ShellAction::OpenTextActionMenu => {
                self.open_text_action_menu();
            }
            ShellAction::OpenNavigator(mode) => {
                self.open_navigator(mode);
            }
            ShellAction::OpenKanban => {
                self.kanban.open_overlay("", &mut self.application);
            }
            ShellAction::RegisteredCommand { id, argument } => {
                if !self.file_command(id.as_str(), &argument) {
                    self.application
                        .handle_shell_action(ShellAction::RegisteredCommand { id, argument });
                }
            }
            ShellAction::Command(command) => {
                let (name, argument) = crate::app::command_parts(&command);
                if name == "trash" {
                    self.file_command("file.trash", argument);
                } else {
                    self.application
                        .handle_shell_action(ShellAction::Command(command));
                }
            }
            action => self.application.handle_shell_action(action),
        }
    }

    fn handle_fold_command(&mut self, cmd: truff_core::vim::FoldCommand) {
        let Some((buffer, view)) = self
            .application
            .shell
            .editor_buffer()
            .zip(self.application.shell.editor_view())
        else {
            return;
        };
        let cursor_line = buffer.line_of_byte(view.head().get());
        let headings = self.editor.headings_for_buffer(buffer);
        let total_lines = buffer.line_len();
        let owner_heading_line =
            truff_core::markdown::find_heading_for_line(&headings, cursor_line, total_lines)
                .map(|idx| headings[idx].line);

        let mut clamp_to_heading = None;
        match cmd {
            truff_core::vim::FoldCommand::Toggle => {
                if let Some(h_line) = owner_heading_line {
                    self.editor.toggle_fold(h_line, buffer);
                    clamp_to_heading = Some(h_line);
                }
            }
            truff_core::vim::FoldCommand::Close => {
                if let Some(h_line) = owner_heading_line {
                    self.editor.fold_close(h_line, buffer);
                    clamp_to_heading = Some(h_line);
                }
            }
            truff_core::vim::FoldCommand::Open => {
                if let Some(h_line) = owner_heading_line {
                    self.editor.fold_open(h_line, buffer);
                }
            }
            truff_core::vim::FoldCommand::CloseAll => {
                self.editor.fold_close_all(buffer);
                clamp_to_heading = owner_heading_line;
            }
            truff_core::vim::FoldCommand::OpenAll => {
                self.editor.fold_open_all(buffer);
            }
        }

        if let Some(h_line) = clamp_to_heading
            && self.editor.is_line_folded(cursor_line)
        {
            let head = buffer.byte_of_line(h_line);
            let _ = self
                .application
                .shell
                .apply_input(EditorInput::SetSelection { anchor: head, head });
        }
    }

    pub fn open_text_action_menu(&mut self) {
        let (target, has_selection) = match self.application.capture_selection_or_cursor() {
            Ok(res) => res,
            Err(err) => {
                self.application.status = err.to_string();
                return;
            }
        };

        let initial_text = if has_selection {
            self.application.target_text(&target).ok()
        } else {
            None
        };
        let cursor_pos = self.editor.cursor_rect().map(|r| [r.left(), r.bottom()]);

        self.text_action_target = Some(target);
        self.application
            .shell
            .text_action_mut()
            .open(initial_text, cursor_pos);
    }

    pub fn handle_text_action_response(
        &mut self,
        response: crate::overlays::text_action::TextActionResponse,
    ) {
        use crate::overlays::text_action::TextActionResponse;
        match response {
            TextActionResponse::None => {}
            TextActionResponse::Close => {
                self.text_action_target = None;
            }
            TextActionResponse::ApplyColor { hex } => {
                if let Some(target) = self.text_action_target.take()
                    && let Err(e) = self.application.color_target(target, hex)
                {
                    self.application.status = e.to_string();
                }
            }
            TextActionResponse::ApplyHighlight { hex } => {
                if let Some(target) = self.text_action_target.take()
                    && let Err(e) = self.application.highlight_target(target, hex)
                {
                    self.application.status = e.to_string();
                }
            }
            TextActionResponse::ApplyLink {
                is_url,
                target,
                title,
            } => {
                if let Some(selection_target) = self.text_action_target.take() {
                    let res = if is_url {
                        self.application.apply_url_link_target(
                            &selection_target,
                            &target,
                            title.as_deref(),
                        )
                    } else {
                        self.application.apply_note_link_target(
                            &selection_target,
                            &target,
                            title.as_deref(),
                        )
                    };
                    if let Err(e) = res {
                        self.application.status = e.to_string();
                    }
                }
            }
            TextActionResponse::ApplyTask { status } => {
                if let Some(target) = self.text_action_target.take()
                    && let Err(e) = self.application.apply_task_target(&target, status)
                {
                    self.application.status = e.to_string();
                }
            }
            TextActionResponse::RemoveStyle => {
                if let Some(target) = self.text_action_target.take()
                    && let Err(e) = self.application.remove_style_target(target)
                {
                    self.application.status = e.to_string();
                }
            }
        }
    }
}
