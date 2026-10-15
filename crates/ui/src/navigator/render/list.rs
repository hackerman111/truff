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
                    ui.add_space(
                        (item.depth as f32 * theme.spacing_md).min(ui.available_width() * 0.25),
                    );
                    let icon = if item.is_folder {
                        if item.is_collapsed { "▸" } else { "▾" }
                    } else {
                        match item.payload {
                            NavigatorItemPayload::Command { .. } => "⌘",
                            NavigatorItemPayload::Heading { .. } => "#",
                            NavigatorItemPayload::Task { checked: true, .. } => "☑",
                            NavigatorItemPayload::Task { checked: false, .. } => "☐",
                            NavigatorItemPayload::Link { .. } => "↗",
                            _ => "·",
                        }
                    };
                    ui.label(egui::RichText::new(icon).color(color(theme.text_muted)));
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
