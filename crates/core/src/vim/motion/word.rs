use crate::text::{Buffer, BytePos};

use super::cursor::snap_grapheme;

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub(super) fn word_continuation(buffer: &Buffer, position: usize, c: char, category: u8) -> bool {
    char_category(c) == category
        || (category != 0
            && c.is_ascii()
            && !c.is_whitespace()
            && snap_grapheme(buffer, position) != position)
}

pub(super) fn char_category(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if is_word_char(c) {
        1
    } else {
        2
    }
}

pub(super) fn word_forward(buffer: &Buffer, pos: usize) -> usize {
    let slice = buffer.slice(pos..buffer.byte_len());
    let mut chars = slice.chars().peekable();
    let category = chars.peek().copied().map(char_category).unwrap_or(0);
    let mut current = pos;
    if category != 0 {
        while chars
            .peek()
            .is_some_and(|c| word_continuation(buffer, current, *c, category))
        {
            current += chars.next().unwrap().len_utf8();
        }
    }
    while chars.peek().is_some_and(|c| c.is_whitespace()) {
        current += chars.next().unwrap().len_utf8();
    }
    current
}
pub(super) fn word_backward(buffer: &Buffer, pos: usize) -> usize {
    let slice = buffer.slice(0..pos);
    let mut chars = slice.chars().rev().peekable();
    let mut current = pos;
    while chars.peek().is_some_and(|c| c.is_whitespace()) {
        current -= chars.next().unwrap().len_utf8();
    }
    let cluster = buffer
        .grapheme_prev(BytePos::new(current))
        .map_or(0, BytePos::get);
    let category = buffer
        .slice(cluster..current)
        .chars()
        .next()
        .map(char_category)
        .unwrap_or(0);
    while chars
        .peek()
        .is_some_and(|c| word_continuation(buffer, current - c.len_utf8(), *c, category))
    {
        current -= chars.next().unwrap().len_utf8();
    }
    current
}
pub(super) fn word_end(buffer: &Buffer, pos: usize) -> usize {
    let start = buffer
        .grapheme_next(BytePos::new(pos))
        .map_or(pos, BytePos::get);
    let slice = buffer.slice(start..buffer.byte_len());
    let mut chars = slice.chars().peekable();
    let mut current = start;
    while chars.peek().is_some_and(|c| c.is_whitespace()) {
        current += chars.next().unwrap().len_utf8();
    }
    let category = chars.peek().copied().map(char_category).unwrap_or(0);
    let mut result = current;
    while chars
        .peek()
        .is_some_and(|c| word_continuation(buffer, current, *c, category))
    {
        result = current;
        current += chars.next().unwrap().len_utf8();
    }
    snap_grapheme(buffer, result)
}
