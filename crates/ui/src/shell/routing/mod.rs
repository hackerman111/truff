mod commands;
mod keymap;

pub(super) use keymap::default_keymap;

use truff_core::ViewId;
use truff_core::commands::{Action, CommandContext, CommandId};
use truff_core::config::BindingLookup;
use truff_core::text::{BytePos, Transaction};
use truff_core::vim::{InputContext, InputResult, Key, Mode};

use crate::overlays::FlashKind;

use super::{Focus, Shell, ShellAction};

impl Shell {
    pub fn apply_planned_tx(&mut self, view_id: ViewId, tx: Transaction) -> ShellAction {
        if let Err(e) = self.apply_transaction(tx) {
            return ShellAction::Error(e.to_string());
        }
        if let Err(e) = self.publish_vim_changes(view_id) {
            return ShellAction::Error(e.to_string());
        }
        ShellAction::EditorEdit
    }

    /// Dispatches a key event according to focus routing rules.
    pub fn feed_key(&mut self, key: Key) -> ShellAction {
        if self.which_key.is_visible()
            && self.which_key.mode() == crate::overlays::WhichKeyMode::Help
        {
            if self.which_key.help.key(key) {
                self.which_key.hide();
                self.pending_prefix = None;
                self.key_prefix.clear();
                self.binding_prefix.clear();
            }
            return ShellAction::Incomplete;
        }
        if self.command().is_some() {
            return self.command_key(key);
        }
        if self.flash.is_active() {
            return self.feed_flash_key(key);
        }
        if self.which_key.is_visible() && key == Key::Esc {
            self.which_key.hide();
            self.pending_prefix = None;
            self.binding_prefix.clear();
            return ShellAction::Incomplete;
        }
        if key == Key::Char('?')
            && self.mode() == Mode::Normal
            && self.command().is_none()
            && self.search_query().is_none()
        {
            self.show_help();
            return ShellAction::Incomplete;
        }
        if key == Key::Char(':') && (self.focus != Focus::Editor || self.mode() != Mode::Insert) {
            self.start_command();
            return ShellAction::Incomplete;
        }
        if key == Key::Ctrl('p') && self.command().is_none() && self.search_query().is_none() {
            let context = self.command_context();
            return match self.keymap.lookup(self.mode(), &[key], context) {
                BindingLookup::Complete(id) => self.execute_registered(id, String::new()),
                _ => ShellAction::Ignored,
            };
        }
        if let Some(Key::Char(' ')) = self.pending_prefix {
            self.pending_prefix = None;
            self.which_key.hide();
            let context = self.command_context();
            if let BindingLookup::Complete(id) =
                self.keymap
                    .lookup(self.mode(), &[Key::Char(' '), key], context)
            {
                return self.execute_registered(id, String::new());
            }
            return ShellAction::Ignored;
        }
        if key == Key::Char(' ')
            && matches!(
                self.mode(),
                Mode::Normal | Mode::VisualChar | Mode::VisualLine
            )
            && self.command().is_none()
            && self.search_query().is_none()
        {
            let context = self.command_context();
            self.pending_prefix = Some(Key::Char(' '));
            self.which_key.set_prefix_hints_with_title(
                "<Space> Навигатор",
                self.keymap.hints(self.mode(), &[Key::Char(' ')], context),
            );
            return ShellAction::Incomplete;
        }
        // 1. Handle pane navigation prefixes (<C-w>h / <C-w>l / <C-w>w)
        if let Some(Key::Ctrl('w')) = self.pending_prefix {
            return self.feed_window_prefix_key(key);
        }
        if key == Key::Ctrl('w') && self.mode() == Mode::Normal {
            let context = self.command_context();
            self.pending_prefix = Some(key);
            self.which_key.set_prefix_hints_with_title(
                "<C-w> Окна и панели",
                self.keymap.hints(Mode::Normal, &[key], context),
            );
            return ShellAction::Incomplete;
        }

        if self.custom_keymap
            && self.mode() != Mode::Insert
            && self.search_query().is_none()
            && key != Key::Esc
        {
            // Counts are parsed by the canonical Vim parser before the binding resolves.
            if !matches!(key, Key::Char('0'..='9')) || !self.binding_prefix.is_empty() {
                self.binding_prefix.push(key);
                let context = self.command_context();
                let mode = if self.focus == Focus::Editor {
                    self.mode()
                } else {
                    Mode::Normal
                };
                match self.keymap.lookup(mode, &self.binding_prefix, context) {
                    BindingLookup::Complete(id) => {
                        self.binding_prefix.clear();
                        self.which_key.hide();
                        return self.execute_registered(id, String::new());
                    }
                    BindingLookup::Prefix => {
                        self.which_key.set_prefix_hints(self.keymap.hints(
                            mode,
                            &self.binding_prefix,
                            context,
                        ));
                        return ShellAction::Incomplete;
                    }
                    BindingLookup::Missing => {
                        let mut prefix = std::mem::take(&mut self.binding_prefix);
                        prefix.pop();
                        self.custom_keymap = false;
                        for key in prefix {
                            self.feed_key(key);
                        }
                        self.custom_keymap = true;
                    }
                }
            }
        }
        if key == Key::Esc {
            self.binding_prefix.clear();
        }

        if self.focus == Focus::Editor
            && self.mode() == Mode::Normal
            && self.command().is_none()
            && self.search_query().is_none()
            && !self.flash.is_active()
            && !self.which_key.is_visible()
            && self.binding_prefix.is_empty()
            && self.pending_prefix.is_none()
            && let Some(id) = self.single_binding(key)
            && id.as_str().starts_with("flash.")
        {
            return ShellAction::RegisteredCommand {
                id,
                argument: String::new(),
            };
        }

        // 2. Route strictly according to active panel focus
        match self.focus {
            Focus::Editor => self.feed_editor_key(key),
            Focus::Sidebar | Focus::RightPanel => self.feed_row_key(key),
            Focus::Navigator | Focus::CommandBar | Focus::TOC | Focus::Wiki | Focus::Stream => {
                if key == Key::Esc {
                    let prev = self.pop_focus();
                    return ShellAction::FocusChanged(prev);
                }
                ShellAction::Incomplete
            }
        }
    }

