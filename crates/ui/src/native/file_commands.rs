use super::NativeApp;
use crate::shell::Focus;

impl NativeApp {
    fn file_target(&self) -> Option<String> {
        if self.application.shell.focus() != Focus::Sidebar {
            return self.application.active_relative_path();
        }
        self.application.shell.sidebar_selected_path()
    }

    /// Native targets for the same stable command IDs used by registry/keymap/colon.
    pub(super) fn file_command(&mut self, id: &str, argument: &str) -> bool {
        if self.application.shell.focus() == Focus::RightPanel
            && argument.is_empty()
            && matches!(id, "file.rename" | "file.move" | "file.trash")
        {
            self.application.status = "Выберите заметку в дереве файлов".into();
            return true;
        }
        match id {
            "note.create" | "file.create" if argument.is_empty() => {
                self.dialogs.file.open = true;
                self.dialogs.file.error = None;
                self.dialogs.file.path = self
                    .application
                    .shell
                    .sidebar_creation_parent()
                    .map_or(String::new(), |path| format!("{path}/"));
                self.dialogs.file.needs_cursor_end = true;
            }
            "folder.create" if argument.is_empty() => {
                self.dialogs.folder.open = true;
                self.dialogs.folder.error = None;
                self.dialogs.folder.path = self
                    .application
                    .shell
                    .sidebar_creation_parent()
                    .map_or(String::new(), |path| format!("{path}/"));
                self.dialogs.folder.needs_cursor_end = true;
            }
            "file.rename" | "file.move" if argument.is_empty() => match self.file_target() {
                Some(path)
                    if self.application.is_note_operation_target(&path)
                        || self.application.is_folder_operation_target(&path) =>
                {
                    self.dialogs.operation.rename(path)
                }
                Some(_)
                    if self
                        .application
                        .shell
                        .sidebar_model()
                        .selected_row()
                        .is_some_and(|r| r.is_folder) =>
                {
                    self.application.status =
                        "Операции с папками недоступны без открытого хранилища".into();
                }
                Some(_) => {
                    self.application.status = "Цель недоступна для переименования".into();
                }
                None => self.application.status = "Выберите заметку или папку".into(),
            },
            "file.trash" => {
                let path = if argument.is_empty() {
                    self.file_target()
                } else {
                    Some(argument.to_owned())
                };
                match path {
                    Some(path)
                        if self.application.is_note_operation_target(&path)
                            || self.application.is_folder_operation_target(&path) =>
                    {
                        self.dialogs.operation.trash(path)
                    }
                    Some(_)
                        if self
                            .application
                            .shell
                            .sidebar_model()
                            .selected_row()
                            .is_some_and(|r| r.is_folder) =>
                    {
                        self.application.status =
                            "Операции с папками недоступны без открытого хранилища".into();
                    }
                    Some(_) => {
                        self.application.status = "Цель недоступна для удаления".into();
                    }
                    None => self.application.status = "Выберите заметку или папку".into(),
                }
            }
            "buffer.open" if argument.is_empty() => {
                self.open_navigator(crate::navigator::NavigatorMode::Files)
            }
            "buffer.select" if argument.is_empty() => {
                self.open_navigator(crate::navigator::NavigatorMode::Sessions)
            }
            _ => return false,
        }
        true
    }
}
