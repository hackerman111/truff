use super::model::{NavigatorAction, NavigatorItemPayload, NavigatorMode, NavigatorState};
use crate::theme::Theme;
use eframe::egui;

fn to_color32(c: crate::theme::Color) -> egui::Color32 {
    let [r, g, b, a] = c.to_rgba_u8();
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Renders the Universal Navigator overlay and returns any triggered action.
pub fn show(state: &mut NavigatorState, context: &egui::Context, theme: &Theme) -> NavigatorAction {
    if !state.is_open() {
        return NavigatorAction::None;
    }

    let mut action = NavigatorAction::None;

    // Render dim backdrop
    let screen_rect = context.input(|i| i.viewport_rect());
    let painter = context.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("navigator_backdrop_layer"),
    ));
    painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(120));

    let modal_frame = egui::Frame::new()
        .fill(to_color32(theme.bg_elevated))
        .stroke(egui::Stroke::new(1.0, to_color32(theme.border_subtle)))
        .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
        .inner_margin(egui::Margin::symmetric(16, 12));

    let window_res = egui::Window::new("universal_navigator")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(760.0)
        .max_width(800.0)
        .frame(modal_frame)
        .show(context, |ui| {
            ui.set_width(760.0);
            ui.set_height(480.0);

            // Handle keyboard shortcuts
            if let Some(act) = handle_window_input(state, ui) {
                action = act;
            }

            // 1. Header
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(state.mode.title())
                        .strong()
                        .size(14.0)
                        .color(to_color32(theme.text_primary)),
                );
                if !state.context_title.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(" / {}", state.context_title))
                            .size(12.0)
                            .color(to_color32(theme.text_muted)),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .label(
                            egui::RichText::new("[Esc] Закрыть")
                                .size(11.0)
                                .color(to_color32(theme.text_muted)),
                        )
                        .clicked()
                    {
                        state.close();
                        action = NavigatorAction::Close;
                    }

                    // Mode badge
                    let (badge_text, badge_bg, badge_fg) = if state.search_mode {
                        ("[Поиск: ввод текста]", theme.accent_primary, theme.bg_canvas)
                    } else {
                        ("[Навигация: hjkl]", theme.bg_tertiary, theme.accent_primary)
                    };

                    egui::Frame::new()
                        .fill(to_color32(badge_bg))
                        .stroke(egui::Stroke::new(1.0, to_color32(theme.border_subtle)))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(badge_text)
                                    .size(10.5)
                                    .monospace()
                                    .color(to_color32(badge_fg)),
                            );
                        });
                });
            });

            ui.add_space(8.0);

            // 2. Search Bar
            let search_frame = egui::Frame::new()
                .fill(to_color32(theme.bg_canvas))
                .stroke(egui::Stroke::new(
                    1.0,
                    if state.search_mode {
                        to_color32(theme.accent_primary)
                    } else {
                        to_color32(theme.border_subtle)
                    },
                ))
                .corner_radius(egui::CornerRadius::same(4))
                .inner_margin(egui::Margin::symmetric(10, 6));

            search_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🔍")
                            .size(13.0)
                            .color(to_color32(theme.text_muted)),
                    );

                    let prev_query = state.query.clone();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut state.query)
                            .hint_text(state.mode.search_placeholder())
                            .desired_width(ui.available_width() - 40.0)
                            .frame(egui::Frame::NONE),
                    );

                    if state.search_mode {
                        response.request_focus();
                    } else if response.clicked() {
                        state.search_mode = true;
                    }

                    if state.query != prev_query {
                        state.recompute_filter();
                    }

                    if !state.query.is_empty() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("✕")
                                .on_hover_text("Очистить поиск")
                                .clicked()
                            {
                                state.query.clear();
                                state.recompute_filter();
                            }
                        });
                    }
                });
            });

            ui.add_space(10.0);

            // 3. Main Split Area: Item List + Preview
            let total_height = 360.0;
            let list_width = 380.0;
            let preview_width = ui.available_width() - list_width - 12.0;

            ui.horizontal(|ui| {
                // Left column: Items
                ui.vertical(|ui| {
                    ui.set_width(list_width);
                    ui.set_height(total_height);

                    let visible_count = state.visible_count();
                    if visible_count == 0 {
                        ui.vertical_centered(|ui| {
                            ui.add_space(40.0);
                            ui.label(
                                egui::RichText::new("Ничего не найдено")
                                    .size(13.0)
                                    .color(to_color32(theme.text_muted)),
                            );
                            if !state.query.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("по запросу «{}»", state.query))
                                        .size(11.0)
                                        .color(to_color32(theme.text_muted)),
                                );
                            }
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .id_salt("navigator_items_scroll")
                            .max_height(total_height)
                            .show_rows(ui, 32.0, visible_count, |ui, row_range| {
                                for row_idx in row_range {
                                    let is_selected = row_idx == state.selected_index;
                                    let Some(&real_idx) = state.filtered_indices.get(row_idx) else {
                                        continue;
                                    };
                                    let Some(item) = state.items.get(real_idx) else {
                                        continue;
                                    };

                                    let bg = if is_selected {
                                        to_color32(theme.bg_selected)
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    let row_frame = egui::Frame::new()
                                        .fill(bg)
                                        .corner_radius(egui::CornerRadius::same(4))
                                        .inner_margin(egui::Margin::symmetric(8, 4));

                                    let row_resp = row_frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // Indent for depth if any
                                            if item.depth > 0 {
                                                ui.add_space((item.depth as f32) * 12.0);
                                            }

                                            // Icon / Prefix
                                            let icon_str = if item.is_folder {
                                                if item.is_collapsed { "📁" } else { "📂" }
                                            } else {
                                                match &item.payload {
                                                    NavigatorItemPayload::File { .. } => "📄",
                                                    NavigatorItemPayload::Buffer { .. } => "📑",
                                                    NavigatorItemPayload::Command { .. } => "⚡",
                                                    NavigatorItemPayload::Heading { .. } => "🔖",
                                                    NavigatorItemPayload::Link { .. } => "🔗",
                                                    NavigatorItemPayload::Board { .. } => "📋",
                                                    NavigatorItemPayload::Custom { .. } => "✦",
                                                }
                                            };
                                            ui.label(egui::RichText::new(icon_str).size(12.0));

                                            // Title
                                            let text_color = if is_selected {
                                                to_color32(theme.text_primary)
                                            } else {
                                                to_color32(theme.text_secondary)
                                            };

                                            let mut title_text = egui::RichText::new(&item.title)
                                                .size(12.5)
                                                .color(text_color);
                                            if is_selected {
                                                title_text = title_text.strong();
                                            }
                                            ui.label(title_text);

                                            // Subtitle or path
                                            if let Some(sub) = &item.subtitle {
                                                ui.label(
                                                    egui::RichText::new(sub)
                                                        .size(10.5)
                                                        .color(to_color32(theme.text_muted)),
                                                );
                                            }

                                            // Shortcut on the far right
                                            if let Some(sc) = &item.shortcut {
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(sc)
                                                                .size(10.0)
                                                                .monospace()
                                                                .color(to_color32(theme.accent_primary)),
                                                        );
                                                    },
                                                );
                                            }
                                        });
                                    });

                                    if row_resp.response.clicked() {
                                        state.selected_index = row_idx;
                                    }
                                    if row_resp.response.double_clicked() {
                                        state.selected_index = row_idx;
                                        action = state.primary_action();
                                        state.close();
                                    }
                                }
                            });
                    }
                });

                // Vertical separator
                ui.separator();

                // Right column: Preview pane
                ui.vertical(|ui| {
                    ui.set_width(preview_width);
                    ui.set_height(total_height);

                    if !state.preview_visible {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.label(
                                egui::RichText::new("Предпросмотр скрыт")
                                    .size(12.0)
                                    .color(to_color32(theme.text_muted)),
                            );
                            ui.label(
                                egui::RichText::new("Нажмите [Space] для отображения")
                                    .size(11.0)
                                    .color(to_color32(theme.text_muted)),
                            );
                        });
                        return;
                    }

                    if let Some(item) = state.selected_item() {
                        // Preview Header
                        ui.label(
                            egui::RichText::new(&item.title)
                                .size(13.5)
                                .strong()
                                .color(to_color32(theme.text_primary)),
                        );

                        if let Some(sub) = &item.subtitle {
                            ui.label(
                                egui::RichText::new(sub)
                                    .size(11.0)
                                    .color(to_color32(theme.text_muted)),
                            );
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Preview body
                        egui::ScrollArea::vertical()
                            .id_salt("navigator_preview_scroll")
                            .max_height(total_height - 60.0)
                            .show(ui, |ui| {
                                if let Some(preview_text) = &item.preview {
                                    ui.label(
                                        egui::RichText::new(preview_text)
                                            .size(11.5)
                                            .color(to_color32(theme.text_secondary)),
                                    );
                                } else {
                                    // Default payload preview
                                    match &item.payload {
                                        NavigatorItemPayload::File { path, is_dir } => {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Тип: {}\nПуть: {}\n\n↵ Открыть файл\na Создать в папке\nr Переименовать\nd Удалить",
                                                    if *is_dir { "Папка" } else { "Markdown заметка" },
                                                    path
                                                ))
                                                .size(11.5)
                                                .color(to_color32(theme.text_muted)),
                                            );
                                        }
                                        NavigatorItemPayload::Buffer { buffer_id, path } => {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Вкладка / Буфер ID: {:?}\nФайл: {}\n\n↵ Переключиться\nd Закрыть",
                                                    buffer_id,
                                                    path.as_deref().unwrap_or("Новый документ")
                                                ))
                                                .size(11.5)
                                                .color(to_color32(theme.text_muted)),
                                            );
                                        }
                                        NavigatorItemPayload::Command { command_id } => {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "Команда: {}\nКатегория: {}\n\n↵ Запустить команду",
                                                    command_id.as_str(),
                                                    item.category.as_deref().unwrap_or("Общее")
                                                ))
                                                .size(11.5)
                                                .color(to_color32(theme.text_muted)),
                                            );
                                        }
                                        _ => {
                                            ui.label(
                                                egui::RichText::new("Нажмите ↵ для выбора")
                                                    .size(11.5)
                                                    .color(to_color32(theme.text_muted)),
                                            );
                                        }
                                    }
                                }
                            });
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.add_space(50.0);
                            ui.label(
                                egui::RichText::new("Выберите элемент для предпросмотра")
                                    .size(12.0)
                                    .color(to_color32(theme.text_muted)),
                            );
                        });
                    }
                });
            });

            // 4. Actions Menu Popup (if open)
            if state.actions_menu_open {
                ui.add_space(6.0);
                let actions_frame = egui::Frame::new()
                    .fill(to_color32(theme.bg_canvas))
                    .stroke(egui::Stroke::new(1.0, to_color32(theme.accent_primary)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(10, 8));

                actions_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Меню действий для выбранного элемента:")
                                .size(11.5)
                                .strong()
                                .color(to_color32(theme.text_primary)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .label(
                                    egui::RichText::new("[Esc] Назад")
                                        .size(10.5)
                                        .color(to_color32(theme.text_muted)),
                                )
                                .clicked()
                            {
                                state.actions_menu_open = false;
                            }
                        });
                    });

                    ui.add_space(4.0);
                    let actions = state.context_actions();
                    for (idx, act) in actions.into_iter().enumerate() {
                        let is_sel = idx == state.actions_menu_selected;
                        let item_bg = if is_sel {
                            to_color32(theme.bg_selected)
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        let item_frame = egui::Frame::new()
                            .fill(item_bg)
                            .corner_radius(egui::CornerRadius::same(3))
                            .inner_margin(egui::Margin::symmetric(6, 2));

                        let resp = item_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                egui::Frame::new()
                                    .fill(to_color32(theme.bg_tertiary))
                                    .stroke(egui::Stroke::new(1.0, to_color32(theme.border_subtle)))
                                    .corner_radius(egui::CornerRadius::same(3))
                                    .inner_margin(egui::Margin::symmetric(4, 1))
                                    .show(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(&act.key)
                                                .size(10.5)
                                                .monospace()
                                                .strong()
                                                .color(to_color32(theme.accent_primary)),
                                        );
                                    });

                                ui.label(
                                    egui::RichText::new(&act.label)
                                        .size(11.5)
                                        .color(to_color32(theme.text_primary)),
                                );
                                ui.label(
                                    egui::RichText::new(format!("— {}", act.description))
                                        .size(10.5)
                                        .color(to_color32(theme.text_muted)),
                                );
                            });
                        });

                        if resp.response.clicked() {
                            state.actions_menu_open = false;
                            action = act.action;
                        }
                    }
                });
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // 5. Bottom Quick Action Footer
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
                for quick in state.quick_actions() {
                    let pill_frame = egui::Frame::new()
                        .fill(to_color32(theme.bg_tertiary))
                        .stroke(egui::Stroke::new(1.0, to_color32(theme.border_subtle)))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(6, 2));

                    let resp = pill_frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&quick.key)
                                    .size(11.0)
                                    .monospace()
                                    .strong()
                                    .color(to_color32(theme.accent_primary)),
                            );
                            ui.label(
                                egui::RichText::new(&quick.label)
                                    .size(11.0)
                                    .color(to_color32(theme.text_secondary)),
                            );
                        });
                    });

                    if resp.response.on_hover_text(&quick.description).clicked() {
                        match quick.key.as_str() {
                            "/" => state.search_mode = true,
                            "?" => state.actions_menu_open = !state.actions_menu_open,
                            _ => {
                                if quick.action != NavigatorAction::None {
                                    action = quick.action;
                                    state.close();
                                }
                            }
                        }
                    }
                }
            });
        });

    if window_res.is_none() {
        return NavigatorAction::None;
    }

    action
}

