use crate::{
    components::color,
    navigator::{NavigatorAction, NavigatorState},
    theme::Theme,
};

pub(super) fn header(
    ui: &mut egui::Ui,
    state: &mut NavigatorState,
    theme: &Theme,
    action: &mut NavigatorAction,
) {
    let is_hub = matches!(
        state.mode,
        crate::navigator::NavigatorMode::Outline | crate::navigator::NavigatorMode::Links
    );
    ui.horizontal(|ui| {
        if is_hub {
            if ui
                .selectable_label(
                    state.mode == crate::navigator::NavigatorMode::Outline,
                    "Оглавление",
                )
                .clicked()
            {
                state.hub_tasks.focus_tasks = false;
                *action = NavigatorAction::SwitchProvider {
                    mode: crate::navigator::NavigatorMode::Outline,
                };
            }
            if ui
                .selectable_label(
                    state.mode == crate::navigator::NavigatorMode::Links,
                    "Backlinks",
                )
                .clicked()
            {
                state.hub_tasks.focus_tasks = false;
                *action = NavigatorAction::SwitchProvider {
                    mode: crate::navigator::NavigatorMode::Links,
                };
            }
            ui.label(
                egui::RichText::new("Tab — переключить · h/l — фокус")
                    .size(theme.geometry.ui_font_size - 2.0)
                    .color(color(theme.text_muted)),
            );
        } else {
            egui::ComboBox::from_id_salt("navigator_provider")
                .selected_text(state.mode.title())
                .show_ui(ui, |ui| {
                    for mode in crate::navigator::NavigatorMode::ALL {
                        if ui
                            .selectable_label(mode == state.mode, mode.title())
                            .clicked()
                        {
                            *action = NavigatorAction::SwitchProvider { mode };
                        }
                    }
                });
            ui.label(
                egui::RichText::new("Tab / Shift-Tab")
                    .size(theme.geometry.ui_font_size - 2.0)
                    .color(color(theme.text_primary)),
            );
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(&state.context_title).color(color(theme.text_muted)),
            )
            .truncate(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Закрыть · Esc").clicked() {
                state.close();
                *action = NavigatorAction::Close;
            }
            let mode_badge = if state.search_mode || state.hub_tasks.editing.is_some() {
                "Ввод"
            } else if is_hub && state.hub_tasks.focus_tasks {
                "Задачи"
            } else {
                "Normal"
            };
            ui.label(
                egui::RichText::new(mode_badge)
                    .monospace()
                    .color(color(theme.accent_primary)),
            );
        });
    });
}

pub(super) fn footer(
    ui: &mut egui::Ui,
    state: &mut NavigatorState,
    theme: &Theme,
    action: &mut NavigatorAction,
) {
    let is_hub = matches!(
        state.mode,
        crate::navigator::NavigatorMode::Outline | crate::navigator::NavigatorMode::Links
    );
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().button_padding = egui::vec2(theme.spacing_sm, theme.spacing_xs);
        ui.spacing_mut().interact_size.y = theme.geometry.list_row_height - theme.spacing_sm;
        if is_hub && state.hub_tasks.focus_tasks {
            ui.label(
                egui::RichText::new(
                    "Space/x статус · a добавить · e изменить · d удалить · 1/2/3 категория · j/k выбор · h к списку · Tab связи",
                )
                .size(theme.geometry.ui_font_size - 2.0)
                .color(color(theme.text_muted)),
            );
            return;
        }
        for quick in state.quick_actions() {
            let display_key = if quick.key == "↵" {
                "Enter"
            } else {
                quick.key
            };
            let label = format!("{display_key} {}", quick.label);
            if ui.button(label).on_hover_text(quick.description).clicked() {
                match quick.key {
                    "/" => state.search_mode = true,
                    "?" => state.actions_menu_open = !state.actions_menu_open,
                    _ => {
                        let next = state.quick_action(quick.key);
                        if next != NavigatorAction::None {
                            if next.closes_navigator() {
                                state.close();
                            }
                            *action = next;
                        }
                    }
                }
            }
        }
        if is_hub {
            ui.label(
                egui::RichText::new("Tab Оглавление/Backlinks · l к задачам · j/k навигация")
                    .size(theme.geometry.ui_font_size - 2.0)
                    .color(color(theme.text_muted)),
            );
        }
        if matches!(
            state.mode,
            crate::navigator::NavigatorMode::Content | crate::navigator::NavigatorMode::Mixed
        ) {
            for (label, forward) in [("[ Назад", false), ("] Далее", true)] {
                if ui.button(label).clicked() {
                    *action = NavigatorAction::SearchPage { forward };
                }
            }
        }
    });
}

pub(super) fn actions(
    ui: &mut egui::Ui,
    state: &mut NavigatorState,
    theme: &Theme,
    height: f32,
    action: &mut NavigatorAction,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Действия").strong());
        if ui.button("Назад · Esc").clicked() {
            state.actions_menu_open = false;
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("navigator_actions")
        .max_height((height - theme.geometry.list_row_height).max(1.0))
        .show(ui, |ui| {
            for (index, item) in state.context_actions().into_iter().enumerate() {
                let display_key = if item.key == "↵" {
                    "Enter"
                } else {
                    item.key.as_str()
                };
                let response = ui.add_sized(
                    [ui.available_width(), theme.geometry.list_row_height],
                    egui::Button::selectable(
                        index == state.actions_menu_selected,
                        format!("{display_key}  {}", item.label),
                    ),
                );
                response.clone().on_hover_text(&item.description);
                if index == state.actions_menu_selected || item.action == NavigatorAction::None {
                    ui.label(egui::RichText::new(&item.description).color(color(theme.text_muted)));
                }
                if response.clicked() {
                    state.actions_menu_open = false;
                    *action = item.action;
                }
            }
        });
}
