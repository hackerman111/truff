use truff_core::markdown::kanban::{
    CardInsertPos, KanbanBoard, KanbanTaskStatus, default_kanban_template, is_kanban_document,
    plan_add_card, plan_add_column, plan_attach_file_link, plan_cycle_card_status,
    plan_delete_card, plan_delete_empty_column, plan_edit_card_title, plan_move_card_column,
    plan_move_card_vertical, plan_move_column, plan_paste_card, plan_rename_column,
    plan_set_card_status,
};
use truff_core::{Buffer, BufferId};

#[test]
fn detects_kanban_by_path_or_marker() {
    assert!(is_kanban_document(Some("roadmap.kanban.md"), "# Roadmap\n"));
    assert!(is_kanban_document(Some("kanban.md"), ""));
    assert!(is_kanban_document(Some("projects/kanban.md"), "# Sprint\n"));
    assert!(is_kanban_document(
        Some("notes/plan.md"),
        "<!-- truff:kanban -->\n# Plan\n"
    ));
    assert!(!is_kanban_document(
        Some("notes/daily.md"),
        "## Todo\n- [ ] Milk\n"
    ));
}

#[test]
fn parses_columns_cards_links_tags_and_continuations() {
    let md = "\
<!-- truff:kanban -->
# Проект

Вводный абзац, который нельзя потерять.

## План
- [ ] Сделать парсер [[core/parser|Парсер]] #core #urgent
  Подробности реализации
- [/] Проверить граничные случаи [[docs/spec]]

## Готово
- [x] Согласовать дизайн
";
    let board = KanbanBoard::parse(md);
    assert_eq!(board.title.as_deref(), Some("Проект"));
    assert_eq!(board.columns.len(), 2);
    assert_eq!(board.columns[0].title, "План");
    assert_eq!(board.columns[0].cards.len(), 2);

    let c0 = &board.columns[0].cards[0];
    assert_eq!(c0.status, KanbanTaskStatus::Todo);
    assert_eq!(c0.clean_title, "Сделать парсер");
    assert_eq!(c0.links.len(), 1);
    assert_eq!(c0.links[0].target, "core/parser");
    assert_eq!(c0.links[0].display(), "Парсер");
    assert_eq!(c0.tags, vec!["core", "urgent"]);
    assert_eq!(c0.body_lines, vec!["Подробности реализации"]);

    let c1 = &board.columns[0].cards[1];
    assert_eq!(c1.status, KanbanTaskStatus::InProgress);
    assert_eq!(c1.links[0].target, "docs/spec");
    assert_eq!(c1.links[0].display(), "spec");

    assert_eq!(board.columns[1].title, "Готово");
    assert_eq!(board.columns[1].cards[0].status, KanbanTaskStatus::Done);
}

#[test]
fn ignores_fenced_code_blocks() {
    let md = "\
# Board

```markdown
## Not a Column
- [ ] Not a Card
```

## Real Column
- [ ] Real Card
";
    let board = KanbanBoard::parse(md);
    assert_eq!(board.columns.len(), 1);
    assert_eq!(board.columns[0].title, "Real Column");
    assert_eq!(board.columns[0].cards.len(), 1);
    assert_eq!(board.columns[0].cards[0].clean_title, "Real Card");
}

#[test]
fn test_task_status_methods() {
    assert_eq!(KanbanTaskStatus::Todo.marker_char(), ' ');
    assert_eq!(KanbanTaskStatus::InProgress.marker_char(), '/');
    assert_eq!(KanbanTaskStatus::Done.marker_char(), 'x');

    assert_eq!(KanbanTaskStatus::Todo.cycle(), KanbanTaskStatus::InProgress);
    assert_eq!(KanbanTaskStatus::InProgress.cycle(), KanbanTaskStatus::Done);
    assert_eq!(KanbanTaskStatus::Done.cycle(), KanbanTaskStatus::Todo);

    assert_eq!(KanbanTaskStatus::Todo.toggle_done(), KanbanTaskStatus::Done);
    assert_eq!(
        KanbanTaskStatus::InProgress.toggle_done(),
        KanbanTaskStatus::Done
    );
    assert_eq!(KanbanTaskStatus::Done.toggle_done(), KanbanTaskStatus::Todo);
}

#[test]
fn test_issue_numbers_and_heading_links() {
    let md = "\
## Column
- [ ] Fix issue #42 and #100 with [[#Subheading]]
";
    let board = KanbanBoard::parse(md);
    let card = &board.columns[0].cards[0];
    assert_eq!(card.clean_title, "Fix issue #42 and #100 with");
    assert!(
        card.tags.is_empty(),
        "Numeric #42 must not be parsed as a tag"
    );
    assert_eq!(card.links.len(), 1);
    assert_eq!(card.links[0].display(), "Subheading");
}

