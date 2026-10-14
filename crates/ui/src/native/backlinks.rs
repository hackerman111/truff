use super::editor::color;
use crate::app::Application;
use crate::shell::Focus;
use crate::theme::Theme;
use eframe::egui;

pub(super) fn show(application: &mut Application, ui: &mut egui::Ui, theme: &Theme) {
    if !application.backlinks.is_empty() {
        egui::Panel::right("references")
            .frame(
                egui::Frame::new()
                    .fill(color(theme.bg_secondary))
                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                    .inner_margin(egui::Margin::symmetric(8, 8)),
            )
            .default_size(220.0)
            .min_size(140.0)
            .max_size(600.0)
            .resizable(true)
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new("BACKLINKS")
                        .size(11.0)
                        .strong()
                        .color(color(theme.text_muted)),
                );
                ui.add_space(4.0);
                let mut open = None;
                egui::ScrollArea::vertical().show_rows(
                    ui,
                    24.0,
                    application.backlinks.len(),
                    |ui, rows| {
                        for index in rows {
                            let is_selected = index == application.backlink_selection;
                            let rich =
                                egui::RichText::new(&application.backlinks[index].source_path)
                                    .size(14.0)
                                    .color(if is_selected {
                                        color(theme.text_primary)
                                    } else {
                                        color(theme.text_muted)
                                    });
                            if ui.selectable_label(is_selected, rich).clicked() {
                                open = Some(index);
                            }
                        }
                    },
                );
                if let Some(index) = open {
                    application.backlink_selection = index;
                    application.shell.set_focus(Focus::RightPanel);
                    if let Err(error) = application.open_backlink() {
                        application.status = error.to_string();
                    }
                }
            });
    }
}
