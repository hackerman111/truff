use super::editor::color;
use super::{EditorWidget, icons, widgets};
use crate::app::Application;
use crate::shell::Focus;
use crate::theme::Theme;
use truff_core::text::Buffer;

pub(super) fn show(
    application: &mut Application,
    editor: &mut EditorWidget,
    kanban: &mut super::kanban::controller::KanbanController,
    recent: &mut super::recent::RecentVaults,
    ui: &mut egui::Ui,
    theme: &Theme,
) -> bool {
    let mut open_vault = false;
    egui::CentralPanel::default().show(ui, |ui| {
        let is_welcome_screen = application.vault_path().is_none()
            && application
                .shell
                .editor_buffer()
                .is_none_or(Buffer::is_empty)
            && application.shell.mode() == truff_core::vim::Mode::Normal;

        if is_welcome_screen {
            open_vault = show_welcome_screen(application, recent, ui, theme);
        } else if let Some(view_id) = application.shell.workspace().active_view()
            && kanban.show_central(view_id, application, ui, theme)
        {
            // Central Kanban board view rendered
        } else {
            let shell = &application.shell;
            let note_title = shell.editor_view().and_then(|view| {
                let buffer_id = view.buffer_id();
                let entry = shell.workspace().buffers().get(buffer_id)?;
                let name = entry.path()?.file_name()?.to_str()?;
                Some(name.strip_suffix(".md").unwrap_or(name))
            });
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
                note_title,
            );
            if let Some(input) = input {
                if let crate::editor::EditorInput::ToggleFold { heading_line } = input {
                    if let Some(buffer) = shell.editor_buffer() {
                        editor.toggle_fold(heading_line, buffer);
                        ui.ctx().request_repaint();
                    }
                } else {
                    application.shell.set_focus(Focus::Editor);
                    super::events::input(application, input);
                }
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
                .size(18.0)
                .strong()
                .color(color(theme.accent)),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(
                "Персональная база знаний в формате Markdown с поддержкой Vim и вики-ссылок",
            )
            .size(13.5)
            .color(color(theme.text_muted)),
        );

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let button_width = 190.0;
            let total_buttons_w = button_width * 2.0 + 12.0;
            let left_offset = ((ui.available_width() - total_buttons_w) * 0.5).max(0.0);
            ui.add_space(left_offset);

            if widgets::primary_button_with_icon(ui, icons::Icon::Plus, "Создать хранилище")
                .clicked()
            {
                open_vault_dialog = true;
            }
            if widgets::secondary_button_with_icon(
                ui,
                icons::Icon::FolderOpen,
                "Открыть существующее",
            )
            .clicked()
            {
                open_vault_dialog = true;
            }
        });

        ui.add_space(32.0);

        if !recent.paths().is_empty() {
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

            ui.allocate_ui_with_layout(
                egui::vec2(card_width, 0.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    card_frame.show(ui, |ui| {
                        ui.set_width(card_width - 28.0);
                        recent.show_rows(application, ui, theme, true);
                    });
                },
            );
        }
    });

    open_vault_dialog
}
