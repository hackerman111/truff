use truff_core::commands::{Action, CommandContext, CommandId, CommandRegistry};
use truff_core::config::{BindingLookup, KeymapConfig, KeymapError};
use truff_core::text::BytePos;
use truff_core::vim::{InputContext, InputResult, Key, Mode};

use crate::overlays::FlashKind;

use super::{Focus, Shell, ShellAction};

impl Shell {
    pub fn set_keymap(&mut self, config: KeymapConfig) -> Result<(), KeymapError> {
        self.keymap = config.compile()?;
        self.custom_keymap = true;
        self.binding_prefix.clear();
        Ok(())
    }

    /// Same registry resolution used by keyboard bindings, picker rows and buttons.
    pub fn execute_registered(&mut self, id: CommandId, argument: String) -> ShellAction {
        let context = if self.focus == Focus::Editor {
            CommandContext::Editor
        } else {
            CommandContext::Rows
        };
        let registry = CommandRegistry::default();
        let command = match registry.vim_command(&id, context) {
            Ok(c) => c,
            Err(e) => return ShellAction::Error(e.to_string()),
        };
        let Some(command) = command else {
            return ShellAction::RegisteredCommand { id, argument };
        };
        let result = if self.focus == Focus::Editor {
            let Some(view_id) = self.workspace.active_view() else {
                return ShellAction::Error("Editor target unavailable".into());
            };
            let Some((buffer, view)) = self.workspace.editor_parts_mut(view_id) else {
                return ShellAction::Error("Editor target unavailable".into());
            };
            let result = self.editor_vim.run_binding(
                command,
                InputContext::Editor {
                    view_id,
                    buffer,
                    view,
                },
            );
            if let Err(e) = self.publish_vim_changes(view_id) {
                return ShellAction::Error(e.to_string());
            }
            result
        } else {
            let model = if self.focus == Focus::Sidebar {
                &mut self.sidebar_model
            } else {
                &mut self.right_model
            };
            self.row_vim.run_binding(command, InputContext::Rows(model))
        };
        match result {
            InputResult::Error(e) => ShellAction::Error(e),
            InputResult::VisualRow { down, count } => ShellAction::VisualRow { down, count },
            InputResult::Action(action) => ShellAction::Core(action),
            _ => ShellAction::EditorEdit,
        }
    }

