use super::editor::color;
use super::icons;
use crate::app::Application;
use crate::theme::Theme;
use truff_core::text::Buffer;

pub(super) fn show(application: &mut Application, ui: &mut egui::Ui, theme: &Theme) {
    egui::Panel::top("tabs")
        .frame(
            egui::Frame::new()
                .fill(color(theme.bg_panel))
                .stroke(egui::Stroke::new(
                    1.0,
                    color(theme.border_subtle).gamma_multiply(0.4),
                ))
                .inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }),
        )
        .show(ui, |ui| {
            ui.set_height(theme.geometry.tab_height);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.spacing_mut().interact_size.y = theme.geometry.tab_height - 10.0;
                let active_view_id = application.shell.workspace().active_view();
                let mut views: Vec<_> = application.shell.workspace().views().values().collect();
                views.sort_by_key(|v| v.id().get());

                let mut switch_view = None;
                let mut close_view = None;

                let is_empty_workspace = application
                    .shell
                    .editor_buffer()
                    .is_none_or(Buffer::is_empty)
                    && application.active_relative_path().is_none()
                    && application.shell.mode() == truff_core::vim::Mode::Normal;

                if views.is_empty() || is_empty_workspace {
                    let vault_name = application
                        .vault_path()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("Truff");
                    ui.label(
                        egui::RichText::new(vault_name)
                            .strong()
                            .color(color(theme.accent)),
                    );
                    ui.label(egui::RichText::new("·").color(color(theme.text_muted)));
                    ui.label(
                        egui::RichText::new(if application.vault_path().is_none() {
                            "Open a vault with :vault <path>"
                        } else {
                            "Select or create a note in the sidebar"
                        })
                        .color(color(theme.text_muted)),
                    );
                } else {
                    for view in views {
                        let view_id = view.id();
                        let is_active = Some(view_id) == active_view_id;
                        let is_pinned = application.view_is_pinned(view_id);
                        let buffer_id = view.buffer_id();
                        let entry = application.shell.workspace().buffers().get(buffer_id);
                        let is_scratch = entry.is_none_or(|e| e.path().is_none());
                        let is_clean_empty = application
                            .shell
                            .workspace()
                            .buffers()
                            .get_buffer(buffer_id)
                            .is_none_or(|b| !b.is_dirty() && b.as_str().is_empty());
                        if is_scratch && is_clean_empty {
                            continue;
                        }
                        let title = entry
                            .and_then(|e| e.path())
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Scratch");
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
                                nw: 6,
                                ne: 6,
                                sw: 0,
                                se: 0,
                            })
                            .inner_margin(egui::Margin::symmetric(10, 5));

                        let frame_resp = frame.show(ui, |ui| {
                            ui.set_height(theme.geometry.tab_height - 10.0);
                            ui.spacing_mut().interact_size.y = theme.geometry.tab_height - 10.0;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;

                                let (icon_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(13.0, 13.0),
                                    egui::Sense::hover(),
                                );
                                icons::render_icon(
                                    ui.painter(),
                                    if is_pinned {
                                        icons::Icon::Pin
                                    } else {
                                        icons::Icon::File
                                    },
                                    icon_rect,
                                    if is_active || is_pinned {
                                        color(theme.accent)
                                    } else {
                                        text_color
                                    },
                                    1.4,
                                );

                                let mut rich = egui::RichText::new(title)
                                    .size(theme.geometry.ui_font_size)
                                    .color(text_color);
                                if is_active {
                                    rich = rich.strong();
                                }
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
                                            color(theme.bg_tertiary),
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

                        if !is_active
                            && frame_resp.response.interact(egui::Sense::click()).clicked()
                            && close_view.is_none()
                        {
                            switch_view = Some(view_id);
                        }
                    }
                }

                if let Some(id) = switch_view
                    && let Err(error) = application.focus_view(id)
                {
                    application.status = error.to_string();
                }
                if let Some(id) = close_view
                    && let Err(error) = application.close_view(id)
                {
                    application.status = error.to_string();
                }
            });
        });
}
