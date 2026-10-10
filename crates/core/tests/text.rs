use truff_core::{
    Buffer, BufferId, BytePos, ChangeSet, EditError, LineEnding, Register, RegisterKind,
    SingleEdit, Transaction, UndoGroup,
};

#[test]
fn edit_unicode_atomic() {
    let mut buffer = Buffer::from_text(BufferId::new(1), "Привет 🌍 мир").unwrap();
    let initial_rev = buffer.revision();
    let initial_text = buffer.rope().to_string();

    // 1. Invalid range beyond length
    let tx_invalid = Transaction::delete(50..100);
    let err = buffer.apply(tx_invalid).unwrap_err();
    assert!(matches!(err, EditError::InvalidByteRange { .. }));
    assert_eq!(buffer.revision(), initial_rev);
    assert_eq!(buffer.rope().to_string(), initial_text);
    assert!(!buffer.is_dirty());

    // 2. Range boundary slicing middle of multibyte UTF-8 char
    // 'П' is 2 bytes: bytes 0..2. Slicing at index 1 is not a char boundary.
    let tx_mid_char = Transaction::delete(1..3);
    let err = buffer.apply(tx_mid_char).unwrap_err();
    assert!(matches!(err, EditError::NotCharBoundary { pos: 1 }));
    assert_eq!(buffer.revision(), initial_rev);
    assert_eq!(buffer.rope().to_string(), initial_text);
    assert!(!buffer.is_dirty());

    // 3. Overlapping edits in a single transaction
    // Slicing "Привет": "П" is 0..2, "р" is 2..4, "и" is 4..6.
    let tx_overlap = Transaction::new()
        .with_edit(SingleEdit::replace(0..4, "X"))
        .with_edit(SingleEdit::replace(2..6, "Y"));
    let err = buffer.apply(tx_overlap).unwrap_err();
    assert!(matches!(err, EditError::OverlappingEdits { .. }));
    assert_eq!(buffer.revision(), initial_rev);
    assert_eq!(buffer.rope().to_string(), initial_text);
    assert!(!buffer.is_dirty());

    // Unsorted edits
    let tx_unsorted = Transaction::new()
        .with_edit(SingleEdit::insert(10, "B"))
        .with_edit(SingleEdit::insert(2, "A"));
    let err = buffer.apply(tx_unsorted).unwrap_err();
    assert!(matches!(err, EditError::UnsortedEdits { .. }));
    assert_eq!(buffer.revision(), initial_rev);

    // Double insert at same position
    let tx_same_pos = Transaction::new()
        .with_edit(SingleEdit::insert(0, "A"))
        .with_edit(SingleEdit::insert(0, "B"));
    let err = buffer.apply(tx_same_pos).unwrap_err();
    assert!(matches!(err, EditError::OverlappingEdits { .. }));
    assert_eq!(buffer.revision(), initial_rev);

    // 4. Valid atomic edit with Cyrillic and emoji
    // Replace "Привет" (0..12 bytes) with "Здравствуй", and "мир" (bytes 17..23) with "вселенная 🚀"
    // "Привет 🌍 мир":
    // "Привет" is 12 bytes: 0..12
    // " " is 1 byte: 12..13
    // "🌍" is 4 bytes: 13..17
    // " " is 1 byte: 17..18
    // "мир" is 6 bytes: 18..24
    let tx_valid = Transaction::new()
        .with_edit(SingleEdit::replace(0..12, "Здравствуй"))
        .with_edit(SingleEdit::replace(18..24, "вселенная 🚀"));
    let cs = buffer
        .apply(tx_valid)
        .expect("valid transaction must succeed");
    assert_eq!(buffer.rope().to_string(), "Здравствуй 🌍 вселенная 🚀");
    assert_eq!(buffer.revision(), initial_rev + 1);
    assert!(buffer.is_dirty());
    assert_eq!(cs.changes.len(), 2);
}

