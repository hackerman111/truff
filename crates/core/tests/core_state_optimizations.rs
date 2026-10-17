use truff_core::{Buffer, BytePos, Change, ChangeSet};

#[test]
fn indexed_mapping_matches_linear_mapping_at_adjacent_boundaries() {
    let edits: Vec<_> = (0..300)
        .map(|i| Change::new(3 * i..3 * i + 3, i % 7))
        .collect();
    let changes = ChangeSet::new(edits.clone());
    for pos in 0..1000 {
        let mut shift = 0isize;
        let mut mapped = None;
        for edit in &edits {
            if pos < edit.old_range.start {
                break;
            }
            if pos <= edit.old_range.end {
                mapped = Some((edit.old_range.start as isize + shift) as usize + edit.new_len);
            }
            shift += edit.delta();
        }
        assert_eq!(
            changes.map_pos(pos),
            mapped.unwrap_or((pos as isize + shift) as usize)
        );
    }
}

#[test]
fn streaming_graphemes_match_individual_steps_across_rope_leaves() {
    let text = format!(
        "{}{}{}",
        "a".repeat(1023),
        "e\u{301}👨‍👩‍👧‍👦🇷🇺\r\n".repeat(100),
        "z"
    );
    let buffer = Buffer::from_str(&text);
    for start in [0, 1023, 1024, text.len() - 1, text.len(), text.len() + 1] {
        let mut pos = BytePos::new(start);
        let mut expected = Vec::new();
        while let Some(next) = buffer.grapheme_next(pos) {
            expected.push(next);
            pos = next;
        }
        let actual: Vec<_> = buffer
            .grapheme_boundaries_from(BytePos::new(start))
            .collect();
        if start == 0 {
            use unicode_segmentation::UnicodeSegmentation;
            let oracle: Vec<_> = text
                .grapheme_indices(true)
                .map(|(i, g)| BytePos::new(i + g.len()))
                .collect();
            for (i, (a, b)) in actual.iter().zip(&oracle).enumerate() {
                assert_eq!(a, b, "stream boundary {i}");
            }
            assert_eq!(actual.len(), oracle.len());
        }
        for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
            assert_eq!(a, b, "start={start} boundary={i}");
        }
        assert_eq!(actual.len(), expected.len(), "start={start}");
    }
}

#[test]
fn replacement_aliases_follow_reopens_and_expire_at_epoch_change() {
    use truff_core::workspace::{BufferRegistry, VaultEpoch};
    let mut registry = BufferRegistry::new(VaultEpoch::new(0));
    let first = registry
        .open_from_text(Some("note.md".into()), "a")
        .unwrap();
    registry.close_force(first).unwrap();
    let second = registry
        .open_from_text(Some("note.md".into()), "b")
        .unwrap();
    registry.close_force(second).unwrap();
    assert_eq!(registry.get_replacement_id(first), None);
    let third = registry.register_unloaded("note.md".into()).unwrap();
    assert_eq!(registry.get_replacement_id(first), Some(third));
    assert_eq!(registry.get_replacement_id(second), Some(third));
    registry.record_replacement(third, first);
    assert_eq!(registry.get_replacement_id(first), Some(third));
    registry.set_epoch(VaultEpoch::new(1));
    assert_eq!(registry.get_replacement_id(first), None);
}

#[test]
fn styles_allow_fence_body_and_ignore_false_closers() {
    use truff_core::markdown::style::{StyleError, apply_text_color};
    let text = "```rust\r\nbody\r\n```not-a-close\r\nmore\r\n```\r\nafter";
    let body = text.find("body").unwrap();
    assert!(apply_text_color(text, body..body + 4, "123456").is_ok());
    let start = text.find("more").unwrap();
    assert!(apply_text_color(text, body..start + 4, "123456").is_ok());
    assert!(matches!(
        apply_text_color(text, start..text.len(), "123456"),
        Err(StyleError::IntersectsCodeFence { .. })
    ));
}

