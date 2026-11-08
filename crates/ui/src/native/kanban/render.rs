use super::picker::{FuzzyFilePicker, InlineFileAutocomplete, LinkJumpPicker};
use super::state::{KanbanBoardState, KanbanInputMode, KanbanIntent};
use crate::native::editor::color;
use crate::native::icons::{Icon, render_icon};
use crate::native::widgets::{
    icon_button, primary_button, secondary_button, secondary_button_with_icon,
};
use crate::theme::Theme;
use egui::{
    Align, Color32, CornerRadius, Frame, Id, Layout, Margin, Rect, RichText, ScrollArea, Sense,
    Stroke, TextEdit, Ui, vec2,
};
use truff_core::markdown::kanban::{
    CardInsertPos, KanbanBoard, KanbanCard, KanbanColumn, KanbanTaskStatus, plan_add_card,
    plan_add_column, plan_attach_file_link, plan_cycle_card_status, plan_edit_card_title,
    plan_rename_column,
};
use truff_core::text::Transaction;

pub const COLUMN_WIDTH: f32 = 290.0;

enum OverlayAction {
    None,
    Close,
    ApplyTx(Transaction),
    OpenLink(String),
}

/// Checks whether a card matches the live filter query.
fn card_matches_query(card: &KanbanCard, query_lower: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }
    if card.clean_title.to_lowercase().contains(query_lower) {
        return true;
    }
    if card
        .tags
        .iter()
        .any(|t| t.to_lowercase().contains(query_lower))
    {
        return true;
    }
    if card
        .links
        .iter()
        .any(|l| l.display().to_lowercase().contains(query_lower))
    {
        return true;
    }
    if card
        .body_lines
        .iter()
        .any(|b| b.to_lowercase().contains(query_lower))
    {
        return true;
    }
    false
}

/// Renders the board surface (header + horizontally scrollable columns + cards + active overlays).
pub fn render_board_surface(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    render_board_surface_internal(state, board, text, vault_files, ui, theme, false)
}

/// Internal board surface renderer supporting modal mode toggle (close button in header).
fn render_board_surface_internal(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    ui: &mut Ui,
    theme: &Theme,
    is_modal: bool,
) -> KanbanIntent {
    ui.push_id(if is_modal { "modal" } else { "central" }, |ui| {
        let mut intent = KanbanIntent::None;

        // 1. Board Header
        let header_intent = render_header(state, board, text, ui, theme, is_modal);
        if header_intent != KanbanIntent::None {
            intent = header_intent;
        }

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);

        // 2. Columns or Welcoming Empty State
        if board.columns.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(36.0);
                let (icon_r, _) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::hover());
                render_icon(
                    ui.painter(),
                    Icon::Kanban,
                    icon_r,
                    color(theme.accent_primary),
                    1.6,
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new("На этой доске пока нет колонок")
                        .size(15.0)
                        .strong()
                        .color(color(theme.text_primary)),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Создайте колонки для организации задач или добавьте стандартный набор (План, В работе, Готово).")
                        .size(12.0)
                        .color(color(theme.text_muted)),
                );
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if primary_button(ui, "✨ Создать стандартные колонки").clicked() {
                        let default_text = truff_core::markdown::kanban::default_kanban_template("Канбан", "\n");
                        let tx = truff_core::text::Transaction::replace(0..text.len(), default_text);
                        intent = KanbanIntent::ApplyTx(tx);
                    }
                    if secondary_button_with_icon(ui, Icon::Plus, "Добавить свою колонку (A)").clicked() {
                        state.mode = KanbanInputMode::ColumnInput {
                            rename_col_idx: None,
                            buffer: String::new(),
                        };
                    }
                });
                ui.add_space(36.0);
            });
        } else {
            let query_lower = state.search_query.trim().to_lowercase();
            ScrollArea::horizontal()
                .id_salt("kanban_board_columns_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;

                        for (col_idx, col) in board.columns.iter().enumerate() {
                            let col_intent = render_column(
                                col_idx,
                                col,
                                state,
                                board,
                                text,
                                vault_files,
                                &query_lower,
                                ui,
                                theme,
                            );
                            if col_intent != KanbanIntent::None {
                                intent = col_intent;
                            }
                        }

                        // Inlined new column input or "+ Колонка" placeholder button
                        let add_col_intent = render_add_column_column(state, board, text, ui, theme);
                        if add_col_intent != KanbanIntent::None {
                            intent = add_col_intent;
                        }
                    });
                });
        }

        // 3. Floating Overlays (FilePicker, LinkJumpPicker)
        let overlay_intent = render_floating_overlays(state, board, text, ui.ctx(), theme);
        if overlay_intent != KanbanIntent::None {
            intent = overlay_intent;
        }

        intent
    })
    .inner
}

