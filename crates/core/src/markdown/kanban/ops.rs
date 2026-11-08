use crate::markdown::kanban::parser::{KanbanBoard, KanbanTaskStatus};
use crate::text::{SingleEdit, Transaction, UndoGroup};

/// Position where a new card should be inserted relative to existing cards in a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardInsertPos {
    Before(usize),
    After(usize),
    End,
}

/// Detects whether text uses CRLF or LF line endings.
pub fn detect_line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Generates a standard Markdown Kanban template with default columns.
pub fn default_kanban_template(title: &str, line_ending: &str) -> String {
    let nl = if line_ending.is_empty() {
        "\n"
    } else {
        line_ending
    };
    let t = if title.trim().is_empty() {
        "Канбан"
    } else {
        title.trim()
    };
    format!("<!-- truff:kanban -->{nl}# {t}{nl}{nl}## План{nl}{nl}## В работе{nl}{nl}## Готово{nl}")
}

/// Plans an addition of a card to a column.
///
/// If `board.columns` is empty, generates the default Kanban template with the card placed
/// in the "План" column; otherwise inserts `- [ ] {card_text}{nl}` at the target position.
pub fn plan_add_card(
    text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    pos: CardInsertPos,
    card_text: &str,
) -> Option<Transaction> {
    let nl = detect_line_ending(text);
    let clean_card_title = card_text.trim_end_matches(['\r', '\n']);

    if board.columns.is_empty() {
        let title = board.title.as_deref().unwrap_or("Канбан");
        let formatted_card = if clean_card_title.starts_with("- [") {
            format!("{clean_card_title}{nl}")
        } else {
            format!("- [ ] {clean_card_title}{nl}")
        };
        let content = format!(
            "<!-- truff:kanban -->{nl}# {title}{nl}{nl}## План{nl}{formatted_card}{nl}## В работе{nl}{nl}## Готово{nl}"
        );
        return Some(
            Transaction::replace(0..text.len(), content).with_undo_group(UndoGroup::next()),
        );
    }

    let col = board.columns.get(col_idx)?;
    let insert_pos = match pos {
        CardInsertPos::Before(idx) => {
            if idx < col.cards.len() {
                col.cards[idx].full_range.start
            } else if col.cards.is_empty() {
                col.insert_card_offset
            } else {
                col.cards.last().unwrap().full_range.end
            }
        }
        CardInsertPos::After(idx) => {
            if idx < col.cards.len() {
                col.cards[idx].full_range.end
            } else if col.cards.is_empty() {
                col.insert_card_offset
            } else {
                col.cards.last().unwrap().full_range.end
            }
        }
        CardInsertPos::End => col.insert_card_offset,
    };

    let card_line = if clean_card_title.starts_with("- [") {
        format!("{clean_card_title}{nl}")
    } else {
        format!("- [ ] {clean_card_title}{nl}")
    };

    Some(Transaction::insert(insert_pos, card_line).with_undo_group(UndoGroup::next()))
}

/// Plans editing a card's title.
pub fn plan_edit_card_title(
    _text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    card_idx: usize,
    new_title: &str,
) -> Option<Transaction> {
    let col = board.columns.get(col_idx)?;
    let card = col.cards.get(card_idx)?;
    Some(
        Transaction::replace(card.title_range.clone(), new_title)
            .with_undo_group(UndoGroup::next()),
    )
}

/// Plans setting a card's task status marker.
pub fn plan_set_card_status(
    _text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    card_idx: usize,
    status: KanbanTaskStatus,
) -> Option<Transaction> {
    let col = board.columns.get(col_idx)?;
    let card = col.cards.get(card_idx)?;
    let range = card.status_char_byte..card.status_char_byte + 1;
    let marker = status.marker_char().to_string();
    Some(Transaction::replace(range, marker).with_undo_group(UndoGroup::next()))
}

/// Plans cycling a card's status (Todo -> InProgress -> Done -> Todo).
pub fn plan_cycle_card_status(
    text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    card_idx: usize,
) -> Option<Transaction> {
    let col = board.columns.get(col_idx)?;
    let card = col.cards.get(card_idx)?;
    let new_status = card.status.cycle();
    plan_set_card_status(text, board, col_idx, card_idx, new_status)
}

