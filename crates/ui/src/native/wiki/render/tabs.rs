use super::WikiAction;
use crate::native::editor::color;
use crate::native::wiki::model::FolderWikiData;
use crate::native::{icons, widgets};
use crate::theme::Theme;

pub(super) fn render_overview(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    let notes = data.filtered_notes();
    ui.vertical(|ui| {
        let card_frame = egui::Frame::new()
            .fill(color(theme.bg_secondary))
            .stroke(egui::Stroke::new(1.0, color(theme.border)))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(14, 10));

        card_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("МАСТЕР-ФАЙЛ: {}", data.master_file_name))
                        .size(11.0)
                        .strong()
                        .color(color(theme.text_muted)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if data.has_master_file {
                        let master_path = if data.folder_path.is_empty() {
                            data.master_file_name.clone()
                        } else {
                            format!("{}/{}", data.folder_path, data.master_file_name)
                        };
                        if widgets::primary_button(ui, "Открыть мастер-файл (Enter)").clicked() {
                            *action = Some(WikiAction::OpenNote {
                                path: master_path,
                                line: None,
                            });
                        }
                    } else if widgets::primary_button_with_icon(
                        ui,
                        icons::Icon::Plus,
                        "Создать index.md",
                    )
                    .clicked()
                    {
                        *action = Some(WikiAction::CreateMasterNote {
                            folder: data.folder_path.clone(),
                        });
                    }
                });
            });

            ui.add_space(4.0);

            if data.has_master_file && !data.master_preview.is_empty() {
                for line in &data.master_preview {
                    ui.label(
                        egui::RichText::new(line)
                            .size(12.0)
                            .color(color(theme.text_secondary)),
                    );
                }
            } else if !data.has_master_file {
                ui.label(
                    egui::RichText::new(
                        "Мастер-файл отсутствует. Создайте index.md для ведения главной вики-страницы этой папки.",
                    )
                    .size(12.0)
                    .color(color(theme.text_muted)),
                );
            }
        });

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            stat_card(ui, theme, "Всего заметок", &format!("{}", data.notes.len()));
            stat_card(ui, theme, "Всего строк", &format!("{}", data.total_lines()));
            stat_card(
                ui,
                theme,
                "Задач к выполнению",
                &format!("{}", data.pending_tasks()),
            );
            stat_card(
                ui,
                theme,
                "Выполнено задач",
                &format!("{}", data.completed_tasks()),
            );
        });

        ui.add_space(12.0);

        ui.label(
            egui::RichText::new("ЗАМЕТКИ В ПАПКЕ")
                .size(11.0)
                .strong()
                .color(color(theme.text_muted)),
        );
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .max_height(240.0)
            .show(ui, |ui| {
                for (idx, note) in notes.iter().enumerate() {
                    let is_sel = idx == data.selected_index;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (icon_rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        icons::render_icon(
                            ui.painter(),
                            icons::Icon::File,
                            icon_rect,
                            if is_sel {
                                color(theme.accent)
                            } else {
                                color(theme.text_muted)
                            },
                            1.3,
                        );
                        let text = format!(
                            "{} ({} строк, {} задач)",
                            note.name, note.line_count, note.task_count
                        );
                        let rich = egui::RichText::new(text).size(12.0).color(if is_sel {
                            color(theme.accent)
                        } else {
                            color(theme.text_primary)
                        });

                        if ui.selectable_label(is_sel, rich).clicked() {
                            *action = Some(WikiAction::OpenNote {
                                path: note.path.clone(),
                                line: None,
                            });
                        }
                    });
                }
            });
    });
}

fn stat_card(ui: &mut egui::Ui, theme: &Theme, title: &str, value: &str) {
    let frame = egui::Frame::new()
        .fill(color(theme.bg_canvas))
        .stroke(egui::Stroke::new(1.0, color(theme.border_subtle)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(12, 6));

    frame.show(ui, |ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .size(10.0)
                    .color(color(theme.text_muted)),
            );
            ui.label(
                egui::RichText::new(value)
                    .size(14.0)
                    .strong()
                    .color(color(theme.accent)),
            );
        });
    });
}

