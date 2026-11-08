use truff_core::commands::{CommandContext, CommandId};
use truff_core::config::{KeyBinding, KeymapConfig, KeymapError};
use truff_core::vim::{Key, Mode};

use crate::shell::{Focus, Shell};

pub(in crate::shell) fn default_keymap() -> KeymapConfig {
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
            "style.text_action_menu",
            "Действия с текстом",
        ),
        (
            Mode::VisualChar,
            vec![Key::Char(' '), Key::Char('c')],
            "style.text_action_menu",
            "Действия с текстом",
        ),
        (
            Mode::VisualLine,
            vec![Key::Char(' '), Key::Char('c')],
            "style.text_action_menu",
            "Действия с текстом",
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

    #[inline]
    pub(super) fn command_context(&self) -> CommandContext {
        if self.focus == Focus::Editor {
            CommandContext::Editor
        } else {
            CommandContext::Rows
        }
    }
}
