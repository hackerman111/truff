use super::editor::color;
use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

pub(super) fn show(application: &Application, ui: &mut egui::Ui, theme: &Theme) {
    egui::Panel::bottom("status")
        .frame(
            egui::Frame::new()
                .fill(color(theme.bg_tertiary))
                .stroke(egui::Stroke::new(1.0, color(theme.border)))
                .inner_margin(egui::Margin::symmetric(12, 4)),
        )
        .show(ui, |ui| {
            if let Some(command) = application.shell.command() {
                ui.label(
                    egui::RichText::new(format!(":{command}"))
                        .monospace()
                        .color(color(theme.accent))
                        .strong(),
                );
            } else {
                ui.horizontal(|ui| {
                    let mode = application.shell.mode();
                    let mode_str = mode.to_string();
                    let (pill_bg, pill_fg) = match mode {
                        truff_core::vim::Mode::Insert => {
                            (color(theme.accent), color(theme.bg_primary))
                        }
                        truff_core::vim::Mode::VisualChar | truff_core::vim::Mode::VisualLine => {
                            (color(theme.callout_warning), color(theme.bg_primary))
                        }
                        _ => (color(theme.border), color(theme.text_primary)),
                    };
                    egui::Frame::new()
                        .fill(pill_bg)
                        .corner_radius(egui::CornerRadius::same(3))
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(mode_str)
                                    .size(11.0)
                                    .strong()
                                    .color(pill_fg),
                            );
                        });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("{:?}", application.shell.focus()))
                            .size(11.5)
                            .color(color(theme.text_muted)),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(&application.status)
                            .size(11.5)
                            .color(color(theme.text_muted)),
                    );

                    // Right-aligned cursor position and metadata
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("Markdown · UTF-8")
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                        ui.add_space(8.0);
                        let cursor_str = if let Some(view) = application.shell.editor_view()
                            && let Some(buffer) = application.shell.editor_buffer()
                        {
                            let head = view.head().get().min(buffer.byte_len());
                            let line = buffer.line_of_byte(head) + 1;
                            let line_start = buffer.byte_of_line(buffer.line_of_byte(head));
                            let col = head.saturating_sub(line_start) + 1;
                            format!("Ln {line}, Col {col}")
                        } else {
                            "Ln 1, Col 1".to_string()
                        };
                        ui.label(
                            egui::RichText::new(cursor_str)
                                .monospace()
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                    });
                });
            }
        });
}
