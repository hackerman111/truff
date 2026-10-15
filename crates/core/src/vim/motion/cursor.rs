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
    let new_find = last_find;

    if total_len == 0 {
        return (BytePos::ZERO, new_find);
    }

    if let Some(FindMotion { target, find }) = eval_find(buffer, current, motion, count, last_find)
    {
        return (
            BytePos::new(snap_grapheme(buffer, target.unwrap_or(current))),
            find,
        );
    }

    match motion {
        Motion::Left => {
            let line_start = buffer.byte_of_line(buffer.line_of_byte(current));
            for _ in 0..count {
                let Some(prev) = buffer.grapheme_prev(BytePos::new(current)) else {
                    break;
                };
                if prev.get() < line_start {
                    break;
                }
                current = prev.get();
            }
        }
        Motion::Right => {
            let line_idx = buffer.line_of_byte(current);
            let next_line_start = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                total_len
            };
            for _ in 0..count {
                let Some(next) = buffer.grapheme_next(BytePos::new(current)) else {
                    break;
                };
                // In normal mode don't move past end of line
                let first = buffer.slice(current..next_line_start).chars().next();
                if matches!(first, Some('\n' | '\r')) || next.get() >= next_line_start {
                    break;
                }
                current = next.get();
            }
        }
        Motion::Up | Motion::ScreenUp | Motion::Down | Motion::ScreenDown => {
            let line_idx = buffer.line_of_byte(current);
            let target_line = if matches!(motion, Motion::Up | Motion::ScreenUp) {
                line_idx.saturating_sub(count)
            } else {
                line_idx
                    .saturating_add(count)
                    .min(buffer.line_len().saturating_sub(1))
            };
            let col = buffer
                .grapheme_boundaries_from(BytePos::new(buffer.byte_of_line(line_idx)))
                .take_while(|boundary| boundary.get() <= current)
                .count();
            let target_start = buffer.byte_of_line(target_line);
            let target_end = line_content_end(buffer, target_line);
            current = buffer
                .grapheme_boundaries_from(BytePos::new(target_start))
                .take(col)
                .map(BytePos::get)
                .take_while(|&end| end <= target_end)
                .last()
                .unwrap_or(target_start);
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
        Motion::FindCharForward(_)
        | Motion::FindCharBackward(_)
        | Motion::TillCharForward(_)
        | Motion::TillCharBackward(_)
        | Motion::RepeatFind
        | Motion::ReverseRepeatFind => unreachable!("find motions resolved above"),
        Motion::CurrentLine => {
            let line_idx = buffer.line_of_byte(current);
            current = buffer.byte_of_line(line_idx);
        }
        Motion::MatchingBracket => {
            current = find_matching_bracket(buffer, current);
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

    let (dest, new_find) = if let Some(FindMotion { target, find }) =
        eval_find(buffer, pos.get(), motion, count, last_find)
    {
        let Some(target) = target else {
            return (pos.get()..pos.get(), false, find);
        };
        (BytePos::new(snap_grapheme(buffer, target)), find)
    } else {
        eval_motion(buffer, pos, motion, count, last_find)
    };
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

struct FindMotion {
    target: Option<usize>,
    find: Option<(FindKind, char)>,
}

// None denotes a non-find motion; a missing target still retains find state.
fn eval_find(
    buffer: &Buffer,
    pos: usize,
    motion: &Motion,
    count: usize,
    last: Option<(FindKind, char)>,
) -> Option<FindMotion> {
    let explicit = match motion {
        Motion::FindCharForward(c) => Some((FindKind::FindForward, *c)),
        Motion::FindCharBackward(c) => Some((FindKind::FindBackward, *c)),
        Motion::TillCharForward(c) => Some((FindKind::TillForward, *c)),
        Motion::TillCharBackward(c) => Some((FindKind::TillBackward, *c)),
        Motion::RepeatFind | Motion::ReverseRepeatFind => None,
        _ => return None,
    };
    let find = explicit.or(last);
    let Some((kind, c)) = find else {
        return Some(FindMotion {
            target: None,
            find: last,
        });
    };
    let forward = matches!(kind, FindKind::FindForward | FindKind::TillForward)
        ^ (*motion == Motion::ReverseRepeatFind);
    let till = matches!(kind, FindKind::TillForward | FindKind::TillBackward);
    Some(FindMotion {
        target: find_char_on_line(buffer, pos, c, forward, till, count),
        find,
    })
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

fn find_matching_bracket(buffer: &Buffer, pos: usize) -> usize {
    let text = buffer.as_str();
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return pos;
    }
    let pos = pos.min(bytes.len() - 1);
    let line_idx = buffer.line_of_byte(pos);
    let line_end = line_content_end(buffer, line_idx);

    // If character at pos is not a bracket, scan forward on current line for the first bracket
    let mut bracket_pos = pos;
    let is_bracket = |b: u8| matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}');
    if !is_bracket(bytes[bracket_pos]) {
        if let Some(first) = (bracket_pos..line_end).find(|&i| is_bracket(bytes[i])) {
            bracket_pos = first;
        } else {
            return pos;
        }
    }

    let b = bytes[bracket_pos];
    match b {
        b'(' => find_forward_match(bytes, bracket_pos, b'(', b')'),
        b'[' => find_forward_match(bytes, bracket_pos, b'[', b']'),
        b'{' => find_forward_match(bytes, bracket_pos, b'{', b'}'),
        b')' => find_backward_match(bytes, bracket_pos, b'(', b')'),
        b']' => find_backward_match(bytes, bracket_pos, b'[', b']'),
        b'}' => find_backward_match(bytes, bracket_pos, b'{', b'}'),
        _ => pos,
    }
}

fn find_forward_match(bytes: &[u8], start: usize, open: u8, close: u8) -> usize {
    let mut depth = 0;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if b == open {
            depth += 1;
        } else if b == close {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
    }
    start
}

fn find_backward_match(bytes: &[u8], start: usize, open: u8, close: u8) -> usize {
    let mut depth = 0;
    for i in (0..=start).rev() {
        if bytes[i] == close {
            depth += 1;
        } else if bytes[i] == open {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
    }
    start
}
