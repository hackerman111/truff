use truff_core::commands::{Action, CommandContext, CommandId, CommandRegistry};
use truff_core::config::{BindingLookup, KeymapConfig, KeymapError};
use truff_core::text::BytePos;
use truff_core::vim::{InputContext, InputResult, Key, Mode};

use crate::overlays::FlashKind;

use super::{Focus, Shell, ShellAction};

pub(super) fn default_keymap() -> KeymapConfig {
    use truff_core::config::KeyBinding;
    let mut config = KeymapConfig::default();
    for (mode, keys, id, description) in [
        (
            Mode::Insert,
            vec![Key::Ctrl('p')],
            "panel.commands",
            "Палитра команд",
        ),
        (
            Mode::VisualChar,
            vec![Key::Ctrl('p')],
            "panel.commands",
            "Палитра команд",
        ),
        (
            Mode::VisualLine,
            vec![Key::Ctrl('p')],
            "panel.commands",
            "Палитра команд",
        ),
        (
            Mode::Normal,
            vec![Key::Char(':')],
            "app.command_line",
            "Командная строка",
        ),
        (Mode::Normal, vec![Key::Char('?')], "app.help", "Справка"),
        (
            Mode::Normal,
            vec![Key::Ctrl('w'), Key::Char('h')],
            "focus.left",
            "Левая панель",
        ),
        (
            Mode::Normal,
            vec![Key::Ctrl('w'), Key::Char('l')],
            "focus.right",
            "Правая панель",
        ),
        (
            Mode::Normal,
            vec![Key::Ctrl('w'), Key::Char('j')],
            "focus.down",
            "Панель снизу",
        ),
        (
            Mode::Normal,
            vec![Key::Ctrl('w'), Key::Char('k')],
            "focus.up",
            "Панель сверху",
        ),
        (
            Mode::Normal,
            vec![Key::Ctrl('p')],
            "panel.commands",
            "Палитра команд",
        ),
        (
            Mode::Normal,
            vec![Key::Char(' '), Key::Char('c')],
            "style.text_color",
            "Цвет текста",
        ),
        (
            Mode::VisualChar,
            vec![Key::Char(' '), Key::Char('c')],
            "style.text_color",
            "Цвет выделения",
        ),
        (
            Mode::VisualLine,
            vec![Key::Char(' '), Key::Char('c')],
            "style.text_color",
            "Цвет выделения",
        ),
        (
            Mode::VisualChar,
            vec![Key::Char(' '), Key::Char('l')],
            "note.link_selection",
            "Ссылка из выделения",
        ),
        (
            Mode::VisualLine,
            vec![Key::Char(' '), Key::Char('l')],
            "note.link_selection",
            "Ссылка из выделения",
        ),
    ] {
        config.bindings.push(KeyBinding {
            mode,
            keys,
            command: CommandId::from_static(id),
            description: description.into(),
        });
    }
    config
}