#[test]
fn range_transactions_preserve_non_card_markdown_and_support_undo_redo() {
    let initial = "\
<!-- truff:kanban -->
# Доска

Заметка сверху.

## План
- [ ] Первая
  Детали первой
- [ ] Вторая

Комментарий внизу колонки.

## В работе
- [/] Активная
";
    let mut buf = Buffer::from_text(BufferId::new(1), initial).unwrap();

    // 1. Move "Первая" (with its continuation line) right to "В работе"
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_move_card_column(&s, &board, 0, 0, 1).expect("move right");
    buf.apply(tx).unwrap();

    let after_move = buf.as_str();
    assert!(after_move.contains("Заметка сверху."));
    assert!(after_move.contains("Комментарий внизу колонки."));
    let reparsed = KanbanBoard::parse(&after_move);
    assert_eq!(reparsed.columns[0].cards.len(), 1);
    assert_eq!(reparsed.columns[0].cards[0].clean_title, "Вторая");
    assert_eq!(reparsed.columns[1].cards.len(), 2);
    assert_eq!(reparsed.columns[1].cards[1].clean_title, "Первая");
    assert_eq!(
        reparsed.columns[1].cards[1].body_lines,
        vec!["Детали первой"]
    );

    // 2. Attach file link to "Вторая"
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_attach_file_link(&s, &board, 0, 0, "notes/arch.md").expect("attach link");
    buf.apply(tx).unwrap();
    let linked = KanbanBoard::parse(&buf.as_str());
    assert_eq!(linked.columns[0].cards[0].links[0].target, "notes/arch");

    // 3. Reorder inside "В работе" (move index 1 up to 0)
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_move_card_vertical(&s, &board, 1, 1, -1).expect("move up");
    buf.apply(tx).unwrap();
    let reordered = KanbanBoard::parse(&buf.as_str());
    assert_eq!(reordered.columns[1].cards[0].clean_title, "Первая");
    assert_eq!(reordered.columns[1].cards[1].clean_title, "Активная");

    // 4. Undo all 3 steps cleanly back to exact initial text
    assert!(buf.undo().unwrap().is_some());
    assert!(buf.undo().unwrap().is_some());
    assert!(buf.undo().unwrap().is_some());
    assert_eq!(buf.as_str(), initial);

    // Redo restores edit
    assert!(buf.redo().unwrap().is_some());
}

#[test]
fn crlf_and_empty_board_initialization_work() {
    let mut buf = Buffer::from_text(BufferId::new(1), "").unwrap();
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_add_card(&s, &board, 0, CardInsertPos::End, "Задача [[readme]]").unwrap();
    buf.apply(tx).unwrap();
    let parsed = KanbanBoard::parse(&buf.as_str());
    assert_eq!(parsed.columns.len(), 3);
    assert_eq!(parsed.columns[0].cards[0].links[0].target, "readme");

    let crlf = "<!-- truff:kanban -->\r\n## Todo\r\n- [ ] One\r\n- [ ] Two\r\n";
    let mut crlf_buf = Buffer::from_text(BufferId::new(2), crlf).unwrap();
    let s = crlf_buf.as_str();
    let b = KanbanBoard::parse(&s);
    let tx = plan_cycle_card_status(&s, &b, 0, 0).unwrap();
    crlf_buf.apply(tx).unwrap();
    assert_eq!(
        crlf_buf.as_str(),
        "<!-- truff:kanban -->\r\n## Todo\r\n- [/] One\r\n- [ ] Two\r\n"
    );
}

