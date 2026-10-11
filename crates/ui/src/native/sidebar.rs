use super::editor::color;
use super::{creation::CreationDialogs, icons, widgets};
use crate::app::Application;
use crate::theme::Theme;
use eframe::egui;

pub(super) fn show(
    application: &mut Application,
    dialogs: &mut CreationDialogs,
    ui: &mut egui::Ui,
    theme: &Theme,
) {
    egui::Panel::left("vault")
        .frame(
            egui::Frame::new()
                .fill(color(theme.bg_secondary))
                .stroke(egui::Stroke::new(1.0, color(theme.border)))
                .inner_margin(egui::Margin::symmetric(8, 8)),
        )
        .default_size(220.0)
        .min_size(140.0)
        .max_size(600.0)
        .resizable(true)
        .show_separator_line(true)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(
                    egui::RichText::new("VAULT")
                        .size(11.0)
                        .strong()
                        .color(color(theme.text_muted)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, icons::Icon::Gear, "Настройки").clicked() {
                        dialogs.settings.open = true;
                        let config = crate::app::global_config::GlobalConfig::load();
                        dialogs
                            .settings
                            .init_from_app(application.shell.typography(), &config.theme);
                    }
                    if widgets::icon_button(ui, icons::Icon::MoreHorizontal, "Свернуть все")
                        .clicked()
                    {
                        application.shell.sidebar_model_mut().collapse_all();
                    }
                    if widgets::icon_button(ui, icons::Icon::Vault, "Выбрать / сменить хранилище")
                        .clicked()
                    {
                        dialogs.vault.open = true;
                    }
                    if application.vault_path().is_some() {
                        if widgets::icon_button(ui, icons::Icon::Folder, "Создать новую папку")
                            .clicked()
                        {
                            dialogs.folder.open = true;
                            dialogs.folder.path.clear();
                            dialogs.folder.error = None;
                        }
                        if widgets::icon_button(ui, icons::Icon::Plus, "Создать новую заметку")
                            .clicked()
                        {
                            dialogs.file.open = true;
                            dialogs.file.path.clear();
                            dialogs.file.error = None;
                        }
                    }
                });
            });
            ui.add_space(4.0);
            if application.vault_path().is_none() {
                ui.add_space(8.0);
                if widgets::primary_button(ui, "➕ Создать хранилище").clicked() {
                    dialogs.vault.open = true;
                }
                ui.add_space(4.0);
                if widgets::secondary_button(ui, "📂 Открыть хранилище").clicked()
                {
                    dialogs.vault.open = true;
                }
                let mut config = crate::app::global_config::GlobalConfig::load();
                let existing_vaults: Vec<_> = config
                    .recent_vaults
                    .iter()
                    .filter(|p| p.is_dir())
                    .cloned()
                    .collect();

                if !existing_vaults.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("НЕДАВНИЕ")
                            .size(11.0)
                            .strong()
                            .color(color(theme.text_muted)),
                    );
                    ui.add_space(4.0);
                    let mut remove_path = None;
                    for path in &existing_vaults {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("vault");
                        let path_str = path.display().to_string();

                        ui.horizontal(|ui| {
                            let (icon_rect, _) = ui
                                .allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            icons::render_icon(
                                ui.painter(),
                                icons::Icon::Folder,
                                icon_rect,
                                color(theme.accent),
                                1.2,
                            );

                            let resp = ui
                                .selectable_label(
                                    false,
                                    egui::RichText::new(name).color(color(theme.text_primary)),
                                )
                                .on_hover_text(&path_str);
                            if resp.clicked()
                                && let Err(e) = application.open_vault(path)
                            {
                                application.status = e.to_string();
                            }

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
                    if let Some(to_remove) = remove_path {
                        config.remove_recent_vault(&to_remove);
                        let _ = config.save();
                    }
                }
            } else {
                let model = application.shell.sidebar_model();
                let count = model.visible_count();
                let selected = model.selected_index();
                let active_file = application.active_relative_path();
                let mut open = None;
                let mut toggle_folder = None;
                egui::ScrollArea::vertical().show_rows(ui, 28.0, count, |ui, rows| {
                    for index in rows {
                        if let Some(row) = model.get_visible_row(index) {
                            let indent = (row.depth as f32) * 16.0;
                            let is_selected = index == selected;
                            let is_active =
                                !row.is_folder && row.data.as_deref() == active_file.as_deref();
                            let is_dirty = row
                                .data
                                .as_deref()
                                .is_some_and(|p| application.is_path_dirty(p));

                            let text_color = if is_active || is_selected || row.is_folder {
                                color(theme.text_primary)
                            } else {
                                color(theme.text_muted)
                            };

                            let frame = egui::Frame::new()
                                .fill(if is_selected {
                                    color(theme.selection)
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .corner_radius(egui::CornerRadius::same(4))
                                .inner_margin(egui::Margin {
                                    left: 4,
                                    right: 4,
                                    top: 3,
                                    bottom: 3,
                                });

                            let frame_resp = frame.show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    ui.add_space(indent);
                                    if row.is_folder {
                                        let chevron = if row.is_collapsed {
                                            icons::Icon::ChevronRight
                                        } else {
                                            icons::Icon::ChevronDown
                                        };
                                        let (chev_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(12.0, 16.0),
                                            egui::Sense::hover(),
                                        );
                                        icons::render_icon(
                                            ui.painter(),
                                            chevron,
                                            chev_rect,
                                            color(theme.text_muted),
                                            1.5,
                                        );

                                        let (folder_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(14.0, 16.0),
                                            egui::Sense::hover(),
                                        );
                                        icons::render_icon(
                                            ui.painter(),
                                            icons::Icon::Folder,
                                            folder_rect,
                                            color(theme.text_muted),
                                            1.5,
                                        );
                                    } else {
                                        let (file_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(14.0, 16.0),
                                            egui::Sense::hover(),
                                        );
                                        icons::render_icon(
                                            ui.painter(),
                                            icons::Icon::File,
                                            file_rect,
                                            color(theme.text_muted),
                                            1.5,
                                        );
                                    }

                                    let rich = egui::RichText::new(&row.title)
                                        .color(text_color)
                                        .size(12.5);
                                    ui.label(rich);

                                    if is_dirty {
                                        ui.add_space(2.0);
                                        let (dot_rect, _) = ui.allocate_exact_size(
                                            egui::vec2(8.0, 16.0),
                                            egui::Sense::hover(),
                                        );
                                        icons::draw_dirty_dot(
                                            ui.painter(),
                                            dot_rect.center(),
                                            color(theme.accent),
                                            2.75,
                                        );
                                    }
                                });
                            });

                            let rect = frame_resp.response.rect;
                            let interact_resp =
                                ui.interact(rect, ui.auto_id_with(row.id), egui::Sense::click());
                            if interact_resp.clicked() {
                                if row.is_folder {
                                    toggle_folder = Some(row.id);
                                } else {
                                    open = row.data.clone();
                                }
                            }

                            if is_active {
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(rect.left(), rect.top() + 3.0),
                                        egui::pos2(rect.left(), rect.bottom() - 3.0),
                                    ],
                                    egui::Stroke::new(2.0, color(theme.accent)),
                                );
                            }
                        }
                    }
                });
                if let Some(id) = toggle_folder {
                    application.shell.sidebar_model_mut().toggle_collapse(id);
                }
                if let Some(path) = open
                    && let Err(error) = application.open_note(&path)
                {
                    application.status = error.to_string();
                }
            }
        });
}
