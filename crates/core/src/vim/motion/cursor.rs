use std::ops::Range;

use crate::text::{Buffer, BytePos};

use super::find::find_char_on_line;
use super::types::{FindKind, Motion};
use super::word::{word_backward, word_end, word_forward};

/// Computes the new cursor position after applying a motion.
pub fn eval_motion(
    buffer: &Buffer,
    pos: BytePos,
    motion: &Motion,
    count: usize,
    last_find: Option<(FindKind, char)>,
) -> (BytePos, Option<(FindKind, char)>) {
    let mut current = pos.get();
    let total_len = buffer.byte_len();
    let mut new_find = last_find;

    if total_len == 0 {
        return (BytePos::ZERO, new_find);
    }

    match motion {
        Motion::Left => {
            for _ in 0..count {
                if let Some(prev) = buffer.grapheme_prev(BytePos::new(current)) {
                    let line_start = buffer.byte_of_line(buffer.line_of_byte(current));
                    if prev.get() >= line_start {
                        current = prev.get();
                    }
                }
            }
        }
        Motion::Right => {
            for _ in 0..count {
                if let Some(next) = buffer.grapheme_next(BytePos::new(current)) {
                    let line_idx = buffer.line_of_byte(current);
                    let next_line_start = if line_idx + 1 < buffer.line_len() {
                        buffer.byte_of_line(line_idx + 1)
                    } else {
                        total_len
                    };
                    // In normal mode don't move past end of line
                    let first = buffer.slice(current..next_line_start).chars().next();
                    if matches!(first, Some('\n' | '\r')) {
                        break;
                    }
                    if next.get() < next_line_start {
                        current = next.get();
                    }
                }
            }
        }
        Motion::Up | Motion::ScreenUp => {
            let line_idx = buffer.line_of_byte(current);
            let target_line = line_idx.saturating_sub(count);
            let col = current - buffer.byte_of_line(line_idx);
            let target_start = buffer.byte_of_line(target_line);
            let target_end = if target_line + 1 < buffer.line_len() {
                buffer.byte_of_line(target_line + 1).saturating_sub(1)
            } else {
                total_len
            };
            current = (target_start + col).min(target_end);
        }
        Motion::Down | Motion::ScreenDown => {
            let line_idx = buffer.line_of_byte(current);
            let target_line =
                (line_idx.saturating_add(count)).min(buffer.line_len().saturating_sub(1));
            let col = current - buffer.byte_of_line(line_idx);
            let target_start = buffer.byte_of_line(target_line);
            let target_end = if target_line + 1 < buffer.line_len() {
                buffer.byte_of_line(target_line + 1).saturating_sub(1)
            } else {
                total_len
            };
            current = (target_start + col).min(target_end);
        }
        Motion::LineStart => {
            let line_idx = buffer.line_of_byte(current);
            current = buffer.byte_of_line(line_idx);
        }
        Motion::LineFirstNonBlank => {
            let line_idx = buffer.line_of_byte(current);
            let line_start = buffer.byte_of_line(line_idx);
            let line_end = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                total_len
            };
            let text = buffer.slice(line_start..line_end);
            let mut offset = 0;
            for c in text.chars() {
                if c == '\n' || c == '\r' || !c.is_whitespace() {
                    break;
                }
                offset += c.len_utf8();
            }
            current = line_start + offset;
        }
        Motion::LineEnd => {
            let line_idx = buffer.line_of_byte(current);
            current = line_content_end(buffer, line_idx);
            if current > buffer.byte_of_line(line_idx)
                && let Some(prev) = buffer.grapheme_prev(BytePos::new(current))
            {
                current = prev.get();
            }
        }
        Motion::FirstLine => {
            let target_line = count.saturating_sub(1);
            let line = target_line.min(buffer.line_len().saturating_sub(1));
            current = buffer.byte_of_line(line);
        }
        Motion::LastLine => {
            let line = if count > 1 {
                (count - 1).min(buffer.line_len().saturating_sub(1))
            } else {
                buffer.line_len().saturating_sub(1)
            };
            current = buffer.byte_of_line(line);
        }
        Motion::Line(n) => {
            let line = (*n)
                .saturating_sub(1)
                .min(buffer.line_len().saturating_sub(1));
            current = buffer.byte_of_line(line);
        }
        Motion::WordForward => {
            for _ in 0..count {
                current = word_forward(buffer, current);
            }
        }
        Motion::WordBackward => {
            for _ in 0..count {
                current = word_backward(buffer, current);
            }
        }
        Motion::WordEnd => {
            for _ in 0..count {
                current = word_end(buffer, current);
            }
        }
        Motion::ParagraphBackward | Motion::ParagraphForward => {
            let forward = *motion == Motion::ParagraphForward;
            let mut line = buffer.line_of_byte(current);
            for _ in 0..count {
                if forward {
                    if line + 1 >= buffer.line_len() {
                        current = total_len;
                        break;
                    }
                    line += 1;
                    while line + 1 < buffer.line_len() && blank_line(buffer, line) {
                        line += 1;
                    }
                    while line + 1 < buffer.line_len() && !blank_line(buffer, line) {
                        line += 1;
                    }
                    current = if line + 1 >= buffer.line_len() && !blank_line(buffer, line) {
                        total_len
                    } else {
                        buffer.byte_of_line(line)
                    };
                } else {
                    if line == 0 {
                        current = 0;
                        break;
                    }
                    line -= 1;
                    while line > 0 && blank_line(buffer, line) {
                        line -= 1;
                    }
                    while line > 0 && !blank_line(buffer, line) {
                        line -= 1;
                    }
                    current = buffer.byte_of_line(line);
                }
            }
        }
        Motion::FindCharForward(c) => {
            new_find = Some((FindKind::FindForward, *c));
            if let Some(target) = find_char_on_line(buffer, current, *c, true, false, count) {
                current = target;
            }
        }
        Motion::FindCharBackward(c) => {
            new_find = Some((FindKind::FindBackward, *c));
            if let Some(target) = find_char_on_line(buffer, current, *c, false, false, count) {
                current = target;
            }
        }
        Motion::TillCharForward(c) => {
            new_find = Some((FindKind::TillForward, *c));
            if let Some(target) = find_char_on_line(buffer, current, *c, true, true, count) {
                current = target;
            }
        }
        Motion::TillCharBackward(c) => {
            new_find = Some((FindKind::TillBackward, *c));
            if let Some(target) = find_char_on_line(buffer, current, *c, false, true, count) {
                current = target;
            }
        }
        Motion::RepeatFind => {
            if let Some((kind, c)) = last_find {
                match kind {
                    FindKind::FindForward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, true, false, count) {
                            current = t;
                        }
                    }
                    FindKind::FindBackward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, false, false, count)
                        {
                            current = t;
                        }
                    }
                    FindKind::TillForward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, true, true, count) {
                            current = t;
                        }
                    }
                    FindKind::TillBackward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, false, true, count) {
                            current = t;
                        }
                    }
                }
            }
        }
        Motion::ReverseRepeatFind => {
            if let Some((kind, c)) = last_find {
                match kind {
                    FindKind::FindForward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, false, false, count)
                        {
                            current = t;
                        }
                    }
                    FindKind::FindBackward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, true, false, count) {
                            current = t;
                        }
                    }
                    FindKind::TillForward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, false, true, count) {
                            current = t;
                        }
                    }
                    FindKind::TillBackward => {
                        if let Some(t) = find_char_on_line(buffer, current, c, true, true, count) {
                            current = t;
                        }
                    }
                }
            }
        }
        Motion::CurrentLine => {
            let line_idx = buffer.line_of_byte(current);
            current = buffer.byte_of_line(line_idx);
        }
    }

    (BytePos::new(snap_grapheme(buffer, current)), new_find)
}

