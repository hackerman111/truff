use super::model::{NavigatorAction, NavigatorMode, NavigatorState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickActionSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

const fn spec(
    key: &'static str,
    label: &'static str,
    description: &'static str,
) -> QuickActionSpec {
    QuickActionSpec {
        key,
        label,
        description,
    }
}

const FILES: &[QuickActionSpec] = &[
    spec("↵", "Открыть", "Открыть выбранную заметку"),
    spec("a", "Создать", "Создать новую заметку"),
    spec("r", "Переименовать", "Переименовать заметку или папку"),
    spec("d", "Удалить", "Удалить в корзину"),
    spec("/", "Поиск", "Режим текстового поиска"),
    spec("?", "Все действия", "Показать полное меню действий"),
];
const SESSIONS: &[QuickActionSpec] = &[
    spec("↵", "Перейти", "Активировать вкладку"),
    spec("d", "Закрыть", "Закрыть буфер"),
    spec("/", "Поиск", "Фильтр буферов"),
    spec("?", "Действия", "Все операции со вкладками"),
];
const COMMANDS: &[QuickActionSpec] = &[
    spec("↵", "Выполнить", "Запустить команду"),
    spec("/", "Поиск", "Поиск команд"),
    spec("?", "Справка", "Подробности команды"),
];
const OTHER: &[QuickActionSpec] = &[
    spec("↵", "Выбрать", "Выполнить переход"),
    spec("/", "Поиск", "Фильтр"),
];

impl NavigatorState {
    pub fn quick_actions(&self) -> &'static [QuickActionSpec] {
        match self.mode {
            NavigatorMode::Files => FILES,
            NavigatorMode::Sessions => SESSIONS,
            NavigatorMode::Commands => COMMANDS,
            _ => OTHER,
        }
    }

    pub(crate) fn quick_action(&self, key: &str) -> NavigatorAction {
        match (self.mode, key) {
            (_, "↵") => self.primary_action(),
            (NavigatorMode::Files, "a") => NavigatorAction::CreateFile {
                parent_dir: self.current_parent_dir(),
            },
            (NavigatorMode::Files, "r") => self
                .current_selected_path()
                .map_or(NavigatorAction::None, |path| NavigatorAction::RenameFile {
                    path,
                }),
            (NavigatorMode::Files, "d") => self
                .current_selected_path()
                .map_or(NavigatorAction::None, |path| NavigatorAction::DeleteFile {
                    path,
                }),
            (NavigatorMode::Sessions, "d") => self
                .current_selected_buffer()
                .map_or(NavigatorAction::None, |buffer_id| {
                    NavigatorAction::CloseBuffer { buffer_id }
                }),
            _ => NavigatorAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigator::NavigatorItem;

    #[test]
    fn static_action_keys_keep_mode_order_and_file_actions_resolve_on_demand() {
        let mut state = NavigatorState::new();
        for (mode, keys) in [
            (NavigatorMode::Files, &["↵", "a", "r", "d", "/", "?"][..]),
            (NavigatorMode::Sessions, &["↵", "d", "/", "?"][..]),
            (NavigatorMode::Commands, &["↵", "/", "?"][..]),
            (NavigatorMode::Boards, &["↵", "/"][..]),
            (NavigatorMode::Outline, &["↵", "/"][..]),
            (NavigatorMode::Links, &["↵", "/"][..]),
        ] {
            state.mode = mode;
            assert_eq!(
                state
                    .quick_actions()
                    .iter()
                    .map(|item| item.key)
                    .collect::<Vec<_>>(),
                keys
            );
        }

        state.open_mode(
            NavigatorMode::Files,
            "Vault",
            vec![NavigatorItem::new_file(
                1,
                "Note.md",
                "docs/Note.md".into(),
                false,
                0,
            )],
        );
        assert_eq!(
            state.quick_action("a"),
            NavigatorAction::CreateFile {
                parent_dir: Some("docs".into())
            }
        );
        assert_eq!(
            state.quick_action("r"),
            NavigatorAction::RenameFile {
                path: "docs/Note.md".into()
            }
        );
        assert_eq!(
            state.quick_action("d"),
            NavigatorAction::DeleteFile {
                path: "docs/Note.md".into()
            }
        );
        assert_eq!(
            state.quick_action("↵"),
            NavigatorAction::OpenFile {
                path: "docs/Note.md".into()
            }
        );
    }
}
