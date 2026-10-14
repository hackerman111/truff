use truff_core::vim::motion::{Motion, eval_motion};
use truff_core::{Buffer, BytePos};

fn moved(text: &str, start: usize, motion: Motion, count: usize) -> usize {
    let buffer = Buffer::from_str(text);
    eval_motion(&buffer, BytePos::new(start), &motion, count, None)
        .0
        .get()
}

#[test]
fn horizontal_motions_preserve_line_and_grapheme_bounds() {
    let emoji = "👨‍👩‍👧‍👦";
    for (text, start, motion, expected) in [
        ("ab\ncd", 0, Motion::Right, 2),
        ("ab\ncd", 2, Motion::Left, 0),
        ("ab\r\ncd", 0, Motion::Right, 2),
        ("ab\r\ncd", 5, Motion::Left, 4),
        ("ab", 0, Motion::Right, 1),
        ("ab", 1, Motion::Left, 0),
        ("x", 0, Motion::Right, 0),
        ("x", 0, Motion::Left, 0),
        ("", 0, Motion::Right, 0),
        ("🙂x", 0, Motion::Right, "🙂".len()),
        ("🙂x", "🙂".len(), Motion::Left, 0),
        ("👨‍👩‍👧‍👦x", 0, Motion::Right, emoji.len()),
        ("👨‍👩‍👧‍👦x", emoji.len(), Motion::Left, 0),
    ] {
        assert_eq!(moved(text, start, motion, usize::MAX), expected, "{text:?}");
    }
}