/// Renders the modal overlay dialog (Mode A) centered over the screen with semi-transparent backdrop.
#[allow(clippy::too_many_arguments)]
pub fn render_kanban_modal(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    available_boards: &[String],
    current_board_name: Option<&str>,
    ctx: &egui::Context,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;
    let screen = ctx.content_rect();

    let width = (screen.width() * 0.86).clamp(680.0, 1400.0);
    let height = (screen.height() * 0.82).clamp(460.0, 880.0);
    let window_rect = Rect::from_center_size(screen.center(), vec2(width, height));

    // Soft scrim backdrop
    let backdrop_layer = egui::LayerId::new(egui::Order::Middle, Id::new("kanban_modal_backdrop"));
    ctx.layer_painter(backdrop_layer).rect_filled(
        screen,
        0.0,
        if theme.is_dark() {
            Color32::from_black_alpha(45)
        } else {
            Color32::from_black_alpha(20)
        },
    );

    // Clicking outside the floating modal window dismisses it
    if ctx.input(|i| i.pointer.primary_clicked())
        && let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos())
        && !window_rect.contains(click_pos)
    {
        intent = KanbanIntent::CloseOverlay;
    }

    let frame = Frame::new()
        .fill(color(theme.bg_panel))
        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
        .corner_radius(CornerRadius::same(theme.geometry.corner_radius as u8))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(100),
        })
        .inner_margin(Margin::symmetric(18, 14));

    egui::Window::new("kanban_modal_window")
        .title_bar(false)
        .frame(frame)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .fixed_size(vec2(width, height))
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_size(vec2(width - 36.0, height - 28.0));

            // Board switcher tabs if multiple boards exist
            if !available_boards.is_empty() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for board_path in available_boards {
                        let display_name = board_path
                            .rsplit('/')
                            .next()
                            .unwrap_or(board_path)
                            .strip_suffix(".kanban.md")
                            .unwrap_or_else(|| {
                                board_path.strip_suffix(".md").unwrap_or(board_path)
                            });
                        let is_active = current_board_name
                            .is_some_and(|cur| cur == board_path || cur == display_name);
                        let tab_text = RichText::new(display_name).size(11.5).color(if is_active {
                            color(theme.accent_primary)
                        } else {
                            color(theme.text_muted)
                        });
                        if ui.selectable_label(is_active, tab_text).clicked() {
                            intent = KanbanIntent::SelectBoard(board_path.clone());
                        }
                    }
                    if icon_button(ui, Icon::Plus, "Создать доску").clicked() {
                        intent = KanbanIntent::CreateNewBoard;
                    }
                });
                ui.add_space(4.0);
            }

            // Board Surface
            let board_intent =
                render_board_surface_internal(state, board, text, vault_files, ui, theme, true);
            if board_intent != KanbanIntent::None {
                intent = board_intent;
            }

            ui.add_space(8.0);

            // Shortcut Hints Footer
            render_shortcut_footer(ui, theme);
        });

    intent
}

/// Renders the embedded central view (Mode B) inside CentralPanel filling available space cleanly.
pub fn render_kanban_central(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    ui.vertical(|ui| {
        let board_intent =
            render_board_surface_internal(state, board, text, vault_files, ui, theme, false);
        if board_intent != KanbanIntent::None {
            intent = board_intent;
        }

        ui.add_space(6.0);
        render_shortcut_footer(ui, theme);
    });

    intent
}

/// Header bar: title/board selector, progress summary `N/M (X%)` with mini progress bar,
/// filter search bar, "+ Колонка" button, "Исходник (gs)" toggle button.
fn render_header(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    _text: &str,
    ui: &mut Ui,
    theme: &Theme,
    is_modal: bool,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        // Vector Kanban Icon
        let (icon_r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        render_icon(
            ui.painter(),
            Icon::Kanban,
            icon_r,
            color(theme.accent_primary),
            1.4,
        );

        // Title
        let board_title = board.title.as_deref().unwrap_or("КАНБАН-ДОСКА");
        ui.label(
            RichText::new(board_title)
                .size(15.0)
                .strong()
                .color(color(theme.text_primary)),
        );

        // Progress summary: N/M (X%)
        let total = board.total_card_count();
        let done = board.done_card_count();
        let pct = if total > 0 {
            (done as f32 / total as f32 * 100.0).round() as usize
        } else {
            0
        };

        ui.label(
            RichText::new(format!("{done}/{total} ({pct}%)"))
                .size(11.5)
                .color(color(theme.text_muted)),
        );

        // Mini progress bar
        let (bar_rect, _) = ui.allocate_exact_size(vec2(60.0, 4.0), Sense::hover());
        ui.painter()
            .rect_filled(bar_rect, CornerRadius::same(2), color(theme.bg_canvas));
        if total > 0 && done > 0 {
            let fill_w = (bar_rect.width() * (done as f32 / total as f32)).min(bar_rect.width());
            let filled_rect = Rect::from_min_size(bar_rect.min, vec2(fill_w, bar_rect.height()));
            ui.painter().rect_filled(
                filled_rect,
                CornerRadius::same(2),
                color(theme.state_success),
            );
        }

        ui.add_space(8.0);

        // Filter search bar
        let mut search_active = state.mode == KanbanInputMode::FilterSearch;
        let search_id = if is_modal {
            "kanban_modal_filter_search"
        } else {
            "kanban_central_filter_search"
        };
        let _ = crate::native::widgets::search_filter_bar(
            ui,
            search_id,
            &mut state.search_query,
            Some(&mut search_active),
            "Фильтр (/)...",
            theme,
        );
        if search_active && state.mode != KanbanInputMode::FilterSearch {
            state.mode = KanbanInputMode::FilterSearch;
        } else if !search_active && state.mode == KanbanInputMode::FilterSearch {
            state.mode = KanbanInputMode::Normal;
        }

        // Action buttons aligned right
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if is_modal && icon_button(ui, Icon::Close, "Закрыть (Esc)").clicked() {
                intent = KanbanIntent::CloseOverlay;
            }

            if secondary_button(ui, "Исходник (gs)").clicked() {
                intent = KanbanIntent::ToggleSourceView;
            }

            if secondary_button_with_icon(ui, Icon::Plus, "+ Колонка").clicked() {
                state.mode = KanbanInputMode::ColumnInput {
                    rename_col_idx: None,
                    buffer: String::new(),
                };
            }
        });
    });

    intent
}

