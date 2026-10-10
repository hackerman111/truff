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
                    let slice = buffer.slice(current..next_line_start).to_string();
                    if slice.starts_with('\n') || slice.starts_with("\r\n") {
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
            let target_line = (line_idx + count).min(buffer.line_len().saturating_sub(1));
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
            let text = buffer.slice(line_start..line_end).to_string();
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
            let line_end = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                total_len
            };
            let slice = buffer.slice(current..line_end).to_string();
            let mut trim = 0;
            if slice.ends_with("\r\n") {
                trim = 2;
            } else if slice.ends_with('\n') {
                trim = 1;
            }
            current = line_end.saturating_sub(trim);
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
        Motion::ParagraphBackward => {
            let mut line = buffer.line_of_byte(current);
            while line > 0 {
                line -= 1;
                let start = buffer.byte_of_line(line);
                let end = if line + 1 < buffer.line_len() {
                    buffer.byte_of_line(line + 1)
                } else {
                    total_len
                };
                let text = buffer.slice(start..end).to_string();
                if text.trim().is_empty() {
                    current = start;
                    break;
                }
            }
            if line == 0 {
                current = 0;
            }
        }
        Motion::ParagraphForward => {
            let mut line = buffer.line_of_byte(current);
            while line + 1 < buffer.line_len() {
                line += 1;
                let start = buffer.byte_of_line(line);
                let end = if line + 1 < buffer.line_len() {
                    buffer.byte_of_line(line + 1)
                } else {
                    total_len
                };
                let text = buffer.slice(start..end).to_string();
                if text.trim().is_empty() {
                    current = start;
                    break;
                }
            }
            if line + 1 >= buffer.line_len() {
                current = total_len;
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

    (BytePos::new(current), new_find)
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
        let end_line_idx = (line_idx + count).min(buffer.line_len());
        let end = if end_line_idx < buffer.line_len() {
            buffer.byte_of_line(end_line_idx)
        } else {
            buffer.byte_len()
        };
        return (line_start..end, true, last_find);
    }

    let (dest, new_find) = eval_motion(buffer, pos, motion, count, last_find);
    let start = pos.get().min(dest.get());
    let mut end = pos.get().max(dest.get());

    // In Vim, motions like 'w' are exclusive, but 'e' or '$' are inclusive
    if matches!(
        motion,
        Motion::WordEnd | Motion::LineEnd | Motion::FindCharForward(_)
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
            let line_idx = buffer.line_of_byte(current);
            let line_start = buffer.byte_of_line(line_idx);
            let line_end = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                total_len
            };

            let text = buffer.slice(line_start..line_end).to_string();
            let rel = current.saturating_sub(line_start);
            let chars: Vec<(usize, char)> = text.char_indices().collect();
            if chars.is_empty() {
                return None;
            }

            let mut char_idx = 0;
            for (i, &(byte_idx, _)) in chars.iter().enumerate() {
                if byte_idx <= rel {
                    char_idx = i;
                } else {
                    break;
                }
            }

            let target_cat = char_category(chars[char_idx].1);
            let mut start_idx = char_idx;
            while start_idx > 0 && char_category(chars[start_idx - 1].1) == target_cat {
                start_idx -= 1;
            }

            let mut end_idx = char_idx + 1;
            while end_idx < chars.len() && char_category(chars[end_idx].1) == target_cat {
                end_idx += 1;
            }

            let inner_start = line_start + chars[start_idx].0;
            let inner_end = if end_idx < chars.len() {
                line_start + chars[end_idx].0
            } else {
                line_end
            };

            if obj.kind == TextObjectKind::Inner {
                return Some(inner_start..inner_end);
            }

            // Around word includes trailing whitespace if present, else leading
            let mut around_end = inner_end;
            let mut temp_end = end_idx;
            while temp_end < chars.len()
                && chars[temp_end].1.is_whitespace()
                && chars[temp_end].1 != '\n'
            {
                temp_end += 1;
            }
            if temp_end > end_idx {
                around_end = if temp_end < chars.len() {
                    line_start + chars[temp_end].0
                } else {
                    line_end
                };
            }

            Some(inner_start..around_end)
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
        TextObjectTarget::MarkdownLink => find_markdown_link_range(buffer, current, obj.kind),
        TextObjectTarget::MarkdownHeading => find_markdown_heading_range(buffer, current, obj.kind),
        TextObjectTarget::MarkdownCodeBlock => {
            find_markdown_code_block_range(buffer, current, obj.kind)
        }
        TextObjectTarget::Paragraph | TextObjectTarget::Sentence => {
            // Simplified paragraph range
            let line_idx = buffer.line_of_byte(current);
            let start = buffer.byte_of_line(line_idx);
            let end = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                total_len
            };
            Some(start..end)
        }
    }
}

