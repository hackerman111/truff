use crate::native::kanban::picker::{
    FuzzyFilePicker, InlineFileAutocomplete, LinkJumpAction, LinkJumpPicker,
};
use truff_core::markdown::kanban::{
    CardInsertPos, KanbanBoard, plan_add_card, plan_add_column, plan_attach_file_link,
    plan_cycle_card_status, plan_delete_card, plan_delete_empty_column, plan_edit_card_title,
    plan_move_card_column, plan_move_card_vertical, plan_move_column, plan_paste_card,
    plan_rename_column, plan_set_card_status,
};
use truff_core::text::Transaction;
use truff_core::vim::Key;

/// High-level intent emitted by the Kanban board state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KanbanIntent {
    None,
    ApplyTx(Transaction),
    OpenLink(String),
    Undo,
    Redo,
    ToggleSourceView,
    SwitchBoard { next: bool },
    SelectBoard(String),
    CreateNewBoard,
    CloseOverlay,
    Status(String),
}

/// Active interaction mode of the Kanban board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum KanbanInputMode {
    #[default]
    Normal,
    CardInput {
        col_idx: usize,
        edit_card_idx: Option<usize>,
        insert_pos: CardInsertPos,
        buffer: String,
        wiki_ac: Option<InlineFileAutocomplete>,
    },
    ColumnInput {
        rename_col_idx: Option<usize>,
        buffer: String,
    },
    FilePicker(FuzzyFilePicker),
    LinkJumpPicker(LinkJumpPicker),
    FilterSearch,
}

/// UI and interaction state machine for a Kanban board.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KanbanBoardState {
    pub selected_col: usize,
    pub selected_card: usize,
    pub count_prefix: Option<usize>,
    pub pending_g: bool,
    pub pending_d: bool,
    pub pending_y: bool,
    pub yank_register: Option<String>,
    pub mode: KanbanInputMode,
    pub search_query: String,
    pub source_override: bool,
}

