use super::model::{NoteStreamModel, StreamFilter};
use crate::native::editor::color;
use crate::native::{icons, widgets};
use crate::theme::Theme;
use eframe::egui;

/// Action resulting from interacting with the Note Stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamAction {
    OpenNote { path: String },
    CreateNewNote,
    Close,
    SwitchFilter(StreamFilter),
}

/// Renders the Note Stream timeline as a large, spacious modal panel.
pub fn render_note_stream(
    model: &mut NoteStreamModel,
    ctx: &egui::Context,
    theme: &Theme,
) -> Option<StreamAction> {
    let mut action = None;
    let screen = ctx.content_rect();

    // Spacious responsive sizing: 84vw × 82vh
    let width = (screen.width() * 0.84).clamp(620.0, 1200.0);
    let height = (screen.height() * 0.82).clamp(460.0, 880.0);

    let frame = egui::Frame::new()
        .fill(color(theme.bg_panel))
        .stroke(egui::Stroke::new(1.0, color(theme.border)))
        .corner_radius(egui::CornerRadius::same(theme.geometry.corner_radius as u8))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: egui::Color32::from_black_alpha(100),
        })
        .inner_margin(egui::Margin::symmetric(20, 16));

    egui::Window::new("note_stream_window")
        .title_bar(false)
        .frame(frame)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .fixed_size(egui::vec2(width, height))
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_size(egui::vec2(width - 40.0, height - 32.0));

            // Top Header: Title, Note count, New note button, Close
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;

                let (stream_rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                icons::render_icon(ui.painter(), icons::Icon::MoreHorizontal, stream_rect, color(theme.accent), 1.4);

                ui.label(
                    egui::RichText::new("ЛЕНТА ЗАМЕТОК")
                        .size(16.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );

                let filtered_count = model.filtered_cards().len();
                ui.label(
                    egui::RichText::new(format!("({filtered_count} записей)"))
                        .size(12.0)
                        .color(color(theme.text_muted)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::icon_button(ui, icons::Icon::Close, "Закрыть (Esc)").clicked() {
                        action = Some(StreamAction::Close);
                    }
                    if widgets::primary_button(ui, "➕ Новая запись (a)").clicked() {
                        action = Some(StreamAction::CreateNewNote);
                    }
                });
            });

            ui.add_space(8.0);

            // Filter Tabs: Сегодня / Незавершенные / Связанные / Перечитать / Все
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for f in StreamFilter::ALL {
                    let is_active = model.filter == f;
                    let text = egui::RichText::new(f.title())
                        .size(12.0)
                        .color(if is_active {
                            color(theme.accent)
                        } else {
                            color(theme.text_muted)
                        });
                    if ui.selectable_label(is_active, text).clicked() {
                        model.filter = f;
                        model.clamp_selection();
                        action = Some(StreamAction::SwitchFilter(f));
                    }
                }
            });

            ui.add_space(6.0);

            // Search filter bar
            ui.horizontal(|ui| {
                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                icons::render_icon(ui.painter(), icons::Icon::Search, icon_rect, color(theme.text_muted), 1.2);

                let resp = ui.add(
                    egui::TextEdit::singleline(&mut model.search_query)
                        .hint_text("Поиск по тексту карточек... (/ для ввода)")
                        .desired_width(ui.available_width() - 24.0),
                );
                if resp.changed() {
                    model.clamp_selection();
                }
                if !model.search_query.is_empty()
                    && widgets::icon_button(ui, icons::Icon::Close, "Очистить").clicked()
                {
                    model.search_query.clear();
                    model.clamp_selection();
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Stream feed: Virtualized cards list
            let cards = model.filtered_cards();
            let total_count = cards.len();

            if total_count == 0 {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new("Нет заметок, соответствующих выбранному фильтру.")
                            .size(13.0)
                            .color(color(theme.text_muted)),
                    );
                    ui.add_space(8.0);
                    if widgets::primary_button(ui, "➕ Создать новую запись в ленту").clicked() {
                        action = Some(StreamAction::CreateNewNote);
                    }
                });
            } else {
                let card_height = 84.0;
                let mut clicked_idx = None;

                crate::rows::selected_rows(
                    ui,
                    "note_stream_feed_scroll",
                    card_height,
                    total_count,
                    model.selected_index,
                    |ui, rows| {
                        ui.spacing_mut().item_spacing.y = 6.0;

                        for index in rows {
                            let Some(card) = cards.get(index) else { continue };
                            let is_selected = index == model.selected_index;

                            let bg_color = if is_selected {
                                color(theme.bg_secondary)
                            } else {
                                color(theme.bg_canvas)
                            };

                            let border_color = if is_selected {
                                color(theme.accent)
                            } else {
                                color(theme.border_subtle)
                            };

                            let card_frame = egui::Frame::new()
                                .fill(bg_color)
                                .stroke(egui::Stroke::new(1.0, border_color))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(14, 8));

                            let frame_resp = card_frame.show(ui, |ui| {
                                ui.set_min_height(card_height - 10.0);
                                ui.set_min_width(ui.available_width());

                                ui.vertical(|ui| {
                                    // Top row: Title, Reason Badge, Date, Tasks pill
                                    ui.horizontal(|ui| {
                                        let rich_title = egui::RichText::new(&card.title)
                                            .size(14.0)
                                            .strong()
                                            .color(if is_selected {
                                                color(theme.accent)
                                            } else {
                                                color(theme.text_primary)
                                            });
                                        ui.add(egui::Label::new(rich_title).truncate());

                                        ui.add_space(4.0);

                                        // Badge explaining why shown
                                        let badge_frame = egui::Frame::new()
                                            .fill(color(theme.selection))
                                            .corner_radius(egui::CornerRadius::same(4))
                                            .inner_margin(egui::Margin::symmetric(6, 2));

                                        badge_frame.show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(&card.reason_badge)
                                                    .size(10.0)
                                                    .color(color(theme.accent)),
                                            );
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(
                                                egui::RichText::new(&card.date_str)
                                                    .size(10.0)
                                                    .color(color(theme.text_muted)),
                                            );

                                            if card.task_count > 0 {
                                                let t_text = format!("{} / {} задач", card.task_count - card.pending_tasks, card.task_count);
                                                ui.label(
                                                    egui::RichText::new(t_text)
                                                        .size(10.0)
                                                        .color(color(theme.text_secondary)),
                                                );
                                            }
                                        });
                                    });

                                    ui.add_space(3.0);

                                    // Preview snippet (up to 2 lines)
                                    if card.preview_snippet.is_empty() {
                                        ui.label(
                                            egui::RichText::new("(Пустая заметка)")
                                                .size(11.0)
                                                .italics()
                                                .color(color(theme.text_muted)),
                                        );
                                    } else {
                                        for line in card.preview_snippet.iter().take(2) {
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .size(11.0)
                                                    .color(color(theme.text_secondary)),
                                            );
                                        }
                                    }
                                });
                            });

                            let interact = frame_resp.response.interact(egui::Sense::click());
                            if interact.clicked() {
                                clicked_idx = Some((index, card.path.clone()));
                            }
                        }
                    },
                );

                if let Some((idx, path)) = clicked_idx {
                    model.selected_index = idx;
                    action = Some(StreamAction::OpenNote { path });
                }
            }

            ui.add_space(8.0);

            // Footer key hints
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("j/k: выбор  Enter: открыть заметку  Tab: сменить фильтр  a: новая запись  /: поиск  Esc: закрыть")
                        .size(11.0)
                        .color(color(theme.text_muted)),
                );
            });
        });

    action
}