#[test]
fn edit_size_limit() {
    let mut buffer = Buffer::from_text(BufferId::new(2), "small text").unwrap();
    let initial_rev = buffer.revision();
    let initial_text = buffer.rope().to_string();

    // Default limit is 16 MiB. Let's set a small limit (e.g. 50 bytes) for testing.
    buffer.set_max_file_bytes(50);

    // Attempt to insert text that exceeds 50 bytes
    let big_text = "A".repeat(100);
    let tx = Transaction::insert(0, big_text);
    let err = buffer.apply(tx).unwrap_err();
    assert!(matches!(
        err,
        EditError::SizeLimitExceeded {
            current_len: 10,
            new_len: 110,
            limit: 50
        }
    ));
    assert_eq!(buffer.revision(), initial_rev);
    assert_eq!(buffer.rope().to_string(), initial_text);
    assert!(!buffer.is_dirty());

    // Valid insert up to limit
    let tx_ok = Transaction::insert(0, "12345");
    buffer.apply(tx_ok).expect("must succeed within limit");
    assert_eq!(buffer.byte_len(), 15);
    assert!(buffer.byte_len() <= 50);

    // Test default limit on a fresh buffer
    let fresh = Buffer::new(BufferId::new(3));
    assert_eq!(fresh.max_file_bytes(), 16 * 1024 * 1024);

    // Test ChangeSet map_pos
    let cs = ChangeSet::new(vec![truff_core::Change::new(5..10, 2)]);
    assert_eq!(cs.map_pos(2), 2);
    assert_eq!(cs.map_pos(7), 7);
    assert_eq!(cs.map_pos(12), 9);

    // Test redo respecting size limit
    let mut buf_redo = Buffer::from_text(BufferId::new(10), "hello").unwrap();
    buf_redo.apply(Transaction::insert(5, " world")).unwrap();
    assert_eq!(buf_redo.rope().to_string(), "hello world");
    buf_redo.undo().unwrap();
    assert_eq!(buf_redo.rope().to_string(), "hello");
    // Restrict limit to 8 bytes, so redo (len 11) will fail
    buf_redo.set_max_file_bytes(8);
    let err_redo = buf_redo.redo().unwrap_err();
    assert!(matches!(err_redo, EditError::SizeLimitExceeded { .. }));
    // Buffer remains "hello"
    assert_eq!(buf_redo.rope().to_string(), "hello");
}

#[test]
fn undo_group_roundtrip() {
    let mut buffer = Buffer::from_text(BufferId::new(4), "Alpha Bravo Charlie").unwrap();
    let r0 = buffer.revision();
    let text_r0 = buffer.rope().to_string();
    assert!(!buffer.is_dirty());

    // Step 1: Edit in UndoGroup 10
    let group1 = UndoGroup::new(10);
    let tx1 = Transaction::replace(0..5, "Delta").with_undo_group(group1);
    buffer.apply(tx1).unwrap();
    let r1 = buffer.revision();
    assert!(r1 > r0);
    assert_eq!(buffer.rope().to_string(), "Delta Bravo Charlie");
    assert!(buffer.is_dirty());

    // Step 2: Another edit in the SAME UndoGroup 10
    let tx2 = Transaction::replace(12..19, "Echo").with_undo_group(group1);
    buffer.apply(tx2).unwrap();
    let r2 = buffer.revision();
    assert!(r2 > r1);
    assert_eq!(buffer.rope().to_string(), "Delta Bravo Echo");

    // Step 3: Undo the entire UndoGroup 10 in one call
    let cs = buffer
        .undo()
        .unwrap()
        .expect("undo should produce change set");
    let r3 = buffer.revision();
    assert!(r3 > r2, "revision must monotonically increase on undo");
    assert_eq!(
        buffer.rope().to_string(),
        text_r0,
        "undo must restore exact byte-for-byte text"
    );
    assert!(
        !buffer.is_dirty(),
        "returning to initial saved state must clear dirty flag"
    );
    assert!(!cs.is_empty());

    // Step 4: Redo the UndoGroup 10
    let cs_redo = buffer
        .redo()
        .unwrap()
        .expect("redo should produce change set");
    let r4 = buffer.revision();
    assert!(r4 > r3, "revision must monotonically increase on redo");
    assert_eq!(buffer.rope().to_string(), "Delta Bravo Echo");
    assert!(buffer.is_dirty());
    assert!(!cs_redo.is_empty());
}