    fn feed_flash_key(&mut self, key: Key) -> ShellAction {
        match key {
            Key::Esc => {
                self.flash.cancel();
                self.editor_vim.cancel_input();
                self.key_prefix.clear();
                self.which_key.hide();
            }
            Key::Backspace if self.flash.is_waiting_search_chars() => {
                self.flash.backspace();
            }
            Key::Char(ch) => {
                if let Some(mut range) = self.flash.feed_char(ch) {
                    use truff_core::vim::{TextObject, TextObjectKind, TextObjectTarget};
                    let target = match self.flash_kind {
                        FlashKind::Sentence => Some(TextObjectTarget::Sentence),
                        FlashKind::Paragraph => Some(TextObjectTarget::Paragraph),
                        _ => None,
                    };
                    if let Some(target) = target
                        && let Some(buffer) = self.editor_buffer()
                        && let Some(expanded) = truff_core::vim::motion::eval_text_object_range(
                            buffer,
                            BytePos::new(range.start),
                            TextObject {
                                kind: TextObjectKind::Around,
                                target,
                            },
                        )
                    {
                        range = expanded;
                    }
                    return self.apply_target_range(range);
                }
            }
            _ => {}
        }
        ShellAction::EditorEdit
    }

    fn feed_window_prefix_key(&mut self, key: Key) -> ShellAction {
        self.pending_prefix = None;
        self.which_key.hide();
        let context = self.command_context();
        if let BindingLookup::Complete(id) =
            self.keymap
                .lookup(Mode::Normal, &[Key::Ctrl('w'), key], context)
        {
            return self.execute_registered(id, String::new());
        }
        match key {
            Key::Char('w') => {
                let next = match self.focus {
                    Focus::Sidebar => Focus::Editor,
                    Focus::Editor => {
                        if self.has_right_panel() {
                            Focus::RightPanel
                        } else if self.has_sidebar() {
                            Focus::Sidebar
                        } else {
                            Focus::Editor
                        }
                    }
                    Focus::RightPanel => {
                        if self.has_sidebar() {
                            Focus::Sidebar
                        } else {
                            Focus::Editor
                        }
                    }
                    _ => Focus::Editor,
                };
                self.set_focus(next);
                ShellAction::FocusChanged(self.focus)
            }
            Key::Char('j' | 'k') | Key::Up | Key::Down => ShellAction::FocusChanged(self.focus),
            _ => ShellAction::Ignored,
        }
    }

