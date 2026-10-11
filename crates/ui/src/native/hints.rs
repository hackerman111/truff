use super::editor::color;
use eframe::egui;

pub(super) fn show(shell: &crate::shell::Shell, context: &egui::Context) {
    if !shell.which_key().is_visible() {
        return;
    }

    let theme = shell.theme();
    let overlay = shell.which_key();

    egui::Window::new("which_key_overlay")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -28.0])
        .frame(
            egui::Frame::new()
                .fill(color(theme.bg_secondary))
                .stroke(egui::Stroke::new(1.0, color(theme.border)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(14, 10)),
        )
        .show(context, |ui| {
            ui.horizontal(|ui| {
                let title = if overlay.title().is_empty() {
                    "Which-Key"
                } else {
                    overlay.title()
                };
                ui.label(
                    egui::RichText::new(format!("⌨  {title}"))
                        .strong()
                        .color(color(theme.accent)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("[Esc] закрыть")
                            .color(color(theme.text_muted))
                            .size(11.0),
                    );
                });
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            match overlay.mode() {
                crate::overlays::WhichKeyMode::Prefix => {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(12.0, 6.0);
                        for hint in overlay.hints() {
                            ui.horizontal(|ui| {
                                egui::Frame::new()
                                    .fill(color(theme.bg_tertiary))
                                    .stroke(egui::Stroke::new(1.0, color(theme.border)))
                                    .corner_radius(egui::CornerRadius::same(4))
                                    .inner_margin(egui::Margin::symmetric(6, 2))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(hint.key.to_string())
                                                .monospace()
                                                .strong()
                                                .color(color(theme.accent)),
                                        );
                                    });
                                ui.label(egui::RichText::new("→").color(color(theme.text_muted)));
                                ui.label(
                                    egui::RichText::new(&hint.description)
                                        .color(color(theme.text_primary)),
                                );
                            });
                        }
                    });
                }
                crate::overlays::WhichKeyMode::Help => {
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                ui.spacing_mut().item_spacing.x = 20.0;
                                for category in overlay.categories() {
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new(&category.name)
                                                .strong()
                                                .color(color(theme.accent)),
                                        );
                                        ui.add_space(4.0);
                                        for hint in &category.hints {
                                            ui.horizontal(|ui| {
                                                egui::Frame::new()
                                                    .fill(color(theme.bg_tertiary))
                                                    .stroke(egui::Stroke::new(
                                                        1.0,
                                                        color(theme.border),
                                                    ))
                                                    .corner_radius(egui::CornerRadius::same(4))
                                                    .inner_margin(egui::Margin::symmetric(5, 1))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            egui::RichText::new(
                                                                hint.key.to_string(),
                                                            )
                                                            .monospace()
                                                            .size(11.0)
                                                            .color(color(theme.accent)),
                                                        );
                                                    });
                                                ui.label(
                                                    egui::RichText::new(&hint.description)
                                                        .size(12.0)
                                                        .color(color(theme.text_primary)),
                                                );
                                            });
                                        }
                                    });
                                }
                            });
                        });
                }
            }
        });
}
