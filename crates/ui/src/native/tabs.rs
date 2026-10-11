use super::editor::color;
use super::icons;
use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

pub(super) fn show(application: &mut Application, ui: &mut egui::Ui, theme: &Theme) {
    egui::Panel::top("tabs")
        .frame(
            egui::Frame::new()
                .fill(color(crate::theme::Color::from_hex("#20242c").unwrap()))
                .stroke(egui::Stroke::new(1.0, color(theme.border)))
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }),
        )
        .show(ui, |ui| {
            ui.set_height(34.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let active_view_id = application.shell.workspace().active_view();
                let mut views: Vec<_> = application.shell.workspace().views().values().collect();
                views.sort_by_key(|v| v.id().get());

                let mut switch_view = None;
                let mut close_view = None;

                let is_welcome = application.vault_path().is_none()
                    && application.shell.editor_text().is_empty()
                    && application.shell.mode() == truff_core::vim::Mode::Normal;

                if views.is_empty() || is_welcome {
                    ui.label(
                        egui::RichText::new("Truff")
                            .strong()
                            .color(color(theme.accent)),
                    );
                    ui.label(egui::RichText::new("·").color(color(theme.text_muted)));
                    ui.label(
                        egui::RichText::new("Open a vault with :vault <path>")
                            .color(color(theme.text_muted)),
                    );
                } else {
                    for view in views {
                        let view_id = view.id();
                        let is_active = Some(view_id) == active_view_id;
                        let buffer_id = view.buffer_id();
                        let entry = application.shell.workspace().buffers().get(buffer_id);
                        let title = entry
                            .and_then(|e| e.path())
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Scratch")
                            .to_string();
                        let is_dirty = application
                            .shell
                            .workspace()
                            .buffers()
                            .get_buffer(buffer_id)
                            .is_some_and(|b| b.is_dirty());

                        let tab_bg = if is_active {
                            color(theme.bg_primary)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let text_color = if is_active {
                            color(theme.text_primary)
                        } else {
                            color(theme.text_muted)
                        };

                        let frame = egui::Frame::new()
                            .fill(tab_bg)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(egui::CornerRadius {
                                nw: 4,
                                ne: 4,
                                sw: 0,
                                se: 0,
                            })
                            .inner_margin(egui::Margin::symmetric(10, 5));

                        let frame_resp = frame.show(ui, |ui| {
                            ui.set_height(24.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                let (icon_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(13.0, 13.0),
                                    egui::Sense::hover(),
                                );
                                icons::render_icon(
                                    ui.painter(),
                                    icons::Icon::File,
                                    icon_rect,
                                    text_color,
                                    1.4,
                                );

                                let rich = egui::RichText::new(&title).size(12.5).color(text_color);
                                let resp = ui.selectable_label(false, rich);

                                let (action_rect, action_resp) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::click(),
                                );

                                let is_hovered = resp.hovered()
                                    || action_resp.hovered()
                                    || ui.rect_contains_pointer(resp.rect)
                                    || ui.rect_contains_pointer(action_rect);

                                if is_hovered {
                                    let close_hovered = action_resp.hovered();
                                    if close_hovered {
                                        ui.painter().rect_filled(
                                            action_rect,
                                            egui::CornerRadius::same(3),
                                            egui::Color32::from_hex("#394351").unwrap(),
                                        );
                                    }
                                    icons::render_icon(
                                        ui.painter(),
                                        icons::Icon::Close,
                                        action_rect,
                                        if close_hovered {
                                            color(theme.text_primary)
                                        } else {
                                            color(theme.text_muted)
                                        },
                                        1.5,
                                    );
                                    if action_resp.clicked() {
                                        close_view = Some(view_id);
                                    }
                                } else if is_dirty {
                                    icons::draw_dirty_dot(
                                        ui.painter(),
                                        action_rect.center(),
                                        color(theme.accent),
                                        2.75,
                                    );
                                }

                                if resp.clicked() {
                                    switch_view = Some(view_id);
                                }
                            });
                        });

                        if is_active {
                            let rect = frame_resp.response.rect;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(rect.left(), rect.bottom()),
                                    egui::pos2(rect.right(), rect.bottom()),
                                ],
                                egui::Stroke::new(2.0, color(theme.accent)),
                            );
                        } else if frame_resp.response.interact(egui::Sense::click()).clicked()
                            && close_view.is_none()
                        {
                            switch_view = Some(view_id);
                        }
                    }
                }

                if let Some(id) = switch_view {
                    application.shell.workspace_mut().set_active_view(Some(id));
                    application.shell.reset_editor_input();
                }
                if let Some(id) = close_view {
                    let _ = application.dispatch(truff_core::commands::Action::CloseView(id));
                }
            });
        });
}
