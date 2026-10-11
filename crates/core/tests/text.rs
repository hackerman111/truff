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
    assert_eq!(cs.changes().len(), 2);
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

#[test]
fn grouped_dependent_edits_roundtrip_and_rebase() {
    let mut buffer = Buffer::from_str("ab");
    let group = UndoGroup::next();
    buffer
        .apply(Transaction::insert(0, "Ж").with_undo_group(group))
        .unwrap();
    buffer
        .apply(Transaction::replace(2..3, "🌍").with_undo_group(group))
        .unwrap();
    assert_eq!(buffer.as_str(), "Ж🌍b");
    let undo = buffer.undo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), "ab");
    assert_eq!(undo.map_pos(6), 1);
    assert_eq!(undo.map_pos(7), 2);
    let redo = buffer.redo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), "Ж🌍b");
    assert_eq!(redo.map_pos(1), 6);
    assert_eq!(redo.map_pos(2), 7);
}

#[test]
fn grouped_undo_failure_preserves_text_revision_history_and_savepoint() {
    let mut buffer = Buffer::from_str("abcdef");
    let group = UndoGroup::next();
    buffer
        .apply(Transaction::delete(0..2).with_undo_group(group))
        .unwrap();
    buffer
        .apply(Transaction::delete(0..2).with_undo_group(group))
        .unwrap();
    buffer.set_savepoint();
    buffer.set_max_file_bytes(4);
    let revision = buffer.revision();
    let history = buffer.history().clone();
    assert!(matches!(
        buffer.undo(),
        Err(EditError::SizeLimitExceeded { .. })
    ));
    assert_eq!(buffer.as_str(), "ef");
    assert_eq!(buffer.revision(), revision);
    assert_eq!(buffer.history(), &history);
    assert!(!buffer.is_dirty());
    buffer.set_max_file_bytes(6);
    buffer.undo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), "abcdef");
}

#[test]
fn grouped_redo_failure_preserves_text_revision_and_history() {
    let mut buffer = Buffer::from_str("");
    let group = UndoGroup::next();
    buffer
        .apply(Transaction::insert(0, "a").with_undo_group(group))
        .unwrap();
    buffer
        .apply(Transaction::insert(1, "bc").with_undo_group(group))
        .unwrap();
    buffer.undo().unwrap();
    buffer.set_max_file_bytes(2);
    let revision = buffer.revision();
    let history = buffer.history().clone();
    assert!(matches!(
        buffer.redo(),
        Err(EditError::SizeLimitExceeded { .. })
    ));
    assert_eq!(buffer.as_str(), "");
    assert_eq!(buffer.revision(), revision);
    assert_eq!(buffer.history(), &history);
    assert!(!buffer.is_dirty());
    buffer.set_max_file_bytes(3);
    buffer.redo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), "abc");
}

#[test]
fn savepoint_splits_an_active_undo_group() {
    let mut buffer = Buffer::from_str("");
    let group = UndoGroup::next();
    buffer
        .apply(Transaction::insert(0, "a").with_undo_group(group))
        .unwrap();
    buffer.set_savepoint();
    buffer
        .apply(Transaction::insert(1, "b").with_undo_group(group))
        .unwrap();
    assert!(buffer.is_dirty());
    buffer.undo().unwrap();
    assert_eq!(buffer.as_str(), "a");
    assert!(!buffer.is_dirty());
    buffer.undo().unwrap();
    assert_eq!(buffer.as_str(), "");
    assert!(buffer.is_dirty());
    buffer.redo().unwrap();
    assert!(!buffer.is_dirty());
    buffer.redo().unwrap();
    assert!(buffer.is_dirty());
}

#[test]
fn later_edit_anchor_includes_preceding_deltas() {
    let changes = ChangeSet::new(vec![
        truff_core::Change::new(0..0, 2),
        truff_core::Change::new(4..6, 1),
        truff_core::Change::new(8..9, 3),
    ]);
    for (position, expected) in [
        (0, 2),
        (3, 5),
        (4, 7),
        (5, 7),
        (6, 7),
        (7, 8),
        (8, 12),
        (9, 12),
        (10, 13),
    ] {
        assert_eq!(changes.map_pos(position), expected, "position {position}");
    }
}

