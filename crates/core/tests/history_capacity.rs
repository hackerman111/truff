use truff_core::{Buffer, BufferId, Transaction};

#[test]
fn oversized_insert_text_capacity_is_compacted_without_changing_undo_redo() {
    let mut text = String::with_capacity(1024 * 1024);
    text.push_str("字abc");
    let expected = text.clone();
    let mut buffer = Buffer::new(BufferId::new(9001));

    buffer.apply(Transaction::insert(0, text)).unwrap();
    assert!(buffer.history().total_bytes() < 16 * 1024);
    assert_eq!(buffer.as_str(), expected);

    buffer.undo().unwrap();
    assert_eq!(buffer.as_str(), "");
    buffer.redo().unwrap();
    assert_eq!(buffer.as_str(), expected);
}

#[test]
fn ordinary_and_empty_inserts_keep_their_text_roundtrip() {
    let mut ordinary = Buffer::new(BufferId::new(9002));
    ordinary
        .apply(Transaction::insert(0, String::from("ordinary")))
        .unwrap();
    assert_eq!(ordinary.as_str(), "ordinary");
    ordinary.undo().unwrap();
    assert_eq!(ordinary.as_str(), "");
    ordinary.redo().unwrap();
    assert_eq!(ordinary.as_str(), "ordinary");

    let mut empty = Buffer::new(BufferId::new(9003));
    empty.apply(Transaction::insert(0, String::new())).unwrap();
    assert_eq!(empty.as_str(), "");
    empty.undo().unwrap();
    empty.redo().unwrap();
    assert_eq!(empty.as_str(), "");
}