/// Evaluates an operator motion into a byte range.
/// Returns `(range, is_linewise, new_find)`.
pub fn eval_operator_motion_range(
    buffer: &Buffer,
    pos: BytePos,
    motion: &Motion,
    count: usize,
    last_find: Option<(FindKind, char)>,
) -> (Range<usize>, bool, Option<(FindKind, char)>) {
    if *motion == Motion::CurrentLine {
        let line_idx = buffer.line_of_byte(pos.get());
        let line_start = buffer.byte_of_line(line_idx);
        let end_line_idx = (line_idx.saturating_add(count)).min(buffer.line_len());
        let end = if end_line_idx < buffer.line_len() {
            buffer.byte_of_line(end_line_idx)
        } else {
            buffer.byte_len()
        };
        return (line_start..end, true, last_find);
    }

    let (dest, new_find) = eval_motion(buffer, pos, motion, count, last_find);
    let find = match motion {
        Motion::FindCharForward(c) => Some((*c, true, false)),
        Motion::FindCharBackward(c) => Some((*c, false, false)),
        Motion::TillCharForward(c) => Some((*c, true, true)),
        Motion::TillCharBackward(c) => Some((*c, false, true)),
        Motion::RepeatFind | Motion::ReverseRepeatFind => last_find.map(|(kind, c)| {
            let forward = matches!(kind, FindKind::FindForward | FindKind::TillForward);
            (
                c,
                if *motion == Motion::ReverseRepeatFind {
                    !forward
                } else {
                    forward
                },
                matches!(kind, FindKind::TillForward | FindKind::TillBackward),
            )
        }),
        _ => None,
    };
    if let Some((c, forward, till)) = find
        && find_char_on_line(buffer, pos.get(), c, forward, till, count).is_none()
    {
        return (pos.get()..pos.get(), false, new_find);
    }
    if matches!(
        motion,
        Motion::Up | Motion::Down | Motion::FirstLine | Motion::LastLine | Motion::Line(_)
    ) {
        let first = buffer.line_of_byte(pos.get().min(dest.get()));
        let last = buffer.line_of_byte(pos.get().max(dest.get()));
        let end = if last + 1 < buffer.line_len() {
            buffer.byte_of_line(last + 1)
        } else {
            buffer.byte_len()
        };
        return (buffer.byte_of_line(first)..end, true, new_find);
    }
    let start = pos.get().min(dest.get());
    let mut end = pos.get().max(dest.get());

    // In Vim, motions like 'w' are exclusive, but 'e' or '$' are inclusive
    if matches!(
        motion,
        Motion::WordEnd
            | Motion::LineEnd
            | Motion::FindCharForward(_)
            | Motion::FindCharBackward(_)
            | Motion::TillCharForward(_)
            | Motion::TillCharBackward(_)
            | Motion::RepeatFind
            | Motion::ReverseRepeatFind
    ) && let Some(next) = buffer.grapheme_next(BytePos::new(end))
    {
        end = next.get();
    }

    (start..end, false, new_find)
}

/// End of content before LF/CRLF, without copying a logical line.
pub fn line_content_end(buffer: &Buffer, line: usize) -> usize {
    let end = if line + 1 < buffer.line_len() {
        buffer.byte_of_line(line + 1)
    } else {
        buffer.byte_len()
    };
    let start = buffer.byte_of_line(line);
    let slice = buffer.slice(start..end);
    let mut chars = slice.chars().rev();
    if chars.next() == Some('\n') {
        end - if chars.next() == Some('\r') { 2 } else { 1 }
    } else {
        end
    }
}

pub fn snap_grapheme(buffer: &Buffer, pos: usize) -> usize {
    let mut pos = pos.min(buffer.byte_len());
    while !buffer.is_char_boundary(pos) {
        pos -= 1;
    }
    if pos == 0 || pos == buffer.byte_len() {
        return pos;
    }
    let next = buffer.grapheme_next(BytePos::new(pos));
    next.and_then(|n| buffer.grapheme_prev(n))
        .map_or(pos, BytePos::get)
}

pub(super) fn blank_line(buffer: &Buffer, line: usize) -> bool {
    buffer
        .slice(buffer.byte_of_line(line)..line_content_end(buffer, line))
        .chars()
        .all(char::is_whitespace)
}