fn render_active_card_input(
    col_idx: usize,
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut next_mode = None;
    let mut card_in_intent = KanbanIntent::None;
    if let KanbanInputMode::CardInput {
        edit_card_idx,
        insert_pos,
        buffer,
        wiki_ac,
        ..
    } = &mut state.mode
    {
        card_in_intent = render_card_input(
            col_idx,
            *edit_card_idx,
            *insert_pos,
            buffer,
            wiki_ac,
            board,
            text,
            vault_files,
            &mut next_mode,
            ui,
            theme,
        );
    }
    if let Some(nm) = next_mode {
        state.mode = nm;
    }
    card_in_intent
}

/// Column rendering: fixed width ~290px, bg_secondary, 8px corner radius, column accent dot,
/// column title, card count badge, completion mini-bar, vertical cards list.
#[allow(clippy::too_many_arguments)]
fn render_column(
    col_idx: usize,
    col: &KanbanColumn,
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    query_lower: &str,
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    let col_frame = Frame::new()
        .fill(color(theme.bg_secondary))
        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 10));

    ui.allocate_ui_with_layout(
        vec2(COLUMN_WIDTH, ui.available_height().max(320.0)),
        Layout::top_down(Align::Min),
        |ui| {
            col_frame.show(ui, |ui| {
                ui.set_width(COLUMN_WIDTH - 20.0);

                // Column Header
                let header_intent = render_column_header(col_idx, col, state, board, text, ui, theme);
                if header_intent != KanbanIntent::None {
                    intent = header_intent;
                }

                ui.add_space(6.0);

                // Cards Scrollable Area
                ScrollArea::vertical()
                    .id_salt(Id::new("kanban_col_cards_scroll").with(col_idx))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;

                        // Check if CardInput should be rendered at top of column
                        let show_at_top = if let KanbanInputMode::CardInput {
                            col_idx: active_col,
                            edit_card_idx: None,
                            insert_pos,
                            ..
                        } = state.mode
                        {
                            active_col == col_idx
                                && (col.cards.is_empty()
                                    || matches!(insert_pos, CardInsertPos::Before(0)))
                        } else {
                            false
                        };

                        if show_at_top {
                            let card_in_intent = render_active_card_input(
                                col_idx,
                                state,
                                board,
                                text,
                                vault_files,
                                ui,
                                theme,
                            );
                            if card_in_intent != KanbanIntent::None {
                                intent = card_in_intent;
                            }
                        }

                        // Cards
                        for (card_idx, card) in col.cards.iter().enumerate() {
                            if !card_matches_query(card, query_lower) {
                                continue;
                            }

                            // Render input if inserting BEFORE this card (e.g. `O` / `i` on card > 0)
                            let is_inserting_before = if let KanbanInputMode::CardInput {
                                col_idx: active_col,
                                edit_card_idx: None,
                                insert_pos,
                                ..
                            } = state.mode
                            {
                                active_col == col_idx
                                    && matches!(insert_pos, CardInsertPos::Before(idx) if idx == card_idx && card_idx > 0)
                            } else {
                                false
                            };
                            if is_inserting_before {
                                let card_in_intent = render_active_card_input(
                                    col_idx,
                                    state,
                                    board,
                                    text,
                                    vault_files,
                                    ui,
                                    theme,
                                );
                                if card_in_intent != KanbanIntent::None {
                                    intent = card_in_intent;
                                }
                            }

                            let is_editing_this_card = if let KanbanInputMode::CardInput {
                                col_idx: active_col,
                                edit_card_idx: Some(c_idx),
                                ..
                            } = state.mode
                            {
                                active_col == col_idx && c_idx == card_idx
                            } else {
                                false
                            };

                            if is_editing_this_card {
                                let card_in_intent = render_active_card_input(
                                    col_idx,
                                    state,
                                    board,
                                    text,
                                    vault_files,
                                    ui,
                                    theme,
                                );
                                if card_in_intent != KanbanIntent::None {
                                    intent = card_in_intent;
                                }
                            } else {
                                let card_intent = render_card(
                                    col_idx,
                                    card_idx,
                                    card,
                                    state,
                                    board,
                                    text,
                                    ui,
                                    theme,
                                );
                                if card_intent != KanbanIntent::None {
                                    intent = card_intent;
                                }
                            }

                            // Render input if inserting AFTER this card (e.g. `o` / `a` on intermediate cards)
                            let is_inserting_after = if let KanbanInputMode::CardInput {
                                col_idx: active_col,
                                edit_card_idx: None,
                                insert_pos,
                                ..
                            } = state.mode
                            {
                                active_col == col_idx
                                    && matches!(insert_pos, CardInsertPos::After(idx) if idx == card_idx && card_idx + 1 < col.cards.len())
                            } else {
                                false
                            };
                            if is_inserting_after {
                                let card_in_intent = render_active_card_input(
                                    col_idx,
                                    state,
                                    board,
                                    text,
                                    vault_files,
                                    ui,
                                    theme,
                                );
                                if card_in_intent != KanbanIntent::None {
                                    intent = card_in_intent;
                                }
                            }
                        }

                        // Check if CardInput should be rendered at bottom of column
                        let show_at_bottom = if let KanbanInputMode::CardInput {
                            col_idx: active_col,
                            edit_card_idx: None,
                            insert_pos,
                            ..
                        } = state.mode
                        {
                            active_col == col_idx
                                && !col.cards.is_empty()
                                && (matches!(insert_pos, CardInsertPos::End)
                                    || matches!(insert_pos, CardInsertPos::After(idx) if idx + 1 >= col.cards.len()))
                        } else {
                            false
                        };

                        if show_at_bottom {
                            let card_in_intent = render_active_card_input(
                                col_idx,
                                state,
                                board,
                                text,
                                vault_files,
                                ui,
                                theme,
                            );
                            if card_in_intent != KanbanIntent::None {
                                intent = card_in_intent;
                            }
                        }
                    });
            });
        },
    );

    intent
}