pub(super) fn render_notes_tree(
    data: &mut FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    let mut toggle_path: Option<String> = None;
    let mut row_idx = 0usize;

    egui::ScrollArea::vertical()
        .id_salt("wiki_notes_tree_scroll")
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            let notes = data.filtered_notes();
            for note in notes {
                let is_collapsed = data.collapsed_notes.contains(&note.path);
                let is_note_sel = row_idx == data.selected_index;
                row_idx += 1;

                ui.horizontal(|ui| {
                    if !note.headings.is_empty() {
                        let chev = if is_collapsed {
                            icons::Icon::ChevronRight
                        } else {
                            icons::Icon::ChevronDown
                        };
                        let (chev_rect, chev_resp) =
                            ui.allocate_exact_size(egui::vec2(12.0, 14.0), egui::Sense::click());
                        icons::render_icon(
                            ui.painter(),
                            chev,
                            chev_rect,
                            color(theme.text_muted),
                            1.2,
                        );
                        if chev_resp.clicked() {
                            toggle_path = Some(note.path.clone());
                        }
                    } else {
                        ui.add_space(12.0);
                    }

                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    icons::render_icon(
                        ui.painter(),
                        icons::Icon::File,
                        icon_rect,
                        color(theme.accent),
                        1.2,
                    );

                    let rich =
                        egui::RichText::new(&note.name)
                            .size(13.0)
                            .strong()
                            .color(if is_note_sel {
                                color(theme.accent)
                            } else {
                                color(theme.text_primary)
                            });

                    if ui.selectable_label(is_note_sel, rich).clicked() {
                        *action = Some(WikiAction::OpenNote {
                            path: note.path.clone(),
                            line: None,
                        });
                    }

                    ui.label(
                        egui::RichText::new(format!("({} строк)", note.line_count))
                            .size(10.0)
                            .color(color(theme.text_muted)),
                    );
                });

                if !is_collapsed {
                    for (level, text, line) in &note.headings {
                        let is_h_sel = row_idx == data.selected_index;
                        row_idx += 1;
                        ui.horizontal(|ui| {
                            let indent = (*level as f32) * 14.0 + 16.0;
                            ui.add_space(indent);

                            let rich_h = egui::RichText::new(format!("H{level} {text}"))
                                .size(11.0)
                                .color(if is_h_sel {
                                    color(theme.accent)
                                } else {
                                    color(theme.text_secondary)
                                });

                            if ui.selectable_label(is_h_sel, rich_h).clicked() {
                                *action = Some(WikiAction::OpenNote {
                                    path: note.path.clone(),
                                    line: Some(*line),
                                });
                            }

                            ui.label(
                                egui::RichText::new(format!(":{line}"))
                                    .size(10.0)
                                    .color(color(theme.text_muted)),
                            );
                        });
                    }
                }
            }
        });

    if let Some(path) = toggle_path {
        data.toggle_note_collapse(&path);
    }
}

pub(super) fn render_tasks(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    let tasks = data.filtered_tasks();
    if tasks.is_empty() {
        ui.label(
            egui::RichText::new("В заметках этой папки нет задач (- [ ] / - [x]).")
                .size(13.0)
                .color(color(theme.text_muted)),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("wiki_tasks_scroll")
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for (idx, task) in tasks.iter().enumerate() {
                let is_sel = idx == data.selected_index;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;

                    let (check_rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    let check_icon = if task.checked {
                        icons::Icon::CheckSquare
                    } else {
                        icons::Icon::Square
                    };
                    let check_color = if task.checked {
                        color(theme.accent)
                    } else {
                        color(theme.text_muted)
                    };
                    icons::render_icon(ui.painter(), check_icon, check_rect, check_color, 1.3);

                    let rich = egui::RichText::new(&task.text)
                        .size(12.0)
                        .color(if task.checked {
                            color(theme.text_muted)
                        } else if is_sel {
                            color(theme.accent)
                        } else {
                            color(theme.text_primary)
                        });

                    if ui.selectable_label(is_sel, rich).clicked() {
                        *action = Some(WikiAction::OpenNote {
                            path: task.note_path.clone(),
                            line: Some(task.line),
                        });
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{} :{}", task.note_title, task.line))
                                .size(10.0)
                                .color(color(theme.text_muted)),
                        );
                    });
                });
            }
        });
}

