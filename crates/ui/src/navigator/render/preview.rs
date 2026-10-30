use crate::{
    components::color,
    native::icons::{self, Icon},
    navigator::{NavigatorItemPayload, NavigatorState},
    theme::Theme,
};
use std::io::Read;

pub(super) fn show(ui: &mut egui::Ui, state: &mut NavigatorState, theme: &Theme, height: f32) {
    if !state.preview_visible {
        ui.label("Предпросмотр скрыт · p — показать");
        return;
    }
    let Some(item_idx) = state.filtered_indices.get(state.selected_index).copied() else {
        ui.label("Выберите элемент для предпросмотра");
        return;
    };
    let Some(item) = state.items.get(item_idx) else {
        ui.label("Выберите элемент для предпросмотра");
        return;
    };
    let item_id = item.id;
    if item.preview.is_none()
        && state
            .cached_preview
            .as_ref()
            .is_none_or(|(id, _)| *id != item_id)
    {
        match &item.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                let full_path = if let Some(root) = &state.vault_root {
                    root.join(path)
                } else {
                    std::path::PathBuf::from(path)
                };
                let loaded = if *is_dir {
                    if let Ok(entries) = std::fs::read_dir(&full_path) {
                        let mut files = 0usize;
                        let mut dirs = 0usize;
                        for e in entries.flatten() {
                            if e.file_type().is_ok_and(|ft| ft.is_dir()) {
                                dirs += 1;
                            } else {
                                files += 1;
                            }
                        }
                        format!("Заметок: {files} · Вложенных папок: {dirs}").into_boxed_str()
                    } else {
                        Box::from("Папка хранилища")
                    }
                } else if let Ok(file) = std::fs::File::open(&full_path) {
                    let mut bytes = Vec::with_capacity(4096);
                    if file.take(4096).read_to_end(&mut bytes).is_ok() {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut out = String::new();
                        for (i, line) in text.lines().take(30).enumerate() {
                            if i > 0 {
                                out.push('\n');
                            }
                            out.push_str(line);
                        }
                        out.into_boxed_str()
                    } else {
                        Box::from("Файл не открыт")
                    }
                } else {
                    Box::from("Файл не открыт")
                };
                state.cached_preview = Some((item_id, loaded));
            }
            NavigatorItemPayload::Location { path, byte } => {
                let full_path = if let Some(root) = &state.vault_root {
                    root.join(path)
                } else {
                    std::path::PathBuf::from(path)
                };
                if let Ok(text) = std::fs::read_to_string(&full_path) {
                    let target_line = text[..(*byte).min(text.len())]
                        .lines()
                        .count()
                        .saturating_sub(1);
                    let start_line = target_line.saturating_sub(4);
                    let snippet: Vec<&str> = text.lines().skip(start_line).take(20).collect();
                    state.cached_preview = Some((item_id, snippet.join("\n").into_boxed_str()));
                }
            }
            _ => {}
        }
    }

    let Some(item) = state.items.get(item_idx) else {
        return;
    };
    let cached_text = state
        .cached_preview
        .as_ref()
        .filter(|(id, _)| *id == item_id)
        .map(|(_, text)| text.as_ref());

    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(item.display_title())
                    .strong()
                    .size(theme.geometry.ui_font_size + 3.0)
                    .color(color(theme.text_primary)),
            )
            .truncate(),
        );
        if let Some(cat) = &item.category {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(cat.as_ref())
                        .size(theme.geometry.ui_font_size - 2.0)
                        .color(color(theme.accent_primary)),
                );
            });
        }
    });
    ui.separator();
    if let Some(reason) = &item.disabled_reason {
        ui.colored_label(color(theme.text_muted), reason.as_ref());
    }
    let preview_width = ui.available_width();
    egui::ScrollArea::vertical()
        .id_salt("navigator_preview_scroll")
        .max_width(preview_width)
        .max_height((height - 36.0).max(1.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_max_width(preview_width);
            match &item.payload {
                NavigatorItemPayload::Color { hex } | NavigatorItemPayload::Highlight { hex } => {
                    let is_highlight =
                        matches!(item.payload, NavigatorItemPayload::Highlight { .. });
                    ui.label(
                        egui::RichText::new(if is_highlight {
                            "Образец цвета маркера (Highlight)"
                        } else {
                            "Образец цвета текста (Foreground)"
                        })
                        .color(color(theme.text_muted)),
                    );
                    ui.add_space(theme.spacing_sm);
                    if let Ok(c) = egui::Color32::from_hex(hex) {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width().min(240.0), 48.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 4.0, c);
                        ui.painter().rect_stroke(
                            rect,
                            4.0,
                            egui::Stroke::new(1.0, color(theme.border_subtle)),
                            egui::StrokeKind::Inside,
                        );

                        ui.add_space(theme.spacing_sm);
                        ui.horizontal(|ui| {
                            ui.label("HEX:");
                            ui.label(
                                egui::RichText::new(*hex)
                                    .monospace()
                                    .strong()
                                    .color(color(theme.text_primary)),
                            );
                        });
                        ui.add_space(theme.spacing_sm);
                        ui.label("Пример оформления:");
                        egui::Frame::new()
                            .fill(color(theme.bg_secondary))
                            .stroke(egui::Stroke::new(1.0, color(theme.border_subtle)))
                            .corner_radius(egui::CornerRadius::same(4))
                            .inner_margin(egui::Margin::same(8))
                            .show(ui, |ui| {
                                if is_highlight {
                                    ui.label(
                                        egui::RichText::new("Выделенный текст в заметке")
                                            .background_color(c)
                                            .color(egui::Color32::from_rgb(46, 52, 64)),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("Цветной фрагмент текста")
                                            .color(c)
                                            .strong(),
                                    );
                                }
                            });
                    }
                }
                NavigatorItemPayload::File { path, is_dir } => {
                    if *is_dir {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) = ui
                                .allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
                            icons::render_icon(
                                ui.painter(),
                                Icon::FolderOpen,
                                icon_rect,
                                color(theme.accent_primary),
                                1.4,
                            );
                            ui.label(
                                egui::RichText::new("Папка")
                                    .strong()
                                    .color(color(theme.accent_primary)),
                            );
                        });
                        ui.label(egui::RichText::new(path).color(color(theme.text_secondary)));
                        ui.add_space(theme.spacing_sm);
                        if let Some(summary) = item.preview.as_deref().or(cached_text) {
                            ui.label(
                                egui::RichText::new(summary).color(color(theme.text_secondary)),
                            );
                        }
                    } else {
                        ui.horizontal(|ui| {
                            let (icon_rect, _) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                            icons::render_icon(
                                ui.painter(),
                                Icon::File,
                                icon_rect,
                                color(theme.text_muted),
                                1.3,
                            );
                            ui.label(egui::RichText::new(path).color(color(theme.text_secondary)));
                        });
                        ui.separator();
                        let preview_text = item
                            .preview
                            .as_deref()
                            .or(cached_text)
                            .unwrap_or("Файл не открыт");
                        if preview_text == "Файл не открыт" {
                            ui.label(
                                egui::RichText::new(preview_text)
                                    .size(theme.geometry.ui_font_size - 1.0)
                                    .color(color(theme.text_muted)),
                            );
                        } else {
                            super::markdown::render(ui, preview_text, theme);
                        }
                    }
                }
                NavigatorItemPayload::Command { command_id } => {
                    ui.label(
                        egui::RichText::new(command_id.as_str())
                            .monospace()
                            .strong()
                            .color(color(theme.accent_primary)),
                    );
                    if let Some(shortcut) = &item.shortcut {
                        ui.add_space(theme.spacing_xs);
                        ui.horizontal(|ui| {
                            ui.label("Сочетание клавиш:");
                            ui.label(
                                egui::RichText::new(shortcut.as_ref())
                                    .monospace()
                                    .color(color(theme.accent_primary)),
                            );
                        });
                    }
                    if let Some(subtitle) = item.display_subtitle() {
                        ui.add_space(theme.spacing_sm);
                        ui.label(egui::RichText::new(subtitle).color(color(theme.text_secondary)));
                    }
                    ui.add_space(theme.spacing_sm);
                    ui.label(
                        egui::RichText::new("Нажмите Enter для выполнения команды.")
                            .size(theme.geometry.ui_font_size - 2.0)
                            .color(color(theme.text_muted)),
                    );
                }
                NavigatorItemPayload::Buffer { path, buffer_id } => {
                    ui.label(
                        egui::RichText::new(format!("Буфер #{:?}", buffer_id))
                            .color(color(theme.text_muted)),
                    );
                    ui.label(
                        egui::RichText::new(path.as_deref().unwrap_or("Новый документ"))
                            .strong()
                            .color(color(theme.text_primary)),
                    );
                    if let Some(preview) = &item.preview {
                        ui.add_space(theme.spacing_sm);
                        super::markdown::render(ui, preview.as_ref(), theme);
                    }
                }
                NavigatorItemPayload::View { view_id, path } => {
                    ui.label(
                        egui::RichText::new(format!("Вкладка #{:?}", view_id))
                            .color(color(theme.text_muted)),
                    );
                    ui.label(
                        egui::RichText::new(path.as_deref().unwrap_or("Новый документ"))
                            .strong()
                            .color(color(theme.text_primary)),
                    );
                    if let Some(preview) = &item.preview {
                        ui.add_space(theme.spacing_sm);
                        super::markdown::render(ui, preview.as_ref(), theme);
                    }
                }
                NavigatorItemPayload::Task { line, checked } => {
                    ui.horizontal(|ui| {
                        let status_color = if *checked {
                            color(theme.callout_tip)
                        } else {
                            color(theme.callout_warning)
                        };
                        let (icon_rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        icons::render_icon(
                            ui.painter(),
                            if *checked {
                                Icon::CheckSquare
                            } else {
                                Icon::Square
                            },
                            icon_rect,
                            status_color,
                            1.4,
                        );
                        let status_badge = if *checked {
                            "Выполнено"
                        } else {
                            "К выполнению"
                        };
                        ui.label(
                            egui::RichText::new(status_badge)
                                .strong()
                                .color(status_color),
                        );
                        ui.label(format!("(Строка {line})"));
                    });
                    if let Some(preview) = &item.preview {
                        ui.add_space(theme.spacing_sm);
                        ui.label(
                            egui::RichText::new(preview.as_ref()).color(color(theme.text_primary)),
                        );
                    }
                }
                NavigatorItemPayload::Heading { line, level, text } => {
                    ui.label(
                        egui::RichText::new(format!("H{level} · Строка {line}"))
                            .color(color(theme.accent_primary)),
                    );
                    if let Some(preview) = item.preview.as_deref().or(cached_text) {
                        ui.separator();
                        super::markdown::render(ui, preview, theme);
                    } else {
                        ui.label(
                            egui::RichText::new(text)
                                .strong()
                                .size(theme.geometry.ui_font_size + 1.0)
                                .color(color(theme.text_primary)),
                        );
                    }
                }
                NavigatorItemPayload::Link { target } => {
                    ui.label(egui::RichText::new("Ссылка:").color(color(theme.text_muted)));
                    ui.label(
                        egui::RichText::new(format!("[[{target}]]"))
                            .strong()
                            .color(color(theme.accent_primary)),
                    );
                    if let Some(preview) = item.preview.as_deref().or(cached_text) {
                        ui.separator();
                        super::markdown::render(ui, preview, theme);
                    }
                }
                NavigatorItemPayload::Location { path, .. } => {
                    ui.label(egui::RichText::new(path).color(color(theme.text_secondary)));
                    if let Some(preview) = item.preview.as_deref().or(cached_text) {
                        ui.separator();
                        super::markdown::render(ui, preview, theme);
                    }
                }
                _ => {
                    if let Some(preview) = item.preview.as_deref().or(cached_text) {
                        super::markdown::render(ui, preview, theme);
                    } else if let Some(sub) = item.display_subtitle() {
                        ui.label(egui::RichText::new(sub).color(color(theme.text_secondary)));
                    } else {
                        ui.label("Enter — выбрать");
                    }
                }
            }
        });
}
