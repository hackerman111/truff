use crate::{
    components::color,
    navigator::{NavigatorAction, NavigatorItemPayload, NavigatorState},
    theme::Theme,
};
use eframe::egui;

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut NavigatorState,
    theme: &Theme,
    height: f32,
    action: &mut NavigatorAction,
) {
    let count = state.visible_count();
    let row_height = theme.geometry.list_row_height;
    let mut area = egui::ScrollArea::vertical()
        .id_salt("navigator_items_scroll")
        .max_height(height)
        .auto_shrink([false, false]);
    if let Some(offset) = state.scroll.prepare(
        state.selected_index,
        count,
        row_height + ui.spacing().item_spacing.y,
        height,
    ) {
        area = area.vertical_scroll_offset(offset);
    }
    let output = area.show_rows(ui, row_height, count, |ui, rows| {
        for index in rows {
            let Some(item) = state
                .filtered_indices
                .get(index)
                .and_then(|index| state.items.get(*index))
            else {
                continue;
            };
            let selected = index == state.selected_index;
            let response = crate::components::list_row(ui, row_height, selected, theme, |ui| {
                ui.spacing_mut().interact_size.y = row_height - theme.spacing_xs * 2.0;
                ui.spacing_mut().item_spacing = egui::vec2(theme.spacing_sm, 0.0);
                ui.horizontal(|ui| {
                    if state.jump_labels_active {
                        const LABELS: &[char] = &[
                            'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'q', 'w', 'e', 'r', 't',
                            'y', 'u', 'i', 'o', 'p', 'z', 'x', 'c', 'v', 'b', 'n', 'm',
                        ];
                        let visible_start = state.scroll.visible_range.start;
                        let badge = index
                            .checked_sub(visible_start)
                            .and_then(|i| LABELS.get(i).copied());
                        if let Some(ch) = badge {
                            ui.label(
                                egui::RichText::new(format!("[{ch}]"))
                                    .monospace()
                                    .strong()
                                    .color(color(theme.accent_primary)),
                            );
                        }
                    }
                    ui.add_space(
                        (item.depth as f32 * theme.spacing_md).min(ui.available_width() * 0.25),
                    );
                    let swatch_color = match item.payload {
                        NavigatorItemPayload::Color { hex }
                        | NavigatorItemPayload::Highlight { hex } => {
                            egui::Color32::from_hex(hex).ok()
                        }
                        _ => None,
                    };
                    if let Some(c) = swatch_color {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, c);
                        ui.painter().rect_stroke(
                            rect,
                            2.0,
                            egui::Stroke::new(1.0, color(theme.border_subtle)),
                            egui::StrokeKind::Inside,
                        );
                    } else {
                        use crate::native::icons::{Icon, render_icon};
                        let (icon, icon_color) = if item.is_folder {
                            (
                                if item.is_collapsed {
                                    Icon::Folder
                                } else {
                                    Icon::FolderOpen
                                },
                                color(theme.accent_primary),
                            )
                        } else {
                            match item.payload {
                                NavigatorItemPayload::Command { .. } => {
                                    (Icon::Command, color(theme.accent_primary))
                                }
                                NavigatorItemPayload::Heading { .. } => {
                                    (Icon::Hash, color(theme.accent_primary))
                                }
                                NavigatorItemPayload::Task { checked: true, .. } => {
                                    (Icon::CheckSquare, color(theme.callout_tip))
                                }
                                NavigatorItemPayload::Task { checked: false, .. } => {
                                    (Icon::Square, color(theme.text_muted))
                                }
                                NavigatorItemPayload::Link { .. } => {
                                    (Icon::Link, color(theme.accent_primary))
                                }
                                NavigatorItemPayload::Vault { .. } => {
                                    (Icon::Vault, color(theme.accent_primary))
                                }
                                _ => (Icon::File, color(theme.text_muted)),
                            }
                        };
                        let (icon_rect, _) =
                            ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        render_icon(ui.painter(), icon, icon_rect, icon_color, 1.35);
                    }
                    let shortcut_width = if item.shortcut.is_some() { 64.0 } else { 0.0 };
                    ui.vertical(|ui| {
                        ui.set_width((ui.available_width() - shortcut_width).max(1.0));
                        let title = egui::RichText::new(item.display_title())
                            .size(theme.geometry.ui_font_size)
                            .color(color(theme.text_primary));
                        ui.add(
                            egui::Label::new(if selected { title.strong() } else { title })
                                .truncate(),
                        );
                        if state.mode == crate::navigator::NavigatorMode::Mixed {
                            ui.label(
                                egui::RichText::new(item.category.as_deref().unwrap_or(""))
                                    .size(theme.geometry.ui_font_size - 2.0)
                                    .color(color(theme.text_muted)),
                            );
                        } else if let Some(subtitle) = item.display_subtitle() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(subtitle)
                                        .size(theme.geometry.ui_font_size - 2.0)
                                        .color(color(theme.text_muted)),
                                )
                                .truncate(),
                            );
                        }
                    });
                    if let Some(shortcut) = &item.shortcut {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(shortcut.as_ref())
                                    .monospace()
                                    .size(theme.geometry.ui_font_size - 2.0)
                                    .color(color(theme.accent_primary)),
                            )
                            .truncate(),
                        );
                    }
                });
            });
            if response.clicked() {
                state.selected_index = index;
            }
            if response.double_clicked() {
                state.selected_index = index;
                *action = state.primary_action();
                if action.closes_navigator() {
                    state.close();
                }
            }
        }
    });
    state.scroll.observe(&output);
    if count == 0 {
        ui.label(egui::RichText::new("Ничего не найдено").color(color(theme.text_muted)));
    }
}