/// Plans deleting a card and returns the transaction alongside the yanked raw card text.
pub fn plan_delete_card(
    text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    card_idx: usize,
) -> Option<(Transaction, String)> {
    let col = board.columns.get(col_idx)?;
    let card = col.cards.get(card_idx)?;
    let raw_card = text.get(card.full_range.clone())?.to_string();
    let tx = Transaction::delete(card.full_range.clone()).with_undo_group(UndoGroup::next());
    Some((tx, raw_card))
}

/// Plans pasting a yanked raw card text block at the target column and position.
pub fn plan_paste_card(
    text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    pos: CardInsertPos,
    raw_card: &str,
) -> Option<Transaction> {
    if raw_card.is_empty() {
        return None;
    }
    let col = board.columns.get(col_idx)?;
    let nl = detect_line_ending(text);

    let insert_pos = match pos {
        CardInsertPos::Before(idx) => {
            if idx < col.cards.len() {
                col.cards[idx].full_range.start
            } else if col.cards.is_empty() {
                col.insert_card_offset
            } else {
                col.cards.last().unwrap().full_range.end
            }
        }
        CardInsertPos::After(idx) => {
            if idx < col.cards.len() {
                col.cards[idx].full_range.end
            } else if col.cards.is_empty() {
                col.insert_card_offset
            } else {
                col.cards.last().unwrap().full_range.end
            }
        }
        CardInsertPos::End => col.insert_card_offset,
    };

    let trimmed = raw_card.trim_end_matches(['\r', '\n']);
    let normalized = format!("{trimmed}{nl}");

    Some(Transaction::insert(insert_pos, normalized).with_undo_group(UndoGroup::next()))
}

/// Plans moving a card vertically (up: -1, down: +1) within its column.
pub fn plan_move_card_vertical(
    text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    card_idx: usize,
    direction: isize,
) -> Option<Transaction> {
    if direction == 0 {
        return None;
    }
    let col = board.columns.get(col_idx)?;
    let target_idx = (card_idx as isize) + direction;
    if target_idx < 0 || target_idx as usize >= col.cards.len() {
        return None;
    }
    let target_idx = target_idx as usize;
    let idx_a = card_idx.min(target_idx);
    let idx_b = card_idx.max(target_idx);

    let card_a = &col.cards[idx_a];
    let card_b = &col.cards[idx_b];
    let raw_a = text.get(card_a.full_range.clone())?;
    let raw_b = text.get(card_b.full_range.clone())?;

    let tx = Transaction::new()
        .with_edit(SingleEdit::replace(card_a.full_range.clone(), raw_b))
        .with_edit(SingleEdit::replace(card_b.full_range.clone(), raw_a))
        .with_undo_group(UndoGroup::next());
    Some(tx)
}

/// Plans moving a card from one column to another.
///
/// Uses sorted disjoint `SingleEdit`s so `Buffer::apply` validates without error.
pub fn plan_move_card_column(
    text: &str,
    board: &KanbanBoard,
    from_col: usize,
    card_idx: usize,
    to_col: usize,
) -> Option<Transaction> {
    if from_col == to_col {
        return None;
    }
    let src_col = board.columns.get(from_col)?;
    let dest_col = board.columns.get(to_col)?;
    let card = src_col.cards.get(card_idx)?;

    let nl = detect_line_ending(text);
    let raw_card = text.get(card.full_range.clone())?;
    let mut raw_card = raw_card.to_string();
    if !raw_card.ends_with('\n') {
        raw_card.push_str(nl);
    }

    let edit_del = SingleEdit::delete(card.full_range.clone());
    let edit_ins = SingleEdit::insert(dest_col.insert_card_offset, raw_card);

    let edits = if edit_del.range.start <= edit_ins.range.start {
        vec![edit_del, edit_ins]
    } else {
        vec![edit_ins, edit_del]
    };

    Some(Transaction {
        edits,
        cursor: None,
        undo_group: Some(UndoGroup::next()),
    })
}

