use truff_core::markdown::smart_edit::{plan_enter, plan_shift_tab, plan_tab};
use truff_core::text::{Buffer, BytePos};

#[test]
fn test_mebc1_enter_all_table_cases() {
    // 1. - one| -> - one\n- |
    let mut b = Buffer::from_str("- one");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("bullet -");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- one\n- ");

    // 2. * one| -> * one\n* |
    let mut b = Buffer::from_str("* one");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("bullet *");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "* one\n* ");

    // 3. + one| -> + one\n+ |
    let mut b = Buffer::from_str("+ one");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("bullet +");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "+ one\n+ ");

    // 4.   - nested| ->   - nested\n  - |
    let mut b = Buffer::from_str("  - nested");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("nested bullet");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "  - nested\n  - ");

    // 5. - [ ] todo| -> - [ ] todo\n- [ ] |
    let mut b = Buffer::from_str("- [ ] todo");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("task todo");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- [ ] todo\n- [ ] ");

    // 6. - [x] done| -> - [x] done\n- [ ] |
    let mut b = Buffer::from_str("- [x] done");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("task done");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- [x] done\n- [ ] ");

    // 7. 1. first| -> 1. first\n2. |
    let mut b = Buffer::from_str("1. first");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("ordered .");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "1. first\n2. ");

    // 8. 7) later| -> 7) later\n8) |
    let mut b = Buffer::from_str("7) later");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("ordered )");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "7) later\n8) ");

    // 9. > quote| -> > quote\n> |
    let mut b = Buffer::from_str("> quote");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("quote");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "> quote\n> ");

    // 10. - | -> exit list (clears marker)
    let mut b = Buffer::from_str("- ");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("empty bullet exit");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "");

    // 11. > | -> exit quote (clears quote marker)
    let mut b = Buffer::from_str("> ");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("empty quote exit");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "");

    // 12. plain| -> plain\n| (plan_enter returns None)
    let b = Buffer::from_str("plain");
    assert!(plan_enter(&b, BytePos::new(b.byte_len())).is_none());
}

#[test]
fn test_mebc1_code_fence_and_yaml_exclusion() {
    // In fenced code with bullet-like line: normal newline, plan_enter returns None
    let code = "```rust\n- not a list\n```";
    let b = Buffer::from_str(code);
    let cursor = BytePos::new("```rust\n- not a list".len());
    assert!(
        plan_enter(&b, cursor).is_none(),
        "Enter in code fence on line starting with dash must return None"
    );

    // In YAML frontmatter with bullet-like line: normal newline, plan_enter returns None
    let yaml = "---\ntitle: x\n- not a list\n---";
    let b = Buffer::from_str(yaml);
    let cursor = BytePos::new("---\ntitle: x\n- not a list".len());
    assert!(
        plan_enter(&b, cursor).is_none(),
        "Enter in YAML frontmatter on line starting with dash must return None"
    );

    // Inside table row with dash: not a list item
    let table = "| col1 | col2 |\n| - | - |\n";
    let b = Buffer::from_str(table);
    let cursor = BytePos::new("| col1 | col2 |\n| -".len());
    assert!(
        plan_enter(&b, cursor).is_none(),
        "Table row containing dash is not a list"
    );
}

#[test]
fn test_mebc1_nested_empty_item_lifting() {
    // "Для вложенного пустого пункта однократный Enter снимает текущий маркер
    // и поднимает вложенность на один уровень; повторный Enter на оставшемся
    // пустом уровне завершает список."
    let mut b = Buffer::from_str("  - ");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("first enter lifts nesting");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- ");

    let tx2 = plan_enter(&b, BytePos::new(b.byte_len())).expect("second enter exits list");
    b.apply(tx2).unwrap();
    assert_eq!(b.as_str(), "");
}

#[test]
fn test_mebc1_mid_line_split() {
    // "Enter посреди строки делит содержание в точке caret и вставляет соответствующий маркер в новую строку: - ab|cd → - ab\n- |cd."
    let text = "- abcd";
    let mut b = Buffer::from_str(text);
    let split_pos = BytePos::new("- ab".len());
    let tx = plan_enter(&b, split_pos).expect("split list item mid-line");
    assert_eq!(
        tx.cursor.map(|c| c.head),
        Some(BytePos::new("- ab\n- ".len()))
    );
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- ab\n- cd");
}

#[test]
fn test_mebc1_quote_with_list() {
    // "Список в blockquote продолжает обе составляющие: > - item| → > - item\n> - |."
    let mut b = Buffer::from_str("> - item");
    let tx = plan_enter(&b, BytePos::new(b.byte_len())).expect("quote + list");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "> - item\n> - ");
}

