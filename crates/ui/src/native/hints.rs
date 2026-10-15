use super::editor::color;
use crate::{overlays::WhichKeyMode, shell::Shell};
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

pub(super) fn show(shell: &mut Shell, context: &egui::Context) {
    if !shell.which_key().is_visible() {
        return;
    }
    if shell.which_key().mode() == WhichKeyMode::Help {
        show_help(shell, context);
        return;
    }
    let theme = shell.theme();
    let overlay = shell.which_key();
    let screen = context.content_rect();
    egui::Window::new("which_key_overlay")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -32.0])
        .frame(crate::components::overlay_frame(theme))
        .show(context, |ui| {
            ui.set_width((screen.width() * 0.8).clamp(32.0, 1000.0) - 32.0);
            ui.label(
                egui::RichText::new(if overlay.title().is_empty() {
                    "Which-Key"
                } else {
                    overlay.title()
                })
                .strong()
                .color(color(theme.accent_primary)),
            );
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(screen.height() * 0.25)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for hint in overlay.hints() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(crate::overlays::key_label(hint.key))
                                        .monospace()
                                        .strong()
                                        .color(color(theme.accent_primary)),
                                );
                                ui.label(humanize_description(&hint.description));
                                ui.add_space(16.0);
                            });
                        }
                    });
                });
        });
}

fn show_help(shell: &mut Shell, context: &egui::Context) {
    let theme = shell.theme().clone();
    let screen = context.content_rect();
    let mut size = theme.geometry.navigator_size(screen.size());
    size.y = screen.height() * 0.82;
    let mut close = false;
    let help = &mut shell.which_key_mut().help;
    egui::Modal::new(egui::Id::new("full_help"))
        .frame(crate::components::overlay_frame(&theme))
        .show(context, |ui| {
            ui.set_width((size.x - theme.spacing_lg * 2.0).max(0.0));
            ui.set_height((size.y - theme.spacing_lg * 2.0).max(0.0));
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Справка по горячим клавишам")
                        .strong()
                        .color(color(theme.accent_primary)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close |= ui.button("Закрыть ×").clicked();
                });
            });
            if help.inserting {
                ui.memory_mut(|memory| memory.request_focus(egui::Id::new("overlay_search_input")));
            }
            let response = crate::components::search_input(
                ui,
                &mut help.query,
                "/ — поиск по клавишам и описаниям",
                &theme,
            );
            if help.inserting {
                response.request_focus();
            } else if response.clicked() {
                help.inserting = true;
            } else {
                response.surrender_focus();
            }
            if response.changed() {
                help.filter();
            }
            ui.separator();
            let height = (ui.available_height() - 36.0).max(1.0);
            let row_height = theme.geometry.list_row_height;
            let stride = row_height + ui.spacing().item_spacing.y;
            let count = help.filtered.len();
            let mut area = egui::ScrollArea::vertical()
                .id_salt("help_items")
                .max_height(height)
                .auto_shrink([false, false]);
            if let Some(offset) = help.scroll.prepare(help.selected, count, stride, height) {
                area = area.vertical_scroll_offset(offset);
            }
            let output = area.show_rows(ui, row_height, count, |ui, rows| {
                for index in rows {
                    let item = &help.items[help.filtered[index]];
                    let selected = index == help.selected;
                    let response =
                        crate::components::list_row(ui, row_height, selected, &theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [140.0_f32.min(ui.available_width() * 0.3), 30.0],
                                    egui::Label::new(
                                        egui::RichText::new(&item.keys)
                                            .monospace()
                                            .color(color(theme.accent_primary)),
                                    )
                                    .truncate(),
                                );
                                ui.vertical(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&item.description)
                                                .size(theme.geometry.ui_font_size)
                                                .color(color(theme.text_primary)),
                                        )
                                        .truncate(),
                                    );
                                    ui.label(
                                        egui::RichText::new(&item.category)
                                            .size(theme.geometry.ui_font_size - 2.0)
                                            .color(color(theme.text_muted)),
                                    );
                                });
                            });
                        });
                    if response.clicked() {
                        help.selected = index;
                    }
                }
            });
            help.scroll.observe(&output);
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "{} / {} · j/k · gg/G · PgUp/PgDn · Ctrl-d/u · / поиск · Esc назад",
                    if count == 0 { 0 } else { help.selected + 1 },
                    count
                ))
                .size(theme.geometry.ui_font_size - 2.0)
                .color(color(theme.text_muted)),
            );
        });
    if close {
        shell.which_key_mut().hide();
    }
}
