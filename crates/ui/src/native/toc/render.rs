use super::model::{TocItemKind, TocModel, TocScope};
use crate::native::editor::color;
use crate::native::{icons, widgets};
use crate::theme::Theme;
use eframe::egui;

/// Action resulting from TOC interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TocAction {
    JumpHeading {
        line: usize,
        path: Option<String>,
    },
    OpenFile {
        path: String,
    },
    OpenAdjacent {
        line: Option<usize>,
        path: Option<String>,
    },
    TogglePin,
    Close,
    SwitchScope(TocScope),
}

/// Renders the Table of Contents within a dock panel (e.g. right sidebar).
pub fn render_toc_dock(
    model: &mut TocModel,
    ui: &mut egui::Ui,
    theme: &Theme,
) -> Option<TocAction> {
    let mut action = None;

    ui.vertical(|ui| {
        // Scope tabs & Pin toggle
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for scope in TocScope::ALL {
                let is_active = model.scope == scope;
                let text = egui::RichText::new(scope.label())
                    .size(11.0)
                    .color(if is_active {
                        color(theme.accent)
                    } else {
                        color(theme.text_muted)
                    });
                if ui.selectable_label(is_active, text).clicked() {
                    action = Some(TocAction::SwitchScope(scope));
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::icon_button(ui, icons::Icon::Pin, "Открепить в плавающее окно")
                    .clicked()
                {
                    action = Some(TocAction::TogglePin);
                }
            });
        });

        ui.add_space(4.0);

        // Sticky breadcrumbs chain
        render_breadcrumbs(model, ui, theme, &mut action);

        ui.add_space(4.0);

        // Search bar
        render_search(model, ui, theme);

        ui.add_space(4.0);

        // Virtualized rows list
        let row_act = render_items_list(model, ui, theme, "dock_toc_scroll");
        if row_act.is_some() {
            action = row_act;
        }
    });

    action
}

/// Renders the Table of Contents as a floating overlay window over the editor.
pub fn render_toc_overlay(
    model: &mut TocModel,
    ctx: &egui::Context,
    theme: &Theme,
) -> Option<TocAction> {
    let mut action = None;
    let screen = ctx.content_rect();

    // Responsive geometry: width 340-460 dp, height 480-720 dp
    let width = (screen.width() * 0.38).clamp(320.0, 480.0);
    let height = (screen.height() * 0.70).clamp(380.0, 720.0);

    let default_pos = egui::pos2(screen.right() - width - 24.0, screen.top() + 48.0);

    let frame = egui::Frame::new()
        .fill(color(theme.bg_secondary))
        .stroke(egui::Stroke::new(1.0, color(theme.border)))
        .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
        .shadow(egui::Shadow {
            offset: [0, 6],
            blur: 16,
            spread: 0,
            color: egui::Color32::from_black_alpha(80),
        })
        .inner_margin(egui::Margin::symmetric(14, 10));

    egui::Window::new("floating_toc_window")
        .title_bar(false)
        .frame(frame)
        .default_pos(default_pos)
        .fixed_size(egui::vec2(width, height))
        .resizable(true)
        .show(ctx, |ui| {
            ui.set_min_size(egui::vec2(width - 28.0, height - 20.0));

            // Top Header: Title, Scope pills, Pin, Close
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new("TOC")
                        .size(12.0)
                        .strong()
                        .color(color(theme.accent)),
                );

                ui.add_space(4.0);

                for scope in TocScope::ALL {
                    let is_active = model.scope == scope;
                    let text = egui::RichText::new(scope.label())
                        .size(11.0)
                        .color(if is_active {
                            color(theme.accent)
                        } else {
                            color(theme.text_muted)
                        });
                    if ui.selectable_label(is_active, text).clicked() {
                        action = Some(TocAction::SwitchScope(scope));
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, icons::Icon::Close, "Закрыть (Esc)").clicked() {
                        action = Some(TocAction::Close);
                    }
                    if widgets::icon_button(
                        ui,
                        icons::Icon::Pin,
                        "Закрепить в боковую панель",
                    )
                    .clicked()
                    {
                        action = Some(TocAction::TogglePin);
                    }
                });
            });

            ui.add_space(6.0);

            // Sticky breadcrumbs bar
            render_breadcrumbs(model, ui, theme, &mut action);

            ui.add_space(4.0);

            // Search bar
            render_search(model, ui, theme);

            ui.add_space(4.0);

            // Virtualized tree list
            let row_act = render_items_list(model, ui, theme, "overlay_toc_scroll");
            if row_act.is_some() {
                action = row_act;
            }

            ui.add_space(4.0);

            // Key hints footer
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("j/k: выбор  h/l: свернуть  Enter: перейти  o: рядом  /: поиск  Esc: закрыть")
                        .size(10.0)
                        .color(color(theme.text_muted)),
                );
            });
        });

    action
}

