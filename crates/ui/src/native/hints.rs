use super::editor::color;
use eframe::egui;

fn humanize_description(desc: &str) -> String {
    match desc {
        "picker.commands" => "Все команды".to_string(),
        "picker.buffers" => "Буферы и вкладки".to_string(),
        "picker.files" => "Файлы хранилища".to_string(),
        "board.column.delete" => "Удалить колонку".to_string(),
        "board.column.left" => "Колонка влево".to_string(),
        "board.column.right" => "Колонка вправо".to_string(),
        "board.card.new" => "Новая карточка".to_string(),
        "board.card.delete" => "Удалить карточку".to_string(),
        "Focus left pane" => "Левая панель (дерево)".to_string(),
        "Focus right pane" => "Правая панель (backlinks)".to_string(),
        "Cycle pane" => "Сменить фокус окна".to_string(),
        _ => {
            if let Some(stripped) = desc.strip_prefix("picker.") {
                match stripped {
                    "commands" => "Команды".to_string(),
                    "buffers" => "Вкладки".to_string(),
                    "files" => "Файлы".to_string(),
                    other => other.to_string(),
                }
            } else if let Some(stripped) = desc.strip_prefix("board.") {
                match stripped {
                    "column.delete" => "Удалить колонку".to_string(),
                    "column.left" => "Колонка влево".to_string(),
                    "column.right" => "Колонка вправо".to_string(),
                    "card.new" => "Создать карточку".to_string(),
                    "card.delete" => "Удалить карточку".to_string(),
                    other => other.to_string(),
                }
            } else {
                desc.to_string()
            }
        }
    }
}

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
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -32.0])
        .frame(
            egui::Frame::new()
                .fill(color(theme.bg_elevated))
                .stroke(egui::Stroke::new(1.0, color(theme.border_subtle)))
                .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
                .inner_margin(egui::Margin::symmetric(16, 12)),
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
                        .size(13.0)
                        .color(color(theme.accent_primary)),
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
            ui.add_space(8.0);

            match overlay.mode() {
                crate::overlays::WhichKeyMode::Prefix => {
                    let hints = overlay.hints();
                    if hints.is_empty() {
                        ui.label(
                            egui::RichText::new("Нет доступных действий")
                                .color(color(theme.text_muted)),
                        );
                        return;
                    }

                    // Structured multi-column layout (3 columns)
                    let num_cols = if hints.len() <= 4 {
                        2
                    } else if hints.len() <= 9 {
                        3
                    } else {
                        4
                    };

                    ui.columns(num_cols, |cols| {
                        let chunk_size = hints.len().div_ceil(num_cols);
                        for (col_idx, col_ui) in cols.iter_mut().enumerate() {
                            let start = col_idx * chunk_size;
                            let end = (start + chunk_size).min(hints.len());
                            if start >= hints.len() {
                                continue;
                            }

                            col_ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 6.0;
                                for hint in &hints[start..end] {
                                    ui.horizontal(|ui| {
                                        // Key badge
                                        egui::Frame::new()
                                            .fill(color(theme.bg_tertiary))
                                            .stroke(egui::Stroke::new(
                                                1.0,
                                                color(theme.border_subtle),
                                            ))
                                            .corner_radius(egui::CornerRadius::same(4))
                                            .inner_margin(egui::Margin::symmetric(6, 2))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    egui::RichText::new(hint.key.to_string())
                                                        .monospace()
                                                        .strong()
                                                        .size(11.0)
                                                        .color(color(theme.accent_primary)),
                                                );
                                            });

                                        let label = humanize_description(&hint.description);
                                        if hint.is_prefix {
                                            ui.label(
                                                egui::RichText::new(format!("{label} →"))
                                                    .strong()
                                                    .size(12.0)
                                                    .color(color(theme.text_primary)),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(label)
                                                    .size(12.0)
                                                    .color(color(theme.text_primary)),
                                            );
                                        }
                                    });
                                }
                            });
                        }
                    });
                }
                crate::overlays::WhichKeyMode::Help => {
                    let categories = overlay.categories();
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            let num_cols = if categories.len() <= 2 { 2 } else { 3 };
                            ui.columns(num_cols, |cols| {
                                let chunk_size = categories.len().div_ceil(num_cols);
                                for (col_idx, col_ui) in cols.iter_mut().enumerate() {
                                    let start = col_idx * chunk_size;
                                    let end = (start + chunk_size).min(categories.len());
                                    if start >= categories.len() {
                                        continue;
                                    }

                                    col_ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 12.0;
                                        for category in &categories[start..end] {
                                            let cat_frame = egui::Frame::new()
                                                .fill(color(theme.bg_panel))
                                                .stroke(egui::Stroke::new(
                                                    1.0,
                                                    color(theme.border_subtle),
                                                ))
                                                .corner_radius(egui::CornerRadius::same(6))
                                                .inner_margin(egui::Margin::symmetric(10, 8));

                                            cat_frame.show(ui, |ui| {
                                                ui.label(
                                                    egui::RichText::new(&category.name)
                                                        .strong()
                                                        .size(12.5)
                                                        .color(color(theme.accent_primary)),
                                                );
                                                ui.add_space(4.0);
                                                ui.separator();
                                                ui.add_space(4.0);

                                                for hint in &category.hints {
                                                    ui.horizontal(|ui| {
                                                        egui::Frame::new()
                                                            .fill(color(theme.bg_tertiary))
                                                            .stroke(egui::Stroke::new(
                                                                1.0,
                                                                color(theme.border_subtle),
                                                            ))
                                                            .corner_radius(
                                                                egui::CornerRadius::same(3),
                                                            )
                                                            .inner_margin(egui::Margin::symmetric(
                                                                5, 1,
                                                            ))
                                                            .show(ui, |ui| {
                                                                ui.label(
                                                                    egui::RichText::new(
                                                                        hint.key.to_string(),
                                                                    )
                                                                    .monospace()
                                                                    .size(10.5)
                                                                    .strong()
                                                                    .color(color(
                                                                        theme.accent_primary,
                                                                    )),
                                                                );
                                                            });
                                                        ui.label(
                                                            egui::RichText::new(
                                                                humanize_description(
                                                                    &hint.description,
                                                                ),
                                                            )
                                                            .size(11.5)
                                                            .color(color(theme.text_primary)),
                                                        );
                                                    });
                                                }
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
