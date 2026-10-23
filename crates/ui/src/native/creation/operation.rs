use crate::{app::Application, components, theme::Theme};

enum Operation {
    Rename {
        from: String,
        to: String,
    },
    Trash {
        path: String,
    },
    Link {
        target: crate::app::SelectionTarget,
        note: String,
    },
}

#[derive(Default)]
pub struct OperationDialog {
    operation: Option<Operation>,
    error: Option<String>,
    focus: bool,
}

impl OperationDialog {
    pub fn is_open(&self) -> bool {
        self.operation.is_some()
    }
    pub fn rename(&mut self, path: String) {
        self.operation = Some(Operation::Rename {
            to: path.clone(),
            from: path,
        });
        self.error = None;
        self.focus = true;
    }
    pub(crate) fn link(&mut self, target: crate::app::SelectionTarget) {
        self.operation = Some(Operation::Link {
            target,
            note: String::new(),
        });
        self.error = None;
        self.focus = true;
    }
    pub fn trash(&mut self, path: String) {
        self.operation = Some(Operation::Trash { path });
        self.error = None;
        self.focus = true;
    }

    pub(super) fn show(
        &mut self,
        application: &mut Application,
        context: &egui::Context,
        theme: &Theme,
    ) {
        let Some(operation) = &mut self.operation else {
            return;
        };
        let mut submit = false;
        let mut close = false;
        let mut sizing = false;
        components::dialog(context, theme, "file_operation_dialog", |ui| {
            sizing = ui.is_sizing_pass();
            match operation {
                Operation::Rename { from, to } => {
                    ui.heading("Переименовать");
                    ui.label(format!("Исходный путь: {from}"));
                    ui.label("Ссылки на заметку обновит Store. Несохранённые изменения проверяются перед операцией.");
                    let id = egui::Id::new("rename_path_input");
                    if self.focus {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(to)
                            .id(id)
                            .desired_width(ui.available_width()),
                    );
                    submit |= response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                }
                Operation::Link { note, .. } => {
                    ui.heading("Ссылка из выделения");
                    ui.label("Цель: заметка или заметка#заголовок");
                    let id = egui::Id::new("link_target_input");
                    if self.focus {
                        ui.memory_mut(|memory| memory.request_focus(id));
                    }
                    let response = ui.add(
                        egui::TextEdit::singleline(note)
                            .id(id)
                            .desired_width(ui.available_width()),
                    );
                    submit |= response.lost_focus()
                        && ui.input(|input| input.key_pressed(egui::Key::Enter));
                }
                Operation::Trash { path } => {
                    ui.heading("Переместить в корзину?");
                    ui.label(path.as_str());
                    ui.label("Заметка будет перемещена в корзину хранилища. Несохранённые изменения блокируют операцию.");
                }
            }
            if let Some(error) = &self.error {
                ui.colored_label(components::color(theme.state_error), error);
            }
            ui.horizontal(|ui| {
                let response = ui.button(if matches!(operation, Operation::Trash { .. }) {
                    "В корзину"
                } else if matches!(operation, Operation::Link { .. }) {
                    "Создать ссылку"
                } else {
                    "Переименовать"
                });
                if self.focus && matches!(operation, Operation::Trash { .. }) {
                    response.request_focus();
                }
                submit |= response.clicked();
                close |= ui.button("Отмена · Esc").clicked();
            });
            close |= ui.input(|input| input.key_pressed(egui::Key::Escape));
        });
        // A new egui Area first measures without interaction; focus must survive that pass.
        self.focus &= sizing;
        if submit {
            let result = match operation {
                Operation::Rename { from, to } => application.rename_path(from, to),
                Operation::Trash { path } => application.trash(path),
                Operation::Link { target, note } => application.link_target(target, note),
            };
            match result {
                Ok(()) => close = true,
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        if close {
            self.operation = None;
            self.error = None;
        }
    }
}
