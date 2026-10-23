use super::editor::color;
use super::graph::GraphWidget;
use crate::app::Application;
use crate::app::global_config::GlobalConfig;
use crate::native::toc::{FloatingToc, TocAction};
use crate::shell::Focus;
use crate::theme::{Theme, next_palette_color_name, resolve_tag_rgba};
use egui::{Color32, CornerRadius, Margin, RichText};

pub(super) fn show(
    application: &mut Application,
    toc: &mut FloatingToc,
    graph: &mut GraphWidget,
    ui: &mut egui::Ui,
    theme: &Theme,
) -> Option<TocAction> {
    let has_backlinks = !application.backlinks.is_empty();
    let has_pinned_toc = toc.is_pinned;
    let vault_open = application.vault_path().is_some();

    let tab_id = egui::Id::new("right_dock_tab");
    if let Some(req_tab) = application.right_dock_tab_requested.take() {
        ui.data_mut(|d| d.insert_temp(tab_id, req_tab));
        application.right_dock_visible = true;
    }

    if !has_backlinks && !has_pinned_toc && !application.right_dock_visible && !vault_open {
        return None;
    }

    let mut toc_action = None;

    egui::Panel::right("references")
        .frame(crate::components::dock_frame(theme))
        .default_size(theme.geometry.right_default_width.max(260.0))
        .min_size(190.0_f32.min(ui.available_width() * 0.45))
        .max_size((ui.available_width() * 0.48).min(520.0))
        .resizable(true)
        .show_separator_line(true)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            let default_tab = if has_backlinks {
                0
            } else if has_pinned_toc {
                1
            } else {
                2 // Default to Tags / Graph when neither backlinks nor pinned TOC is active
            };
            let mut active_tab =
                ui.data_mut(|d| d.get_temp::<usize>(tab_id).unwrap_or(default_tab));

            // 4-tab bar: Связи (0) | Оглавление (1) | Теги (2) | Граф (3)
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (idx, label) in [
                    (0, format!("Связи ({})", application.backlinks.len())),
                    (1, "Оглавление".to_string()),
                    (2, "Теги".to_string()),
                    (3, "Граф".to_string()),
                ] {
                    let is_active = active_tab == idx;
                    let txt = RichText::new(label).size(11.0).color(if is_active {
                        color(theme.accent)
                    } else {
                        color(theme.text_muted)
                    });
                    if ui.selectable_label(is_active, txt).clicked() {
                        active_tab = idx;
                        ui.data_mut(|d| d.insert_temp(tab_id, idx));
                        if idx == 1 && !toc.is_pinned {
                            toc.set_pinned(true);
                        }
                    }
                }
            });
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            match active_tab {
                1 => {
                    toc_action = toc.show_dock(ui, theme);
                }
                2 => {
                    show_tags_panel(application, graph, ui, theme, tab_id);
                }
                3 => {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("ИНТЕГРИРОВАННЫЙ ГРАФ")
                                .size(10.5)
                                .strong()
                                .color(color(theme.text_muted)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("⤢ На весь экран")
                                .on_hover_text(
                                    "Открыть граф в центральной рабочей области (:graph)",
                                )
                                .clicked()
                            {
                                application.graph_view_open = !application.graph_view_open;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    graph.show(ui, application, theme, true);
                }
                _ => {
                    ui.label(
                        RichText::new("ОБРАТНЫЕ ССЫЛКИ (BACKLINKS)")
                            .size(10.5)
                            .strong()
                            .color(color(theme.text_muted)),
                    );
                    ui.add_space(4.0);
                    if application.backlinks.is_empty() {
                        ui.label(
                            RichText::new("На эту заметку пока нет входящих ссылок [[...]].")
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                    } else {
                        let mut open = None;
                        let model = application.shell.right_model();
                        crate::rows::selected_rows(
                            ui,
                            "backlinks_scroll",
                            theme.geometry.list_row_height,
                            model.visible_count(),
                            model.selected_index(),
                            |ui, rows| {
                                for index in rows {
                                    let Some(row) = model.get_visible_row(index) else {
                                        continue;
                                    };
                                    let is_selected = index == model.selected_index();
                                    let rich = RichText::new(&row.title)
                                        .size(theme.geometry.ui_font_size)
                                        .color(if is_selected {
                                            color(theme.text_primary)
                                        } else {
                                            color(theme.text_muted)
                                        });
                                    if ui
                                        .add_sized(
                                            [ui.available_width(), theme.geometry.list_row_height],
                                            egui::Button::selectable(is_selected, rich).truncate(),
                                        )
                                        .clicked()
                                    {
                                        open = Some(row.id);
                                    }
                                }
                            },
                        );
                        if let Some(index) = open {
                            application.shell.right_model_mut().select_id(index);
                            application.shell.set_focus(Focus::RightPanel);
                            if let Err(error) = application.open_backlink() {
                                application.status = error.to_string();
                            }
                        }
                    }
                }
            }
        });

    toc_action
}

