use super::editor::color;
use super::{creation::CreationDialogs, icons, widgets};
use crate::app::Application;
use crate::theme::Theme;

pub(super) fn show(
    application: &mut Application,
    dialogs: &mut CreationDialogs,
    recent: &mut super::recent::RecentVaults,
    ui: &mut egui::Ui,
    theme: &Theme,
) {
    egui::Panel::left("vault")
        .frame(crate::components::dock_frame(theme))
        .default_size(theme.geometry.sidebar_default_width)
        .min_size(180.0_f32.min(ui.available_width() * 0.35))
        .max_size((ui.available_width() * 0.35).min(480.0))
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
                            let parent =
                                application.shell.sidebar_creation_parent().or_else(|| {
                                    application.active_relative_path().and_then(|p| {
                                        std::path::Path::new(&p)
                                            .parent()
                                            .and_then(|d| d.to_str())
                                            .filter(|s| !s.is_empty())
                                            .map(str::to_owned)
                                    })
                                });
                            dialogs.folder.path = parent.map_or(String::new(), |d| format!("{d}/"));
                            dialogs.folder.error = None;
                            dialogs.folder.needs_cursor_end = true;
                        }
                        if widgets::icon_button(ui, icons::Icon::Plus, "Создать новую заметку")
                            .clicked()
                        {
                            dialogs.file.open = true;
                            let parent =
                                application.shell.sidebar_creation_parent().or_else(|| {
                                    application.active_relative_path().and_then(|p| {
                                        std::path::Path::new(&p)
                                            .parent()
                                            .and_then(|d| d.to_str())
                                            .filter(|s| !s.is_empty())
                                            .map(str::to_owned)
                                    })
                                });
                            dialogs.file.path = parent.map_or(String::new(), |d| format!("{d}/"));
                            dialogs.file.error = None;
                            dialogs.file.needs_cursor_end = true;
                        }
                    }
                });
            });
            ui.add_space(4.0);
            if application.vault_path().is_none() {
                ui.add_space(8.0);
                if widgets::primary_button_with_icon(ui, icons::Icon::Plus, "Создать хранилище")
                    .clicked()
                {
                    dialogs.vault.open = true;
                }
                ui.add_space(4.0);
                if widgets::secondary_button_with_icon(
                    ui,
                    icons::Icon::FolderOpen,
                    "Открыть хранилище",
                )
                .clicked()
                {
                    dialogs.vault.open = true;
                }
                let existing_vaults = recent.paths();

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
                    for path in existing_vaults {
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
                        recent.remove(&to_remove);
                    }
                }
            } else {
                let model = application.shell.sidebar_model();
                let count = model.visible_count();
                let selected = model.selected_index();
                let active_file = application.active_relative_path();
                let any_dirty = application.has_any_dirty_buffer();
                let mut open = None;
                let mut toggle_folder = None;
                let mut select_row = None;
                let mut new_note_in = None;
                let mut new_folder_in = None;
                let mut rename_item = None;
                let mut trash_item = None;
                crate::rows::selected_rows(
                    ui,
                    "file_tree_scroll",
                    theme.geometry.tree_row_height,
                    count,
                    selected,
                    |ui, rows| {
                        ui.spacing_mut().interact_size.y = theme.geometry.tree_row_height - 6.0;
                        for index in rows {
                            if let Some(row) = model.get_visible_row(index) {
                                let indent = (row.depth as f32) * 16.0;
                                let is_selected = model.selected_range().contains(&index);
                                let is_active =
                                    !row.is_folder && row.data.as_deref() == active_file.as_deref();
                                let is_dirty = any_dirty
                                    && row
                                        .data
                                        .as_deref()
                                        .is_some_and(|p| application.is_path_dirty(p));

                                let text_color = if is_active {
                                    color(theme.accent)
                                } else if is_selected || row.is_folder {
                                    color(theme.text_primary)
                                } else {
                                    color(theme.text_secondary)
                                };

                                let frame = egui::Frame::new()
                                    .fill(if is_selected {
                                        color(theme.selection)
                                    } else if is_active {
                                        color(theme.selection).gamma_multiply(0.75)
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    })
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .inner_margin(egui::Margin {
                                        left: 8,
                                        right: 8,
                                        top: 3,
                                        bottom: 3,
                                    });

                                let frame_resp = frame.show(ui, |ui| {
                                    ui.set_min_height(theme.geometry.tree_row_height - 6.0);
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 5.0;
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
                                                1.4,
                                            );

                                            let (folder_rect, _) = ui.allocate_exact_size(
                                                egui::vec2(14.0, 16.0),
                                                egui::Sense::hover(),
                                            );
                                            let folder_icon = if row.is_collapsed {
                                                icons::Icon::Folder
                                            } else {
                                                icons::Icon::FolderOpen
                                            };
                                            icons::render_icon(
                                                ui.painter(),
                                                folder_icon,
                                                folder_rect,
                                                color(theme.accent).gamma_multiply(0.85),
                                                1.4,
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
                                                if is_active {
                                                    color(theme.accent)
                                                } else {
                                                    color(theme.text_muted)
                                                },
                                                1.4,
                                            );
                                        }

                                        let mut rich = egui::RichText::new(&row.title)
                                            .color(text_color)
                                            .size(theme.geometry.ui_font_size);
                                        if is_active || row.is_folder {
                                            rich = rich.strong();
                                        }
                                        ui.add(egui::Label::new(rich).truncate());

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

                                let interact_resp =
                                    frame_resp.response.interact(egui::Sense::click());
                                if interact_resp.clicked() {
                                    select_row = Some(row.id);
                                    if row.is_folder {
                                        toggle_folder = Some(row.id);
                                    } else {
                                        open = row.data.clone();
                                    }
                                }
                                interact_resp.context_menu(|ui| {
                                    select_row = Some(row.id);
                                    if row.is_folder {
                                        let folder_path =
                                            row.data.clone().unwrap_or_else(|| row.title.clone());
                                        if ui.button("Создать заметку").clicked() {
                                            new_note_in = Some(folder_path.clone());
                                            ui.close();
                                        }
                                        if ui.button("Создать папку").clicked() {
                                            new_folder_in = Some(folder_path.clone());
                                            ui.close();
                                        }
                                        if ui.button("Переименовать").clicked() {
                                            rename_item = Some(folder_path.clone());
                                            ui.close();
                                        }
                                        if ui.button("В корзину").clicked() {
                                            trash_item = Some(folder_path);
                                            ui.close();
                                        }
                                    } else if let Some(path) = &row.data {
                                        if ui.button("Переименовать").clicked() {
                                            rename_item = Some(path.clone());
                                            ui.close();
                                        }
                                        if ui.button("В корзину").clicked() {
                                            trash_item = Some(path.clone());
                                            ui.close();
                                        }
                                    }
                                });

                                let rect = interact_resp.rect;
                                let row_step = ui.spacing().item_spacing.y;
                                // Distinct, continuous Obsidian-style vertical indentation guide lines
                                for d in 1..=row.depth {
                                    let guide_x = rect.left() + 14.0 + ((d - 1) as f32) * 16.0;
                                    let guide_color = if theme.is_dark() {
                                        color(theme.text_muted).gamma_multiply(0.70)
                                    } else {
                                        color(theme.text_muted).gamma_multiply(0.60)
                                    };
                                    ui.painter().line_segment(
                                        [
                                            egui::pos2(guide_x, rect.top()),
                                            egui::pos2(guide_x, rect.bottom() + row_step),
                                        ],
                                        egui::Stroke::new(1.0, guide_color),
                                    );
                                }
                            }
                        }
                    },
                );
                if let Some(id) = select_row {
                    application.shell.sidebar_model_mut().select_id(id);
                }
                if let Some(folder) = new_note_in {
                    dialogs.file.open = true;
                    dialogs.file.path = format!("{folder}/");
                    dialogs.file.error = None;
                    dialogs.file.needs_cursor_end = true;
                }
                if let Some(folder) = new_folder_in {
                    dialogs.folder.open = true;
                    dialogs.folder.path = format!("{folder}/");
                    dialogs.folder.error = None;
                    dialogs.folder.needs_cursor_end = true;
                }
                if let Some(path) = rename_item {
                    dialogs.operation.rename(path);
                }
                if let Some(path) = trash_item {
                    dialogs.operation.trash(path);
                }
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