#[test]
fn counted_delimited_objects_expand_both_boundaries() {
    use truff_core::vim::{InputContext, InputResult, Key, VimState};
    use truff_core::workspace::{View, ViewId};
    for (kind, expected) in [('i', "()"), ('a', "")] {
        let mut buffer = Buffer::from_str("(left (inner) right)");
        let mut view = View::new(ViewId::new(1), buffer.id());
        view.set_cursor(BytePos::new(8), BytePos::new(8));
        let mut vim = VimState::default();
        for key in ['2', 'd', kind, '('] {
            let result = vim.feed_key(
                Key::Char(key),
                InputContext::Editor {
                    view_id: view.id(),
                    buffer: &mut buffer,
                    view: &mut view,
                },
            );
            assert!(!matches!(result, InputResult::Error(_)), "{result:?}");
        }
        assert_eq!(buffer.as_str(), expected);
        buffer.undo().unwrap();
        assert_eq!(buffer.as_str(), "(left (inner) right)");
    }
}

#[test]
fn grapheme_stream_matches_unicode_segmentation_from_inside_clusters() {
    use unicode_segmentation::UnicodeSegmentation;
    for padding in [0, 1, 7, 31, 127, 511, 1009, 1023] {
        let text = format!(
            "{}{}z",
            "a".repeat(padding),
            "🇷🇺🇬🇧e\u{301}👨‍👩‍👧‍👦\r\n".repeat(70)
        );
        let buffer = Buffer::from_str(&text);
        let ends: Vec<_> = text
            .grapheme_indices(true)
            .map(|(i, g)| BytePos::new(i + g.len()))
            .collect();
        for (start, _) in text.char_indices().step_by(29) {
            let expected: Vec<_> = ends
                .iter()
                .copied()
                .filter(|end| end.get() > start)
                .collect();
            let actual: Vec<_> = buffer
                .grapheme_boundaries_from(BytePos::new(start))
                .collect();
            assert_eq!(actual, expected, "padding={padding}, start={start}");
        }
    }
}

#[test]
fn indexed_batches_map_grouped_undo_and_redo_in_sequence() {
    use truff_core::{SingleEdit, Transaction, UndoGroup};
    let original = "abcd".repeat(100);
    let mut buffer = Buffer::from_str(&original);
    let group = UndoGroup::next();
    let mut applied = Vec::new();
    for _ in 0..2 {
        let mut tx = Transaction::new().with_undo_group(group);
        for i in 0..40 {
            tx.edits.push(SingleEdit::insert(i * 5, "XYZ"));
        }
        applied.push(buffer.apply(tx).unwrap());
    }
    let edited = buffer.as_str();
    let undo = buffer.undo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), original);
    let redo = buffer.redo().unwrap().unwrap();
    assert_eq!(buffer.as_str(), edited);
    assert_eq!(redo.batches().count(), 2);
    assert_eq!(undo.batches().count(), 2);
    for pos in 0..=original.len() {
        let expected = applied
            .iter()
            .fold(pos, |pos, changes| changes.map_pos(pos));
        assert_eq!(redo.map_pos(pos), expected);
    }
}

#[test]
fn registers_share_named_and_unnamed_then_preserve_append_semantics() {
    use truff_core::{Register, RegisterKind, Registers};
    let mut registers = Registers::new();
    registers.set(
        Some('a'),
        Register::new("original", RegisterKind::Characterwise),
    );
    assert!(std::ptr::eq(
        registers.get(None).unwrap(),
        registers.get(Some('a')).unwrap()
    ));
    registers.set(Some('b'), Register::new("other", RegisterKind::Linewise));
    registers.set(
        Some('A'),
        Register::new(" appended", RegisterKind::Characterwise),
    );
    assert_eq!(registers.get(Some('a')).unwrap().text, "original appended");
    assert_eq!(registers.get(Some('b')).unwrap().text, "other");
    assert!(std::ptr::eq(
        registers.get(None).unwrap(),
        registers.get(Some('a')).unwrap()
    ));
}