impl Shell {
    pub(crate) fn take_keymap_from(&mut self, previous: &mut Self) {
        std::mem::swap(&mut self.keymap, &mut previous.keymap);
        self.custom_keymap = previous.custom_keymap;
        previous.custom_keymap = false;
    }

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
        if let Err(error) = registry.require(&id, context) {
            return ShellAction::Error(error.to_string());
        }
        match id.as_str() {
            "app.help" => {
                self.show_help();
                return ShellAction::Incomplete;
            }
            "app.command_line" => {
                self.start_command();
                return ShellAction::Incomplete;
            }
            "row.activate" => return self.feed_key(Key::Enter),
            "focus.left" | "focus.right" | "focus.up" | "focus.down" => {
                let next = match (id.as_str(), self.focus) {
                    ("focus.left", Focus::RightPanel) | ("focus.right", Focus::Sidebar) => {
                        Focus::Editor
                    }
                    ("focus.left", Focus::Editor) if self.has_sidebar() => Focus::Sidebar,
                    ("focus.right", Focus::Editor) if self.has_right_panel() => Focus::RightPanel,
                    _ => self.focus,
                };
                self.set_focus(next);
                return ShellAction::FocusChanged(next);
            }
            _ => {}
        }
        let panel = match id.as_str() {
            "panel.files" => Some(crate::navigator::NavigatorMode::Files),
            "panel.sessions" => Some(crate::navigator::NavigatorMode::Sessions),
            "panel.boards" => Some(crate::navigator::NavigatorMode::Boards),
            "panel.outline" => Some(crate::navigator::NavigatorMode::Outline),
            "panel.links" => Some(crate::navigator::NavigatorMode::Links),
            "panel.commands" => Some(crate::navigator::NavigatorMode::Commands),
            "style.text_color" if argument.is_empty() => {
                Some(crate::navigator::NavigatorMode::Colors)
            }
            _ => None,
        };
        if let Some(mode) = panel {
            return ShellAction::OpenNavigator(mode);
        }
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
            let context = if self.focus == Focus::Editor {
                CommandContext::Editor
            } else {
                CommandContext::Rows
            };
            return match self.keymap.lookup(self.mode(), &[key], context) {
                BindingLookup::Complete(id) => self.execute_registered(id, String::new()),
                _ => ShellAction::Ignored,
            };
        }
        if let Some(Key::Char(' ')) = self.pending_prefix {
            self.pending_prefix = None;
            self.which_key.hide();
            let context = if self.focus == Focus::Editor {
                CommandContext::Editor
            } else {
                CommandContext::Rows
            };
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
            self.pending_prefix = Some(Key::Char(' '));
            self.which_key.set_prefix_hints_with_title(
                "<Space> Навигатор",
                self.keymap.hints(
                    self.mode(),
                    &[Key::Char(' ')],
                    if self.focus == Focus::Editor {
                        CommandContext::Editor
                    } else {
                        CommandContext::Rows
                    },
                ),
            );
            return ShellAction::Incomplete;
        }
        // 1. Handle pane navigation prefixes (<C-w>h / <C-w>l / <C-w>w)
        if let Some(Key::Ctrl('w')) = self.pending_prefix {
            self.pending_prefix = None;
            self.which_key.hide();
            let context = if self.focus == Focus::Editor {
                CommandContext::Editor
            } else {
                CommandContext::Rows
            };
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
                    return ShellAction::FocusChanged(self.focus);
                }
                Key::Char('j' | 'k') | Key::Up | Key::Down => {
                    return ShellAction::FocusChanged(self.focus);
                }
                _ => return ShellAction::Ignored,
            }
        }

        if key == Key::Ctrl('w') && self.mode() == Mode::Normal {
            self.pending_prefix = Some(key);
            let context = if self.focus == Focus::Editor {
                CommandContext::Editor
            } else {
                CommandContext::Rows
            };
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
            Focus::Editor => {
                let Some(view_id) = self.workspace.active_view() else {
                    return ShellAction::Ignored;
                };

                if key == Key::CtrlEnter {
                    let toggle_tx =
                        self.workspace
                            .editor_parts_mut(view_id)
                            .and_then(|(buffer, view)| {
                                truff_core::markdown::plan_toggle_task(buffer, view.head())
                            });
                    if let Some(tx) = toggle_tx {
                        if let Err(e) = self.apply_transaction(tx) {
                            return ShellAction::Error(e.to_string());
                        }
                        if let Err(e) = self.publish_vim_changes(view_id) {
                            return ShellAction::Error(e.to_string());
                        }
                        return ShellAction::EditorEdit;
                    }
                    return ShellAction::Ignored;
                }

                if self.editor_vim.mode == Mode::Insert
                    && matches!(key, Key::Enter | Key::Tab | Key::ShiftTab)
                {
                    let smart_tx = self.workspace.editor_parts_mut(view_id).and_then(
                        |(buffer, view)| match key {
                            Key::Enter => truff_core::markdown::plan_enter(buffer, view.head()),
                            Key::Tab => truff_core::markdown::plan_tab(buffer, view.head()),
                            Key::ShiftTab => {
                                truff_core::markdown::plan_shift_tab(buffer, view.head())
                            }
                            _ => None,
                        },
                    );
                    if let Some(tx) = smart_tx {
                        if let Err(e) = self.apply_transaction(tx) {
                            return ShellAction::Error(e.to_string());
                        }
                        if let Err(e) = self.publish_vim_changes(view_id) {
                            return ShellAction::Error(e.to_string());
                        }
                        return ShellAction::EditorEdit;
                    }
                }

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
                    InputResult::ScrollCursor(pos) => ShellAction::ScrollCursor(pos),
                    InputResult::FollowLink => ShellAction::RegisteredCommand {
                        id: truff_core::commands::CommandId::new("note.follow_link"),
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
            Focus::Sidebar | Focus::RightPanel => {
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
                if matches!(key, Key::Char('i' | 'o' | 'I' | 'O'))
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
            Focus::Navigator | Focus::CommandBar | Focus::TOC | Focus::Wiki | Focus::Stream => {
                if key == Key::Esc {
                    let prev = self.pop_focus();
                    return ShellAction::FocusChanged(prev);
                }
                ShellAction::Incomplete
            }
        }
    }
}