fn handle_window_input(state: &mut NavigatorState, ui: &mut egui::Ui) -> Option<NavigatorAction> {
    let input = ui.input(|i| i.clone());

    if state.actions_menu_open {
        if input.key_pressed(egui::Key::Escape) {
            state.actions_menu_open = false;
            return Some(NavigatorAction::None);
        }
        if input.key_pressed(egui::Key::ArrowDown) || input.key_pressed(egui::Key::J) {
            let count = state.context_actions().len();
            if count > 0 {
                state.actions_menu_selected = (state.actions_menu_selected + 1).min(count - 1);
            }
            return Some(NavigatorAction::None);
        }
        if input.key_pressed(egui::Key::ArrowUp) || input.key_pressed(egui::Key::K) {
            state.actions_menu_selected = state.actions_menu_selected.saturating_sub(1);
            return Some(NavigatorAction::None);
        }
        if input.key_pressed(egui::Key::Enter) {
            let actions = state.context_actions();
            if let Some(act) = actions.get(state.actions_menu_selected) {
                let action = act.action.clone();
                state.actions_menu_open = false;
                state.close();
                return Some(action);
            }
        }
        return None;
    }

    if state.search_mode {
        if input.key_pressed(egui::Key::Escape) {
            state.search_mode = false;
            return Some(NavigatorAction::None);
        }
        if input.key_pressed(egui::Key::Enter) {
            let act = state.primary_action();
            if act != NavigatorAction::None {
                state.close();
                return Some(act);
            }
        }
        if input.key_pressed(egui::Key::ArrowDown)
            || (input.modifiers.ctrl && input.key_pressed(egui::Key::J))
        {
            state.move_down();
            return Some(NavigatorAction::None);
        }
        if input.key_pressed(egui::Key::ArrowUp)
            || (input.modifiers.ctrl && input.key_pressed(egui::Key::K))
        {
            state.move_up();
            return Some(NavigatorAction::None);
        }
        return None;
    }

    // Normal navigation mode
    if input.key_pressed(egui::Key::Escape) {
        state.close();
        return Some(NavigatorAction::Close);
    }
    if input.key_pressed(egui::Key::Slash) {
        state.search_mode = true;
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown) {
        state.move_down();
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::K) || input.key_pressed(egui::Key::ArrowUp) {
        state.move_up();
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::H) || input.key_pressed(egui::Key::ArrowLeft) {
        state.collapse_or_parent();
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::L) || input.key_pressed(egui::Key::ArrowRight) {
        state.expand_or_child();
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::Enter) {
        let act = state.primary_action();
        if act != NavigatorAction::None {
            state.close();
            return Some(act);
        }
    }
    if input.key_pressed(egui::Key::Questionmark) {
        state.actions_menu_open = !state.actions_menu_open;
        state.actions_menu_selected = 0;
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::Space) {
        state.preview_visible = !state.preview_visible;
        return Some(NavigatorAction::None);
    }
    if input.key_pressed(egui::Key::A) {
        let parent = state.current_parent_dir();
        state.close();
        if input.modifiers.shift {
            return Some(NavigatorAction::CreateFolder { parent_dir: parent });
        } else {
            return Some(NavigatorAction::CreateFile { parent_dir: parent });
        }
    }
    if input.key_pressed(egui::Key::R)
        && let Some(p) = state.current_selected_path()
    {
        state.close();
        return Some(NavigatorAction::RenameFile { path: p });
    }
    if input.key_pressed(egui::Key::D) {
        if state.mode == NavigatorMode::Sessions {
            if let Some(b) = state.current_selected_buffer() {
                return Some(NavigatorAction::CloseBuffer { buffer_id: b });
            }
        } else if let Some(p) = state.current_selected_path() {
            state.close();
            return Some(NavigatorAction::DeleteFile { path: p });
        }
    }

    None
}