#[test]
fn oversized_history_group_is_dropped_in_full() {
    let mut buffer = Buffer::from_str("");
    buffer.history_mut().set_max_bytes(1024);
    let group = UndoGroup::next();
    buffer
        .apply(Transaction::insert(0, "a".repeat(2048)).with_undo_group(group))
        .unwrap();
    assert!(buffer.history().total_bytes() <= 1024);
    assert!(!buffer.history().can_undo());
    buffer
        .apply(Transaction::insert(0, "b").with_undo_group(group))
        .unwrap();
    assert!(
        !buffer.history().can_undo(),
        "do not retain a suffix of an evicted group"
    );
    assert!(buffer.is_dirty());
    buffer.apply(Transaction::insert(0, "c")).unwrap();
    buffer.undo().unwrap().unwrap();
    assert!(buffer.as_str().starts_with("ba"));
    assert!(
        buffer.is_dirty(),
        "eviction must not alias the initial savepoint"
    );
}

#[test]
fn lowering_history_limit_evicts_redo_and_does_not_alias_savepoint() {
    let mut buffer = Buffer::from_str("base");
    buffer.apply(Transaction::insert(0, "a")).unwrap();
    buffer.set_savepoint();
    buffer.apply(Transaction::insert(0, "b")).unwrap();
    buffer.undo().unwrap();
    assert!(!buffer.is_dirty());
    buffer.history_mut().set_max_bytes(0);
    assert_eq!(buffer.history().total_bytes(), 0);
    assert!(!buffer.history().can_undo());
    assert!(!buffer.history().can_redo());
    assert!(!buffer.is_dirty());
    buffer.apply(Transaction::insert(0, "c")).unwrap();
    assert!(buffer.is_dirty());
}

#[test]
fn growing_group_stays_bounded() {
    let mut buffer = Buffer::from_str("");
    buffer.history_mut().set_max_bytes(2048);
    let group = UndoGroup::next();
    for _ in 0..100 {
        buffer
            .apply(Transaction::insert(0, "x").with_undo_group(group))
            .unwrap();
        assert!(buffer.history().total_bytes() <= 2048);
    }
    assert!(!buffer.history().can_undo());
    assert!(buffer.is_dirty());
}

#[test]
fn adjacent_deletions_can_be_undone_atomically() {
    let mut buffer = Buffer::from_str("aЖ🌍z");
    buffer
        .apply(
            Transaction::new()
                .with_edit(SingleEdit::delete(0..1))
                .with_edit(SingleEdit::delete(1..3))
                .with_edit(SingleEdit::delete(3..7)),
        )
        .unwrap();
    assert_eq!(buffer.as_str(), "z");
    buffer.undo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), "aЖ🌍z");
    buffer.redo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), "z");
}

#[test]
fn grouped_changes_expose_each_coordinate_space_in_execution_order() {
    let mut buffer = Buffer::from_str("abcd");
    let group = UndoGroup::next();
    buffer
        .apply(Transaction::insert(0, "XX").with_undo_group(group))
        .unwrap();
    buffer
        .apply(Transaction::replace(4..6, "Y").with_undo_group(group))
        .unwrap();
    let undo = buffer.undo().unwrap().unwrap();
    let batches: Vec<_> = undo.batches().collect();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0], &[truff_core::Change::new(4..5, 2)]);
    assert_eq!(batches[1], &[truff_core::Change::new(0..2, 0)]);
    let redo = buffer.redo().unwrap().unwrap();
    assert_eq!(redo.map_pos(3), 5);
    assert_eq!(redo.batches().count(), 2);
}

#[test]
fn evicted_savepoint_does_not_match_an_unrelated_empty_stack() {
    let mut buffer = Buffer::from_str("base");
    buffer.apply(Transaction::insert(0, "a")).unwrap();
    buffer.set_savepoint();
    buffer.apply(Transaction::insert(0, "b")).unwrap();
    let one_group_limit = buffer.history().total_bytes() / 2;
    buffer.history_mut().set_max_bytes(one_group_limit);
    buffer.history_mut().set_max_bytes(0);
    assert!(buffer.is_dirty());
    assert_eq!(buffer.history().total_bytes(), 0);
}

