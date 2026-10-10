use std::ops::Range;

use crate::text::{Buffer, BytePos};

/// Find direction and target for `f`, `F`, `t`, `T`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FindKind {
    FindForward,
    FindBackward,
    TillForward,
    TillBackward,
}

/// Motion destination or pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    ScreenUp,
    ScreenDown,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    LineFirstNonBlank,
    LineEnd,
    FirstLine,
    LastLine,
    Line(usize),
    FindCharForward(char),
    FindCharBackward(char),
    TillCharForward(char),
    TillCharBackward(char),
    RepeatFind,
    ReverseRepeatFind,
    ParagraphBackward,
    ParagraphForward,
    CurrentLine,
}

/// Type of text object boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextObjectKind {
    Inner,
    Around,
}

/// Text object target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextObjectTarget {
    Word,
    SingleQuote,
    DoubleQuote,
    Backtick,
    Parentheses,
    Bracket,
    Brace,
    AngleBracket,
    Paragraph,
    Sentence,
    MarkdownLink,
    MarkdownHeading,
    MarkdownCodeBlock,
}

/// Structured text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextObject {
    pub kind: TextObjectKind,
    pub target: TextObjectTarget,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn word_continuation(buffer: &Buffer, position: usize, c: char, category: u8) -> bool {
    char_category(c) == category
        || (category != 0 && !c.is_whitespace() && snap_grapheme(buffer, position) != position)
}

fn char_category(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if is_word_char(c) {
        1
    } else {
        2
    }
}

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

/// Evaluates a text object into a byte range around `pos`.
pub fn eval_text_object_range(
    buffer: &Buffer,
    pos: BytePos,
    obj: TextObject,
) -> Option<Range<usize>> {
    let current = pos.get().min(buffer.byte_len());
    let total_len = buffer.byte_len();
    if total_len == 0 {
        return None;
    }

    match obj.target {
        TextObjectTarget::Word => {
            let category = buffer
                .slice(current..total_len)
                .chars()
                .next()
                .map(char_category)?;
            let mut start = current;
            for c in buffer.slice(0..current).chars().rev() {
                if matches!(c, '\r' | '\n')
                    || !word_continuation(buffer, start - c.len_utf8(), c, category)
                {
                    break;
                }
                start -= c.len_utf8();
            }
            let mut end = current;
            for c in buffer.slice(current..total_len).chars() {
                if matches!(c, '\r' | '\n') || !word_continuation(buffer, end, c, category) {
                    break;
                }
                end += c.len_utf8();
            }
            if obj.kind == TextObjectKind::Around {
                let original = end;
                for c in buffer.slice(end..total_len).chars() {
                    if matches!(c, '\r' | '\n') || !c.is_whitespace() {
                        break;
                    }
                    end += c.len_utf8();
                }
                if end == original {
                    for c in buffer.slice(0..start).chars().rev() {
                        if matches!(c, '\r' | '\n') || !c.is_whitespace() {
                            break;
                        }
                        start -= c.len_utf8();
                    }
                }
            }
            Some(start..end)
        }
        TextObjectTarget::DoubleQuote
        | TextObjectTarget::SingleQuote
        | TextObjectTarget::Backtick => {
            let quote_char = match obj.target {
                TextObjectTarget::DoubleQuote => '"',
                TextObjectTarget::SingleQuote => '\'',
                TextObjectTarget::Backtick => '`',
                _ => unreachable!(),
            };
            find_pair_on_line(buffer, current, quote_char, quote_char, obj.kind)
        }
        TextObjectTarget::Parentheses => find_pair_on_line(buffer, current, '(', ')', obj.kind),
        TextObjectTarget::Bracket => find_pair_on_line(buffer, current, '[', ']', obj.kind),
        TextObjectTarget::Brace => find_pair_on_line(buffer, current, '{', '}', obj.kind),
        TextObjectTarget::AngleBracket => find_pair_on_line(buffer, current, '<', '>', obj.kind),
        TextObjectTarget::MarkdownLink
        | TextObjectTarget::MarkdownHeading
        | TextObjectTarget::MarkdownCodeBlock => None,
        TextObjectTarget::Paragraph => Some(paragraph_range(buffer, current, obj.kind)),
        TextObjectTarget::Sentence => Some(sentence_range(buffer, current, obj.kind)),
    }
}