impl KanbanBoardState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Safely clamps the selected column and card to the current board dimensions.
    pub fn clamp_selection(&mut self, board: &KanbanBoard) {
        if board.columns.is_empty() {
            self.selected_col = 0;
            self.selected_card = 0;
        } else {
            if self.selected_col >= board.columns.len() {
                self.selected_col = board.columns.len() - 1;
            }
            let col = &board.columns[self.selected_col];
            if col.cards.is_empty() {
                self.selected_card = 0;
            } else if self.selected_card >= col.cards.len() {
                self.selected_card = col.cards.len() - 1;
            }
        }
    }

    /// Feeds a keyboard input event into the Kanban state machine, returning a high-level intent.
    pub fn feed_key(
        &mut self,
        key: Key,
        text: &str,
        board: &KanbanBoard,
        vault_files: &[String],
    ) -> KanbanIntent {
        self.clamp_selection(board);

        match &mut self.mode {
            KanbanInputMode::FilePicker(picker) => match key {
                Key::Char(c) => {
                    picker.type_char(c);
                    KanbanIntent::None
                }
                Key::Backspace => {
                    picker.backspace();
                    KanbanIntent::None
                }
                Key::Down | Key::Ctrl('j') => {
                    picker.next();
                    KanbanIntent::None
                }
                Key::Up | Key::Ctrl('k') => {
                    picker.prev();
                    KanbanIntent::None
                }
                Key::Esc => {
                    self.mode = KanbanInputMode::Normal;
                    KanbanIntent::None
                }
                Key::Enter => {
                    let file = picker.selected_file().map(str::to_string);
                    self.mode = KanbanInputMode::Normal;
                    if let Some(file_path) = file
                        && let Some(tx) = plan_attach_file_link(
                            text,
                            board,
                            self.selected_col,
                            self.selected_card,
                            &file_path,
                        )
                    {
                        return KanbanIntent::ApplyTx(tx);
                    }
                    KanbanIntent::None
                }
                _ => KanbanIntent::None,
            },

            KanbanInputMode::LinkJumpPicker(picker) => match picker.handle_key(key) {
                Some(LinkJumpAction::Jump(target)) => {
                    self.mode = KanbanInputMode::Normal;
                    KanbanIntent::OpenLink(target)
                }
                Some(LinkJumpAction::Cancel) => {
                    self.mode = KanbanInputMode::Normal;
                    KanbanIntent::None
                }
                Some(LinkJumpAction::Handled) | None => KanbanIntent::None,
            },

            KanbanInputMode::ColumnInput {
                rename_col_idx,
                buffer,
            } => match key {
                Key::Esc => {
                    self.mode = KanbanInputMode::Normal;
                    KanbanIntent::None
                }
                Key::Backspace => {
                    buffer.pop();
                    KanbanIntent::None
                }
                Key::Char(c) => {
                    buffer.push(c);
                    KanbanIntent::None
                }
                Key::Enter => {
                    let rename_idx = *rename_col_idx;
                    let title = std::mem::take(buffer);
                    self.mode = KanbanInputMode::Normal;
                    if let Some(col_idx) = rename_idx {
                        if let Some(tx) = plan_rename_column(text, board, col_idx, &title) {
                            KanbanIntent::ApplyTx(tx)
                        } else {
                            KanbanIntent::None
                        }
                    } else if let Some(tx) = plan_add_column(text, board, &title) {
                        KanbanIntent::ApplyTx(tx)
                    } else {
                        KanbanIntent::None
                    }
                }
                _ => KanbanIntent::None,
            },

            KanbanInputMode::CardInput {
                col_idx,
                edit_card_idx,
                insert_pos,
                buffer,
                wiki_ac,
            } => {
                if let Some(ac) = wiki_ac.as_mut() {
                    match key {
                        Key::Down | Key::Ctrl('j') => {
                            ac.next();
                            return KanbanIntent::None;
                        }
                        Key::Up | Key::Ctrl('k') => {
                            ac.prev();
                            return KanbanIntent::None;
                        }
                        Key::Tab if ac.apply_completion(buffer) => {
                            *wiki_ac = None;
                            return KanbanIntent::None;
                        }
                        Key::Esc => {
                            *wiki_ac = None;
                            return KanbanIntent::None;
                        }
                        _ => {}
                    }
                }

                match key {
                    Key::Esc => {
                        self.mode = KanbanInputMode::Normal;
                        KanbanIntent::None
                    }
                    Key::Backspace => {
                        buffer.pop();
                        *wiki_ac = InlineFileAutocomplete::detect(buffer, vault_files);
                        KanbanIntent::None
                    }
                    Key::Char(c) => {
                        buffer.push(c);
                        *wiki_ac = InlineFileAutocomplete::detect(buffer, vault_files);
                        KanbanIntent::None
                    }
                    Key::Enter => {
                        let c_idx = *col_idx;
                        let e_idx = *edit_card_idx;
                        let pos = *insert_pos;
                        let card_title = std::mem::take(buffer);
                        self.mode = KanbanInputMode::Normal;

                        if let Some(idx) = e_idx {
                            if let Some(tx) =
                                plan_edit_card_title(text, board, c_idx, idx, &card_title)
                            {
                                KanbanIntent::ApplyTx(tx)
                            } else {
                                KanbanIntent::None
                            }
                        } else if card_title.trim().is_empty() {
                            KanbanIntent::None
                        } else if let Some(tx) = plan_add_card(text, board, c_idx, pos, &card_title)
                        {
                            KanbanIntent::ApplyTx(tx)
                        } else {
                            KanbanIntent::None
                        }
                    }
                    _ => KanbanIntent::None,
                }
            }

            KanbanInputMode::FilterSearch => match key {
                Key::Esc | Key::Enter => {
                    self.mode = KanbanInputMode::Normal;
                    KanbanIntent::None
                }
                Key::Backspace => {
                    self.search_query.pop();
                    KanbanIntent::None
                }
                Key::Char(c) => {
                    self.search_query.push(c);
                    KanbanIntent::None
                }
                _ => KanbanIntent::None,
            },

            KanbanInputMode::Normal => self.feed_normal_key(key, text, board, vault_files),
        }
    }

    fn feed_normal_key(
        &mut self,
        key: Key,
        text: &str,
        board: &KanbanBoard,
        vault_files: &[String],
    ) -> KanbanIntent {
        // Normalize Cyrillic navigation and motion keys
        let key = match key {
            Key::Char('р') => Key::Char('h'),
            Key::Char('о') => Key::Char('j'),
            Key::Char('л') => Key::Char('k'),
            Key::Char('д') => Key::Char('l'),
            Key::Char('Р') => Key::Char('H'),
            Key::Char('О') => Key::Char('J'),
            Key::Char('Л') => Key::Char('K'),
            Key::Char('Д') => Key::Char('L'),
            other => other,
        };

        // If Esc is pressed and any count prefix or pending sequence is active, cancel it cleanly
        if key == Key::Esc
            && (self.count_prefix.is_some()
                || self.pending_g
                || self.pending_d
                || self.pending_y)
        {
            self.count_prefix = None;
            self.pending_g = false;
            self.pending_d = false;
            self.pending_y = false;
            return KanbanIntent::None;
        }

        // Two-key sequence handling for 'g'
        if self.pending_g {
            self.pending_g = false;
            self.count_prefix = None;
            return match key {
                Key::Char('g') => {
                    self.selected_card = 0;
                    KanbanIntent::None
                }
                Key::Char('s') => {
                    self.source_override = !self.source_override;
                    KanbanIntent::ToggleSourceView
                }
                Key::Char('f') => self.handle_link_jump(board),
                _ => KanbanIntent::None,
            };
        }

        // Two-key sequence handling for 'd'
        if self.pending_d {
            self.pending_d = false;
            self.count_prefix = None;
            if key == Key::Char('d')
                && let Some((tx, raw_card)) =
                    plan_delete_card(text, board, self.selected_col, self.selected_card)
            {
                self.yank_register = Some(raw_card);
                if self.selected_card > 0
                    && let Some(col) = board.columns.get(self.selected_col)
                    && self.selected_card + 1 >= col.cards.len()
                {
                    self.selected_card -= 1;
                }
                return KanbanIntent::ApplyTx(tx);
            }
            return KanbanIntent::None;
        }

        // Two-key sequence handling for 'y'
        if self.pending_y {
            self.pending_y = false;
            self.count_prefix = None;
            if key == Key::Char('y')
                && let Some(col) = board.columns.get(self.selected_col)
                && let Some(card) = col.cards.get(self.selected_card)
                && let Some(raw) = text.get(card.full_range.clone())
            {
                self.yank_register = Some(raw.to_string());
            }
            return KanbanIntent::None;
        }

        // Digits for count prefix
        if let Key::Char(c) = key
            && c.is_ascii_digit()
        {
            if self.count_prefix.is_none() && c == '0' {
                // '0' by itself is motion: jump to first column
                self.selected_col = 0;
                self.clamp_selection(board);
                return KanbanIntent::None;
            }
            let digit = (c as u8 - b'0') as usize;
            self.count_prefix = Some(self.count_prefix.unwrap_or(0) * 10 + digit);
            return KanbanIntent::None;
        }

        let count = self.count_prefix.take().unwrap_or(1);

        match key {
            // Motions
            Key::Char('h') | Key::Left => {
                self.selected_col = self.selected_col.saturating_sub(count);
                self.clamp_selection(board);
                KanbanIntent::None
            }
            Key::Char('l') | Key::Right => {
                let max_col = board.columns.len().saturating_sub(1);
                self.selected_col = (self.selected_col + count).min(max_col);
                self.clamp_selection(board);
                KanbanIntent::None
            }
            Key::Char('k') | Key::Up => {
                self.selected_card = self.selected_card.saturating_sub(count);
                KanbanIntent::None
            }
            Key::Char('j') | Key::Down => {
                let max_card = board
                    .columns
                    .get(self.selected_col)
                    .map_or(0, |c| c.cards.len().saturating_sub(1));
                self.selected_card = (self.selected_card + count).min(max_card);
                KanbanIntent::None
            }
            Key::Char('$') | Key::End => {
                self.selected_col = board.columns.len().saturating_sub(1);
                self.clamp_selection(board);
                KanbanIntent::None
            }
            Key::Char('G') => {
                let max_card = board
                    .columns
                    .get(self.selected_col)
                    .map_or(0, |c| c.cards.len().saturating_sub(1));
                self.selected_card = max_card;
                KanbanIntent::None
            }
            Key::Char('g') => {
                self.pending_g = true;
                KanbanIntent::None
            }
            Key::Char('d') => {
                self.pending_d = true;
                KanbanIntent::None
            }
            Key::Char('y') => {
                self.pending_y = true;
                KanbanIntent::None
            }

            // Card movement across columns and vertically
            Key::Char('H') => {
                if self.selected_col > 0
                    && let Some(tx) = plan_move_card_column(
                        text,
                        board,
                        self.selected_col,
                        self.selected_card,
                        self.selected_col - 1,
                    )
                {
                    let dest_cards = board.columns[self.selected_col - 1].cards.len();
                    self.selected_col -= 1;
                    self.selected_card = dest_cards;
                    return KanbanIntent::ApplyTx(tx);
                }
                KanbanIntent::None
            }
            Key::Char('L') => {
                if self.selected_col + 1 < board.columns.len()
                    && let Some(tx) = plan_move_card_column(
                        text,
                        board,
                        self.selected_col,
                        self.selected_card,
                        self.selected_col + 1,
                    )
                {
                    let dest_cards = board.columns[self.selected_col + 1].cards.len();
                    self.selected_col += 1;
                    self.selected_card = dest_cards;
                    return KanbanIntent::ApplyTx(tx);
                }
                KanbanIntent::None
            }
            Key::Char('J') => {
                if let Some(tx) =
                    plan_move_card_vertical(text, board, self.selected_col, self.selected_card, 1)
                {
                    self.selected_card += 1;
                    KanbanIntent::ApplyTx(tx)
                } else {
                    KanbanIntent::None
                }
            }
            Key::Char('K') => {
                if let Some(tx) =
                    plan_move_card_vertical(text, board, self.selected_col, self.selected_card, -1)
                {
                    self.selected_card = self.selected_card.saturating_sub(1);
                    KanbanIntent::ApplyTx(tx)
                } else {
                    KanbanIntent::None
                }
            }

            // Card status toggling
            Key::Char(' ') | Key::Char('x') => {
                if let Some(tx) =
                    plan_cycle_card_status(text, board, self.selected_col, self.selected_card)
                {
                    KanbanIntent::ApplyTx(tx)
                } else {
                    KanbanIntent::None
                }
            }
            Key::Char('X') => {
                if let Some(col) = board.columns.get(self.selected_col)
                    && let Some(card) = col.cards.get(self.selected_card)
                {
                    let new_status = card.status.toggle_done();
                    if let Some(tx) = plan_set_card_status(
                        text,
                        board,
                        self.selected_col,
                        self.selected_card,
                        new_status,
                    ) {
                        return KanbanIntent::ApplyTx(tx);
                    }
                }
                KanbanIntent::None
            }

            // Card creation and editing
            Key::Char('o') | Key::Char('a') => {
                let has_cards = board
                    .columns
                    .get(self.selected_col)
                    .is_some_and(|c| !c.cards.is_empty());
                let pos = if has_cards {
                    CardInsertPos::After(self.selected_card)
                } else {
                    CardInsertPos::End
                };
                self.mode = KanbanInputMode::CardInput {
                    col_idx: self.selected_col,
                    edit_card_idx: None,
                    insert_pos: pos,
                    buffer: String::new(),
                    wiki_ac: None,
                };
                KanbanIntent::None
            }
            Key::Char('O') => {
                let has_cards = board
                    .columns
                    .get(self.selected_col)
                    .is_some_and(|c| !c.cards.is_empty());
                let pos = if has_cards {
                    CardInsertPos::Before(self.selected_card)
                } else {
                    CardInsertPos::End
                };
                self.mode = KanbanInputMode::CardInput {
                    col_idx: self.selected_col,
                    edit_card_idx: None,
                    insert_pos: pos,
                    buffer: String::new(),
                    wiki_ac: None,
                };
                KanbanIntent::None
            }
            Key::Char('i') | Key::Char('e') | Key::Char('c') => {
                if let Some(col) = board.columns.get(self.selected_col)
                    && let Some(card) = col.cards.get(self.selected_card)
                {
                    self.mode = KanbanInputMode::CardInput {
                        col_idx: self.selected_col,
                        edit_card_idx: Some(self.selected_card),
                        insert_pos: CardInsertPos::Before(self.selected_card),
                        buffer: card.raw_title.clone(),
                        wiki_ac: None,
                    };
                    return KanbanIntent::None;
                }
                self.mode = KanbanInputMode::CardInput {
                    col_idx: self.selected_col,
                    edit_card_idx: None,
                    insert_pos: CardInsertPos::End,
                    buffer: String::new(),
                    wiki_ac: None,
                };
                KanbanIntent::None
            }

            // File picker
            Key::Char('f') => {
                if board
                    .columns
                    .get(self.selected_col)
                    .and_then(|c| c.cards.get(self.selected_card))
                    .is_some()
                {
                    self.mode = KanbanInputMode::FilePicker(FuzzyFilePicker::new(vault_files));
                }
                KanbanIntent::None
            }

            // Enter link jump or inline edit
            Key::Enter => {
                if let Some(col) = board.columns.get(self.selected_col)
                    && let Some(card) = col.cards.get(self.selected_card)
                {
                    if card.links.len() == 1 {
                        return KanbanIntent::OpenLink(card.links[0].navigation_target());
                    } else if card.links.len() > 1 {
                        self.mode = KanbanInputMode::LinkJumpPicker(LinkJumpPicker::new(
                            card.links.clone(),
                        ));
                        return KanbanIntent::None;
                    } else {
                        self.mode = KanbanInputMode::CardInput {
                            col_idx: self.selected_col,
                            edit_card_idx: Some(self.selected_card),
                            insert_pos: CardInsertPos::Before(self.selected_card),
                            buffer: card.raw_title.clone(),
                            wiki_ac: None,
                        };
                        return KanbanIntent::None;
                    }
                }
                KanbanIntent::None
            }

            // Clipboard paste
            Key::Char('p') => {
                if let Some(raw) = &self.yank_register {
                    let has_cards = board
                        .columns
                        .get(self.selected_col)
                        .is_some_and(|c| !c.cards.is_empty());
                    let pos = if has_cards {
                        CardInsertPos::After(self.selected_card)
                    } else {
                        CardInsertPos::End
                    };
                    if let Some(tx) = plan_paste_card(text, board, self.selected_col, pos, raw) {
                        return KanbanIntent::ApplyTx(tx);
                    }
                }
                KanbanIntent::None
            }
            Key::Char('P') => {
                if let Some(raw) = &self.yank_register {
                    let has_cards = board
                        .columns
                        .get(self.selected_col)
                        .is_some_and(|c| !c.cards.is_empty());
                    let pos = if has_cards {
                        CardInsertPos::Before(self.selected_card)
                    } else {
                        CardInsertPos::End
                    };
                    if let Some(tx) = plan_paste_card(text, board, self.selected_col, pos, raw) {
                        return KanbanIntent::ApplyTx(tx);
                    }
                }
                KanbanIntent::None
            }

            // History undo/redo
            Key::Char('u') => KanbanIntent::Undo,
            Key::Ctrl('r') => KanbanIntent::Redo,

            // Column operations
            Key::Char('A') => {
                self.mode = KanbanInputMode::ColumnInput {
                    rename_col_idx: None,
                    buffer: String::new(),
                };
                KanbanIntent::None
            }
            Key::Char('R') => {
                if let Some(col) = board.columns.get(self.selected_col) {
                    self.mode = KanbanInputMode::ColumnInput {
                        rename_col_idx: Some(self.selected_col),
                        buffer: col.title.clone(),
                    };
                }
                KanbanIntent::None
            }
            Key::Char('<') => {
                if self.selected_col > 0
                    && let Some(tx) = plan_move_column(text, board, self.selected_col, -1)
                {
                    self.selected_col -= 1;
                    return KanbanIntent::ApplyTx(tx);
                }
                KanbanIntent::None
            }
            Key::Char('>') => {
                if self.selected_col + 1 < board.columns.len()
                    && let Some(tx) = plan_move_column(text, board, self.selected_col, 1)
                {
                    self.selected_col += 1;
                    return KanbanIntent::ApplyTx(tx);
                }
                KanbanIntent::None
            }
            Key::Char('D') => {
                if let Some(tx) = plan_delete_empty_column(text, board, self.selected_col) {
                    if self.selected_col > 0 && self.selected_col + 1 >= board.columns.len() {
                        self.selected_col -= 1;
                    }
                    KanbanIntent::ApplyTx(tx)
                } else {
                    KanbanIntent::Status("Cannot delete non-empty column".to_string())
                }
            }

            // Filter search
            Key::Char('/') => {
                self.mode = KanbanInputMode::FilterSearch;
                self.search_query.clear();
                KanbanIntent::None
            }

            // View and overlays
            Key::Tab => KanbanIntent::SwitchBoard { next: true },
            Key::ShiftTab => KanbanIntent::SwitchBoard { next: false },
            Key::Esc | Key::Char('q') => KanbanIntent::CloseOverlay,

            _ => KanbanIntent::None,
        }
    }

    fn handle_link_jump(&mut self, board: &KanbanBoard) -> KanbanIntent {
        if let Some(col) = board.columns.get(self.selected_col)
            && let Some(card) = col.cards.get(self.selected_card)
        {
            if card.links.len() == 1 {
                return KanbanIntent::OpenLink(card.links[0].navigation_target());
            } else if card.links.len() > 1 {
                self.mode =
                    KanbanInputMode::LinkJumpPicker(LinkJumpPicker::new(card.links.clone()));
                return KanbanIntent::None;
            } else {
                return KanbanIntent::Status("No link on current card".to_string());
            }
        }
        KanbanIntent::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use truff_core::markdown::kanban::KanbanBoard;
    use truff_core::vim::Key;

    const SAMPLE_BOARD: &str = "\
<!-- truff:kanban -->
# Проект

## План
- [ ] Задача 0.0 [[docs/one]]
- [ ] Задача 0.1 [[docs/two|Два]] [[notes/extra]]
- [ ] Задача 0.2
- [ ] Задача 0.3

## В работе
- [/] В процессе 1.0
- [ ] В процессе 1.1

## Готово
- [x] Готово 2.0
- [x] Готово 2.1
- [x] Готово 2.2

## Пустая
";

    #[test]
    fn test_vim_motions_with_counts() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        assert_eq!(state.selected_col, 0);
        assert_eq!(state.selected_card, 0);

        // 3j -> card 3 in col 0
        state.feed_key(Key::Char('3'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('j'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 0);
        assert_eq!(state.selected_card, 3);

        // 2l -> col 2, card clamped to 2 (max card in col 2 is 2)
        state.feed_key(Key::Char('2'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('l'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 2);
        assert_eq!(state.selected_card, 2);

        // h -> col 1, card clamped to 1 (max in col 1 is 1)
        state.feed_key(Key::Char('h'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 1);
        assert_eq!(state.selected_card, 1);

        // k -> col 1, card 0
        state.feed_key(Key::Char('k'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 1);
        assert_eq!(state.selected_card, 0);

        // G -> bottom of col 1 (card 1)
        state.feed_key(Key::Char('G'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 1);
        assert_eq!(state.selected_card, 1);

        // gg -> top of col 1 (card 0)
        state.feed_key(Key::Char('g'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('g'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 1);
        assert_eq!(state.selected_card, 0);

        // $ -> last column (col 3: Пустая)
        state.feed_key(Key::Char('$'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 3);
        assert_eq!(state.selected_card, 0);

        // 0 -> first column (col 0)
        state.feed_key(Key::Char('0'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 0);
    }

    #[test]
    fn test_cyrillic_layout_equivalents() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // о = j -> card 1
        state.feed_key(Key::Char('о'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_card, 1);

        // л = k -> card 0
        state.feed_key(Key::Char('л'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_card, 0);

        // д = l -> col 1
        state.feed_key(Key::Char('д'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 1);

        // р = h -> col 0
        state.feed_key(Key::Char('р'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.selected_col, 0);

        // О = J -> move card vertical down (emits ApplyTx)
        let intent = state.feed_key(Key::Char('О'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // Л = K -> move card vertical up (emits ApplyTx)
        state.selected_card = 1;
        let intent = state.feed_key(Key::Char('Л'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // Д = L -> move card right col (emits ApplyTx)
        let intent = state.feed_key(Key::Char('Д'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // Р = H -> move card left col (emits ApplyTx)
        state.selected_col = 1;
        state.selected_card = 0;
        let intent = state.feed_key(Key::Char('Р'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
    }

    #[test]
    fn test_card_movement_h_l_j_k() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // L on col 0 card 0 -> moves to col 1
        let intent = state.feed_key(Key::Char('L'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // H on col 1 card 0 -> moves to col 0
        state.selected_col = 1;
        state.selected_card = 0;
        let intent = state.feed_key(Key::Char('H'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // J on col 0 card 0 -> moves card down to card 1
        state.selected_col = 0;
        state.selected_card = 0;
        let intent = state.feed_key(Key::Char('J'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // K on col 0 card 1 -> moves card up to card 0
        state.selected_card = 1;
        let intent = state.feed_key(Key::Char('K'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
    }

    #[test]
    fn test_card_status_toggling() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // Space -> cycles status
        let intent = state.feed_key(Key::Char(' '), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // x -> cycles status
        let intent = state.feed_key(Key::Char('x'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // X -> toggle done
        let intent = state.feed_key(Key::Char('X'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
    }

    #[test]
    fn test_card_creation_and_editing() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // o -> enters CardInput mode after current card
        state.feed_key(Key::Char('o'), SAMPLE_BOARD, &board, &files);
        match &state.mode {
            KanbanInputMode::CardInput {
                edit_card_idx,
                insert_pos,
                ..
            } => {
                assert_eq!(*edit_card_idx, None);
                assert_eq!(*insert_pos, CardInsertPos::After(0));
            }
            _ => panic!("Expected CardInput mode"),
        }

        // Type characters
        state.feed_key(Key::Char('N'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('e'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('w'), SAMPLE_BOARD, &board, &files);

        // Enter -> commits transaction
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.mode, KanbanInputMode::Normal);

        // O -> enters CardInput mode before current card
        state.feed_key(Key::Char('O'), SAMPLE_BOARD, &board, &files);
        match &state.mode {
            KanbanInputMode::CardInput {
                edit_card_idx,
                insert_pos,
                ..
            } => {
                assert_eq!(*edit_card_idx, None);
                assert_eq!(*insert_pos, CardInsertPos::Before(0));
            }
            _ => panic!("Expected CardInput mode"),
        }
        state.feed_key(Key::Esc, SAMPLE_BOARD, &board, &files);
        assert_eq!(state.mode, KanbanInputMode::Normal);

        // e -> enters CardInput editing existing card
        state.feed_key(Key::Char('e'), SAMPLE_BOARD, &board, &files);
        match &state.mode {
            KanbanInputMode::CardInput {
                edit_card_idx,
                buffer,
                ..
            } => {
                assert_eq!(*edit_card_idx, Some(0));
                assert!(buffer.contains("Задача 0.0"));
            }
            _ => panic!("Expected CardInput mode"),
        }
        // Commit edit
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.mode, KanbanInputMode::Normal);
    }

    #[test]
    fn test_file_picker_workflow() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files = vec![
            "docs/architecture.md".to_string(),
            "notes/daily.md".to_string(),
            "docs/api.md".to_string(),
        ];

        // f -> opens FilePicker
        state.feed_key(Key::Char('f'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(state.mode, KanbanInputMode::FilePicker(_)));

        // Type query "arch"
        state.feed_key(Key::Char('a'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('r'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('c'), SAMPLE_BOARD, &board, &files);
        state.feed_key(Key::Char('h'), SAMPLE_BOARD, &board, &files);

        // Enter -> commits plan_attach_file_link
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.mode, KanbanInputMode::Normal);
    }

    #[test]
    fn test_link_jump_workflow() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // Card 0.0 has 1 link: [[docs/one]]
        state.selected_col = 0;
        state.selected_card = 0;
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::OpenLink("docs/one".to_string()));

        // Card 0.0 with gf -> OpenLink
        let _ = state.feed_key(Key::Char('g'), SAMPLE_BOARD, &board, &files);
        let intent = state.feed_key(Key::Char('f'), SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::OpenLink("docs/one".to_string()));

        // Card 0.1 has 2 links: [[docs/two|Два]] and [[notes/extra]]
        state.selected_card = 1;
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::None);
        match &state.mode {
            KanbanInputMode::LinkJumpPicker(picker) => {
                assert_eq!(picker.links.len(), 2);
            }
            _ => panic!("Expected LinkJumpPicker mode"),
        }

        // Quick jump with key '2'
        let intent = state.feed_key(Key::Char('2'), SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::OpenLink("notes/extra".to_string()));
        assert_eq!(state.mode, KanbanInputMode::Normal);

        // Card 0.2 has 0 links -> Enter enters edit mode
        state.selected_card = 2;
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::None);
        assert!(matches!(state.mode, KanbanInputMode::CardInput { .. }));
        state.feed_key(Key::Esc, SAMPLE_BOARD, &board, &files);

        // Card 0.2 has 0 links -> gf gives status
        let _ = state.feed_key(Key::Char('g'), SAMPLE_BOARD, &board, &files);
        let intent = state.feed_key(Key::Char('f'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::Status(_)));
    }

    #[test]
    fn test_clipboard_and_history() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // dd -> cut card
        state.feed_key(Key::Char('d'), SAMPLE_BOARD, &board, &files);
        assert!(state.pending_d);
        let intent = state.feed_key(Key::Char('d'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert!(!state.pending_d);
        assert!(state.yank_register.is_some());

        // p -> paste after
        let intent = state.feed_key(Key::Char('p'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // P -> paste before
        let intent = state.feed_key(Key::Char('P'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // yy -> yank
        state.feed_key(Key::Char('y'), SAMPLE_BOARD, &board, &files);
        assert!(state.pending_y);
        state.feed_key(Key::Char('y'), SAMPLE_BOARD, &board, &files);
        assert!(!state.pending_y);
        assert!(state.yank_register.as_ref().unwrap().contains("Задача"));

        // u -> undo
        let intent = state.feed_key(Key::Char('u'), SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::Undo);

        // Ctrl-r -> redo
        let intent = state.feed_key(Key::Ctrl('r'), SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::Redo);
    }

    #[test]
    fn test_columns_operations() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // A -> add column
        state.feed_key(Key::Char('A'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(
            state.mode,
            KanbanInputMode::ColumnInput {
                rename_col_idx: None,
                ..
            }
        ));
        state.feed_key(Key::Char('T'), SAMPLE_BOARD, &board, &files);
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.mode, KanbanInputMode::Normal);

        // R -> rename column
        state.feed_key(Key::Char('R'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(
            state.mode,
            KanbanInputMode::ColumnInput {
                rename_col_idx: Some(0),
                ..
            }
        ));
        let intent = state.feed_key(Key::Enter, SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.mode, KanbanInputMode::Normal);

        // > -> move column right
        let intent = state.feed_key(Key::Char('>'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // < -> move column left
        state.selected_col = 1;
        let intent = state.feed_key(Key::Char('<'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));

        // D on empty column (col 3: Пустая) -> deletes column and clamps selected_col
        state.selected_col = 3;
        let intent = state.feed_key(Key::Char('D'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.selected_col, 2);

        // D on non-empty column (col 0) -> rejected with Status message
        state.selected_col = 0;
        let intent = state.feed_key(Key::Char('D'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::Status(_)));
    }

    #[test]
    fn test_dd_last_card_clamping() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // Col 1 has 2 cards (idx 0 and 1). Select last card (idx 1).
        state.selected_col = 1;
        state.selected_card = 1;
        state.feed_key(Key::Char('d'), SAMPLE_BOARD, &board, &files);
        let intent = state.feed_key(Key::Char('d'), SAMPLE_BOARD, &board, &files);
        assert!(matches!(intent, KanbanIntent::ApplyTx(_)));
        assert_eq!(state.selected_card, 0);
    }

    #[test]
    fn test_esc_cancels_count_and_pending() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // Count prefix + Esc -> cancels count, does not close overlay
        state.feed_key(Key::Char('3'), SAMPLE_BOARD, &board, &files);
        assert_eq!(state.count_prefix, Some(3));
        let intent = state.feed_key(Key::Esc, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::None);
        assert_eq!(state.count_prefix, None);

        // Pending 'g' + Esc -> cancels pending, does not close overlay
        state.feed_key(Key::Char('g'), SAMPLE_BOARD, &board, &files);
        assert!(state.pending_g);
        let intent = state.feed_key(Key::Esc, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::None);
        assert!(!state.pending_g);

        // Normal mode with no prefix + Esc -> CloseOverlay
        let intent = state.feed_key(Key::Esc, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::CloseOverlay);
    }

    #[test]
    fn test_view_and_overlay() {
        let board = KanbanBoard::parse(SAMPLE_BOARD);
        let mut state = KanbanBoardState::new();
        let files: Vec<String> = vec![];

        // gs -> ToggleSourceView
        state.feed_key(Key::Char('g'), SAMPLE_BOARD, &board, &files);
        assert!(state.pending_g);
        let intent = state.feed_key(Key::Char('s'), SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::ToggleSourceView);
        assert!(!state.pending_g);

        // Tab -> SwitchBoard next
        let intent = state.feed_key(Key::Tab, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::SwitchBoard { next: true });

        // ShiftTab -> SwitchBoard prev
        let intent = state.feed_key(Key::ShiftTab, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::SwitchBoard { next: false });

        // Esc -> CloseOverlay
        let intent = state.feed_key(Key::Esc, SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::CloseOverlay);

        // q -> CloseOverlay
        let intent = state.feed_key(Key::Char('q'), SAMPLE_BOARD, &board, &files);
        assert_eq!(intent, KanbanIntent::CloseOverlay);
    }
}
