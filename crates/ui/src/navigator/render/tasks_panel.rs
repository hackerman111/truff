use crate::{
    components::{self, color},
    navigator::{
        HubTaskCategory, HubTaskEdit, HubTaskOp, HubTaskStatus, NavigatorAction, NavigatorState,
    },
    theme::Theme,
};

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut NavigatorState,
    theme: &Theme,
    height: f32,
    action: &mut NavigatorAction,
) {
    let spacing = theme.spacing_sm;
    let box_height = ((height - spacing * 2.0) / 3.0).max(60.0);
    let width = ui.available_width().max(1.0);

    for (idx, category) in HubTaskCategory::ALL.into_iter().enumerate() {
        if idx > 0 {
            ui.add_space(spacing);
        }
        show_category_box(ui, state, theme, category, width, box_height, action);
    }
}

fn show_category_box(
    ui: &mut egui::Ui,
    state: &mut NavigatorState,
    theme: &Theme,
    category: HubTaskCategory,
    width: f32,
    box_height: f32,
    action: &mut NavigatorAction,
) {
    let cat_idx = category.index();
    let is_active_cat = state.hub_tasks.focus_tasks && state.hub_tasks.active_category == cat_idx;
    let border_color = if is_active_cat {
        color(theme.accent_primary)
    } else {
        color(theme.border_subtle)
    };
    let border_width = if is_active_cat { 1.5 } else { 1.0 };

    egui::Frame::new()
        .fill(color(theme.bg_secondary))
        .corner_radius(egui::CornerRadius::same(6))
        .stroke(egui::Stroke::new(border_width, border_color))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            let inner_width = (width - 18.0).max(1.0);
            let inner_height = (box_height - 18.0).max(24.0);
            ui.set_width(inner_width);
            ui.set_max_width(inner_width);
            ui.set_height(inner_height);

            let total = state.hub_tasks.lists[cat_idx].len();
            let done = state.hub_tasks.lists[cat_idx]
                .iter()
                .filter(|t| t.status == HubTaskStatus::Done)
                .count();

            ui.horizontal(|ui| {
                let header_resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{} · {}", cat_idx + 1, category.title()))
                            .strong()
                            .size(theme.geometry.ui_font_size - 0.5)
                            .color(if is_active_cat {
                                color(theme.accent_primary)
                            } else {
                                color(theme.text_primary)
                            }),
                    )
                    .sense(egui::Sense::click()),
                );
                if header_resp.clicked() {
                    state.search_mode = false;
                    state.hub_tasks.focus_tasks = true;
                    state.hub_tasks.set_category(category);
                }
                ui.label(
                    egui::RichText::new(category.subtitle())
                        .size(theme.geometry.ui_font_size - 3.0)
                        .color(color(theme.text_muted)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("+")
                        .on_hover_text("Добавить задачу (a)")
                        .clicked()
                    {
                        state.search_mode = false;
                        state.hub_tasks.focus_tasks = true;
                        state.hub_tasks.set_category(category);
                        state.hub_tasks.editing = Some(HubTaskEdit::Add {
                            category,
                            input: String::new(),
                        });
                    }
                    ui.label(
                        egui::RichText::new(format!("{done}/{total}"))
                            .monospace()
                            .size(theme.geometry.ui_font_size - 2.0)
                            .color(color(theme.text_muted)),
                    );
                });
            });

            ui.separator();

            let list_height = (ui.available_height()).max(16.0);
            egui::ScrollArea::vertical()
                .id_salt(("hub_tasks_scroll", cat_idx))
                .max_height(list_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(inner_width);
                    let row_height = (theme.geometry.list_row_height - 4.0).max(22.0);
                    let selected_idx = state.hub_tasks.selected[cat_idx];
                    let tasks_snapshot = state.hub_tasks.lists[cat_idx].clone();

                    if tasks_snapshot.is_empty()
                        && !matches!(
                            &state.hub_tasks.editing,
                            Some(HubTaskEdit::Add { category: c, .. }) if *c == category
                        )
                    {
                        let empty_resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new("Нет задач · нажмите a для добавления")
                                    .size(theme.geometry.ui_font_size - 2.0)
                                    .color(color(theme.text_muted)),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if empty_resp.clicked() {
                            state.search_mode = false;
                            state.hub_tasks.focus_tasks = true;
                            state.hub_tasks.set_category(category);
                        }
                    }

                    for (task_idx, task) in tasks_snapshot.iter().enumerate() {
                        let is_selected = is_active_cat && task_idx == selected_idx;
                        if let Some(HubTaskEdit::Edit {
                            category: edit_cat,
                            task_index: edit_idx,
                            input,
                            ..
                        }) = &state.hub_tasks.editing
                            && *edit_cat == category
                            && *edit_idx == task_idx
                        {
                            components::list_row(ui, row_height, true, theme, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("✎").color(color(theme.accent_primary)),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{input}█"))
                                            .monospace()
                                            .color(color(theme.text_primary)),
                                    );
                                });
                            });
                            continue;
                        }

                        let mut toggle_clicked = false;
                        let mut delete_clicked = false;
                        let mut select_clicked = false;

                        let (rect, hover_resp) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), row_height),
                            egui::Sense::hover(),
                        );
                        if is_selected || hover_resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                theme.geometry.corner_radius,
                                color(if is_selected {
                                    theme.bg_selected
                                } else {
                                    theme.bg_tertiary
                                }),
                            );
                        }
                        if is_selected {
                            ui.painter().line_segment(
                                [
                                    rect.left_top() + egui::vec2(0.0, 4.0),
                                    rect.left_bottom() - egui::vec2(0.0, 4.0),
                                ],
                                egui::Stroke::new(2.0, color(theme.accent_primary)),
                            );
                        }

                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(
                            rect.shrink2(egui::vec2(theme.spacing_sm, theme.spacing_xs)),
                        ));
                        child.spacing_mut().interact_size.y = row_height - theme.spacing_xs * 2.0;
                        child.spacing_mut().item_spacing.y = 0.0;
                        child.horizontal(|ui| {
                            let badge_color = match task.status {
                                HubTaskStatus::Todo => color(theme.callout_important),
                                HubTaskStatus::InProgress => color(theme.callout_warning),
                                HubTaskStatus::Done => color(theme.callout_tip),
                            };
                            if ui
                                .add(
                                    egui::Label::new(
                                        egui::RichText::new(task.status.badge())
                                            .monospace()
                                            .color(badge_color),
                                    )
                                    .sense(egui::Sense::click()),
                                )
                                .on_hover_text("Сменить статус (x / Пробел)")
                                .clicked()
                            {
                                toggle_clicked = true;
                            }
                            let mut rich = egui::RichText::new(&task.text)
                                .size(theme.geometry.ui_font_size - 1.0)
                                .color(match task.status {
                                    HubTaskStatus::Todo => color(theme.callout_important),
                                    HubTaskStatus::InProgress => color(theme.callout_warning),
                                    HubTaskStatus::Done => color(theme.text_muted),
                                });
                            if task.status == HubTaskStatus::Done {
                                rich = rich.strikethrough();
                            }
                            let text_max_w = (ui.available_width() - 22.0).max(20.0);
                            if ui
                                .add_sized(
                                    [text_max_w, row_height - theme.spacing_xs * 2.0],
                                    egui::Label::new(rich)
                                        .truncate()
                                        .sense(egui::Sense::click()),
                                )
                                .clicked()
                            {
                                select_clicked = true;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Удалить задачу (d / Del)")
                                        .clicked()
                                    {
                                        delete_clicked = true;
                                    }
                                },
                            );
                        });

                        if delete_clicked {
                            state.search_mode = false;
                            state.hub_tasks.focus_tasks = true;
                            state.hub_tasks.set_category(category);
                            state.hub_tasks.selected[cat_idx] = task_idx;
                            let op = HubTaskOp::Delete {
                                category,
                                line_index: task.line_index,
                            };
                            state.hub_tasks.apply_op_in_memory(&op);
                            *action = NavigatorAction::HubTask(op);
                        } else if toggle_clicked {
                            state.search_mode = false;
                            state.hub_tasks.focus_tasks = true;
                            state.hub_tasks.set_category(category);
                            state.hub_tasks.selected[cat_idx] = task_idx;
                            let op = HubTaskOp::SetStatus {
                                category,
                                line_index: task.line_index,
                                status: task.status.cycle(),
                            };
                            state.hub_tasks.apply_op_in_memory(&op);
                            *action = NavigatorAction::HubTask(op);
                        } else if select_clicked {
                            state.search_mode = false;
                            state.hub_tasks.focus_tasks = true;
                            state.hub_tasks.set_category(category);
                            state.hub_tasks.selected[cat_idx] = task_idx;
                        }
                    }

                    if let Some(HubTaskEdit::Add {
                        category: edit_cat,
                        input,
                    }) = &state.hub_tasks.editing
                        && *edit_cat == category
                    {
                        components::list_row(ui, row_height, true, theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("+ ☐")
                                        .monospace()
                                        .color(color(theme.accent_primary)),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{input}█"))
                                        .monospace()
                                        .color(color(theme.text_primary)),
                                );
                            });
                        });
                    }
                });
        });
}
