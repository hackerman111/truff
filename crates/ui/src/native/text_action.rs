use crate::components::{color, overlay_frame};
use crate::native::{NativeApp, widgets};
use crate::overlays::text_action::{
    PALETTE, TASK_STATUS_OPTIONS, TaskStatus, TextActionMode, TextActionResponse,
};
use egui::{Color32, Id, Order, RichText, Sense, pos2, vec2};

pub(super) fn show(app: &mut NativeApp, ctx: &egui::Context) {
    if !app.application.shell.text_action().is_open() {
        return;
    }

    let theme = app.application.shell.theme().clone();
    let screen = ctx.content_rect();

    // Determine anchor position:
    // Prefer cursor pos from overlay or current editor caret, fall back to center.
    let target_pos = app
        .application
        .shell
        .text_action()
        .cursor_pos()
        .map(|[x, y]| pos2(x, y))
        .or_else(|| app.editor.cursor_rect().map(|r| r.left_bottom()))
        .unwrap_or_else(|| screen.center());

    // Approximate menu dimensions for boundary clamping
    let est_size = vec2(320.0, 300.0);
    let mut anchor = target_pos + vec2(0.0, 4.0);

    // Keep popup within viewport
    if anchor.x + est_size.x > screen.max.x - 12.0 {
        anchor.x = (screen.max.x - est_size.x - 12.0).max(screen.min.x + 12.0);
    }
    if anchor.y + est_size.y > screen.max.y - 12.0 {
        // Pop above the cursor if it would overflow the bottom
        let above_y = target_pos.y - est_size.y - 24.0;
        if above_y >= screen.min.y + 12.0 {
            anchor.y = above_y;
        } else {
            anchor.y = (screen.max.y - est_size.y - 12.0).max(screen.min.y + 12.0);
        }
    }

    let note_suggestions: Option<Vec<String>> = if let TextActionMode::LinkInput {
        is_url: false,
        target_input,
        ..
    } = app.application.shell.text_action().mode()
    {
        let query = target_input.trim().to_lowercase();
        Some(
            app.application
                .shell
                .sidebar_file_paths()
                .filter_map(|p| {
                    let note = p.strip_suffix(".md").unwrap_or(p);
                    (query.is_empty() || note.to_lowercase().contains(&query))
                        .then(|| note.to_owned())
                })
                .take(6)
                .collect(),
        )
    } else {
        None
    };

    let mut key_to_handle: Option<truff_core::vim::Key> = None;
    let mut close_requested = false;

    egui::Area::new(Id::new("text_action_overlay_popup"))
        .fixed_pos(anchor)
        .order(Order::Foreground)
        .show(ctx, |ui| {
            overlay_frame(&theme).show(ui, |ui| {
                ui.set_max_width(320.0);

                let selected_idx = app.application.shell.text_action().selected_color_index();
                let mode = app.application.shell.text_action_mut().mode_mut();

                match mode {
                    TextActionMode::Root { has_selection } => {
                        let has_sel = *has_selection;
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Действия с текстом")
                                    .strong()
                                    .color(color(theme.accent)),
                            );
                        });
                        ui.separator();

                        let items = [
                            ("c", "Цвет текста", "Окрашивание шрифта", has_sel),
                            ("h", "Подсветка (маркер)", "Фоновый маркер текста", has_sel),
                            (
                                "n",
                                "Ссылка на заметку",
                                if has_sel {
                                    "Вставка [[Заметка|Текст]]"
                                } else {
                                    "Вставка [[Заметка]]"
                                },
                                true,
                            ),
                            (
                                "u",
                                "URL ссылка",
                                if has_sel {
                                    "Вставка [Текст](url)"
                                } else {
                                    "Вставка [Заголовок](url)"
                                },
                                true,
                            ),
                            ("t", "Задача", "Создать задачу / статус", true),
                            ("x", "Очистить стиль", "Снять все теги стилей", has_sel),
                        ];

                        for (key, title, desc, enabled) in items {
                            let resp = ui.allocate_ui(vec2(ui.available_width(), 26.0), |ui| {
                                ui.horizontal(|ui| {
                                    if enabled {
                                        widgets::key_pill(ui, key, &theme, true);
                                        ui.label(
                                            RichText::new(title)
                                                .strong()
                                                .color(color(theme.text_primary)),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(desc)
                                                        .small()
                                                        .color(color(theme.text_muted)),
                                                );
                                            },
                                        );
                                    } else {
                                        widgets::key_pill(ui, key, &theme, false);
                                        ui.label(
                                            RichText::new(title)
                                                .small()
                                                .color(color(theme.text_muted)),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new("(требуется выделение)")
                                                        .small()
                                                        .color(color(theme.text_muted)),
                                                );
                                            },
                                        );
                                    }
                                });
                            });

                            if resp.response.interact(Sense::click()).clicked() {
                                if enabled {
                                    key_to_handle = Some(truff_core::vim::Key::Char(
                                        key.chars().next().unwrap(),
                                    ));
                                } else {
                                    app.application.status =
                                        "Выделите текст или поместите курсор на слово для применения стиля".into();
                                }
                            }
                        }

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            widgets::key_pill(ui, "Esc", &theme, true);
                            ui.label(
                                RichText::new("Закрыть")
                                    .small()
                                    .color(color(theme.text_muted)),
                            );
                        });
                    }
                    TextActionMode::ColorSelect { is_highlight } => {
                        let is_highlight = *is_highlight;
                        let title = if is_highlight {
                            "Подсветка текста (маркер)"
                        } else {
                            "Цвет текста"
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(title).strong().color(color(theme.accent)));
                        });
                        ui.label(
                            RichText::new("Выберите стрелками + Enter или нажмите букву:")
                                .small()
                                .color(color(theme.text_muted)),
                        );
                        ui.separator();

                        ui.vertical(|ui| {
                            for (idx, opt) in PALETTE.iter().enumerate() {
                                let is_selected = idx == selected_idx;
                                let bg_color = if is_selected {
                                    color(theme.selection)
                                } else {
                                    Color32::TRANSPARENT
                                };

                                let row_resp = egui::Frame::new()
                                    .fill(bg_color)
                                    .corner_radius(4.0)
                                    .inner_margin(egui::Margin::symmetric(6, 3))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            widgets::key_pill(ui, &opt.key.to_string(), &theme, true);

                                            let swatch_color = Color32::from_hex(opt.hex)
                                                .unwrap_or(Color32::WHITE);
                                            let (rect, _) = ui.allocate_exact_size(
                                                vec2(12.0, 12.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().circle_filled(
                                                rect.center(),
                                                5.0,
                                                swatch_color,
                                            );

                                            ui.label(RichText::new(opt.name).color(
                                                if is_selected {
                                                    color(theme.text_primary)
                                                } else {
                                                    color(theme.text_secondary)
                                                },
                                            ));

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(opt.hex)
                                                            .monospace()
                                                            .small()
                                                            .color(color(theme.text_muted)),
                                                    );
                                                },
                                            );
                                        });
                                    });

                                if row_resp.response.interact(Sense::click()).clicked() {
                                    key_to_handle = Some(truff_core::vim::Key::Char(opt.key));
                                }
                            }
                        });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            widgets::key_pill(ui, "Esc", &theme, true);
                            ui.label(
                                RichText::new("Назад")
                                    .small()
                                    .color(color(theme.text_muted)),
                            );
                        });
                    }
                    TextActionMode::TaskStatusSelect { selected_index } => {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Статус задачи")
                                    .strong()
                                    .color(color(theme.accent)),
                            );
                        });
                        ui.separator();

                        for (idx, opt) in TASK_STATUS_OPTIONS.iter().enumerate() {
                            let is_selected = idx == *selected_index;
                            let bg_color = if is_selected {
                                color(theme.selection).gamma_multiply(0.4)
                            } else {
                                Color32::TRANSPARENT
                            };

                            let row_resp = egui::Frame::new()
                                .fill(bg_color)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(6, 3))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        widgets::key_pill(ui, &opt.key.to_string(), &theme, true);
                                        let marker_color = match opt.status {
                                            TaskStatus::Todo => color(theme.text_muted),
                                            TaskStatus::InProgress => color(theme.accent),
                                            TaskStatus::Done => Color32::from_rgb(163, 190, 140),
                                            TaskStatus::Remove => color(theme.text_muted),
                                        };
                                        ui.label(
                                            RichText::new(opt.marker)
                                                .strong()
                                                .color(marker_color),
                                        );
                                        ui.label(
                                            RichText::new(opt.title)
                                                .strong()
                                                .color(color(theme.text_primary)),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(opt.desc)
                                                        .small()
                                                        .color(color(theme.text_muted)),
                                                );
                                            },
                                        );
                                    });
                                });

                            if row_resp.response.interact(Sense::click()).clicked() {
                                key_to_handle = Some(truff_core::vim::Key::Char(opt.key));
                            }
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("[↑/↓] Выбор · [Enter] Применить · [1-4] Быстро · [Esc] Назад")
                                    .small()
                                    .color(color(theme.text_muted)),
                            );
                        });
                    }
                    TextActionMode::LinkInput {
                        is_url,
                        has_selection,
                        target_input,
                        title_input,
                        active_field,
                        suggestions,
                        selected_suggestion,
                    } => {
                        let is_url = *is_url;
                        let has_sel = *has_selection;

                        if let Some(next_suggestions) = note_suggestions {
                            *suggestions = next_suggestions;
                            if let Some(sel) = *selected_suggestion
                                && sel >= suggestions.len()
                            {
                                *selected_suggestion = None;
                            }
                        }

                        let header = if is_url {
                            "Вставка URL ссылки"
                        } else {
                            "Вставка ссылки на заметку"
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(header).strong().color(color(theme.accent)));
                        });
                        ui.separator();

                        let target_edit_id = Id::new("text_action_target_input");
                        let title_edit_id = Id::new("text_action_title_input");

                        let active_id = if *active_field == 0 {
                            title_edit_id
                        } else {
                            target_edit_id
                        };
                        let focused = ui.memory(|m| m.focused());
                        if focused != Some(target_edit_id) && focused != Some(title_edit_id) {
                            ui.memory_mut(|m| m.request_focus(active_id));
                        } else if (*active_field == 0 && focused == Some(target_edit_id))
                            || (*active_field == 1 && focused == Some(title_edit_id))
                        {
                            ui.memory_mut(|m| m.request_focus(active_id));
                        }

                        // 1. Optional text / title first
                        let title_label = if has_sel {
                            "Текст ссылки:"
                        } else {
                            "Отображаемый заголовок (опционально):"
                        };
                        ui.label(
                            RichText::new(title_label)
                                .small()
                                .color(color(theme.text_secondary)),
                        );
                        let title_resp = ui.add(
                            egui::TextEdit::singleline(title_input)
                                .id(title_edit_id)
                                .lock_focus(true)
                                .desired_width(ui.available_width()),
                        );
                        if title_resp.clicked()
                            || (title_resp.has_focus() && ctx.input(|i| i.pointer.any_down()))
                        {
                            *active_field = 0;
                        }

                        ui.add_space(4.0);

                        // 2. Note selection / target second
                        let target_label = if is_url {
                            "URL адрес:"
                        } else {
                            "Ссылка на заметку (цель):"
                        };
                        ui.label(
                            RichText::new(target_label)
                                .small()
                                .color(color(theme.text_secondary)),
                        );
                        let target_resp = ui.add(
                            egui::TextEdit::singleline(target_input)
                                .id(target_edit_id)
                                .lock_focus(true)
                                .desired_width(ui.available_width()),
                        );
                        if target_resp.clicked()
                            || (target_resp.has_focus() && ctx.input(|i| i.pointer.any_down()))
                        {
                            *active_field = 1;
                        }

                        if !is_url && *active_field == 1 && !suggestions.is_empty() {
                            ui.add_space(3.0);
                            ui.label(
                                RichText::new("Документы (↑/↓ для выбора, Tab/Enter):")
                                    .small()
                                    .color(color(theme.text_muted)),
                            );
                            let mut chosen_suggestion = None;
                            for (idx, suggestion) in suggestions.iter().enumerate() {
                                let is_selected = *selected_suggestion == Some(idx);
                                let item_text = RichText::new(format!("📄 {suggestion}"))
                                    .small()
                                    .color(if is_selected {
                                        color(theme.accent)
                                    } else {
                                        color(theme.text_primary)
                                    });
                                if ui.selectable_label(is_selected, item_text).clicked() {
                                    chosen_suggestion = Some(suggestion.clone());
                                }
                            }
                            if let Some(s) = chosen_suggestion {
                                *target_input = s;
                                *selected_suggestion = None;
                                key_to_handle = Some(truff_core::vim::Key::Enter);
                            }
                        }

                        ui.add_space(8.0);
                        let mut submit = false;
                        ui.horizontal(|ui| {
                            if ui.button("Применить · Enter").clicked() {
                                submit = true;
                            }
                            ui.label(
                                RichText::new("[Tab] Сменить поле")
                                    .small()
                                    .color(color(theme.text_muted)),
                            );
                            if ui.button("Отмена · Esc").clicked() {
                                close_requested = true;
                            }
                        });

                        submit |= (target_resp.lost_focus() || title_resp.lost_focus())
                            && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                        if submit {
                            key_to_handle = Some(truff_core::vim::Key::Enter);
                        }
                    }
                }
            });
        });

    if close_requested {
        app.handle_text_action_response(TextActionResponse::Close);
    } else if let Some(key) = key_to_handle {
        let resp = app.application.shell.text_action_mut().handle_key(key);
        app.handle_text_action_response(resp);
    }
}