fn word_forward(buffer: &Buffer, pos: usize) -> usize {
    let total_len = buffer.byte_len();
    if pos >= total_len {
        return total_len;
    }
    let slice = buffer.slice(pos..total_len).to_string();
    let chars: Vec<(usize, char)> = slice.char_indices().collect();
    if chars.is_empty() {
        return total_len;
    }

    let initial_cat = char_category(chars[0].1);
    let mut idx = 0;

    // Advance while same category (if not whitespace)
    if initial_cat != 0 {
        while idx < chars.len() && char_category(chars[idx].1) == initial_cat {
            idx += 1;
        }
    }

    // Skip whitespace
    while idx < chars.len() && char_category(chars[idx].1) == 0 {
        idx += 1;
    }

    if idx < chars.len() {
        pos + chars[idx].0
    } else {
        total_len
    }
}

fn word_backward(buffer: &Buffer, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let slice = buffer.slice(0..pos).to_string();
    let chars: Vec<(usize, char)> = slice.char_indices().collect();
    if chars.is_empty() {
        return 0;
    }

    let mut idx = chars.len() - 1;
    // Skip trailing whitespace
    while idx > 0 && char_category(chars[idx].1) == 0 {
        idx -= 1;
    }

    let target_cat = char_category(chars[idx].1);
    while idx > 0 && char_category(chars[idx - 1].1) == target_cat {
        idx -= 1;
    }

    chars[idx].0
}

fn word_end(buffer: &Buffer, pos: usize) -> usize {
    let total_len = buffer.byte_len();
    if pos >= total_len {
        return total_len;
    }
    let slice = buffer.slice(pos..total_len).to_string();
    let chars: Vec<(usize, char)> = slice.char_indices().collect();
    if chars.len() <= 1 {
        return total_len;
    }

    let mut idx = 1;
    // Skip whitespace
    while idx < chars.len() && char_category(chars[idx].1) == 0 {
        idx += 1;
    }

    if idx >= chars.len() {
        return total_len;
    }

    let target_cat = char_category(chars[idx].1);
    while idx + 1 < chars.len() && char_category(chars[idx + 1].1) == target_cat {
        idx += 1;
    }

    pos + chars[idx].0
}