pub(super) fn render_metadata_table(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    let notes = data.filtered_notes();
    egui::ScrollArea::vertical()
        .id_salt("wiki_table_scroll")
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [180.0, 20.0],
                    egui::Label::new(egui::RichText::new("Название").strong().size(11.0)),
                );
                ui.add_sized(
                    [80.0, 20.0],
                    egui::Label::new(egui::RichText::new("Строк").strong().size(11.0)),
                );
                ui.add_sized(
                    [80.0, 20.0],
                    egui::Label::new(egui::RichText::new("Размер").strong().size(11.0)),
                );
                ui.add_sized(
                    [100.0, 20.0],
                    egui::Label::new(egui::RichText::new("Задачи").strong().size(11.0)),
                );
                ui.add_sized(
                    [120.0, 20.0],
                    egui::Label::new(egui::RichText::new("Статус").strong().size(11.0)),
                );
            });
            ui.separator();

            for (idx, note) in notes.iter().enumerate() {
                let is_sel = idx == data.selected_index;
                ui.horizontal(|ui| {
                    let rich_name = egui::RichText::new(&note.name).size(12.0).color(if is_sel {
                        color(theme.accent)
                    } else {
                        color(theme.text_primary)
                    });

                    if ui
                        .add_sized(
                            [180.0, 20.0],
                            egui::Button::selectable(is_sel, rich_name).truncate(),
                        )
                        .clicked()
                    {
                        *action = Some(WikiAction::OpenNote {
                            path: note.path.clone(),
                            line: None,
                        });
                    }

                    ui.add_sized(
                        [80.0, 20.0],
                        egui::Label::new(format!("{}", note.line_count)),
                    );
                    ui.add_sized(
                        [80.0, 20.0],
                        egui::Label::new(format!("{} Б", note.byte_size)),
                    );

                    let task_text = if note.task_count > 0 {
                        format!(
                            "{}/{}",
                            note.task_count - note.pending_tasks,
                            note.task_count
                        )
                    } else {
                        "—".to_string()
                    };
                    ui.add_sized([100.0, 20.0], egui::Label::new(task_text));

                    let (status_icon, status_text, status_color) = if note.is_master {
                        (icons::Icon::Star, "Мастер", color(theme.accent))
                    } else if note.pending_tasks > 0 {
                        (
                            icons::Icon::Warning,
                            "Есть задачи",
                            color(theme.callout_warning),
                        )
                    } else {
                        (icons::Icon::Check, "Чисто", color(theme.callout_tip))
                    };
                    ui.allocate_ui_with_layout(
                        egui::vec2(120.0, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let (icon_rect, _) = ui
                                .allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
                            icons::render_icon(
                                ui.painter(),
                                status_icon,
                                icon_rect,
                                status_color,
                                1.3,
                            );
                            ui.label(
                                egui::RichText::new(status_text)
                                    .size(11.0)
                                    .color(status_color),
                            );
                        },
                    );
                });
            }
        });
}

pub(super) fn render_backlinks(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    let backlinks = data.filtered_backlinks();
    if backlinks.is_empty() {
        ui.label(
            egui::RichText::new("Нет входящих ссылок на заметки этой папки.")
                .size(13.0)
                .color(color(theme.text_muted)),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("wiki_backlinks_scroll")
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for (idx, bl) in backlinks.iter().enumerate() {
                let is_sel = idx == data.selected_index;
                ui.horizontal(|ui| {
                    let rich = egui::RichText::new(&bl.source_note)
                        .size(12.0)
                        .color(if is_sel {
                            color(theme.accent)
                        } else {
                            color(theme.text_primary)
                        });

                    if ui.selectable_label(is_sel, rich).clicked() {
                        *action = Some(WikiAction::OpenNote {
                            path: bl.source_note.clone(),
                            line: None,
                        });
                    }

                    ui.label(
                        egui::RichText::new(format!("→ [[{}]]", bl.target_note))
                            .size(11.0)
                            .color(color(theme.text_secondary)),
                    );

                    if !bl.snippet.is_empty() {
                        ui.label(
                            egui::RichText::new(format!("\"{}\"", bl.snippet))
                                .size(11.0)
                                .color(color(theme.text_muted)),
                        );
                    }
                });
            }
        });
}
