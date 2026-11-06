use truff_core::markdown::smart_edit::{plan_enter, plan_table_tab};
use truff_core::text::{Buffer, BytePos};

#[test]
fn test_enter_in_table_creates_new_row() {
    let doc = "\
| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |
";
    let mut buffer = Buffer::from_str(doc);
    let pos = doc.find("Val 2").unwrap() + 3;

    let tx = plan_enter(&buffer, BytePos::new(pos)).expect("enter in table row");
    buffer.apply(tx).unwrap();

    let expected = "\
| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |
|          |          |
";
    assert_eq!(buffer.as_str(), expected);
}

#[test]
fn test_enter_on_empty_table_row_exits_table() {
    let doc = "\
| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |
|          |          |
";
    let mut buffer = Buffer::from_str(doc);
    let empty_row_pos = doc.rfind('|').unwrap() - 1;

    let tx = plan_enter(&buffer, BytePos::new(empty_row_pos)).expect("enter on empty table row");
    buffer.apply(tx).unwrap();

    let expected = "\
| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |

";
    assert_eq!(buffer.as_str(), expected);
}

#[test]
fn test_tab_in_table_navigates_to_next_cell() {
    let doc = "\
| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |
";
    let buffer = Buffer::from_str(doc);
    let cell1_pos = doc.find("Val 1").unwrap();

    let tx = plan_table_tab(&buffer, BytePos::new(cell1_pos), false).expect("tab to next cell");
    // Cursor should be moved to Val 2
    let new_cursor = tx.cursor.expect("cursor set").head.get();
    let expected_cursor = doc.find("Val 2").unwrap();
    assert_eq!(new_cursor, expected_cursor);
}

#[test]
fn test_tab_on_last_cell_creates_new_row() {
    let doc = "\
| Header 1 | Header 2 |
|----------|----------|
| Val 1    | Val 2    |
";
    let mut buffer = Buffer::from_str(doc);
    let cell2_pos = doc.find("Val 2").unwrap() + 2;

    let tx = plan_table_tab(&buffer, BytePos::new(cell2_pos), false).expect("tab on last cell");
    buffer.apply(tx).unwrap();

    assert!(buffer.as_str().contains("|          |          |"));
}