    /// Dispatches a key event according to focus routing rules.
    pub fn feed_key(&mut self, key: Key) -> ShellAction {
        if let Some(command) = &mut self.command {
            match key {
                Key::Esc => {
                    self.command = None;
                }
                Key::Enter => {
                    return ShellAction::Command(self.command.take().unwrap());
                }
                Key::Backspace => {
                    command.pop();
                }
                Key::Char(ch) => {
                    if command.len() + ch.len_utf8() > 65536 {
                        return ShellAction::Error("Command text limit exceeded".into());
                    }
                    command.push(ch);
                }
                _ => {}
            }
            return ShellAction::Incomplete;
        }
        if self.flash.is_active() {
            match key {
                Key::Esc => {
                    self.flash.cancel();
                    self.editor_vim.cancel_input();
                    self.key_prefix.clear();
                    self.which_key.hide();
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
            return ShellAction::EditorEdit;
        }
        if self.which_key.is_visible() && key == Key::Esc {
            self.which_key.hide();
            self.pending_prefix = None;
            self.binding_prefix.clear();
            return ShellAction::Incomplete;
        }
        if key == Key::Char('?')
            && self.mode() == Mode::Normal
            && self.command.is_none()
            && self.search_query().is_none()
        {
            self.which_key
                .show_help(crate::overlays::WhichKeyOverlay::default_help_categories());
            return ShellAction::Incomplete;
        }
        if key == Key::Char(':') && (self.focus != Focus::Editor || self.mode() != Mode::Insert) {
            self.command = Some(String::new());
            self.which_key.hide();
            return ShellAction::Incomplete;
        }
        // 1. Handle pane navigation prefixes (<C-w>h / <C-w>l / <C-w>w)
        if let Some(Key::Ctrl('w')) = self.pending_prefix {
            self.pending_prefix = None;
            self.which_key.hide();
            match key {
                Key::Char('h') | Key::Left => {
                    let next = match self.focus {
                        Focus::RightPanel => Focus::Editor,
                        Focus::Editor => {
                            if self.has_sidebar() {
                                Focus::Sidebar
                            } else {
                                Focus::Editor
                            }
                        }
                        Focus::Sidebar => Focus::Sidebar,
                    };
                    self.set_focus(next);
                    return ShellAction::FocusChanged(self.focus);
                }
                Key::Char('l') | Key::Right => {
                    let next = match self.focus {
                        Focus::Sidebar => Focus::Editor,
                        Focus::Editor => {
                            if self.has_right_panel() {
                                Focus::RightPanel
                            } else {
                                Focus::Editor
                            }
                        }
                        Focus::RightPanel => Focus::RightPanel,
                    };
                    self.set_focus(next);
                    return ShellAction::FocusChanged(self.focus);
                }
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
                    };
                    self.set_focus(next);
                    return ShellAction::FocusChanged(self.focus);
                }
                Key::Char('j' | 'k') | Key::Up | Key::Down => {
                    return ShellAction::FocusChanged(self.focus);
                }
                _ => return ShellAction::Ignored,
            }
        }

        if key == Key::Ctrl('w') {
            self.pending_prefix = Some(key);
            self.which_key.set_prefix_hints_with_title(
                "<C-w> Окна и панели",
                vec![
                    truff_core::config::KeyHint {
                        key: Key::Char('h'),
                        description: "Focus left pane".into(),
                        is_prefix: false,
                    },
                    truff_core::config::KeyHint {
                        key: Key::Char('l'),
                        description: "Focus right pane".into(),
                        is_prefix: false,
                    },
                    truff_core::config::KeyHint {
                        key: Key::Char('w'),
                        description: "Cycle pane".into(),
                        is_prefix: false,
                    },
                ],
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
                let context = if self.focus == Focus::Editor {
                    CommandContext::Editor
                } else {
                    CommandContext::Rows
                };
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

        // 2. Route strictly according to active panel focus
        match self.focus {
            Focus::Editor => {
                let Some(view_id) = self.workspace.active_view() else {
                    return ShellAction::Ignored;
                };
                let Some((buffer, view)) = self.workspace.editor_parts_mut(view_id) else {
                    return ShellAction::Ignored;
                };
                let buffer_id = buffer.id();
                let res = self.editor_vim.feed_key(
                    key,
                    InputContext::Editor {
                        view_id,
                        buffer,
                        view,
                    },
                );
                let _ = buffer_id;
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
                    InputResult::VisualRow { down, count } => {
                        ShellAction::VisualRow { down, count }
                    }
                    InputResult::Command { id, argument } => {
                        ShellAction::RegisteredCommand { id, argument }
                    }
                    InputResult::Action(action) => ShellAction::Core(action),
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
            Focus::Sidebar | Focus::RightPanel => {
                if key == Key::Esc && self.row_vim.search_query().is_none() {
                    self.set_focus(Focus::Editor);
                    return ShellAction::FocusChanged(Focus::Editor);
                }
                if matches!(key, Key::Char('i' | 'a' | 'o' | 'I' | 'A' | 'O'))
                    && self.row_vim.search_query().is_none()
                {
                    self.set_focus(Focus::Editor);
                    return self.feed_key(key);
                }
                let model = if self.focus == Focus::Sidebar {
                    &mut self.sidebar_model
                } else {
                    &mut self.right_model
                };
                if key == Key::Char('d')
                    && self.focus == Focus::Sidebar
                    && self.row_vim.search_query().is_none()
                {
                    return model
                        .selected_row()
                        .and_then(|row| row.data.clone())
                        .map(ShellAction::SidebarDeleteFile)
                        .unwrap_or(ShellAction::Ignored);
                }
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
                let result = self.row_vim.feed_key(key, InputContext::Rows(model));
                match result {
                    InputResult::Error(e) => ShellAction::Error(e),
                    InputResult::Ignored => ShellAction::Ignored,
                    _ => ShellAction::Incomplete,
                }
            }
        }
    }
}
