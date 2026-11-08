mod actions;
mod translate;

use super::NativeApp;
use crate::editor::EditorInput;
use crate::shell::{Focus, ShellAction};
use truff_core::vim::{Key, Mode};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum FieldOwner {
    Command,
    Navigator,
    Help,
    TextAction,
    Toc,
    Wiki,
    Stream,
}

impl NativeApp {
    pub fn key(&mut self, key: Key) {
        if self.application.shell.text_action().is_open() {
            if let crate::overlays::text_action::TextActionMode::Root {
                has_selection: false,
            } = self.application.shell.text_action().mode()
                && matches!(key, Key::Char('c' | 'C' | 'h' | 'H' | 'x' | 'X' | '0'))
            {
                self.application.status =
                    "Выделите текст или поместите курсор на слово для применения стиля".into();
                return;
            }
            let resp = self.application.shell.text_action_mut().handle_key(key);
            self.handle_text_action_response(resp);
            return;
        }
        if self.dialogs.is_any_open() {
            return;
        }
        if self.navigator.is_open() {
            if let Some(key) = crate::navigator::model::NavKey::from_core(key) {
                let action = self.navigator.handle_key_event(key, false);
                self.handle_navigator_action(action);
            }
            return;
        }
        if self.wiki.is_open() {
            let action = self.wiki.feed_key(key);
            self.handle_wiki_action(action);
            return;
        }
        if self.stream.is_open() {
            let action = self.stream.feed_key(key);
            self.handle_stream_action(action);
            return;
        }
        if self.toc.is_open && !self.toc.is_pinned {
            let action = self.toc.feed_key(key);
            self.handle_toc_action(action);
            return;
        }
        if self.kanban.is_overlay_open() && self.kanban.feed_overlay_key(key, &mut self.application)
        {
            return;
        }
        if self.application.shell.focus() == Focus::Editor
            && self.application.shell.mode() == Mode::Normal
            && let Some(view_id) = self.application.shell.workspace().active_view()
            && self.application.is_kanban_source_view(view_id)
        {
            if self.kanban.source_pending_g {
                self.kanban.source_pending_g = false;
                if key == Key::Char('s') {
                    self.application.shell.reset_editor_input();
                    self.application.toggle_kanban_source_view(view_id);
                    return;
                }
            } else if key == Key::Char('g') {
                self.kanban.source_pending_g = true;
            }
        }
        if self.is_central_kanban_active()
            && self.application.shell.focus() == Focus::Editor
            && self.application.shell.command().is_none()
            && self.application.shell.search_query().is_none()
            && !self.dialogs.is_any_open()
            && !self.navigator.is_open()
            && !self.application.shell.which_key().is_visible()
            && self.application.shell.which_key().hints().is_empty()
            && let Some(view_id) = self.application.shell.workspace().active_view()
            && self
                .kanban
                .feed_central_key(view_id, key, &mut self.application)
        {
            return;
        }
        if self.application.file_operation_pending()
            || self.dialogs.is_any_open()
            || self.navigator.is_open()
            || self.application.settings_requested
        {
            return;
        }
        if self.application.shell.focus() == Focus::Editor
            && self.application.shell.mode() == Mode::Insert
            && self.application.link_completion.active
        {
            match key {
                Key::Down | Key::Ctrl('n') => {
                    self.application.link_completion.next();
                    return;
                }
                Key::Up | Key::Ctrl('p') => {
                    self.application.link_completion.prev();
                    return;
                }
                Key::Enter | Key::Tab => {
                    if let Err(error) = self.application.apply_link_completion() {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                Key::Esc => {
                    self.application.link_completion.dismiss();
                    return;
                }
                _ => {}
            }
        }
        let shell = &self.application.shell;
        if shell.command().is_none()
            && shell.search_query().is_none()
            && !shell.flash().is_active()
            && !shell.which_key().is_visible()
            && shell.which_key().hints().is_empty()
        {
            if shell.focus() == Focus::Editor
                && shell.mode() == Mode::Normal
                && let Some(id) = shell.single_binding(key)
                && id.as_str().starts_with("flash.")
            {
                let action = self.application.shell.execute_registered(id, String::new());
                self.action(action);
                return;
            }
            match key {
                Key::Enter if shell.focus() == Focus::RightPanel => {
                    if let Err(error) = self.application.open_backlink() {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                Key::PageUp | Key::PageDown | Key::Ctrl('d' | 'u')
                    if shell.focus() == Focus::Editor =>
                {
                    let target =
                        shell
                            .editor_buffer()
                            .zip(shell.editor_view())
                            .map(|(buffer, view)| {
                                self.editor.page_target(
                                    buffer,
                                    view,
                                    matches!(key, Key::PageDown | Key::Ctrl('d')),
                                    matches!(key, Key::Ctrl(_)),
                                )
                            });
                    if let Some(target) = target
                        && let Err(error) = self.application.shell.apply_visual_row(target)
                    {
                        self.application.status = error.to_string();
                    }
                    return;
                }
                _ => {}
            }
        }
        let prev_mode = self.application.shell.mode();
        let action = self.application.shell.feed_key(key);
        self.action(action);
        let new_mode = self.application.shell.mode();
        if prev_mode == Mode::Insert
            && new_mode == Mode::Normal
            && self.application.active_path().is_some()
            && self.application.has_any_dirty_buffer()
        {
            let _ = self.application.save(false);
        }
        if self.application.shell.focus() == Focus::Editor
            && let Some((buffer, view)) = self
                .application
                .shell
                .editor_buffer()
                .zip(self.application.shell.editor_view())
        {
            let cursor_line = buffer.line_of_byte(view.head().get());
            if self.editor.is_line_folded(cursor_line) {
                let target_line = if matches!(key, Key::Char('j') | Key::Down) {
                    self.editor
                        .active_hidden_ranges
                        .iter()
                        .find(|r| r.contains(&cursor_line))
                        .map(|r| r.end.min(buffer.line_len().saturating_sub(1)))
                        .unwrap_or(cursor_line)
                } else {
                    self.editor
                        .active_hidden_ranges
                        .iter()
                        .find(|r| r.contains(&cursor_line))
                        .map(|r| r.start.saturating_sub(1))
                        .unwrap_or(cursor_line)
                };
                let head = buffer.byte_of_line(target_line);
                let _ = self
                    .application
                    .shell
                    .apply_input(EditorInput::SetSelection { anchor: head, head });
            }
        }
        self.application.sync_link_completion();
    }
}

pub(super) fn input(application: &mut crate::app::Application, input: EditorInput) {
    if let EditorInput::FollowLink(target) = input {
        application.navigate_to_link(&target);
        return;
    }
    if application.shell.mode() == Mode::Insert && application.shell.focus() != Focus::Editor {
        application.shell.set_focus(Focus::Editor);
    }
    if application.shell.focus() != Focus::Editor || application.file_operation_pending() {
        return;
    }
    if let Err(error) = application.shell.apply_input(input) {
        application.status = error.to_string();
    }
    application.sync_link_completion();
    application.handle_shell_action(ShellAction::EditorEdit);
}