#[test]
fn anchor_at_shared_edit_boundary_has_right_affinity() {
    let changes = ChangeSet::new(vec![
        truff_core::Change::new(0..2, 1),
        truff_core::Change::new(2..3, 2),
        truff_core::Change::new(3..3, 1),
    ]);
    assert_eq!(changes.map_pos(1), 1);
    assert_eq!(changes.map_pos(2), 3);
    assert_eq!(changes.map_pos(3), 4);
    assert_eq!(changes.map_pos(4), 5);
}

#[test]
fn rope_graphemes_match_unicode_reference_across_fragmented_chunks() {
    use unicode_segmentation::UnicodeSegmentation;
    let pattern = "Жa\u{301}\r\n👨‍👩‍👧‍👦🇷🇺🇫🇷क्‍ष";
    let mut buffer = Buffer::from_str(&pattern.repeat(90));
    // Split clusters across Rope leaves and change gap positions before traversal.
    for offset in [0, 731, 1547] {
        let at = (offset..buffer.byte_len())
            .find(|&at| buffer.is_char_boundary(at))
            .unwrap();
        buffer.apply(Transaction::insert(at, "\u{301}🇷🇺")).unwrap();
    }
    let text = buffer.as_str();
    let boundaries: Vec<_> = text
        .grapheme_indices(true)
        .map(|(at, _)| at)
        .chain(std::iter::once(text.len()))
        .collect();
    assert_eq!(buffer.grapheme_len(), boundaries.len() - 1);
    for at in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
        let next = boundaries.iter().copied().find(|&boundary| boundary > at);
        let prev = boundaries.iter().copied().rfind(|&boundary| boundary < at);
        assert_eq!(
            buffer.grapheme_next(BytePos::new(at)),
            next.map(BytePos::new),
            "next at {at}"
        );
        assert_eq!(
            buffer.grapheme_prev(BytePos::new(at)),
            prev.map(BytePos::new),
            "prev at {at}"
        );
    }
}

#[test]
fn rope_graphemes_handle_large_clusters_and_invalid_byte_positions() {
    let text = format!("x{}🇷🇺z", "\u{301}".repeat(8192));
    let buffer = Buffer::from_str(&text);
    let cluster_end = 1 + 8192 * 2;
    assert_eq!(
        buffer.grapheme_next(BytePos::ZERO),
        Some(BytePos::new(cluster_end))
    );
    assert_eq!(
        buffer.grapheme_prev(BytePos::new(cluster_end)),
        Some(BytePos::ZERO)
    );
    assert_eq!(buffer.grapheme_len(), 3);
    assert_eq!(buffer.grapheme_next(BytePos::new(2)), None);
    assert_eq!(buffer.grapheme_prev(BytePos::new(2)), None);
}

#[test]
fn buffer_floor_char_boundary_and_line_of_byte_multibyte() {
    let buffer = Buffer::from_str("АБВ\nГДЕёжз");
    // "А" is bytes 0..2, "Б" is 2..4, "В" is 4..6, "\n" is 6..7
    // "Г" is 7..9, "Д" is 9..11, "Е" is 11..13, "ё" is 13..15, "ж" is 15..17, "з" is 17..19
    assert_eq!(buffer.floor_char_boundary(0), 0);
    assert_eq!(buffer.floor_char_boundary(1), 0);
    assert_eq!(buffer.floor_char_boundary(2), 2);
    assert_eq!(buffer.floor_char_boundary(3), 2);
    assert_eq!(buffer.floor_char_boundary(18), 17);
    assert_eq!(buffer.floor_char_boundary(100), 19);

    assert_eq!(buffer.line_of_byte(0), 0);
    assert_eq!(buffer.line_of_byte(1), 0);
    assert_eq!(buffer.line_of_byte(5), 0);
    assert_eq!(buffer.line_of_byte(6), 0);
    assert_eq!(buffer.line_of_byte(7), 1);
    assert_eq!(buffer.line_of_byte(8), 1);
    assert_eq!(buffer.line_of_byte(18), 1);
    assert_eq!(buffer.line_of_byte(19), 1);
    assert_eq!(buffer.line_of_byte(100), 1);
}