/// Column Header: Accent dot, title (or rename input), card count badge, mini-bar, "+ карточка" button.
fn render_column_header(
    col_idx: usize,
    col: &KanbanColumn,
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;

        // Column accent dot
        let dot_color = match col_idx % 4 {
            0 => color(theme.accent_primary),
            1 => color(theme.state_warning),
            2 => color(theme.state_success),
            _ => color(theme.callout_note),
        };
        let (dot_rect, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
        ui.painter()
            .circle_filled(dot_rect.center(), 3.5, dot_color);

        // Column title or inlined rename edit
        let is_renaming = if let KanbanInputMode::ColumnInput {
            rename_col_idx: Some(idx),
            ..
        } = state.mode
        {
            idx == col_idx
        } else {
            false
        };

        if is_renaming {
            let mut commit = false;
            let mut cancel = false;
            if let KanbanInputMode::ColumnInput {
                rename_col_idx: Some(idx),
                buffer,
            } = &mut state.mode
                && *idx == col_idx
            {
                let edit_resp = ui.add(
                    TextEdit::singleline(buffer)
                        .id(Id::new("kanban_rename_col_input").with(col_idx))
                        .desired_width(120.0),
                );
                edit_resp.request_focus();

                if ui.button(RichText::new("✓").strong()).clicked()
                    || (edit_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    commit = true;
                }
                if ui.button(RichText::new("✕")).clicked()
                    || (edit_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)))
                {
                    cancel = true;
                }
            }
            if cancel {
                state.mode = KanbanInputMode::Normal;
            } else if commit {
                if let KanbanInputMode::ColumnInput { buffer, .. } = &state.mode {
                    let trimmed = buffer.trim();
                    if !trimmed.is_empty()
                        && let Some(tx) = plan_rename_column(text, board, col_idx, trimmed)
                    {
                        intent = KanbanIntent::ApplyTx(tx);
                    }
                }
                state.mode = KanbanInputMode::Normal;
            }
        } else {
            ui.label(
                RichText::new(&col.title)
                    .strong()
                    .size(13.0)
                    .color(color(theme.text_primary)),
            );
        }

        // Card count badge
        let col_card_count = col.cards.len();
        Frame::new()
            .fill(color(theme.bg_canvas))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{col_card_count}"))
                        .size(10.5)
                        .color(color(theme.text_muted)),
                );
            });

        // Plus icon button to add a new card to this column
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, Icon::Plus, "Добавить карточку (o)").clicked() {
                state.selected_col = col_idx;
                state.mode = KanbanInputMode::CardInput {
                    col_idx,
                    edit_card_idx: None,
                    insert_pos: CardInsertPos::End,
                    buffer: String::new(),
                    wiki_ac: None,
                };
            }
        });
    });

    // Column completion mini-bar
    let col_cards_cnt = col.cards.len();
    let col_done = col
        .cards
        .iter()
        .filter(|c| c.status == KanbanTaskStatus::Done)
        .count();
    if col_cards_cnt > 0 {
        ui.add_space(3.0);
        let (bar_rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0), Sense::hover());
        ui.painter()
            .rect_filled(bar_rect, CornerRadius::same(1), color(theme.bg_canvas));
        if col_done > 0 {
            let fill_w = bar_rect.width() * (col_done as f32 / col_cards_cnt as f32);
            let filled_rect = Rect::from_min_size(bar_rect.min, vec2(fill_w, bar_rect.height()));
            ui.painter().rect_filled(
                filled_rect,
                CornerRadius::same(1),
                color(theme.state_success),
            );
        }
    }

    intent
}

