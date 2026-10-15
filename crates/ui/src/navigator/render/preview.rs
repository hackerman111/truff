use crate::{
    components::color,
    navigator::{NavigatorItemPayload, NavigatorState},
    theme::Theme,
};
use eframe::egui;

pub(super) fn show(ui: &mut egui::Ui, state: &NavigatorState, theme: &Theme, height: f32) {
    if !state.preview_visible {
        ui.label("Предпросмотр скрыт · p — показать");
        return;
    }
    let Some(item) = state.selected_item() else {
        ui.label("Выберите элемент для предпросмотра");
        return;
    };
    ui.add(
        egui::Label::new(
            egui::RichText::new(item.display_title())
                .strong()
                .size(theme.geometry.ui_font_size + 3.0)
                .color(color(theme.text_primary)),
        )
        .truncate(),
    );
    ui.separator();
    if let Some(reason) = &item.disabled_reason {
        ui.colored_label(color(theme.text_muted), reason.as_ref());
    }
    egui::ScrollArea::vertical()
        .id_salt("navigator_preview_scroll")
        .max_height((height - 36.0).max(1.0))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(preview) = &item.preview {
                ui.label(egui::RichText::new(preview.as_ref()).color(color(theme.text_secondary)));
            } else {
                match &item.payload {
                    NavigatorItemPayload::File { path, is_dir } => {
                        ui.label(if *is_dir {
                            "Папка"
                        } else {
                            "Markdown заметка"
                        });
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(path).color(color(theme.text_secondary)),
                            )
                            .wrap(),
                        );
                    }
                    NavigatorItemPayload::Buffer { path, .. } => {
                        ui.label("Открытая вкладка");
                        ui.add(
                            egui::Label::new(path.as_deref().unwrap_or("Новый документ")).wrap(),
                        );
                    }
                    NavigatorItemPayload::Command { command_id } => {
                        ui.label(egui::RichText::new(command_id.as_str()).monospace());
                        ui.label(item.category.as_deref().unwrap_or("Общее"));
                    }
                    _ => {
                        ui.label("Enter — выбрать");
                    }
                }
            }
        });
}