fn render_breadcrumbs(
    model: &TocModel,
    ui: &mut egui::Ui,
    theme: &Theme,
    action: &mut Option<TocAction>,
) {
    if model.sticky_breadcrumbs.is_empty() {
        return;
    }

    let frame = egui::Frame::new()
        .fill(color(theme.bg_canvas))
        .stroke(egui::Stroke::new(1.0, color(theme.border_subtle)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 4));

    frame.show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let total = model.sticky_breadcrumbs.len();
            for (idx, crumb) in model.sticky_breadcrumbs.iter().enumerate() {
                let is_last = idx + 1 == total;
                let text = egui::RichText::new(crumb).size(11.0).color(if is_last {
                    color(theme.accent)
                } else {
                    color(theme.text_secondary)
                });

                if ui.selectable_label(false, text).clicked() {
                    // Find item with this title to jump
                    if let Some(target) = model.all_items.iter().find(|it| it.title == *crumb)
                        && let Some(line) = target.line
                    {
                        *action = Some(TocAction::JumpHeading {
                            line,
                            path: target.path.clone(),
                        });
                    }
                }

                if !is_last {
                    ui.label(
                        egui::RichText::new(">")
                            .size(10.0)
                            .color(color(theme.text_muted)),
                    );
                }
            }
        });
    });
}

fn render_search(model: &mut TocModel, ui: &mut egui::Ui, theme: &Theme) {
    ui.horizontal(|ui| {
        let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
        icons::render_icon(
            ui.painter(),
            icons::Icon::Search,
            icon_rect,
            color(theme.text_muted),
            1.2,
        );

        let resp = ui.add(
            egui::TextEdit::singleline(&mut model.search_query)
                .hint_text("Фильтр разделов... (/ для ввода)")
                .desired_width(ui.available_width() - 20.0),
        );
        if resp.changed() {
            model.refresh_visible();
        }
        if !model.search_query.is_empty()
            && widgets::icon_button(ui, icons::Icon::Close, "Очистить").clicked()
        {
            model.search_query.clear();
            model.refresh_visible();
        }
    });
}

