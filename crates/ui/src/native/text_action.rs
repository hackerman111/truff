use crate::components::{color, overlay_frame};
use crate::native::NativeApp;
use crate::overlays::text_action::{PALETTE, TextActionMode, TextActionResponse};
use egui::{Color32, Id, Order, RichText, Sense, Stroke, pos2, vec2};

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
    let est_size = vec2(300.0, 220.0);
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
                                    "Обернуть в [[...]]"
                                } else {
                                    "Вставка [[Заметка|Текст]]"
                                },
                                true,
                            ),
                            ("u", "URL ссылка", "Вставка [Текст](url)", true),
                            ("x", "Очистить стиль", "Снять все теги стилей", has_sel),
                        ];

                        for (key, title, desc, enabled) in items {
                            let resp = ui.allocate_ui(vec2(ui.available_width(), 26.0), |ui| {
                                ui.horizontal(|ui| {
                                    if enabled {
                                        render_key_pill(ui, key, &theme);
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
                                        render_dimmed_key_pill(ui, key, &theme);
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
                            render_key_pill(ui, "Esc", &theme);
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
                                            render_key_pill(ui, &opt.key.to_string(), &theme);

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
                            render_key_pill(ui, "Esc", &theme);
                            ui.label(
                                RichText::new("Назад")
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
                    } => {
                        let is_url = *is_url;
                        let has_sel = *has_selection;
                        let active_field = *active_field;

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

                        if active_field == 0 {
                            ui.memory_mut(|m| m.request_focus(target_edit_id));
                        } else {
                            ui.memory_mut(|m| m.request_focus(title_edit_id));
                        }

                        let mut title_resp = None;

                        if has_sel && is_url {
                            ui.label(
                                RichText::new(format!("Текст: \"{title_input}\""))
                                    .small()
                                    .color(color(theme.text_muted)),
                            );
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new("URL адрес:")
                                    .small()
                                    .color(color(theme.text_secondary)),
                            );
                        } else {
                            let target_label = if is_url {
                                "URL адрес:"
                            } else {
                                "Цель (имя заметки):"
                            };
                            ui.label(
                                RichText::new(target_label)
                                    .small()
                                    .color(color(theme.text_secondary)),
                            );
                        }

                        let target_resp = ui.add(
                            egui::TextEdit::singleline(target_input)
                                .id(target_edit_id)
                                .desired_width(ui.available_width()),
                        );

                        if !has_sel {
                            ui.add_space(4.0);
                            let title_label = if is_url {
                                "Отображаемый заголовок:"
                            } else {
                                "Отображаемый заголовок (опционально):"
                            };
                            ui.label(
                                RichText::new(title_label)
                                    .small()
                                    .color(color(theme.text_secondary)),
                            );
                            title_resp = Some(ui.add(
                                egui::TextEdit::singleline(title_input)
                                    .id(title_edit_id)
                                    .desired_width(ui.available_width()),
                            ));
                        }

                        ui.add_space(8.0);
                        let mut submit = false;
                        ui.horizontal(|ui| {
                            if ui.button("Применить · Enter").clicked() {
                                submit = true;
                            }
                            if !has_sel {
                                ui.label(
                                    RichText::new("[Tab] Сменить поле")
                                        .small()
                                        .color(color(theme.text_muted)),
                                );
                            }
                            if ui.button("Отмена · Esc").clicked() {
                                close_requested = true;
                            }
                        });

                        submit |= (target_resp.lost_focus()
                            || title_resp.is_some_and(|r| r.lost_focus()))
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

fn render_key_pill(ui: &mut egui::Ui, key_str: &str, theme: &crate::theme::Theme) {
    let bg = color(theme.bg_canvas);
    let border = color(theme.accent);
    let txt = color(theme.accent);

    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new(key_str)
                    .monospace()
                    .strong()
                    .small()
                    .color(txt),
            );
        });
}

fn render_dimmed_key_pill(ui: &mut egui::Ui, key_str: &str, theme: &crate::theme::Theme) {
    let bg = color(theme.bg_canvas);
    let border = color(theme.text_muted);
    let txt = color(theme.text_muted);

    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(
                RichText::new(key_str)
                    .monospace()
                    .strong()
                    .small()
                    .color(txt),
            );
        });
}
