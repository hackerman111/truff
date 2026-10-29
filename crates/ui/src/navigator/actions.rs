use super::model::{
    NavigatorAction, NavigatorItemPayload, NavigatorMode, NavigatorState, QuickAction,
};

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
    spec("p", "Закрепить", "Закрепить / открепить вкладку"),
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
            NavigatorMode::Files | NavigatorMode::Folders => FILES,
            NavigatorMode::Sessions => SESSIONS,
            NavigatorMode::Commands => COMMANDS,
            _ => OTHER,
        }
    }

    pub(crate) fn quick_action(&self, key: &str) -> NavigatorAction {
        if key == "↵" {
            return self.primary_action();
        }
        if key == "a" && matches!(self.mode, NavigatorMode::Files | NavigatorMode::Folders) {
            return NavigatorAction::CreateFile {
                parent_dir: self.current_parent_dir(),
            };
        }
        self.context_actions()
            .into_iter()
            .find(|action| {
                action.key == key
                    || (key.eq_ignore_ascii_case("p") && (action.key == "p" || action.key == "P"))
            })
            .map_or(NavigatorAction::None, |action| action.action)
    }
}

impl NavigatorState {
    /// Returns the primary default action for the selected item.
    pub fn primary_action(&self) -> NavigatorAction {
        let Some(item) = self.selected_item() else {
            return NavigatorAction::None;
        };

        if item.disabled_reason.is_some() {
            return NavigatorAction::None;
        }
        match &item.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                if *is_dir {
                    NavigatorAction::RevealFolder { path: path.clone() }
                } else {
                    NavigatorAction::OpenFile { path: path.clone() }
                }
            }
            NavigatorItemPayload::View { view_id, .. } => {
                NavigatorAction::SwitchView { view_id: *view_id }
            }
            NavigatorItemPayload::Vault { path } => {
                NavigatorAction::OpenVault { path: path.clone() }
            }
            NavigatorItemPayload::Location { path, byte } => NavigatorAction::OpenLocation {
                path: path.clone(),
                byte: *byte,
            },
            NavigatorItemPayload::Color { hex } => NavigatorAction::ApplyColor { hex },
            NavigatorItemPayload::Highlight { hex } => NavigatorAction::ApplyHighlight { hex },
            NavigatorItemPayload::Callout { kind } => {
                NavigatorAction::ApplyCallout { kind: kind.clone() }
            }
            NavigatorItemPayload::RemoveStyle => NavigatorAction::RemoveStyle,
            NavigatorItemPayload::Buffer { buffer_id, .. } => NavigatorAction::SwitchBuffer {
                buffer_id: *buffer_id,
            },
            NavigatorItemPayload::Command { command_id } => NavigatorAction::ExecuteCommand {
                command_id: command_id.clone(),
            },
            NavigatorItemPayload::Heading { line, .. }
            | NavigatorItemPayload::Task { line, .. } => {
                NavigatorAction::JumpHeading { line: *line }
            }
            NavigatorItemPayload::Link { target } => NavigatorAction::OpenLink {
                target: target.clone(),
            },
            NavigatorItemPayload::Board { name } => NavigatorAction::SelectCustom {
                id: "board.open".into(),
                data: name.clone(),
            },
            NavigatorItemPayload::Custom { id, data } => NavigatorAction::SelectCustom {
                id: id.clone(),
                data: data.clone(),
            },
        }
    }

    /// Returns contextual actions list when `?` actions menu is open.
    pub fn context_actions(&self) -> Vec<QuickAction> {
        let Some(item) = self.selected_item() else {
            return Vec::new();
        };

        if let Some(reason) = &item.disabled_reason {
            return vec![QuickAction {
                key: String::new(),
                label: "Недоступно".into(),
                description: reason.to_string(),
                action: NavigatorAction::None,
            }];
        }
        match &item.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                let mut acts = Vec::new();
                if !*is_dir {
                    acts.push(QuickAction {
                        key: "↵".to_string(),
                        label: "Открыть заметку".to_string(),
                        description: format!("Открыть файл {}", item.display_title()),
                        action: NavigatorAction::OpenFile { path: path.clone() },
                    });
                }
                acts.push(QuickAction {
                    key: "a".to_string(),
                    label: "Создать заметку".to_string(),
                    description: "Создать новую заметку в текущей директории".to_string(),
                    action: NavigatorAction::CreateFile {
                        parent_dir: self.current_parent_dir(),
                    },
                });
                acts.push(QuickAction {
                    key: "A".to_string(),
                    label: "Создать папку".to_string(),
                    description: "Создать новую директорию".to_string(),
                    action: NavigatorAction::CreateFolder {
                        parent_dir: self.current_parent_dir(),
                    },
                });
                acts.push(QuickAction {
                    key: "r".to_string(),
                    label: "Переименовать".to_string(),
                    description: format!("Изменить имя {}", item.display_title()),
                    action: NavigatorAction::RenameFile { path: path.clone() },
                });
                acts.push(QuickAction {
                    key: "d".to_string(),
                    label: "Удалить".to_string(),
                    description: format!("Переместить {} в корзину", item.display_title()),
                    action: NavigatorAction::DeleteFile { path: path.clone() },
                });
                acts.push(QuickAction {
                    key: "y".to_string(),
                    label: "Скопировать путь".to_string(),
                    description: format!("Скопировать путь {}", path),
                    action: NavigatorAction::CopyPath { path: path.clone() },
                });
                acts
            }
            NavigatorItemPayload::View { view_id, .. } => vec![
                QuickAction {
                    key: "↵".into(),
                    label: "Перейти".into(),
                    description: "Активировать выбранный View".into(),
                    action: NavigatorAction::SwitchView { view_id: *view_id },
                },
                QuickAction {
                    key: "d".into(),
                    label: "Закрыть".into(),
                    description: "Закрыть выбранный View".into(),
                    action: NavigatorAction::CloseView { view_id: *view_id },
                },
                QuickAction {
                    key: "p".into(),
                    label: "Закрепить / открепить".into(),
                    description: "Сохранить вкладку при закрытии остальных".into(),
                    action: NavigatorAction::PinView { view_id: *view_id },
                },
                QuickAction {
                    key: "o".into(),
                    label: "Закрыть остальные".into(),
                    description: "Закреплённые вкладки остаются открыты".into(),
                    action: NavigatorAction::CloseOtherViews { view_id: *view_id },
                },
                QuickAction {
                    key: "u".into(),
                    label: "Открыть закрытую".into(),
                    description: "Восстановить последнюю закрытую вкладку".into(),
                    action: NavigatorAction::ReopenView,
                },
            ],
            NavigatorItemPayload::Buffer { buffer_id, path: _ } => vec![
                QuickAction {
                    key: "↵".to_string(),
                    label: "Перейти к вкладке".to_string(),
                    description: "Сделать вкладку активной".to_string(),
                    action: NavigatorAction::SwitchBuffer {
                        buffer_id: *buffer_id,
                    },
                },
                QuickAction {
                    key: "d".to_string(),
                    label: "Закрыть вкладку".to_string(),
                    description: "Закрыть этот буфер".to_string(),
                    action: NavigatorAction::CloseBuffer {
                        buffer_id: *buffer_id,
                    },
                },
            ],
            NavigatorItemPayload::Command { command_id } => vec![QuickAction {
                key: "↵".to_string(),
                label: "Выполнить команду".to_string(),
                description: item.display_subtitle().unwrap_or_default().to_owned(),
                action: NavigatorAction::ExecuteCommand {
                    command_id: command_id.clone(),
                },
            }],
            _ => vec![QuickAction {
                key: "↵".to_string(),
                label: "Выбрать".to_string(),
                description: "Перейти".to_string(),
                action: self.primary_action(),
            }],
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
            (NavigatorMode::Sessions, &["↵", "p", "d", "/", "?"][..]),
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
        assert_eq!(
            state.quick_action("y"),
            NavigatorAction::CopyPath {
                path: "docs/Note.md".into()
            }
        );
    }
}
