use truff_core::markdown::kanban::{KanbanBoard, KanbanTaskStatus, is_kanban_document};

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
