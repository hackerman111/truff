use crate::{app::Application, components::color, theme::Theme};
use eframe::egui;

pub(super) fn show(application: &mut Application, ui: &mut egui::Ui, theme: &Theme) {
    let Some(input) = application.shell.command_input_mut() else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(":")
                .monospace()
                .color(color(theme.accent_primary)),
        );
        if input.inserting {
            let id = egui::Id::new("native_command_input");
            ui.memory_mut(|memory| memory.request_focus(id));
            let response = ui.add(
                egui::TextEdit::singleline(&mut input.text)
                    .id(id)
                    .font(egui::TextStyle::Monospace)
                    .char_limit(65536)
                    .desired_width(ui.available_width()),
            );
            if response.changed() {
                input.selected = None;
            }
        } else if ui
            .add(
                egui::Label::new(egui::RichText::new(&input.text).monospace())
                    .sense(egui::Sense::click()),
            )
            .clicked()
        {
            input.inserting = true;
            input.selected = None;
        }
    });
    application.refresh_command_completions();
    let Some(input) = application.shell.command_input_mut() else {
        return;
    };
    let mut choose = None;
    egui::ScrollArea::vertical()
        .id_salt("command_suggestions")
        .max_height((ui.ctx().content_rect().height() * 0.22).min(180.0))
        .show(ui, |ui| {
            for (index, item) in input.suggestions.iter().enumerate() {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() * 0.35).min(360.0);
                    let response = ui.add_sized(
                        [width, theme.geometry.ui_font_size + 10.0],
                        egui::Button::selectable(input.selected == Some(index), &item.text),
                    );
                    if input.selected == Some(index) {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                    if response.clicked() {
                        choose = Some(index);
                    }
                    response.on_hover_text(&item.text);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&item.description).color(color(theme.text_muted)),
                        )
                        .truncate(),
                    );
                });
            }
        });
    if let Some(index) = choose {
        input.text.clone_from(&input.suggestions[index].text);
        input.selected = None;
        input.inserting = true;
    }
    ui.label(
        egui::RichText::new(if input.inserting {
            "Insert · Tab дополнить · ↑/↓ история · Enter выполнить · Esc Normal"
        } else {
            "Normal · j/k варианты или история · i ввод · Enter выполнить · Esc закрыть"
        })
        .color(color(theme.text_muted)),
    );
}