fn find_char_on_line(
    buffer: &Buffer,
    pos: usize,
    target: char,
    forward: bool,
    till: bool,
    count: usize,
) -> Option<usize> {
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = if line_idx + 1 < buffer.line_len() {
        buffer.byte_of_line(line_idx + 1)
    } else {
        buffer.byte_len()
    };

    let text = buffer.slice(line_start..line_end).to_string();
    let rel = pos.saturating_sub(line_start);
    let chars: Vec<(usize, char)> = text.char_indices().collect();

    let mut matches = 0;
    if forward {
        for &(byte_idx, c) in &chars {
            if byte_idx > rel && c == target {
                matches += 1;
                if matches == count {
                    let mut result = line_start + byte_idx;
                    if till && let Some(prev) = buffer.grapheme_prev(BytePos::new(result)) {
                        result = prev.get();
                    }
                    return Some(result);
                }
            }
        }
    } else {
        for &(byte_idx, c) in chars.iter().rev() {
            if byte_idx < rel && c == target {
                matches += 1;
                if matches == count {
                    let mut result = line_start + byte_idx;
                    if till && let Some(next) = buffer.grapheme_next(BytePos::new(result)) {
                        result = next.get();
                    }
                    return Some(result);
                }
            }
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
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = if line_idx + 1 < buffer.line_len() {
        buffer.byte_of_line(line_idx + 1)
    } else {
        buffer.byte_len()
    };

    let text = buffer.slice(line_start..line_end).to_string();
    let rel = pos.saturating_sub(line_start);

    // Find opening before or at pos, closing after pos
    let open_pos = text[..rel].rfind(open)?;
    let close_pos = text[rel..].find(close).map(|i| rel + i)?;

    let abs_open = line_start + open_pos;
    let abs_close = line_start + close_pos;

    if kind == TextObjectKind::Inner {
        let inner_start = abs_open + open.len_utf8();
        Some(inner_start..abs_close)
    } else {
        let outer_end = abs_close + close.len_utf8();
        Some(abs_open..outer_end)
    }
}

fn find_markdown_link_range(
    buffer: &Buffer,
    pos: usize,
    kind: TextObjectKind,
) -> Option<Range<usize>> {
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = if line_idx + 1 < buffer.line_len() {
        buffer.byte_of_line(line_idx + 1)
    } else {
        buffer.byte_len()
    };
    let text = buffer.slice(line_start..line_end).to_string();
    let rel = pos.saturating_sub(line_start);

    // Look for [[...]] or [...]
    if let Some(open) = text[..rel].rfind("[[")
        && let Some(close) = text[rel..].find("]]")
    {
        let start = line_start + open;
        let end = line_start + rel + close + 2;
        return if kind == TextObjectKind::Inner {
            Some((start + 2)..(end - 2))
        } else {
            Some(start..end)
        };
    }
    None
}

fn find_markdown_heading_range(
    buffer: &Buffer,
    pos: usize,
    kind: TextObjectKind,
) -> Option<Range<usize>> {
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = if line_idx + 1 < buffer.line_len() {
        buffer.byte_of_line(line_idx + 1)
    } else {
        buffer.byte_len()
    };
    let text = buffer.slice(line_start..line_end).to_string();
    if text.starts_with('#') {
        if kind == TextObjectKind::Inner {
            let trimmed = text.trim_start_matches('#').trim_start();
            let offset = text.len() - trimmed.len();
            let end_offset = if text.ends_with('\n') {
                line_end.saturating_sub(1)
            } else {
                line_end
            };
            return Some((line_start + offset)..end_offset);
        } else {
            return Some(line_start..line_end);
        }
    }
    None
}

fn find_markdown_code_block_range(
    buffer: &Buffer,
    pos: usize,
    kind: TextObjectKind,
) -> Option<Range<usize>> {
    let total_len = buffer.byte_len();
    let text = buffer.slice(0..total_len).to_string();
    // Scan for ``` boundaries enclosing pos
    let mut in_block = false;
    let mut block_start = 0;
    for (line_num, line) in text.lines().enumerate() {
        let line_offset = buffer.byte_of_line(line_num);
        if line.starts_with("```") {
            if !in_block {
                in_block = true;
                block_start = line_offset;
            } else {
                let block_end = line_offset + line.len() + 1;
                if pos >= block_start && pos <= block_end {
                    return if kind == TextObjectKind::Inner {
                        let inner_start = buffer.byte_of_line(line_num.saturating_sub(1));
                        Some(inner_start..line_offset)
                    } else {
                        Some(block_start..block_end.min(total_len))
                    };
                }
                in_block = false;
            }
        }
    }
    None
}