#[test]
fn grapheme_navigation() {
    // String containing ASCII, Cyrillic, and ZWJ emoji sequence:
    // "A" (1 byte), "Ж" (2 bytes), "👨‍👩‍👧‍👦" (25 bytes), "B" (1 byte)
    let text = "AЖ👨‍👩‍👧‍👦B";
    let buffer = Buffer::from_text(BufferId::new(5), text).unwrap();

    // Check grapheme length
    assert_eq!(buffer.grapheme_len(), 4);

    // Grapheme next navigation:
    // 0: 'A' (offset 0..1)
    let p0 = BytePos::new(0);
    let p1 = buffer.grapheme_next(p0).unwrap();
    assert_eq!(p1, BytePos::new(1)); // at 'Ж'

    // 1: 'Ж' (offset 1..3)
    let p2 = buffer.grapheme_next(p1).unwrap();
    assert_eq!(p2, BytePos::new(3)); // at '👨‍👩‍👧‍👦'

    // 3: '👨‍👩‍👧‍👦' (offset 3..28)
    let p3 = buffer.grapheme_next(p2).unwrap();
    assert_eq!(p3, BytePos::new(28)); // at 'B'

    // 28: 'B' (offset 28..29)
    let p4 = buffer.grapheme_next(p3).unwrap();
    assert_eq!(p4, BytePos::new(29)); // at end of buffer

    // At end of buffer:
    assert_eq!(buffer.grapheme_next(p4), None);

    // Grapheme prev navigation:
    assert_eq!(
        buffer.grapheme_prev(BytePos::new(29)),
        Some(BytePos::new(28))
    );
    assert_eq!(
        buffer.grapheme_prev(BytePos::new(28)),
        Some(BytePos::new(3))
    );
    assert_eq!(buffer.grapheme_prev(BytePos::new(3)), Some(BytePos::new(1)));
    assert_eq!(buffer.grapheme_prev(BytePos::new(1)), Some(BytePos::new(0)));
    assert_eq!(buffer.grapheme_prev(BytePos::new(0)), None);
}

#[test]
fn history_bounded_memory() {
    let mut buffer = Buffer::from_text(BufferId::new(6), "Base").unwrap();

    // Set small history memory limit of 1000 bytes
    buffer.history_mut().set_max_bytes(1000);

    // Perform many edits in distinct undo groups, each with ~200 bytes
    for i in 0..50 {
        let text = format!(" Edit number {:04} with padding data! ", i);
        let group = UndoGroup::new(i as u64);
        let tx = Transaction::replace(0..buffer.byte_len(), text).with_undo_group(group);
        buffer.apply(tx).unwrap();
    }

    // Verify history total bytes <= 1000
    assert!(
        buffer.history().total_bytes() <= 1000,
        "history memory ({} bytes) must be bounded by limit (1000 bytes)",
        buffer.history().total_bytes()
    );

    // Only recent groups can be undone, oldest ones were evicted
    let mut undo_count = 0;
    while let Ok(Some(_)) = buffer.undo() {
        undo_count += 1;
    }
    assert!(undo_count > 0, "should be able to undo recent edits");
    assert!(
        undo_count < 50,
        "oldest edits must have been evicted due to memory bound"
    );
}

#[test]
fn registers_and_crlf() {
    // CRLF preservation
    let crlf_text = "line1\r\nline2\r\n";
    let mut buffer = Buffer::from_text(BufferId::new(7), crlf_text).unwrap();
    assert_eq!(buffer.line_ending(), LineEnding::CrLf);

    // Insert text containing LF into CRLF buffer: should normalize inserted LF to CRLF
    let tx = Transaction::insert(buffer.byte_len(), "line3\n");
    buffer.apply(tx).unwrap();
    assert_eq!(buffer.rope().to_string(), "line1\r\nline2\r\nline3\r\n");

    // Undo reverts back to exact initial CRLF text
    buffer.undo().unwrap();
    assert_eq!(buffer.rope().to_string(), crlf_text);

    // Test registers
    buffer.history_mut().registers_mut().set(
        Some('a'),
        Register::new("yanked content", RegisterKind::Characterwise),
    );
    let reg_a = buffer.history().registers().get(Some('a')).unwrap();
    assert_eq!(reg_a.text, "yanked content");
    assert_eq!(reg_a.kind, RegisterKind::Characterwise);

    // Unnamed register fallback
    let reg_unnamed = buffer.history().registers().get(None).unwrap();
    assert_eq!(reg_unnamed.text, "yanked content");
}