#[test]
fn test_mebc1_tab_and_shift_tab_subtrees() {
    // Tab indents by 2 spaces and indents child items as well
    let doc = "- Parent\n  - Child 1\n  - Child 2\n- Other\n";
    let mut b = Buffer::from_str(doc);
    let cursor = BytePos::new("- Par".len());
    let tx = plan_tab(&b, cursor).expect("tab on parent");
    let new_cursor = tx.cursor.unwrap().head;
    b.apply(tx).unwrap();
    assert_eq!(
        b.as_str(),
        "  - Parent\n    - Child 1\n    - Child 2\n- Other\n"
    );

    // Shift+Tab outdents by 2 spaces along with children
    let tx2 = plan_shift_tab(&b, new_cursor).expect("shift-tab on parent");
    b.apply(tx2).unwrap();
    assert_eq!(b.as_str(), doc);
}

#[test]
fn test_mebc1_numbering_only_new_item_not_mass_renumbered() {
    // "Numbering: автоматически увеличивается число только у нового пункта,
    // не производится массовая перенумерация документа."
    let initial = "1. First\n2. Second\n";
    let mut b = Buffer::from_str(initial);
    let cursor = BytePos::new("1. First".len());
    let tx = plan_enter(&b, cursor).expect("enter after 1. First");
    b.apply(tx).unwrap();
    // Subsequent item "2. Second" remains "2. Second", not renumbered to "3. Second"
    assert_eq!(b.as_str(), "1. First\n2. \n2. Second\n");
}

#[test]
fn test_mebc1_enter_list_continuation_dedup() {
    // 1. Bullet list `-`: "- item 1- item 2" -> "- item 1\n- item 2"
    let mut b = Buffer::from_str("- item 1- item 2");
    let cursor = BytePos::new("- item 1".len());
    let tx = plan_enter(&b, cursor).expect("enter between bullet items");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- item 1\n- item 2");

    // 2. Bullet list `*`: "* item 1* item 2" -> "* item 1\n* item 2"
    let mut b = Buffer::from_str("* item 1* item 2");
    let cursor = BytePos::new("* item 1".len());
    let tx = plan_enter(&b, cursor).expect("enter between * bullet items");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "* item 1\n* item 2");

    // 3. Bullet list `+`: "+ item 1+ item 2" -> "+ item 1\n+ item 2"
    let mut b = Buffer::from_str("+ item 1+ item 2");
    let cursor = BytePos::new("+ item 1".len());
    let tx = plan_enter(&b, cursor).expect("enter between + bullet items");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "+ item 1\n+ item 2");

    // 4. Ordered list: "1. item 12. item 2" -> "1. item 1\n2. item 2"
    let mut b = Buffer::from_str("1. item 12. item 2");
    let cursor = BytePos::new("1. item 1".len());
    let tx = plan_enter(&b, cursor).expect("enter between ordered items");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "1. item 1\n2. item 2");

    // 5. Task list: "- [ ] item 1- [ ] item 2" -> "- [ ] item 1\n- [ ] item 2"
    let mut b = Buffer::from_str("- [ ] item 1- [ ] item 2");
    let cursor = BytePos::new("- [ ] item 1".len());
    let tx = plan_enter(&b, cursor).expect("enter between task items");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- [ ] item 1\n- [ ] item 2");

    // 6. Task list with [ ] after split: "- [ ] item 1[ ] item 2" -> "- [ ] item 1\n- [ ] item 2"
    let mut b = Buffer::from_str("- [ ] item 1[ ] item 2");
    let cursor = BytePos::new("- [ ] item 1".len());
    let tx = plan_enter(&b, cursor).expect("enter between task items with [ ] marker");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "- [ ] item 1\n- [ ] item 2");
}

#[test]
fn test_plan_enter_auto_aligns_list_indentation() {
    // Tab-indented list item normalizes both current and new line to 2-space indent stops
    let mut b = Buffer::from_str("\t- Sub item");
    let cursor = BytePos::new("\t- Sub item".len());
    let tx = plan_enter(&b, cursor).expect("enter on tab-indented list item");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "  - Sub item\n  - ");

    // Odd 3-space indented list item aligns to 2-space step (4 spaces)
    let mut b = Buffer::from_str("   - Misaligned item");
    let cursor = BytePos::new("   - Misaligned item".len());
    let tx = plan_enter(&b, cursor).expect("enter on 3-space misaligned item");
    b.apply(tx).unwrap();
    assert_eq!(b.as_str(), "    - Misaligned item\n    - ");
}
