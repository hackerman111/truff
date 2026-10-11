use super::super::{editor::color, icons, widgets};
use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

pub struct VaultDialog {
    pub open: bool,
    pub path: String,
    pub initial_note_name: String,
    pub create_starter_note: bool,
    pub error: Option<String>,
}
impl Default for VaultDialog {
    fn default() -> Self {
        Self {
            open: false,
            path: "~/TruffVault".into(),
            initial_note_name: "Welcome.md".into(),
            create_starter_note: true,
            error: None,
        }
    }
}
impl VaultDialog {
    pub(super) fn show(
        &mut self,
        application: &mut Application,
        context: &egui::Context,
        theme: &Theme,
    ) {
        if self.open {
            let mut close_dialog = false;
            let mut submit_creation = false;
            let modal_frame = egui::Frame::new()
                .fill(color(theme.bg_secondary))
                .stroke(egui::Stroke::new(1.0, color(theme.border)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(20));

            egui::Window::new("create_vault_window")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .default_width(460.0)
                .max_width(460.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .frame(modal_frame)
                .show(context, |ui| {
                    ui.visuals_mut().extreme_bg_color =
                        color(crate::theme::Color::from_hex("#20242c").unwrap());
                    ui.visuals_mut().widgets.inactive.bg_stroke =
                        egui::Stroke::new(1.0, color(theme.border));
                    ui.visuals_mut().widgets.hovered.bg_stroke =
                        egui::Stroke::new(1.0, color(theme.accent));

                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Создать или открыть хранилище")
                                    .size(16.0)
                                    .strong()
                                    .color(color(theme.text_primary)),
                            );
                            ui.label(
                                egui::RichText::new("Выберите директорию для заметок")
                                    .size(12.0)
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

                    ui.add_space(14.0);
                    ui.label(
                        egui::RichText::new("Расположение")
                            .size(12.0)
                            .strong()
                            .color(color(theme.text_primary)),
                    );
                    ui.add_space(4.0);

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.path)
                            .desired_width(420.0)
                            .margin(egui::Margin::symmetric(10, 8)),
                    );

                    if response.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Tab))
                        && let Some(completed) =
                            crate::app::path_completion::tab_complete(&self.path)
                    {
                        self.path = completed;
                    }
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit_creation = true;
                    }

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Поддерживаются абсолютные и относительные пути (~, ./, ../)",
                        )
                        .size(11.0)
                        .color(color(theme.text_muted)),
                    );

                    let suggestions =
                        crate::app::path_completion::complete_directory_path(&self.path);
                    if !suggestions.is_empty() {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Подсказки:")
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                        ui.horizontal_wrapped(|ui| {
                            for suggestion in suggestions.iter().take(6) {
                                if ui.small_button(suggestion).clicked() {
                                    self.path = suggestion.clone();
                                }
                            }
                        });
                    }

                    ui.add_space(10.0);
                    ui.checkbox(&mut self.create_starter_note, "Создать стартовую заметку");
                    if self.create_starter_note {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Имя файла:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.initial_note_name)
                                    .desired_width(280.0)
                                    .margin(egui::Margin::symmetric(8, 6)),
                            );
                        });
                    }

                    if let Some(err) = &self.error {
                        ui.add_space(6.0);
                        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
                    }

                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if widgets::primary_button(ui, "Создать и открыть").clicked()
                        {
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
                let note = if self.create_starter_note && !self.initial_note_name.trim().is_empty()
                {
                    Some(self.initial_note_name.as_str())
                } else {
                    None
                };
                let path = std::path::Path::new(&self.path);
                match application.create_and_open_vault(path, note) {
                    Ok(()) => {
                        self.error = None;
                        close_dialog = true;
                    }
                    Err(err) => {
                        self.error = Some(err.to_string());
                    }
                }
            }

            if close_dialog {
                self.open = false;
            }
        }
    }
}
