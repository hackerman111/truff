use super::super::{editor::color, icons, widgets};
use crate::app::Application;
use crate::theme::Theme;
use truff_core::markdown::NoteTemplateKind;

#[derive(Default)]
pub struct FileDialog {
    pub open: bool,
    pub path: String,
    pub selected_template: Option<NoteTemplateKind>,
    pub error: Option<String>,
}

impl FileDialog {
    pub(super) fn show(
        &mut self,
        application: &mut Application,
        context: &egui::Context,
        theme: &Theme,
    ) {
        if self.open {
            let mut close_dialog = false;
            let mut submit_creation = false;
            crate::components::dialog(context, theme, "file_dialog", |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Новая заметка")
                                .size(theme.geometry.ui_font_size + 3.0)
                                .strong()
                                .color(color(theme.text_primary)),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Укажите имя файла и при необходимости выберите шаблон (Tab — переключить шаблон)",
                            )
                            .size(theme.geometry.ui_font_size)
                            .color(color(theme.text_muted)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if widgets::icon_button(ui, icons::Icon::Close, "Закрыть").clicked()
                        {
                            close_dialog = true;
                        }
                    });
                });

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Шаблон заметки")
                        .size(theme.geometry.ui_font_size)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                    let none_active = self.selected_template.is_none();
                    if ui
                        .selectable_label(
                            none_active,
                            egui::RichText::new("Без шаблона").size(11.5),
                        )
                        .clicked()
                    {
                        self.selected_template = None;
                    }
                    for kind in NoteTemplateKind::all() {
                        let is_active = self.selected_template == Some(*kind);
                        if ui
                            .selectable_label(
                                is_active,
                                egui::RichText::new(kind.short_label_ru()).size(11.5),
                            )
                            .on_hover_text(kind.label_ru())
                            .clicked()
                        {
                            self.selected_template = Some(*kind);
                            if *kind == NoteTemplateKind::WikiMaster && self.path.trim().is_empty()
                            {
                                self.path = "index.md".to_string();
                            } else if *kind == NoteTemplateKind::KanbanBoard
                                && self.path.trim().is_empty()
                            {
                                self.path = "board.kanban.md".to_string();
                            }
                        }
                    }
                });

                if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                    self.selected_template = match self.selected_template {
                        None => Some(NoteTemplateKind::ZettelkastenPermanent),
                        Some(NoteTemplateKind::SqlDatabase) => None,
                        Some(k) => Some(k.next()),
                    };
                }

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Имя файла")
                        .size(theme.geometry.ui_font_size)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);

                let id = egui::Id::new("file_path_input");
                if ui.memory(|memory| memory.focused().is_none()) {
                    ui.memory_mut(|memory| memory.request_focus(id));
                }
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.path)
                        .id(id)
                        .desired_width(ui.available_width())
                        .margin(egui::Margin::symmetric(10, 8)),
                );
                if ui.memory(|memory| memory.focused().is_none()) {
                    response.request_focus();
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit_creation = true;
                }

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Например: note.md, zk/idea.md или sprint.kanban.md")
                        .size(theme.geometry.ui_font_size - 2.0)
                        .color(color(theme.text_muted)),
                );

                if let Some(err) = &self.error {
                    ui.add_space(6.0);
                    ui.colored_label(color(theme.state_error), err);
                }

                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if widgets::primary_button(ui, "Создать").clicked() {
                        submit_creation = true;
                    }
                    if widgets::secondary_button(ui, "Отмена").clicked() {
                        self.error = None;
                        close_dialog = true;
                    }
                });

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.error = None;
                    close_dialog = true;
                }
            });

            if submit_creation {
                let trimmed = self.path.trim().trim_matches(['/', '\\']);
                if trimmed.is_empty() {
                    self.error = Some("File path cannot be empty".into());
                } else {
                    match application.create_note_with_template(trimmed, self.selected_template) {
                        Ok(()) => {
                            self.error = None;
                            self.path.clear();
                            close_dialog = true;
                        }
                        Err(err) => {
                            self.error = Some(err.to_string());
                        }
                    }
                }
            }

            if close_dialog {
                self.open = false;
            }
        }
    }
}
