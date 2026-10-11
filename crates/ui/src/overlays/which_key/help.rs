use truff_core::config::KeyHint;
use truff_core::vim::Key;

use super::{WhichKeyCategory, WhichKeyOverlay};

impl WhichKeyOverlay {
    /// Default structured categories for the help cheatsheet.
    pub fn default_help_categories() -> Vec<WhichKeyCategory> {
        vec![
            WhichKeyCategory {
                name: "Навигация (Motions)".to_string(),
                hints: vec![
                    KeyHint {
                        key: Key::Char('h'),
                        description: "Влево".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('j'),
                        description: "Вниз".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('k'),
                        description: "Вверх".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('l'),
                        description: "Вправо".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('w'),
                        description: "Слово вперёд".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('b'),
                        description: "Слово назад".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('e'),
                        description: "Конец слова".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('0'),
                        description: "Начало строки".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('$'),
                        description: "Конец строки".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('g'),
                        description: "gg: Первая строка".into(),
                        is_prefix: true,
                    },
                    KeyHint {
                        key: Key::Char('G'),
                        description: "Последняя строка".into(),
                        is_prefix: false,
                    },
                ],
            },
            WhichKeyCategory {
                name: "Редактирование (Edits)".to_string(),
                hints: vec![
                    KeyHint {
                        key: Key::Char('i'),
                        description: "Режим вставки".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('a'),
                        description: "Вставка после курсора".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('x'),
                        description: "Удалить символ".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('d'),
                        description: "dd: Удалить строку".into(),
                        is_prefix: true,
                    },
                    KeyHint {
                        key: Key::Char('c'),
                        description: "cc: Изменить строку".into(),
                        is_prefix: true,
                    },
                    KeyHint {
                        key: Key::Char('y'),
                        description: "yy: Скопировать строку".into(),
                        is_prefix: true,
                    },
                    KeyHint {
                        key: Key::Char('p'),
                        description: "Вставить после".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('P'),
                        description: "Вставить до".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('u'),
                        description: "Отмена (Undo)".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Ctrl('r'),
                        description: "Повтор (Redo)".into(),
                        is_prefix: false,
                    },
                ],
            },
            WhichKeyCategory {
                name: "Окна и режимы (Panels)".to_string(),
                hints: vec![
                    KeyHint {
                        key: Key::Ctrl('w'),
                        description: "<C-w> h/l: Переход между панелями".into(),
                        is_prefix: true,
                    },
                    KeyHint {
                        key: Key::Char('s'),
                        description: "Flash-прыжок по словам".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('v'),
                        description: "Визуальное выделение".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('V'),
                        description: "Выделение строк".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('?'),
                        description: "Эта справка".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Esc,
                        description: "Normal mode / Закрыть".into(),
                        is_prefix: false,
                    },
                ],
            },
            WhichKeyCategory {
                name: "Команды (:commands)".to_string(),
                hints: vec![
                    KeyHint {
                        key: Key::Char('w'),
                        description: ":w — Сохранить заметку".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('q'),
                        description: ":wq — Сохранить и закрыть".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('v'),
                        description: ":vault <путь> — Открыть хранилище".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('n'),
                        description: ":new <file.md> — Создать заметку".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('l'),
                        description: ":link <note> — Создать ссылку".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('b'),
                        description: ":backlinks — Обратные ссылки".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('t'),
                        description: ":theme <nord/dark/light>".into(),
                        is_prefix: false,
                    },
                    KeyHint {
                        key: Key::Char('h'),
                        description: ":help — Открыть справку".into(),
                        is_prefix: false,
                    },
                ],
            },
        ]
    }
}