fn show_tags_panel(
    application: &mut Application,
    graph: &mut GraphWidget,
    ui: &mut egui::Ui,
    theme: &Theme,
    tab_id: egui::Id,
) {
    let tags_map = application.collect_vault_tags();
    let selected_tag_id = egui::Id::new("right_dock_selected_tag");
    let mut selected_tag = ui.data_mut(|d| d.get_temp::<Option<String>>(selected_tag_id).flatten());

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("ТЕГИ ХРАНИЛИЩА ({})", tags_map.len()))
                .size(10.5)
                .strong()
                .color(color(theme.text_muted)),
        );
        if selected_tag.is_some() && ui.small_button("Сбросить фильтр").clicked() {
            selected_tag = None;
            ui.data_mut(|d| d.insert_temp::<Option<String>>(selected_tag_id, None));
        }
    });
    ui.add_space(4.0);

    if tags_map.is_empty() {
        ui.label(
            RichText::new(
                "Добавьте #теги в любой заметке — здесь появятся цветные метки и фильтрация.",
            )
            .size(11.5)
            .color(color(theme.text_muted)),
        );
        return;
    }

    let mut cycle_color_for_tag: Option<String> = None;
    let mut open_graph_for_tag: Option<String> = None;
    let mut open_note_path: Option<String> = None;

    egui::ScrollArea::vertical()
        .id_salt("right_dock_tags_scroll")
        .show(ui, |ui| {
            for (tag, notes) in &tags_map {
                let [r, g, b, _] = resolve_tag_rgba(tag);
                let is_selected = selected_tag.as_deref() == Some(tag.as_str());

                ui.horizontal(|ui| {
                    // Color swatch button to cycle tag color in GlobalConfig
                    let swatch_btn = egui::Button::new(
                        RichText::new("●")
                            .size(13.0)
                            .color(Color32::from_rgb(r, g, b)),
                    )
                    .frame(false);
                    if ui
                        .add(swatch_btn)
                        .on_hover_text("Нажмите, чтобы сменить цвет этого тега во всем хранилище")
                        .clicked()
                    {
                        cycle_color_for_tag = Some(tag.clone());
                    }

                    // Tag pill button
                    let pill_frame = egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(
                            r,
                            g,
                            b,
                            if is_selected { 85 } else { 42 },
                        ))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(Margin::symmetric(6, 2));
                    let inner = pill_frame.show(ui, |ui| {
                        let resp = ui.add(
                            egui::Label::new(
                                RichText::new(format!("#{tag} ({})", notes.len()))
                                    .size(11.5)
                                    .color(Color32::from_rgb(r, g, b)),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if resp.clicked() {
                            if is_selected {
                                selected_tag = None;
                            } else {
                                selected_tag = Some(tag.clone());
                            }
                            ui.data_mut(|d| {
                                d.insert_temp(selected_tag_id, selected_tag.clone());
                            });
                        }
                    });
                    let _ = inner;

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("Граф")
                            .on_hover_text("Показать граф по этому тегу")
                            .clicked()
                        {
                            open_graph_for_tag = Some(tag.clone());
                        }
                    });
                });

                if is_selected {
                    ui.indent(("tag_notes_indent", tag), |ui| {
                        for path in notes {
                            if ui
                                .selectable_label(
                                    false,
                                    RichText::new(path)
                                        .size(11.5)
                                        .color(color(theme.text_primary)),
                                )
                                .clicked()
                            {
                                open_note_path = Some(path.clone());
                            }
                        }
                    });
                }
                ui.add_space(2.0);
            }
        });

    if let Some(tag_name) = cycle_color_for_tag {
        let mut config = GlobalConfig::load();
        let current = config.tag_colors.get(&tag_name).map(String::as_str);
        let next_col = next_palette_color_name(current);
        config
            .tag_colors
            .insert(tag_name.clone(), next_col.to_string());
        let _ = config.save();
        application.status = format!("Цвет тега #{tag_name} изменен на {next_col}");
    }

    if let Some(tag_name) = open_graph_for_tag {
        graph.config.tag = Some(tag_name);
        graph.rebuild(application);
        ui.data_mut(|d| d.insert_temp(tab_id, 3usize));
    }

    if let Some(path) = open_note_path
        && path != "scratch.md"
        && let Err(e) = application.open_note(&path)
    {
        application.status = e.to_string();
    }
}
