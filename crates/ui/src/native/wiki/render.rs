use super::model::{FolderWikiData, WikiTab};
use crate::native::editor::color;
use crate::native::{icons, widgets};
use crate::theme::Theme;

/// Action produced by interacting with the Folder Wiki dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WikiAction {
    OpenNote { path: String, line: Option<usize> },
    CreateMasterNote { folder: String },
    CreateNewNote { folder: String },
    Close,
}

/// Renders the Folder Wiki dashboard as a spacious, Obsidian Bases-like overlay.
pub fn render_folder_wiki(
    data: &mut FolderWikiData,
    ctx: &egui::Context,
    theme: &Theme,
) -> Option<WikiAction> {
    let mut action = None;
    let screen = ctx.content_rect();

    // Spacious responsive sizing: 82vw × 80vh
    let width = (screen.width() * 0.82).clamp(580.0, 1280.0);
    let height = (screen.height() * 0.80).clamp(440.0, 860.0);

    let frame = egui::Frame::new()
        .fill(color(theme.bg_panel))
        .stroke(egui::Stroke::new(1.0, color(theme.border)))
        .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_black_alpha(100),
        })
        .inner_margin(egui::Margin::symmetric(20, 16));

    egui::Window::new("folder_wiki_dashboard")
        .title_bar(false)
        .frame(frame)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .fixed_size(egui::vec2(width, height))
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_size(egui::vec2(width - 40.0, height - 32.0));

            // Top Header: Folder title, Stats pills, New note, Close
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                let (folder_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                icons::render_icon(ui.painter(), icons::Icon::Folder, folder_rect, color(theme.accent), 1.4);

                ui.label(
                    egui::RichText::new(&data.folder_name)
                        .size(16.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );

                if !data.folder_path.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("({})", data.folder_path))
                            .size(11.0)
                            .color(color(theme.text_muted)),
                    );
                }

                // Stats pills
                let notes_cnt = data.notes.len();
                let pending_cnt = data.pending_tasks();
                let pill_text = format!("{notes_cnt} зам. · {pending_cnt} задач");
                ui.label(
                    egui::RichText::new(pill_text)
                        .size(11.0)
                        .color(color(theme.accent)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, icons::Icon::Close, "Закрыть (Esc)").clicked() {
                        action = Some(WikiAction::Close);
                    }
                    if widgets::primary_button_with_icon(
                        ui,
                        icons::Icon::Plus,
                        "Новая заметка (a)",
                    )
                    .clicked()
                    {
                        action = Some(WikiAction::CreateNewNote {
                            folder: data.folder_path.clone(),
                        });
                    }
                });
            });

            ui.add_space(8.0);

            // Tab bar: Overview / Notes & Headings / Tasks / Metadata Table / Backlinks
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for tab in WikiTab::ALL {
                    let is_active = data.active_tab == tab;
                    let text = egui::RichText::new(tab.title())
                        .size(12.0)
                        .color(if is_active {
                            color(theme.accent)
                        } else {
                            color(theme.text_muted)
                        });
                    if ui.selectable_label(is_active, text).clicked() {
                        data.active_tab = tab;
                        data.selected_index = 0;
                    }
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Body rendering according to active tab
            match data.active_tab {
                WikiTab::Overview => {
                    render_overview(data, ui, theme, &mut action);
                }
                WikiTab::NotesTree => {
                    render_notes_tree(data, ui, theme, &mut action);
                }
                WikiTab::Tasks => {
                    render_tasks(data, ui, theme, &mut action);
                }
                WikiTab::MetadataTable => {
                    render_metadata_table(data, ui, theme, &mut action);
                }
                WikiTab::Backlinks => {
                    render_backlinks(data, ui, theme, &mut action);
                }
            }

            ui.add_space(8.0);

            // Footer key hints
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("j/k: выбор  Enter: открыть  Tab: переключить вкладку  a: новая заметка  Esc: закрыть")
                        .size(11.0)
                        .color(color(theme.text_muted)),
                );
            });
        });

    action
}

fn render_overview(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    ui.vertical(|ui| {
        // Master file card
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
                    egui::RichText::new("Мастер-файл отсутствует. Создайте index.md для ведения главной вики-страницы этой папки.")
                        .size(12.0)
                        .color(color(theme.text_muted)),
                );
            }
        });

        ui.add_space(12.0);

        // Stats summary row
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;

            stat_card(ui, theme, "Всего заметок", &format!("{}", data.notes.len()));
            stat_card(ui, theme, "Всего строк", &format!("{}", data.total_lines()));
            stat_card(ui, theme, "Задач к выполнению", &format!("{}", data.pending_tasks()));
            stat_card(ui, theme, "Выполнено задач", &format!("{}", data.completed_tasks()));
        });

        ui.add_space(12.0);

        // Recent notes list
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
                for (idx, note) in data.notes.iter().enumerate() {
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

fn render_notes_tree(
    data: &mut FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    egui::ScrollArea::vertical()
        .id_salt("wiki_notes_tree_scroll")
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            for note in &data.notes {
                let is_collapsed = data.collapsed_notes.contains(&note.path);

                ui.horizontal(|ui| {
                    if !note.headings.is_empty() {
                        let chev = if is_collapsed {
                            icons::Icon::ChevronRight
                        } else {
                            icons::Icon::ChevronDown
                        };
                        let (chev_rect, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 14.0), egui::Sense::hover());
                        icons::render_icon(
                            ui.painter(),
                            chev,
                            chev_rect,
                            color(theme.text_muted),
                            1.2,
                        );
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

                    let rich = egui::RichText::new(&note.name)
                        .size(13.0)
                        .strong()
                        .color(color(theme.text_primary));

                    if ui.selectable_label(false, rich).clicked() {
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
                        ui.horizontal(|ui| {
                            let indent = (*level as f32) * 14.0 + 16.0;
                            ui.add_space(indent);

                            let rich_h = egui::RichText::new(format!("H{level} {text}"))
                                .size(11.0)
                                .color(color(theme.text_secondary));

                            if ui.selectable_label(false, rich_h).clicked() {
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
}

fn render_tasks(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    if data.tasks.is_empty() {
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
            for (idx, task) in data.tasks.iter().enumerate() {
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

fn render_metadata_table(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    egui::ScrollArea::vertical()
        .id_salt("wiki_table_scroll")
        .max_height(ui.available_height() - 20.0)
        .show(ui, |ui| {
            // Header row
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

            for (idx, note) in data.notes.iter().enumerate() {
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

fn render_backlinks(
    data: &FolderWikiData,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<WikiAction>,
) {
    if data.backlinks.is_empty() {
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
            for bl in &data.backlinks {
                ui.horizontal(|ui| {
                    let rich = egui::RichText::new(&bl.source_note)
                        .size(12.0)
                        .color(color(theme.accent));

                    if ui.selectable_label(false, rich).clicked() {
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