fn render_items_list(
    model: &mut TocModel,
    ui: &mut egui::Ui,
    theme: &Theme,
    salt: &'static str,
) -> Option<TocAction> {
    let mut action = None;
    let total_count = model.visible_indices.len();
    let selected = model.selected_index;
    let row_height = 24.0;

    let mut clicked_idx = None;
    let mut toggle_collapse_idx = None;

    crate::rows::selected_rows(ui, salt, row_height, total_count, selected, |ui, rows| {
        ui.spacing_mut().interact_size.y = row_height - 2.0;

        for index in rows {
            let Some(&all_idx) = model.visible_indices.get(index) else {
                continue;
            };
            let Some(item) = model.all_items.get(all_idx) else {
                continue;
            };

            let is_selected = index == model.selected_index;
            let is_active = item.is_active;

            let bg_color = if is_selected {
                color(theme.selection)
            } else if is_active {
                color(theme.bg_secondary)
            } else {
                egui::Color32::TRANSPARENT
            };

            let frame = egui::Frame::new()
                .fill(bg_color)
                .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
                .inner_margin(egui::Margin::symmetric(4, 2));

            let frame_resp = frame.show(ui, |ui| {
                ui.set_min_height(row_height - 2.0);
                ui.set_min_width(ui.available_width());

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Level indentation
                    let indent = (item.level as f32) * 14.0;
                    ui.add_space(indent);

                    // Chevron for collapsible items
                    if item.has_children {
                        let chevron = if item.is_collapsed {
                            icons::Icon::ChevronRight
                        } else {
                            icons::Icon::ChevronDown
                        };
                        let (chev_rect, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 14.0), egui::Sense::hover());
                        icons::render_icon(
                            ui.painter(),
                            chevron,
                            chev_rect,
                            color(theme.text_muted),
                            1.2,
                        );
                    } else {
                        ui.add_space(12.0);
                    }

                    // Icon per item kind
                    let (icon_rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    match &item.kind {
                        TocItemKind::Folder { .. } => {
                            icons::render_icon(
                                ui.painter(),
                                icons::Icon::Folder,
                                icon_rect,
                                color(theme.accent),
                                1.2,
                            );
                        }
                        TocItemKind::Note { .. } => {
                            icons::render_icon(
                                ui.painter(),
                                icons::Icon::File,
                                icon_rect,
                                color(theme.text_secondary),
                                1.2,
                            );
                        }
                        TocItemKind::Heading { level, .. } => {
                            let label_h = format!("H{level}");
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                label_h,
                                egui::FontId::monospace(10.0),
                                if is_active {
                                    color(theme.accent)
                                } else {
                                    color(theme.text_muted)
                                },
                            );
                        }
                        TocItemKind::Block { .. } => {
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "^",
                                egui::FontId::monospace(11.0),
                                color(theme.text_muted),
                            );
                        }
                        TocItemKind::Related { .. } => {
                            icons::render_icon(
                                ui.painter(),
                                icons::Icon::File,
                                icon_rect,
                                color(theme.text_muted),
                                1.2,
                            );
                        }
                    }

                    // Text label
                    let text_color = if is_active {
                        color(theme.accent)
                    } else if is_selected {
                        color(theme.text_primary)
                    } else {
                        color(theme.text_secondary)
                    };

                    let rich = egui::RichText::new(&item.title)
                        .size(theme.geometry.ui_font_size)
                        .color(text_color);

                    ui.add(egui::Label::new(rich).truncate());

                    // Line number indicator on the right
                    if let Some(line) = item.line {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new(format!("{line}"))
                                    .size(10.0)
                                    .color(color(theme.text_muted)),
                            );
                        });
                    }
                });
            });

            let interact = frame_resp.response.interact(egui::Sense::click());
            if interact.clicked() {
                clicked_idx = Some(index);
            }
            if interact.double_clicked() && item.has_children {
                toggle_collapse_idx = Some(index);
            }
        }
    });

    if let Some(idx) = clicked_idx {
        model.selected_index = idx;
        if let Some(&all_idx) = model.visible_indices.get(idx)
            && let Some(item) = model.all_items.get(all_idx)
        {
            match &item.kind {
                TocItemKind::Heading { line, .. } | TocItemKind::Block { line, .. } => {
                    action = Some(TocAction::JumpHeading {
                        line: *line,
                        path: item.path.clone(),
                    });
                }
                TocItemKind::Note { path } | TocItemKind::Related { path } => {
                    action = Some(TocAction::OpenFile { path: path.clone() });
                }
                TocItemKind::Folder { .. } => {
                    model.toggle_collapse();
                }
            }
        }
    }

    if let Some(idx) = toggle_collapse_idx {
        model.selected_index = idx;
        model.toggle_collapse();
    }

    action
}
