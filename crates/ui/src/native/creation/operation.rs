use crate::{app::Application, components, theme::Theme};

enum Operation {
    Rename {
        from: String,
        to: String,
    },
    Trash {
        path: String,
    },
    BatchTrash {
        paths: Vec<String>,
    },
    BatchRename {
        paths: Vec<String>,
        pattern: String,
        replace: String,
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
    pub fn batch_trash(&mut self, paths: Vec<String>) {
        self.operation = Some(Operation::BatchTrash { paths });
        self.error = None;
        self.focus = true;
    }
    pub fn batch_rename(&mut self, paths: Vec<String>) {
        self.operation = Some(Operation::BatchRename {
            paths,
            pattern: String::new(),
            replace: String::new(),
        });
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
                Operation::BatchTrash { paths } => {
                    ui.heading("Переместить файлы в корзину?");
                    ui.label(format!("Выбрано файлов: {}", paths.len()));
                    ui.label("Все выбранные файлы будут перемещены в корзину хранилища.");
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for path in paths.iter().take(20) {
                                ui.label(
                                    egui::RichText::new(format!("• {path}"))
                                        .size(theme.geometry.ui_font_size - 2.0),
                                );
                            }
                            if paths.len() > 20 {
                                ui.label(format!("... и еще {} файлов", paths.len() - 20));
                            }
                        });
                }
                Operation::BatchRename {
                    paths,
                    pattern,
                    replace,
                } => {
                    ui.heading("Пакетное переименование");
                    ui.label(format!("Выбрано файлов: {}", paths.len()));
                    ui.label("Замена текста в именах (поддерживается {n} для номера):");
                    ui.horizontal(|ui| {
                        ui.label("Найти:");
                        let id = egui::Id::new("batch_rename_pattern");
                        if self.focus {
                            ui.memory_mut(|memory| memory.request_focus(id));
                        }
                        ui.add(egui::TextEdit::singleline(pattern).id(id).hint_text("текст"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Заменить:");
                        ui.add(egui::TextEdit::singleline(replace).hint_text("новое_{n}"));
                    });
                    ui.label("Предпросмотр:");
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for (i, p) in paths.iter().enumerate().take(8) {
                                let old_name = std::path::Path::new(p)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(p);
                                let rep = replace.replace("{n}", &(i + 1).to_string());
                                let new_name = if pattern.is_empty() {
                                    if replace.contains("{n}") {
                                        rep
                                    } else {
                                        old_name.to_string()
                                    }
                                } else {
                                    old_name.replace(pattern.as_str(), &rep)
                                };
                                ui.label(
                                    egui::RichText::new(format!("{old_name} → {new_name}"))
                                        .size(theme.geometry.ui_font_size - 2.0),
                                );
                            }
                        });
                }
            }
            if let Some(error) = &self.error {
                ui.colored_label(components::color(theme.state_error), error);
            }
            ui.horizontal(|ui| {
                let response = ui.button(
                    if matches!(
                        operation,
                        Operation::Trash { .. } | Operation::BatchTrash { .. }
                    ) {
                        "В корзину"
                    } else if matches!(operation, Operation::Link { .. }) {
                        "Создать ссылку"
                    } else if matches!(operation, Operation::BatchRename { .. }) {
                        "Переименовать все"
                    } else {
                        "Переименовать"
                    },
                );
                if self.focus
                    && matches!(
                        operation,
                        Operation::Trash { .. } | Operation::BatchTrash { .. }
                    )
                {
                    response.request_focus();
                }
                submit |=
                    response.clicked() || ui.input(|input| input.key_pressed(egui::Key::Enter));
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
                Operation::BatchTrash { paths } => application.trash_files(paths),
                Operation::BatchRename {
                    paths,
                    pattern,
                    replace,
                } => {
                    let mut renames = Vec::new();
                    for (i, from) in paths.iter().enumerate() {
                        let from_path = std::path::Path::new(from);
                        let old_name = from_path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(from);
                        let rep = replace.replace("{n}", &(i + 1).to_string());
                        let new_name = if pattern.is_empty() {
                            if replace.contains("{n}") {
                                rep
                            } else {
                                old_name.to_string()
                            }
                        } else {
                            old_name.replace(pattern.as_str(), &rep)
                        };
                        if new_name != old_name {
                            let to = from_path
                                .parent()
                                .map(|p| p.join(&new_name))
                                .unwrap_or_else(|| std::path::PathBuf::from(&new_name));
                            renames.push((from.clone(), to.to_string_lossy().into_owned()));
                        }
                    }
                    application.batch_rename(renames)
                }
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
