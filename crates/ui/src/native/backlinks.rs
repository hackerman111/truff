use super::editor::color;
use crate::app::Application;
use crate::native::toc::{FloatingToc, TocAction};
use crate::shell::Focus;
use crate::theme::Theme;

pub(super) fn show(
    application: &mut Application,
    toc: &mut FloatingToc,
    ui: &mut egui::Ui,
    theme: &Theme,
) -> Option<TocAction> {
    let has_backlinks = !application.backlinks.is_empty();
    let has_pinned_toc = toc.is_pinned;

    if !has_backlinks && !has_pinned_toc {
        return None;
    }

    let mut toc_action = None;

    egui::Panel::right("references")
        .frame(crate::components::dock_frame(theme))
        .default_size(theme.geometry.right_default_width)
        .min_size(160.0_f32.min(ui.available_width() * 0.4))
        .max_size((ui.available_width() * 0.4).min(480.0))
        .resizable(true)
        .show_separator_line(true)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Right panel tabs: Backlinks (0) vs TOC (1)
            let tab_id = egui::Id::new("right_dock_tab");
            let mut active_tab = ui.data_mut(|d| {
                d.get_temp::<usize>(tab_id)
                    .unwrap_or(if has_pinned_toc && !has_backlinks {
                        1
                    } else {
                        0
                    })
            });

            if has_backlinks && has_pinned_toc {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let bl_text =
                        egui::RichText::new(format!("Связи ({})", application.backlinks.len()))
                            .size(11.0)
                            .color(if active_tab == 0 {
                                color(theme.accent)
                            } else {
                                color(theme.text_muted)
                            });
                    if ui.selectable_label(active_tab == 0, bl_text).clicked() {
                        active_tab = 0;
                        ui.data_mut(|d| d.insert_temp(tab_id, 0));
                    }

                    let toc_text = egui::RichText::new("Оглавление (TOC)").size(11.0).color(
                        if active_tab == 1 {
                            color(theme.accent)
                        } else {
                            color(theme.text_muted)
                        },
                    );
                    if ui.selectable_label(active_tab == 1, toc_text).clicked() {
                        active_tab = 1;
                        ui.data_mut(|d| d.insert_temp(tab_id, 1));
                    }
                });
                ui.add_space(4.0);
            } else if has_pinned_toc {
                active_tab = 1;
            } else {
                active_tab = 0;
            }

            if active_tab == 1 {
                toc_action = toc.show_dock(ui, theme);
            } else {
                ui.label(
                    egui::RichText::new("BACKLINKS")
                        .size(11.0)
                        .strong()
                        .color(color(theme.text_muted)),
                );
                ui.add_space(4.0);
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
                            let rich = egui::RichText::new(&row.title)
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
        });

    toc_action
}