fn word_forward(buffer: &Buffer, pos: usize) -> usize {
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
fn word_backward(buffer: &Buffer, pos: usize) -> usize {
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
fn word_end(buffer: &Buffer, pos: usize) -> usize {
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

fn find_char_on_line(
    buffer: &Buffer,
    pos: usize,
    target: char,
    forward: bool,
    till: bool,
    count: usize,
) -> Option<usize> {
    let line = buffer.line_of_byte(pos);
    let start = buffer.byte_of_line(line);
    let end = line_content_end(buffer, line);
    let slice = buffer.slice(start..end);
    if forward {
        find_in_chars(buffer, slice.chars(), start, pos, target, true, till, count)
    } else {
        find_in_chars(
            buffer,
            slice.chars().rev(),
            end,
            pos,
            target,
            false,
            till,
            count,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn find_in_chars(
    buffer: &Buffer,
    chars: impl Iterator<Item = char>,
    mut offset: usize,
    pos: usize,
    target: char,
    forward: bool,
    till: bool,
    count: usize,
) -> Option<usize> {
    let mut matches = 0;
    for c in chars {
        if !forward {
            offset -= c.len_utf8();
        }
        if ((forward && offset > pos) || (!forward && offset < pos)) && c == target {
            matches += 1;
            if matches == count {
                return Some(if till {
                    if forward {
                        buffer
                            .grapheme_prev(BytePos::new(offset))
                            .map_or(offset, BytePos::get)
                    } else {
                        buffer
                            .grapheme_next(BytePos::new(offset))
                            .map_or(offset, BytePos::get)
                    }
                } else {
                    snap_grapheme(buffer, offset)
                });
            }
        }
        if forward {
            offset += c.len_utf8();
        }
    }
    None
}

fn find_pair_on_line(
    buffer: &Buffer,
    pos: usize,
    open: char,
    close: char,
    kind: TextObjectKind,
) -> Option<Range<usize>> {
    let line = buffer.line_of_byte(pos);
    let (start, end) = if open == close {
        (buffer.byte_of_line(line), line_content_end(buffer, line))
    } else {
        (0, buffer.byte_len())
    };
    let prefix = buffer.slice(start..pos);
    let mut cursor = pos;
    let mut depth = 0;
    let mut left = None;
    // Include an opening delimiter at the cursor.
    if buffer.slice(pos..end).chars().next() == Some(open) {
        left = Some(pos);
    }
    if left.is_none() {
        for c in prefix.chars().rev() {
            cursor -= c.len_utf8();
            if open != close && c == close {
                depth += 1;
            }
            if c == open {
                if depth == 0 {
                    left = Some(cursor);
                    break;
                }
                depth -= 1;
            }
        }
    }
    let left = left?;
    cursor = left + open.len_utf8();
    depth = 0;
    let suffix = buffer.slice(cursor..end);
    for c in suffix.chars() {
        if c == close {
            if depth == 0 {
                if cursor < pos {
                    return None;
                }
                return Some(if kind == TextObjectKind::Inner {
                    left + open.len_utf8()..cursor
                } else {
                    left..cursor + close.len_utf8()
                });
            }
            depth -= 1;
        } else if open != close && c == open {
            depth += 1;
        }
        cursor += c.len_utf8();
    }
    None
}

fn blank_line(buffer: &Buffer, line: usize) -> bool {
    buffer
        .slice(buffer.byte_of_line(line)..line_content_end(buffer, line))
        .chars()
        .all(char::is_whitespace)
}

fn paragraph_range(buffer: &Buffer, pos: usize, kind: TextObjectKind) -> Range<usize> {
    let mut first = buffer.line_of_byte(pos);
    let mut last = first;
    let blank = blank_line(buffer, first);
    while first > 0 && blank_line(buffer, first - 1) == blank {
        first -= 1;
    }
    while last + 1 < buffer.line_len() && blank_line(buffer, last + 1) == blank {
        last += 1;
    }
    if kind == TextObjectKind::Around {
        while last + 1 < buffer.line_len() && blank_line(buffer, last + 1) {
            last += 1;
        }
    }
    let end = if last + 1 < buffer.line_len() {
        buffer.byte_of_line(last + 1)
    } else {
        buffer.byte_len()
    };
    buffer.byte_of_line(first)..end
}

fn sentence_range(buffer: &Buffer, pos: usize, kind: TextObjectKind) -> Range<usize> {
    let paragraph = paragraph_range(buffer, pos, TextObjectKind::Inner);
    let slice = buffer.slice(paragraph.clone());
    let mut start = paragraph.start;
    let mut offset = start;
    let mut end = paragraph.end;
    let mut boundary = false;
    for c in slice.chars() {
        if boundary && c.is_whitespace() {
            if offset >= pos {
                end = offset;
                break;
            }
            start = offset + c.len_utf8();
        } else {
            boundary = matches!(c, '.' | '!' | '?');
        }
        offset += c.len_utf8();
    }
    if kind == TextObjectKind::Around {
        for c in buffer.slice(end..paragraph.end).chars() {
            if !c.is_whitespace() {
                break;
            }
            end += c.len_utf8();
        }
    }
    start..end
}

/// Syntax producer supplies source coordinates at the current buffer revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxObjectRange {
    pub target: TextObjectTarget,
    pub inner: Range<usize>,
    pub around: Range<usize>,
}

/// Literal UTF-8 search with O(query length) working memory; Rope chunks stay borrowed.
pub fn literal_search(
    buffer: &Buffer,
    pos: usize,
    query: &str,
    forward: bool,
    count: usize,
) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    let needle = query.as_bytes();
    let mut failure = vec![0; needle.len()];
    let mut matched = 0;
    for i in 1..needle.len() {
        while matched > 0 && needle[i] != needle[matched] {
            matched = failure[matched - 1];
        }
        if needle[i] == needle[matched] {
            matched += 1;
        }
        failure[i] = matched;
    }
    let find = |position: usize| {
        let slice = buffer.slice(0..buffer.byte_len());
        let mut matched = 0;
        let mut offset = 0;
        let mut first = None;
        let mut last = None;
        let mut before = None;
        let mut after = None;
        for chunk in slice.chunks() {
            for &byte in chunk.as_bytes() {
                while matched > 0 && byte != needle[matched] {
                    matched = failure[matched - 1];
                }
                if byte == needle[matched] {
                    matched += 1;
                }
                offset += 1;
                if matched == needle.len() {
                    let start = offset - needle.len();
                    first.get_or_insert(start);
                    last = Some(start);
                    if start < position {
                        before = Some(start);
                    }
                    if start > position && after.is_none() {
                        after = Some(start);
                    }
                    matched = failure[matched - 1];
                }
            }
        }
        if forward {
            after.or(first)
        } else {
            before.or(last)
        }
    };
    let mut current = pos.min(buffer.byte_len());
    for _ in 0..count {
        current = find(current)?;
    }
    Some(snap_grapheme(buffer, current))
}