/// Card rendering: `bg_elevated`, `6px` corner radius, selected card has 1.5px accent border +
/// 3px left indicator bar. Status checkbox icon (clickable to cycle status), clean title
/// (strikethrough if Done/Cancelled), body snippet if present, #tag pills, file-link chips.
#[allow(clippy::too_many_arguments)]
fn render_card(
    col_idx: usize,
    card_idx: usize,
    card: &KanbanCard,
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;
    let is_selected = state.selected_col == col_idx && state.selected_card == card_idx;

    let card_stroke = if is_selected {
        Stroke::new(1.5, color(theme.accent_primary))
    } else {
        Stroke::new(1.0, color(theme.border_subtle))
    };

    let card_frame = Frame::new()
        .fill(color(theme.bg_elevated))
        .stroke(card_stroke)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(10, 8));

    let card_resp = card_frame.show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Header row: Checkbox + Title
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Status checkbox icon
            let (status_rect, status_resp) =
                ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
            let status_icon = match card.status {
                KanbanTaskStatus::Todo => Icon::Square,
                KanbanTaskStatus::InProgress => Icon::InProgress,
                KanbanTaskStatus::Done => Icon::CheckSquare,
            };
            let status_color = match card.status {
                KanbanTaskStatus::Todo => color(theme.text_muted),
                KanbanTaskStatus::InProgress => color(theme.accent_primary),
                KanbanTaskStatus::Done => color(theme.state_success),
            };
            render_icon(ui.painter(), status_icon, status_rect, status_color, 1.4);

            if status_resp.clicked() {
                state.selected_col = col_idx;
                state.selected_card = card_idx;
                if let Some(tx) = plan_cycle_card_status(text, board, col_idx, card_idx) {
                    intent = KanbanIntent::ApplyTx(tx);
                }
            }

            // Title with strikethrough if Done
            let mut title_rich = RichText::new(&card.clean_title).size(12.5);
            if card.status == KanbanTaskStatus::Done {
                title_rich = title_rich.color(color(theme.text_muted)).strikethrough();
            } else {
                title_rich = title_rich.color(color(theme.text_primary));
            }
            ui.label(title_rich);
        });

        // Continuation body snippet
        if !card.body_lines.is_empty() {
            ui.add_space(2.0);
            ui.label(
                RichText::new(&card.body_lines[0])
                    .size(11.0)
                    .color(color(theme.text_secondary)),
            );
        }

        // Tags and File Links
        if !card.tags.is_empty() || !card.links.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);

                // #tag pills
                for tag in &card.tags {
                    Frame::new()
                        .fill(color(theme.bg_canvas))
                        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
                        .corner_radius(CornerRadius::same(3))
                        .inner_margin(Margin::symmetric(4, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("#{tag}"))
                                    .size(10.0)
                                    .color(color(theme.accent_primary)),
                            );
                        });
                }

                // File link chips
                for link in &card.links {
                    let chip_frame = Frame::new()
                        .fill(color(theme.bg_canvas))
                        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
                        .corner_radius(CornerRadius::same(3))
                        .inner_margin(Margin::symmetric(5, 2));

                    let chip_resp = chip_frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 3.0;
                            let (icon_r, _) =
                                ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                            render_icon(
                                ui.painter(),
                                Icon::File,
                                icon_r,
                                color(theme.accent_primary),
                                1.2,
                            );
                            ui.label(
                                RichText::new(link.display())
                                    .size(10.5)
                                    .color(color(theme.accent_primary)),
                            );
                        });
                    });

                    if chip_resp.response.interact(Sense::click()).clicked() {
                        intent = KanbanIntent::OpenLink(link.navigation_target());
                    }
                }
            });
        }
    });

    let card_rect = card_resp.response.rect;

    // 3px left indicator bar if selected
    if is_selected {
        let indicator_rect =
            Rect::from_min_size(card_rect.left_top(), vec2(3.0, card_rect.height()));
        ui.painter().rect_filled(
            indicator_rect,
            CornerRadius {
                nw: 6,
                ne: 0,
                sw: 6,
                se: 0,
            },
            color(theme.accent_primary),
        );
    }

    // Clicking anywhere on card updates selection
    if card_resp.response.interact(Sense::click()).clicked() {
        state.selected_col = col_idx;
        state.selected_card = card_idx;
    }

    intent
}

/// Inlined CardInput: singleline text edit for adding or editing cards with `[[` autocomplete dropdown.
#[allow(clippy::too_many_arguments)]
fn render_card_input(
    col_idx: usize,
    edit_card_idx: Option<usize>,
    insert_pos: CardInsertPos,
    buffer: &mut String,
    wiki_ac: &mut Option<InlineFileAutocomplete>,
    board: &KanbanBoard,
    text: &str,
    vault_files: &[String],
    next_mode: &mut Option<KanbanInputMode>,
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    let frame = Frame::new()
        .fill(color(theme.bg_elevated))
        .stroke(Stroke::new(1.5, color(theme.accent_primary)))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(10, 8));

    let mut commit = false;
    let mut cancel = false;

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());

        let label = if edit_card_idx.is_some() {
            "Редактирование карточки:"
        } else {
            "Новая карточка:"
        };
        ui.label(
            RichText::new(label)
                .size(11.0)
                .strong()
                .color(color(theme.text_secondary)),
        );
        ui.add_space(3.0);

        let edit_resp = ui.add(
            TextEdit::singleline(buffer)
                .id(Id::new("kanban_card_input_field"))
                .hint_text("Текст карточки... ([[ для ссылки)")
                .desired_width(ui.available_width()),
        );
        edit_resp.request_focus();

        if edit_resp.changed() {
            *wiki_ac = InlineFileAutocomplete::detect(buffer, vault_files);
        }

        // Autocomplete candidate list dropdown
        if let Some(ac) = wiki_ac
            && !ac.candidates.is_empty()
        {
            ui.add_space(4.0);
            Frame::new()
                .fill(color(theme.bg_panel))
                .stroke(Stroke::new(1.0, color(theme.border_subtle)))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("Файлы для ссылки:")
                            .size(10.0)
                            .color(color(theme.text_muted)),
                    );
                    let mut completed = false;
                    for (cand_idx, cand) in ac.candidates.iter().enumerate() {
                        let is_active = cand_idx == ac.selected;
                        let cand_frame = Frame::new()
                            .fill(if is_active {
                                color(theme.bg_selected)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(CornerRadius::same(2))
                            .inner_margin(Margin::symmetric(4, 2));

                        let cand_resp = cand_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (icon_r, _) =
                                    ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                                render_icon(
                                    ui.painter(),
                                    Icon::File,
                                    icon_r,
                                    color(theme.accent_primary),
                                    1.2,
                                );
                                ui.label(RichText::new(cand).size(11.0).color(if is_active {
                                    color(theme.accent_primary)
                                } else {
                                    color(theme.text_primary)
                                }));
                            });
                        });
                        if cand_resp.response.interact(Sense::click()).clicked() {
                            ac.selected = cand_idx;
                            completed = true;
                        }
                    }
                    if completed {
                        ac.apply_completion(buffer);
                    }
                });
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button(
                    RichText::new("Сохранить")
                        .strong()
                        .color(color(theme.text_primary)),
                )
                .clicked()
                || (edit_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                commit = true;
            }
            if ui
                .button(RichText::new("Отмена").color(color(theme.text_muted)))
                .clicked()
                || (edit_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)))
            {
                cancel = true;
            }
        });
    });

    if cancel {
        *next_mode = Some(KanbanInputMode::Normal);
    } else if commit {
        let trimmed = buffer.trim();
        if !trimmed.is_empty() {
            let tx = if let Some(card_idx) = edit_card_idx {
                plan_edit_card_title(text, board, col_idx, card_idx, trimmed)
            } else {
                plan_add_card(text, board, col_idx, insert_pos, trimmed)
            };
            if let Some(tx) = tx {
                intent = KanbanIntent::ApplyTx(tx);
            }
        }
        *next_mode = Some(KanbanInputMode::Normal);
    }

    intent
}

