use super::editor::color;
use super::{EditorWidget, icons, widgets};
use crate::app::Application;
use crate::shell::Focus;
use crate::theme::Theme;
use eframe::egui;
use truff_core::text::Buffer;

pub(super) fn show(
    application: &mut Application,
    editor: &mut EditorWidget,
    recent: &mut super::recent::RecentVaults,
    ui: &mut egui::Ui,
    theme: &Theme,
) -> bool {
    let mut open_vault = false;
    for change in application.shell.take_buffer_changes() {
        application.notify_buffer_change_for_facts(&change);
        if let Some(buffer) = application
            .shell
            .workspace()
            .buffers()
            .get_buffer(change.buffer_id)
        {
            editor.apply_change(buffer, &change);
        }
    }
    egui::CentralPanel::default().show(ui, |ui| {
        let is_welcome_screen = application.vault_path().is_none()
            && application
                .shell
                .editor_buffer()
                .is_none_or(Buffer::is_empty)
            && application.shell.mode() == truff_core::vim::Mode::Normal;

        if is_welcome_screen {
            open_vault = show_welcome_screen(application, recent, ui, theme);
        } else {
            let shell = &application.shell;
            let input = editor.show(
                ui,
                shell.editor_buffer(),
                shell.editor_view(),
                shell.mode(),
                application.projection(),
                theme,
                shell.typography(),
                shell.ime_preedit(),
                shell.flash().targets(),
            );
            if let Some(input) = input {
                application.shell.set_focus(Focus::Editor);
                super::events::input(application, input);
            }
            if application.link_completion.active
                && !application.link_completion.candidates.is_empty()
            {
                let cursor_pos = editor
                    .cursor_rect()
                    .map(|r| r.left_bottom())
                    .unwrap_or_else(|| ui.cursor().left_bottom());
                egui::Area::new(egui::Id::new("link_completion_popup"))
                    .fixed_pos(cursor_pos)
                    .order(egui::Order::Foreground)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style())
                            .fill(color(theme.bg_secondary))
                            .stroke(egui::Stroke::new(1.0, color(theme.accent)))
                            .show(ui, |ui| {
                                ui.set_max_width(320.0);
                                let mut clicked_idx = None;
                                for (idx, candidate) in
                                    application.link_completion.candidates.iter().enumerate()
                                {
                                    let is_selected = idx == application.link_completion.selected;
                                    let text = egui::RichText::new(&candidate.display).color(
                                        if is_selected {
                                            color(theme.accent)
                                        } else {
                                            color(theme.text_primary)
                                        },
                                    );
                                    if ui.selectable_label(is_selected, text).clicked() {
                                        clicked_idx = Some(idx);
                                    }
                                }
                                if let Some(idx) = clicked_idx {
                                    application.link_completion.selected = idx;
                                    let _ = application.apply_link_completion();
                                }
                            });
                    });
            }
        }
    });
    open_vault
}

fn show_welcome_screen(
    application: &mut Application,
    recent: &mut super::recent::RecentVaults,
    ui: &mut egui::Ui,
    theme: &Theme,
) -> bool {
    let mut open_vault_dialog = false;
    let available_height = ui.available_height();
    let top_padding = (available_height * 0.12).clamp(24.0, 90.0);

    ui.vertical_centered(|ui| {
        ui.add_space(top_padding);
        ui.label(
            egui::RichText::new("Truff v2")
                .size(16.0)
                .strong()
                .color(color(theme.accent)),
        );
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Персональная база знаний в формате Markdown с поддержкой Vim и вики-ссылок",
            )
            .size(14.0)
            .color(color(theme.text_muted)),
        );

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let button_width = 190.0;
            let total_buttons_w = button_width * 2.0 + 12.0;
            let left_offset = ((ui.available_width() - total_buttons_w) * 0.5).max(0.0);
            ui.add_space(left_offset);

            if widgets::primary_button(ui, "➕  Создать хранилище").clicked() {
                open_vault_dialog = true;
            }
            if widgets::secondary_button(ui, "📂  Открыть существующее").clicked()
            {
                open_vault_dialog = true;
            }
        });

        ui.add_space(32.0);

        let existing_vaults = recent.paths();

        if !existing_vaults.is_empty() {
            ui.label(
                egui::RichText::new("НЕДАВНИЕ ХРАНИЛИЩА")
                    .size(11.0)
                    .strong()
                    .color(color(theme.text_muted)),
            );
            ui.add_space(8.0);

            let card_frame = egui::Frame::new()
                .fill(color(theme.bg_secondary))
                .stroke(egui::Stroke::new(1.0, color(theme.border)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(14, 8));

            let card_width = (ui.available_width() - 32.0).min(480.0);
            let mut remove_path = None;

            ui.allocate_ui_with_layout(
                egui::vec2(card_width, 0.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    card_frame.show(ui, |ui| {
                        ui.set_width(card_width - 28.0);
                        for path in existing_vaults {
                            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("vault");
                            let path_str = path.display().to_string();

                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                icons::render_icon(
                                    ui.painter(),
                                    icons::Icon::Folder,
                                    rect,
                                    color(theme.accent),
                                    1.2,
                                );

                                let resp = ui
                                    .selectable_label(
                                        false,
                                        egui::RichText::new(name)
                                            .strong()
                                            .color(color(theme.text_primary)),
                                    )
                                    .on_hover_text(&path_str);
                                if resp.clicked()
                                    && let Err(e) = application.open_vault(path)
                                {
                                    application.status = e.to_string();
                                }

                                ui.label(
                                    egui::RichText::new(&path_str)
                                        .size(11.0)
                                        .color(color(theme.text_muted)),
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if widgets::icon_button(
                                            ui,
                                            icons::Icon::Close,
                                            "Удалить из недавних",
                                        )
                                        .clicked()
                                        {
                                            remove_path = Some(path.clone());
                                        }
                                    },
                                );
                            });
                            ui.add_space(2.0);
                        }
                    });
                },
            );

            if let Some(to_remove) = remove_path {
                recent.remove(&to_remove);
            }
        }
    });

    open_vault_dialog
}