/// Plans attaching a file link to a card title (e.g. `[[path]]`).
///
/// Strips `.md` extension from `file_path`. If the card already links to this target,
/// returns `None`.
pub fn plan_attach_file_link(
    _text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    card_idx: usize,
    file_path: &str,
) -> Option<Transaction> {
    let col = board.columns.get(col_idx)?;
    let card = col.cards.get(card_idx)?;

    let clean_path = file_path.strip_suffix(".md").unwrap_or(file_path);
    if card.links.iter().any(|link| link.target == clean_path) {
        return None;
    }

    let link_text = if card.raw_title.trim().is_empty() || card.raw_title.ends_with(' ') {
        format!("[[{clean_path}]]")
    } else {
        format!(" [[{clean_path}]]")
    };

    Some(Transaction::insert(card.title_range.end, link_text).with_undo_group(UndoGroup::next()))
}

/// Plans adding a new column to the Kanban board.
pub fn plan_add_column(text: &str, board: &KanbanBoard, title: &str) -> Option<Transaction> {
    let clean_title = title.trim();
    if clean_title.is_empty() {
        return None;
    }
    let nl = detect_line_ending(text);

    if board.columns.is_empty() && text.trim().is_empty() {
        let t = board.title.as_deref().unwrap_or("Канбан");
        let content = format!("<!-- truff:kanban -->{nl}# {t}{nl}{nl}## {clean_title}{nl}");
        return Some(
            Transaction::replace(0..text.len(), content).with_undo_group(UndoGroup::next()),
        );
    }

    let col_text = if text.ends_with("\n\n") || text.ends_with("\r\n\r\n") {
        format!("## {clean_title}{nl}")
    } else if text.ends_with('\n') {
        format!("{nl}## {clean_title}{nl}")
    } else if text.is_empty() {
        format!("## {clean_title}{nl}")
    } else {
        format!("{nl}{nl}## {clean_title}{nl}")
    };

    Some(Transaction::insert(text.len(), col_text).with_undo_group(UndoGroup::next()))
}

/// Plans renaming a column.
pub fn plan_rename_column(
    _text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    new_title: &str,
) -> Option<Transaction> {
    let clean_title = new_title.trim();
    if clean_title.is_empty() {
        return None;
    }
    let col = board.columns.get(col_idx)?;
    Some(
        Transaction::replace(col.title_range.clone(), clean_title)
            .with_undo_group(UndoGroup::next()),
    )
}

/// Plans deleting an empty column.
///
/// Returns `None` if the column contains any cards or `col_idx` is out of bounds.
pub fn plan_delete_empty_column(
    _text: &str,
    board: &KanbanBoard,
    col_idx: usize,
) -> Option<Transaction> {
    let col = board.columns.get(col_idx)?;
    if !col.cards.is_empty() || col.section_range.is_empty() {
        return None;
    }
    Some(Transaction::delete(col.section_range.clone()).with_undo_group(UndoGroup::next()))
}

/// Plans moving a column horizontally (left: -1, right: +1).
pub fn plan_move_column(
    text: &str,
    board: &KanbanBoard,
    col_idx: usize,
    direction: isize,
) -> Option<Transaction> {
    if direction == 0 {
        return None;
    }
    let target_idx = (col_idx as isize) + direction;
    if target_idx < 0 || target_idx as usize >= board.columns.len() {
        return None;
    }
    let target_idx = target_idx as usize;
    let idx_a = col_idx.min(target_idx);
    let idx_b = col_idx.max(target_idx);

    let col_a = &board.columns[idx_a];
    let col_b = &board.columns[idx_b];
    let nl = detect_line_ending(text);

    let raw_a = text.get(col_a.section_range.clone())?;
    let raw_b = text.get(col_b.section_range.clone())?;

    let mut raw_a = raw_a.to_string();
    if !raw_a.ends_with('\n') {
        raw_a.push_str(nl);
    }
    let mut raw_b = raw_b.to_string();
    if !raw_b.ends_with('\n') {
        raw_b.push_str(nl);
    }

    let tx = Transaction::new()
        .with_edit(SingleEdit::replace(col_a.section_range.clone(), raw_b))
        .with_edit(SingleEdit::replace(col_b.section_range.clone(), raw_a))
        .with_undo_group(UndoGroup::next());
    Some(tx)
}
