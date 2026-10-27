use super::super::{editor::color, widgets};
use crate::app::Application;
use crate::theme::Theme;

#[derive(Default)]
pub struct QuitConfirmDialog {
    pub open: bool,
    pub error: Option<String>,
}

impl QuitConfirmDialog {
    pub(super) fn show(
        &mut self,
        application: &mut Application,
        context: &egui::Context,
        theme: &Theme,
    ) {
        if !self.open {
            return;
        }
        let mut save_and_quit = false;
        let mut quit_without_save = false;
        let mut close_dialog = false;

        crate::components::dialog(context, theme, "quit_confirm_dialog", |ui| {
            ui.label(
                egui::RichText::new("Несохранённые изменения")
                    .size(theme.geometry.ui_font_size + 2.0)
                    .strong()
                    .color(color(theme.text_primary)),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "В хранилище есть несохранённые заметки. Сохранить изменения перед выходом?",
                )
                .size(theme.geometry.ui_font_size)
                .color(color(theme.text_muted)),
            );

            if let Some(err) = &self.error {
                ui.add_space(6.0);
                ui.colored_label(color(theme.state_error), err);
            }

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if widgets::primary_button(ui, "Сохранить и выйти · Enter").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    save_and_quit = true;
                }
                if widgets::secondary_button(ui, "Выйти без сохранения").clicked()
                {
                    quit_without_save = true;
                }
                if widgets::secondary_button(ui, "Отмена · Esc").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    close_dialog = true;
                }
            });
        });

        if save_and_quit {
            if let Err(e) = application.save_all_dirty() {
                self.error = Some(e.to_string());
            } else {
                match application.shutdown() {
                    Ok(()) => {
                        self.open = false;
                        context.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
        } else if quit_without_save {
            match application.shutdown() {
                Ok(()) => {
                    self.open = false;
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        } else if close_dialog {
            self.open = false;
            self.error = None;
        }
    }
}
