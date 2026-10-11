use super::super::{editor::color, icons, widgets};
use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

pub struct FolderDialog {
    pub open: bool,
    pub path: String,
    pub create_initial_note: bool,
    pub error: Option<String>,
}
impl Default for FolderDialog {
    fn default() -> Self {
        Self {
            open: false,
            path: String::new(),
            create_initial_note: true,
            error: None,
        }
    }
}
impl FolderDialog {
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

            egui::Window::new("create_folder_window")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .default_width(440.0)
                .max_width(440.0)
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
                                egui::RichText::new("Новая папка")
                                    .size(16.0)
                                    .strong()
                                    .color(color(theme.text_primary)),
                            );
                            ui.label(
                                egui::RichText::new("Создание новой директории в хранилище")
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
                        egui::RichText::new("Имя папки")
                            .size(12.0)
                            .strong()
                            .color(color(theme.text_primary)),
                    );
                    ui.add_space(4.0);

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.path)
                            .desired_width(400.0)
                            .margin(egui::Margin::symmetric(10, 8)),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit_creation = true;
                    }

                    ui.add_space(10.0);
                    ui.checkbox(&mut self.create_initial_note, "Создать index.md в папке");

                    if let Some(err) = &self.error {
                        ui.add_space(6.0);
                        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
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
                let folder = self.path.trim();
                let res = if self.create_initial_note {
                    let note_path = format!("{folder}/index.md");
                    application.create_note(&note_path)
                } else {
                    application.create_folder(folder)
                };
                match res {
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

            if close_dialog {
                self.open = false;
            }
        }
    }
}