    fn feed_editor_key(&mut self, key: Key) -> ShellAction {
        let Some(view_id) = self.workspace.active_view() else {
            return ShellAction::Ignored;
        };

        if key == Key::CtrlEnter {
            let toggle_tx = self
                .workspace
                .editor_parts_mut(view_id)
                .and_then(|(buffer, view)| {
                    truff_core::markdown::plan_toggle_task(buffer, view.head())
                });
            return match toggle_tx {
                Some(tx) => self.apply_planned_tx(view_id, tx),
                None => ShellAction::Ignored,
            };
        }

        if self.editor_vim.mode == Mode::Normal && self.key_prefix.is_empty() && key == Key::Enter {
            let cycle_tx = self
                .workspace
                .editor_parts_mut(view_id)
                .and_then(|(buffer, view)| {
                    truff_core::markdown::plan_cycle_task(buffer, view.head())
                });
            if let Some(tx) = cycle_tx {
                return self.apply_planned_tx(view_id, tx);
            }
        }

        if self.editor_vim.mode == Mode::Insert
            && matches!(key, Key::Enter | Key::Tab | Key::ShiftTab | Key::Backspace)
        {
            let smart_tx = self
                .workspace
                .editor_parts_mut(view_id)
                .and_then(|(buffer, view)| match key {
                    Key::Enter => truff_core::markdown::plan_enter(buffer, view.head()),
                    Key::Tab => truff_core::markdown::plan_tab(buffer, view.head()),
                    Key::ShiftTab => truff_core::markdown::plan_shift_tab(buffer, view.head()),
                    Key::Backspace => truff_core::markdown::plan_backspace(buffer, view.head()),
                    _ => None,
                });
            if let Some(tx) = smart_tx {
                return self.apply_planned_tx(view_id, tx);
            }
        }

        let Some((buffer, view)) = self.workspace.editor_parts_mut(view_id) else {
            return ShellAction::Ignored;
        };

        let res = self.editor_vim.feed_key(
            key,
            InputContext::Editor {
                view_id,
                buffer,
                view,
            },
        );
        if let Err(e) = self.publish_vim_changes(view_id) {
            return ShellAction::Error(e.to_string());
        }
        if res != InputResult::Incomplete {
            self.key_prefix.clear();
            self.which_key.hide();
        }
        match res {
            InputResult::Incomplete => {
                self.key_prefix.push(key);
                self.which_key.set_prefix_hints(self.keymap.hints(
                    self.mode(),
                    &self.key_prefix,
                    CommandContext::Editor,
                ));
                ShellAction::Incomplete
            }
            InputResult::VisualRow { down, count } => ShellAction::VisualRow { down, count },
            InputResult::Command { id, argument } => {
                ShellAction::RegisteredCommand { id, argument }
            }
            InputResult::Action(action) => ShellAction::Core(action),
            InputResult::ScrollCursor(pos) => ShellAction::ScrollCursor(pos),
            InputResult::Fold(fold_cmd) => ShellAction::Fold(fold_cmd),
            InputResult::FollowLink => ShellAction::RegisteredCommand {
                id: CommandId::new("note.follow_link"),
                argument: String::new(),
            },
            InputResult::Actions(actions) => {
                if actions.len() == 2
                    && matches!(actions[0], Action::SaveBuffer(_))
                    && matches!(actions[1], Action::CloseView(_))
                {
                    ShellAction::Command("wq".into())
                } else {
                    ShellAction::Error("Application action sequence unavailable".into())
                }
            }
            InputResult::Error(error) => ShellAction::Error(error),
            InputResult::Ignored => ShellAction::Ignored,
            _ => {
                self.key_prefix.clear();
                self.which_key.hide();
                ShellAction::EditorEdit
            }
        }
    }

    fn feed_row_key(&mut self, key: Key) -> ShellAction {
        if key == Key::Esc && self.row_vim.search_query().is_none() {
            self.set_focus(Focus::Editor);
            return ShellAction::FocusChanged(Focus::Editor);
        }
        if self.focus == Focus::Sidebar
            && self.row_vim.search_query().is_none()
            && self.row_vim.mode == Mode::Normal
            && let BindingLookup::Complete(id) =
                self.keymap
                    .lookup(Mode::Normal, &[key], CommandContext::Rows)
            && matches!(
                id.as_str(),
                "file.create" | "folder.create" | "file.rename" | "file.trash"
            )
        {
            return self.execute_registered(id, String::new());
        }
        if matches!(key, Key::Char('i' | 'o' | 'I' | 'O')) && self.row_vim.search_query().is_none()
        {
            self.set_focus(Focus::Editor);
            return self.feed_key(key);
        }
        let model = if self.focus == Focus::Sidebar {
            &mut self.sidebar_model
        } else {
            &mut self.right_model
        };
        if key == Key::Enter && self.row_vim.search_query().is_none() {
            if let Some(row) = model.selected_row() {
                if row.is_folder {
                    let id = row.id;
                    model.toggle_collapse(id);
                    return ShellAction::Incomplete;
                } else if let Some(path) = row.data.clone() {
                    return ShellAction::SidebarSelectFile(path);
                }
            }
            return ShellAction::Ignored;
        }
        match self.row_vim.feed_key(key, InputContext::Rows(model)) {
            InputResult::Error(e) => ShellAction::Error(e),
            InputResult::Ignored => ShellAction::Ignored,
            _ => ShellAction::Incomplete,
        }
    }
}