/// Inlined ColumnInput for adding a new column or placeholder button "+ Добавить колонку".
fn render_add_column_column(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    ui: &mut Ui,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    if matches!(
        state.mode,
        KanbanInputMode::ColumnInput {
            rename_col_idx: None,
            ..
        }
    ) {
        let col_frame = Frame::new()
            .fill(color(theme.bg_secondary))
            .stroke(Stroke::new(1.0, color(theme.border_subtle)))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::symmetric(10, 10));

        let mut commit = false;
        let mut cancel = false;

        ui.allocate_ui_with_layout(
            vec2(COLUMN_WIDTH, 140.0),
            Layout::top_down(Align::Min),
            |ui| {
                col_frame.show(ui, |ui| {
                    ui.set_width(COLUMN_WIDTH - 20.0);
                    ui.label(
                        RichText::new("Новая колонка:")
                            .strong()
                            .size(12.5)
                            .color(color(theme.text_primary)),
                    );
                    ui.add_space(4.0);

                    let buffer = match &mut state.mode {
                        KanbanInputMode::ColumnInput {
                            rename_col_idx: None,
                            buffer,
                        } => buffer,
                        _ => return,
                    };

                    let edit_resp = ui.add(
                        TextEdit::singleline(buffer)
                            .id(Id::new("kanban_add_column_input"))
                            .hint_text("Название колонки...")
                            .desired_width(ui.available_width()),
                    );
                    edit_resp.request_focus();

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui
                            .button(
                                RichText::new("Создать")
                                    .strong()
                                    .color(color(theme.text_primary)),
                            )
                            .clicked()
                            || (edit_resp.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            commit = true;
                        }
                        if ui
                            .button(RichText::new("Отмена").color(color(theme.text_muted)))
                            .clicked()
                            || (edit_resp.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Escape)))
                        {
                            cancel = true;
                        }
                    });
                });
            },
        );

        if cancel {
            state.mode = KanbanInputMode::Normal;
        } else if commit {
            if let KanbanInputMode::ColumnInput {
                rename_col_idx: None,
                buffer,
            } = &state.mode
            {
                let trimmed = buffer.trim();
                if !trimmed.is_empty()
                    && let Some(tx) = plan_add_column(text, board, trimmed)
                {
                    intent = KanbanIntent::ApplyTx(tx);
                }
            }
            state.mode = KanbanInputMode::Normal;
        }
    } else {
        ui.allocate_ui_with_layout(vec2(160.0, 44.0), Layout::top_down(Align::Min), |ui| {
            let btn = egui::Button::new(
                RichText::new("+ Добавить колонку")
                    .size(12.0)
                    .color(color(theme.text_muted)),
            )
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, color(theme.border_subtle)))
            .corner_radius(CornerRadius::same(8))
            .min_size(vec2(160.0, 36.0));

            if ui.add(btn).clicked() {
                state.mode = KanbanInputMode::ColumnInput {
                    rename_col_idx: None,
                    buffer: String::new(),
                };
            }
        });
    }

    intent
}

/// Floating overlays dispatcher (FilePicker and LinkJumpPicker).
fn render_floating_overlays(
    state: &mut KanbanBoardState,
    board: &KanbanBoard,
    text: &str,
    ctx: &egui::Context,
    theme: &Theme,
) -> KanbanIntent {
    let mut intent = KanbanIntent::None;

    match &mut state.mode {
        KanbanInputMode::FilePicker(picker) => {
            let action = render_file_picker_overlay(
                picker,
                state.selected_col,
                state.selected_card,
                board,
                text,
                ctx,
                theme,
            );
            match action {
                OverlayAction::ApplyTx(tx) => {
                    intent = KanbanIntent::ApplyTx(tx);
                    state.mode = KanbanInputMode::Normal;
                }
                OverlayAction::Close => {
                    state.mode = KanbanInputMode::Normal;
                }
                _ => {}
            }
        }
        KanbanInputMode::LinkJumpPicker(picker) => {
            let action = render_link_jump_overlay(picker, ctx, theme);
            match action {
                OverlayAction::OpenLink(target) => {
                    intent = KanbanIntent::OpenLink(target);
                    state.mode = KanbanInputMode::Normal;
                }
                OverlayAction::Close => {
                    state.mode = KanbanInputMode::Normal;
                }
                _ => {}
            }
        }
        _ => {}
    }

    intent
}

