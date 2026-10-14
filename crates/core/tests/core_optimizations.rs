use truff_core::markdown::IncrementalSyntaxTree;
use truff_core::vim::motion::{literal_search, snap_grapheme};
use truff_core::{Buffer, BytePos};

#[test]
fn literal_search_matches_reference_with_overlap_wrap_and_unicode() {
    for text in [
        "",
        "aaaaa",
        "abc abc",
        "éé e\u{301}e\u{301} 👨‍👩‍👧‍👦",
        "a\r\na\na",
    ] {
        let buffer = Buffer::from_str(text);
        for query in [
            "", "a", "aa", "abc", "é", "\u{301}", "👩", "\r\n", "missing",
        ] {
            let matches: Vec<_> = (0..text.len())
                .filter(|&pos| text.as_bytes()[pos..].starts_with(query.as_bytes()))
                .collect();
            for pos in 0..=text.len() + 1 {
                for forward in [false, true] {
                    for count in [0, 1, 2, 3, 7, 100] {
                        let mut expected = Some(pos.min(text.len()));
                        if query.is_empty() {
                            expected = None;
                        } else {
                            for _ in 0..count {
                                expected = expected.and_then(|current| {
                                    if forward {
                                        matches
                                            .iter()
                                            .copied()
                                            .find(|&m| m > current)
                                            .or(matches.first().copied())
                                    } else {
                                        matches
                                            .iter()
                                            .rev()
                                            .copied()
                                            .find(|&m| m < current)
                                            .or(matches.last().copied())
                                    }
                                });
                            }
                        }
                        let expected = expected.map(|m| snap_grapheme(&buffer, m));
                        assert_eq!(
                            literal_search(&buffer, pos, query, forward, count),
                            expected,
                            "text={text:?}, query={query:?}, pos={pos}, forward={forward}, count={count}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn search_matches_across_rope_chunks_and_large_counts() {
    let text = format!("{}éneedle{}éneedle", "x".repeat(1023), "y".repeat(2049));
    let buffer = Buffer::from_str(&text);
    let first = text.find("éneedle").unwrap();
    let last = text.rfind("éneedle").unwrap();
    assert_eq!(literal_search(&buffer, 0, "éneedle", true, 3), Some(first));
    assert_eq!(
        literal_search(&buffer, last, "éneedle", false, 2),
        Some(last)
    );
    // A huge repetition count must use modular indexing without count * document scans.
    assert_eq!(
        literal_search(&buffer, 0, "éneedle", true, usize::MAX),
        Some(first)
    );
}

#[test]
fn vertical_motion_preserves_grapheme_column_across_utf8_and_crlf() {
    use truff_core::vim::motion::{Motion, eval_motion};
    let buffer = Buffer::from_str("абв\r\na👨‍👩‍👧‍👦c\r\nabc");
    let target = eval_motion(&buffer, BytePos::new(4), &Motion::Down, 1, None).0;
    assert_eq!(target.get(), "абв\r\na👨‍👩‍👧‍👦".len());
    let target = eval_motion(&buffer, target, &Motion::Down, 1, None).0;
    assert_eq!(target.get(), buffer.byte_len() - 1);
}

#[test]
fn incremental_parse_preserves_old_coordinates_when_lines_are_deleted() {
    use tree_sitter::{InputEdit, Point};
    let before = "# Заголовок\r\n\r\n```rust\r\nlet a = 1;\r\n```\r\n\r\nend\n";
    let start = before.find("```rust").unwrap();
    let end = before.find("end").unwrap();
    let buffer = Buffer::from_str(before);
    let point = |pos| {
        let row = buffer.line_of_byte(pos);
        Point::new(row, pos - buffer.byte_of_line(row))
    };
    let edit = InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: start,
        start_position: point(start),
        old_end_position: point(end),
        new_end_position: point(start),
    };
    let mut rope = buffer.rope().clone();
    rope.replace(start..end, "");
    let mut syntax = IncrementalSyntaxTree::new();
    syntax.init_rope(buffer.rope());
    let incremental = syntax.edit_and_parse(edit, &rope);
    let fresh = IncrementalSyntaxTree::parse_fresh(&rope.to_string()).snapshot();
    assert_eq!(incremental.syntax_tree_sexp(), fresh.syntax_tree_sexp());
    let mut a = incremental.root_node().walk();
    let mut b = fresh.root_node().walk();
    loop {
        assert_eq!(a.node().range(), b.node().range());
        if a.goto_first_child() {
            assert!(b.goto_first_child());
            continue;
        }
        assert!(!b.goto_first_child());
        loop {
            if a.goto_next_sibling() {
                assert!(b.goto_next_sibling());
                break;
            }
            assert!(!b.goto_next_sibling());
            if !a.goto_parent() {
                assert!(!b.goto_parent());
                return;
            }
            assert!(b.goto_parent());
        }
    }
}
