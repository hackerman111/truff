use super::model::{NavigatorAction, NavigatorState};
use crate::{components, theme::Theme};
use eframe::egui;

mod chrome;
mod list;
mod preview;

pub fn show(state: &mut NavigatorState, context: &egui::Context, theme: &Theme) -> NavigatorAction {
    show_inner(state, context, theme, true)
}

pub(crate) fn show_without_input(
    state: &mut NavigatorState,
    context: &egui::Context,
    theme: &Theme,
) -> NavigatorAction {
    show_inner(state, context, theme, false)
}

fn show_inner(
    state: &mut NavigatorState,
    context: &egui::Context,
    theme: &Theme,
    handle_input: bool,
) -> NavigatorAction {
    if !state.is_open() {
        return NavigatorAction::None;
    }
    let size = theme.geometry.navigator_size(context.content_rect().size());
    let mut action = NavigatorAction::None;
    egui::Modal::new(egui::Id::new("universal_navigator"))
        .frame(components::overlay_frame(theme))
        .show(context, |ui| {
            ui.set_width((size.x - theme.spacing_lg * 2.0).max(1.0));
            ui.set_height((size.y - theme.spacing_lg * 2.0).max(1.0));
            if handle_input && let Some(next) = handle_window_input(state, ui) {
                action = next;
            }
            chrome::header(ui, state, theme, &mut action);
            ui.add_space(theme.spacing_sm);
            if state.search_mode {
                ui.memory_mut(|memory| memory.request_focus(egui::Id::new("overlay_search_input")));
            }
            let response = components::search_input(
                ui,
                &mut state.query,
                state.mode.search_placeholder(),
                theme,
            );
            if state.search_mode {
                response.request_focus();
            } else if response.clicked() {
                state.search_mode = true;
            } else {
                response.surrender_focus();
            }
            if response.changed() {
                state.recompute_filter();
            }
            ui.add_space(theme.spacing_md);
            let height =
                (ui.available_height() - theme.geometry.list_row_height - theme.spacing_lg)
                    .max(1.0);
            let gap = theme.spacing_lg;
            let list_width = (ui.available_width() - gap) * 0.48;
            let preview_width = (ui.available_width() - gap - list_width).max(1.0);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap / 2.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(list_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| list::show(ui, state, theme, height, &mut action),
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(preview_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        if state.actions_menu_open {
                            chrome::actions(ui, state, theme, height, &mut action);
                        } else {
                            preview::show(ui, state, theme, height);
                        }
                    },
                );
            });
            ui.add_space(theme.spacing_sm);
            ui.separator();
            chrome::footer(ui, state, theme, &mut action);
        });
    action
}

fn handle_window_input(state: &mut NavigatorState, ui: &mut egui::Ui) -> Option<NavigatorAction> {
    use super::model::NavKey;
    let events = ui.input(|input| input.events.clone());
    for event in events {
        let (key, ctrl) = match event {
            egui::Event::Text(text) if !state.search_mode => {
                for ch in text.chars() {
                    let action = state.handle_key_event(NavKey::Char(ch), false);
                    if action != NavigatorAction::None {
                        return Some(action);
                    }
                }
                continue;
            }
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                let key = match key {
                    egui::Key::Escape => NavKey::Escape,
                    egui::Key::Enter => NavKey::Enter,
                    egui::Key::ArrowDown => NavKey::Down,
                    egui::Key::ArrowUp => NavKey::Up,
                    egui::Key::ArrowLeft => NavKey::Left,
                    egui::Key::ArrowRight => NavKey::Right,
                    egui::Key::PageDown => NavKey::PageDown,
                    egui::Key::PageUp => NavKey::PageUp,
                    egui::Key::Home => NavKey::Home,
                    egui::Key::End => NavKey::End,
                    egui::Key::Tab if modifiers.shift => NavKey::BackTab,
                    egui::Key::Tab => NavKey::Tab,
                    egui::Key::J if modifiers.ctrl => NavKey::Char('j'),
                    egui::Key::K if modifiers.ctrl => NavKey::Char('k'),
                    _ => continue,
                };
                (key, modifiers.ctrl)
            }
            _ => continue,
        };
        let action = state.handle_key_event(key, ctrl);
        if action != NavigatorAction::None {
            return Some(action);
        }
    }
    None
}