/// FilePicker floating dialog: search input, ranked file list with `Icon::File`, hints footer.
fn render_file_picker_overlay(
    picker: &mut FuzzyFilePicker,
    selected_col: usize,
    selected_card: usize,
    board: &KanbanBoard,
    text: &str,
    ctx: &egui::Context,
    theme: &Theme,
) -> OverlayAction {
    let mut action = OverlayAction::None;
    let width = 420.0;
    let height = 300.0;

    let frame = Frame::new()
        .fill(color(theme.bg_panel))
        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
        .corner_radius(CornerRadius::same(8))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(100),
        })
        .inner_margin(Margin::symmetric(14, 12));

    egui::Window::new("kanban_file_picker_overlay")
        .title_bar(false)
        .frame(frame)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .fixed_size(vec2(width, height))
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (icon_r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                render_icon(
                    ui.painter(),
                    Icon::File,
                    icon_r,
                    color(theme.accent_primary),
                    1.4,
                );
                ui.label(
                    RichText::new("Прикрепить ссылку на заметку")
                        .strong()
                        .size(13.0)
                        .color(color(theme.text_primary)),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, Icon::Close, "Закрыть (Esc)").clicked() {
                        action = OverlayAction::Close;
                    }
                });
            });

            ui.add_space(8.0);

            // Search text edit
            let query_resp = ui.add(
                TextEdit::singleline(&mut picker.query)
                    .id(Id::new("kanban_file_picker_query"))
                    .hint_text("Поиск файлов в хранилище...")
                    .desired_width(ui.available_width()),
            );
            query_resp.request_focus();
            if query_resp.changed() {
                picker.filter();
            }

            ui.add_space(8.0);

            // Filtered files list
            ScrollArea::vertical()
                .id_salt("kanban_file_picker_scroll")
                .max_height(180.0)
                .show(ui, |ui| {
                    if picker.filtered.is_empty() {
                        ui.label(
                            RichText::new("Файлы не найдены")
                                .size(11.5)
                                .color(color(theme.text_muted)),
                        );
                    } else {
                        for (i, file) in picker.filtered.iter().enumerate() {
                            let is_selected = i == picker.selected;
                            let row_frame = Frame::new()
                                .fill(if is_selected {
                                    color(theme.bg_selected)
                                } else {
                                    Color32::TRANSPARENT
                                })
                                .corner_radius(CornerRadius::same(4))
                                .inner_margin(Margin::symmetric(6, 3));

                            let row_resp = row_frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (icon_r, _) =
                                        ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                                    render_icon(
                                        ui.painter(),
                                        Icon::File,
                                        icon_r,
                                        color(theme.accent_primary),
                                        1.2,
                                    );
                                    ui.label(RichText::new(file).size(12.0).color(
                                        if is_selected {
                                            color(theme.accent_primary)
                                        } else {
                                            color(theme.text_primary)
                                        },
                                    ));
                                })
                            });

                            if row_resp.response.interact(Sense::click()).clicked() {
                                picker.selected = i;
                                if let Some(tx) = plan_attach_file_link(
                                    text,
                                    board,
                                    selected_col,
                                    selected_card,
                                    file,
                                ) {
                                    action = OverlayAction::ApplyTx(tx);
                                } else {
                                    action = OverlayAction::Close;
                                }
                            }
                        }
                    }
                });

            ui.add_space(8.0);
            ui.label(
                RichText::new("↑/↓: выбор   Enter: прикрепить   Esc: отмена")
                    .size(10.5)
                    .color(color(theme.text_muted)),
            );
        });

    action
}

/// LinkJumpPicker popover: compact list of links on card with `1..N` shortcuts.
fn render_link_jump_overlay(
    picker: &mut LinkJumpPicker,
    ctx: &egui::Context,
    theme: &Theme,
) -> OverlayAction {
    let mut action = OverlayAction::None;
    let width = 340.0;
    let height = 220.0;

    let frame = Frame::new()
        .fill(color(theme.bg_panel))
        .stroke(Stroke::new(1.0, color(theme.border_subtle)))
        .corner_radius(CornerRadius::same(8))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(100),
        })
        .inner_margin(Margin::symmetric(14, 12));

    egui::Window::new("kanban_link_jump_overlay")
        .title_bar(false)
        .frame(frame)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .fixed_size(vec2(width, height))
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (icon_r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                render_icon(
                    ui.painter(),
                    Icon::Link,
                    icon_r,
                    color(theme.accent_primary),
                    1.4,
                );
                ui.label(
                    RichText::new("Перейти по ссылке")
                        .strong()
                        .size(13.0)
                        .color(color(theme.text_primary)),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, Icon::Close, "Закрыть (Esc)").clicked() {
                        action = OverlayAction::Close;
                    }
                });
            });

            ui.add_space(8.0);

            ScrollArea::vertical()
                .id_salt("kanban_link_jump_scroll")
                .max_height(130.0)
                .show(ui, |ui| {
                    for (i, link) in picker.links.iter().enumerate() {
                        let is_selected = i == picker.selected;
                        let row_frame = Frame::new()
                            .fill(if is_selected {
                                color(theme.bg_selected)
                            } else {
                                Color32::TRANSPARENT
                            })
                            .corner_radius(CornerRadius::same(4))
                            .inner_margin(Margin::symmetric(6, 3));

                        let row_resp = row_frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Number shortcut badge
                                Frame::new()
                                    .fill(color(theme.bg_canvas))
                                    .stroke(Stroke::new(1.0, color(theme.border_subtle)))
                                    .corner_radius(CornerRadius::same(3))
                                    .inner_margin(Margin::symmetric(4, 1))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!("{}", i + 1))
                                                .monospace()
                                                .strong()
                                                .size(10.0)
                                                .color(color(theme.accent_primary)),
                                        );
                                    });

                                let (icon_r, _) =
                                    ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                                render_icon(
                                    ui.painter(),
                                    Icon::File,
                                    icon_r,
                                    color(theme.accent_primary),
                                    1.2,
                                );

                                ui.label(RichText::new(link.display()).size(12.0).color(
                                    if is_selected {
                                        color(theme.accent_primary)
                                    } else {
                                        color(theme.text_primary)
                                    },
                                ));
                            })
                        });

                        if row_resp.response.interact(Sense::click()).clicked() {
                            picker.selected = i;
                            action = OverlayAction::OpenLink(link.navigation_target());
                        }
                    }
                });

            ui.add_space(8.0);
            ui.label(
                RichText::new("1-9: быстрая ссылка   Enter: открыть   Esc: отмена")
                    .size(10.5)
                    .color(color(theme.text_muted)),
            );
        });

    action
}