#[test]
fn column_operations_add_rename_move_delete() {
    let initial = "\
<!-- truff:kanban -->
# Доска

## План
- [ ] Задача 1

## В работе
";
    let mut buf = Buffer::from_text(BufferId::new(1), initial).unwrap();

    // 1. Add column "Готово"
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_add_column(&s, &board, "Готово").expect("add column");
    buf.apply(tx).unwrap();
    let b1 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b1.columns.len(), 3);
    assert_eq!(b1.columns[2].title, "Готово");

    // 2. Rename column 0 ("План" -> "Бэклог")
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_rename_column(&s, &board, 0, "Бэклог").expect("rename column");
    buf.apply(tx).unwrap();
    let b2 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b2.columns[0].title, "Бэклог");
    assert_eq!(b2.columns[0].cards.len(), 1);

    // 3. Delete non-empty column 0 should fail (return None)
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    assert!(plan_delete_empty_column(&s, &board, 0).is_none());

    // 4. Delete empty column 1 ("В работе")
    let tx = plan_delete_empty_column(&s, &board, 1).expect("delete empty column");
    buf.apply(tx).unwrap();
    let b3 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b3.columns.len(), 2);
    assert_eq!(b3.columns[0].title, "Бэклог");
    assert_eq!(b3.columns[1].title, "Готово");

    // 5. Move column 1 left (-1) to swap "Бэклог" and "Готово"
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_move_column(&s, &board, 1, -1).expect("move column left");
    buf.apply(tx).unwrap();
    let b4 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b4.columns[0].title, "Готово");
    assert_eq!(b4.columns[1].title, "Бэклог");
    assert_eq!(b4.columns[1].cards[0].clean_title, "Задача 1");

    // 6. Undo all steps back to initial
    for _ in 0..4 {
        assert!(buf.undo().unwrap().is_some());
    }
    assert_eq!(buf.as_str(), initial);
}

#[test]
fn delete_edit_set_status_and_paste_card_operations() {
    let initial = "\
<!-- truff:kanban -->
## План
- [ ] Первая
  Контекст
- [ ] Вторая

## Готово
";
    let mut buf = Buffer::from_text(BufferId::new(1), initial).unwrap();

    // 1. Edit card title
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_edit_card_title(&s, &board, 0, 1, "Вторая обновленная").unwrap();
    buf.apply(tx).unwrap();
    let b1 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b1.columns[0].cards[1].clean_title, "Вторая обновленная");

    // 2. Set card status directly
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_set_card_status(&s, &board, 0, 1, KanbanTaskStatus::Done).unwrap();
    buf.apply(tx).unwrap();
    let b2 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b2.columns[0].cards[1].status, KanbanTaskStatus::Done);

    // 3. Delete card 0 ("Первая" with body line "  Контекст")
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let (del_tx, yanked) = plan_delete_card(&s, &board, 0, 0).unwrap();
    assert!(yanked.contains("Первая"));
    assert!(yanked.contains("Контекст"));
    buf.apply(del_tx).unwrap();
    let b3 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b3.columns[0].cards.len(), 1);

    // 4. Paste yanked card into column 1 ("Готово")
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let paste_tx = plan_paste_card(&s, &board, 1, CardInsertPos::End, &yanked).unwrap();
    buf.apply(paste_tx).unwrap();
    let b4 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b4.columns[1].cards.len(), 1);
    assert_eq!(b4.columns[1].cards[0].clean_title, "Первая");
    assert_eq!(b4.columns[1].cards[0].body_lines, vec!["Контекст"]);

    // 5. Undo all
    for _ in 0..4 {
        assert!(buf.undo().unwrap().is_some());
    }
    assert_eq!(buf.as_str(), initial);
}

#[test]
fn default_template_and_insert_positions() {
    let tpl_lf = default_kanban_template("План работ", "\n");
    assert!(tpl_lf.contains("# План работ\n"));
    assert!(tpl_lf.contains("## План\n"));
    assert!(tpl_lf.contains("## В работе\n"));
    assert!(tpl_lf.contains("## Готово\n"));

    let tpl_crlf = default_kanban_template("План работ", "\r\n");
    assert!(tpl_crlf.contains("# План работ\r\n"));

    let initial = "\
## Col
- [ ] Card 1
- [ ] Card 3
";
    let mut buf = Buffer::from_text(BufferId::new(1), initial).unwrap();
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);

    // Insert Before(1) -> between Card 1 and Card 3
    let tx = plan_add_card(&s, &board, 0, CardInsertPos::Before(1), "Card 2").unwrap();
    buf.apply(tx).unwrap();
    let b1 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b1.columns[0].cards.len(), 3);
    assert_eq!(b1.columns[0].cards[0].clean_title, "Card 1");
    assert_eq!(b1.columns[0].cards[1].clean_title, "Card 2");
    assert_eq!(b1.columns[0].cards[2].clean_title, "Card 3");

    // Insert After(0)
    let s = buf.as_str();
    let board = KanbanBoard::parse(&s);
    let tx = plan_add_card(&s, &board, 0, CardInsertPos::After(0), "Card 1.5").unwrap();
    buf.apply(tx).unwrap();
    let b2 = KanbanBoard::parse(&buf.as_str());
    assert_eq!(b2.columns[0].cards.len(), 4);
    assert_eq!(b2.columns[0].cards[1].clean_title, "Card 1.5");
}
