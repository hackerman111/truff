use crate::commands::CommandId;
use crate::vim::{Key, Mode};

use super::{KeyBinding, KeymapConfig, LimitsConfig};

impl Default for KeymapConfig {
    fn default() -> Self {
        let bindings = vec![
            // Motions
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('h')],
                command: CommandId::from_static("motion.left"),
                description: "Move left".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('j')],
                command: CommandId::from_static("motion.down"),
                description: "Move down".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('k')],
                command: CommandId::from_static("motion.up"),
                description: "Move up".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('l')],
                command: CommandId::from_static("motion.right"),
                description: "Move right".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('w')],
                command: CommandId::from_static("motion.word_forward"),
                description: "Word forward".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('b')],
                command: CommandId::from_static("motion.word_backward"),
                description: "Word backward".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('e')],
                command: CommandId::from_static("motion.word_end"),
                description: "End of word".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('0')],
                command: CommandId::from_static("motion.line_start"),
                description: "Line start".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('^')],
                command: CommandId::from_static("motion.first_non_blank"),
                description: "First non-blank character".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('$')],
                command: CommandId::from_static("motion.line_end"),
                description: "Line end".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('g'), Key::Char('g')],
                command: CommandId::from_static("motion.first_line"),
                description: "First line of buffer".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('g'), Key::Char('j')],
                command: CommandId::from_static("motion.screen_down"),
                description: "Screen line down".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('g'), Key::Char('k')],
                command: CommandId::from_static("motion.screen_up"),
                description: "Screen line up".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('G')],
                command: CommandId::from_static("motion.last_line"),
                description: "Last line of buffer".to_string(),
            },
            // Edits & operators
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('d'), Key::Char('d')],
                command: CommandId::from_static("edit.delete_line"),
                description: "Delete line".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('c'), Key::Char('c')],
                command: CommandId::from_static("edit.change_line"),
                description: "Change line".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('y'), Key::Char('y')],
                command: CommandId::from_static("edit.yank_line"),
                description: "Yank line".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('x')],
                command: CommandId::from_static("edit.delete_char"),
                description: "Delete character".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('p')],
                command: CommandId::from_static("edit.paste_after"),
                description: "Paste after".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('P')],
                command: CommandId::from_static("edit.paste_before"),
                description: "Paste before".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('u')],
                command: CommandId::from_static("edit.undo"),
                description: "Undo".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Ctrl('r')],
                command: CommandId::from_static("edit.redo"),
                description: "Redo".to_string(),
            },
            KeyBinding {
                mode: Mode::Normal,
                keys: vec![Key::Char('.')],
                command: CommandId::from_static("edit.repeat"),
                description: "Repeat last edit".to_string(),
            },
        ];

        Self {
            bindings,
            limits: LimitsConfig::default(),
        }
    }
}