/// Shortcut hints footer: `h/l col, j/k card, o/O add, Space toggle, f link, Enter open, gs source, Esc close`.
pub fn render_shortcut_footer(ui: &mut Ui, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let shortcuts = [
            ("h/l", "колонка"),
            ("j/k", "карточка"),
            ("o/O", "добавить"),
            ("Space", "статус"),
            ("f", "ссылка"),
            ("Enter", "открыть"),
            ("gs", "исходник"),
            ("Esc", "закрыть"),
        ];
        for (i, (key, desc)) in shortcuts.iter().enumerate() {
            if i > 0 {
                ui.label(RichText::new("·").size(11.0).color(color(theme.text_muted)));
            }
            ui.label(
                RichText::new(*key)
                    .monospace()
                    .strong()
                    .size(11.0)
                    .color(color(theme.accent_primary)),
            );
            ui.label(
                RichText::new(*desc)
                    .size(11.0)
                    .color(color(theme.text_muted)),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::kanban::picker::LinkJumpPicker;
    use egui::Pos2;
    use truff_core::markdown::kanban::KanbanCardLink;

    const SAMPLE_KANBAN: &str = r#"---
kanban-plugin: basic
---

# Project Board

## Backlog
- [ ] Task 1 #backend
- [ ] Task 2 [[notes/Architecture]]
  Details line 1
  Details line 2

## In Progress
- [/] Active Task #urgent
- [/] Subsystem Refactoring [[design|System Design]]

## Done
- [x] Completed item #docs
"#;

    #[test]
    fn test_render_board_surface_dark_theme() {
        let board = KanbanBoard::parse(SAMPLE_KANBAN);
        let mut state = KanbanBoardState::new();
        let vault_files = vec!["notes/Architecture.md".to_string(), "design.md".to_string()];
        let theme = Theme::nord();

        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let intent = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
                assert_eq!(intent, KanbanIntent::None);
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn test_render_board_surface_light_theme() {
        let board = KanbanBoard::parse(SAMPLE_KANBAN);
        let mut state = KanbanBoardState::new();
        let vault_files = vec!["notes/Architecture.md".to_string()];
        let theme = Theme::light();

        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let intent = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
                assert_eq!(intent, KanbanIntent::None);
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn test_render_kanban_modal_overlay() {
        let board = KanbanBoard::parse(SAMPLE_KANBAN);
        let mut state = KanbanBoardState::new();
        let vault_files = vec!["notes/Architecture.md".to_string()];
        let available_boards = vec![
            "Project.kanban.md".to_string(),
            "Sprint.kanban.md".to_string(),
        ];
        let theme = Theme::obsidian_dark();

        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))),
                ..Default::default()
            },
            |ctx| {
                let intent = render_kanban_modal(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    &available_boards,
                    Some("Project.kanban.md"),
                    ctx,
                    &theme,
                );
                assert_eq!(intent, KanbanIntent::None);
            },
        );
        output.textures_delta.clear();
    }

    #[test]
    fn test_render_kanban_central_view() {
        let board = KanbanBoard::parse(SAMPLE_KANBAN);
        let mut state = KanbanBoardState::new();
        let vault_files = vec![];
        let theme = Theme::nord();

        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let intent = render_kanban_central(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
                assert_eq!(intent, KanbanIntent::None);
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn test_render_inlined_inputs_and_overlays() {
        let board = KanbanBoard::parse(SAMPLE_KANBAN);
        let mut state = KanbanBoardState::new();
        let vault_files = vec!["Doc1.md".to_string(), "Doc2.md".to_string()];
        let theme = Theme::nord();

        // 1. Test CardInput mode
        state.mode = KanbanInputMode::CardInput {
            col_idx: 0,
            edit_card_idx: None,
            insert_pos: CardInsertPos::End,
            buffer: "New card with [[Doc".to_string(),
            wiki_ac: InlineFileAutocomplete::detect("New card with [[Doc", &vault_files),
        };

        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
            });
        });
        output.textures_delta.clear();

        // 2. Test FilePicker overlay
        state.mode = KanbanInputMode::FilePicker(FuzzyFilePicker::new(&vault_files));
        let mut output2 = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
            });
        });
        output2.textures_delta.clear();

        // 3. Test LinkJumpPicker overlay
        let links = vec![KanbanCardLink {
            target: "Doc1".to_string(),
            heading: None,
            alias: None,
            raw_range_in_line: 0..6,
        }];
        state.mode = KanbanInputMode::LinkJumpPicker(LinkJumpPicker::new(links));
        let mut output3 = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
            });
        });
        output3.textures_delta.clear();
    }

    #[test]
    fn test_render_card_input_intermediate_insertion() {
        let ctx = egui::Context::default();
        let board = KanbanBoard::parse(SAMPLE_KANBAN);
        let theme = Theme::nord();
        let vault_files = vec!["Notes/Design.md".to_string()];

        // 1. Test insertion AFTER intermediate card 0 (in a column with 2+ cards)
        let mut state = KanbanBoardState::new();
        state.mode = KanbanInputMode::CardInput {
            col_idx: 0,
            edit_card_idx: None,
            insert_pos: CardInsertPos::After(0),
            buffer: "Middle Card".to_string(),
            wiki_ac: None,
        };

        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
            });
        });
        output.textures_delta.clear();

        // 2. Test insertion BEFORE intermediate card 1
        state.mode = KanbanInputMode::CardInput {
            col_idx: 0,
            edit_card_idx: None,
            insert_pos: CardInsertPos::Before(1),
            buffer: "Before Card 1".to_string(),
            wiki_ac: None,
        };

        let mut output2 = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = render_board_surface(
                    &mut state,
                    &board,
                    SAMPLE_KANBAN,
                    &vault_files,
                    ui,
                    &theme,
                );
            });
        });
        output2.textures_delta.clear();
    }
}
