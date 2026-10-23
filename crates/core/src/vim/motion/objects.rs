use std::ops::Range;

use crate::text::{Buffer, BytePos};

use super::cursor::{blank_line, line_content_end};
use super::types::{TextObject, TextObjectKind, TextObjectTarget};
use super::word::{char_category, word_continuation};

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
            find_pair(buffer, current, quote_char, quote_char, obj.kind, 1)
        }
        TextObjectTarget::Parentheses
        | TextObjectTarget::Bracket
        | TextObjectTarget::Brace
        | TextObjectTarget::AngleBracket => eval_delimited_object_range(buffer, pos, obj, 1),
        TextObjectTarget::MarkdownLink
        | TextObjectTarget::MarkdownHeading
        | TextObjectTarget::MarkdownCodeBlock
        | TextObjectTarget::MarkdownSection
        | TextObjectTarget::MarkdownListItem
        | TextObjectTarget::MarkdownQuote
        | TextObjectTarget::MarkdownTableCell
        | TextObjectTarget::MarkdownTableRow => eval_markdown_object_range(buffer, current, obj),
        TextObjectTarget::Paragraph => Some(paragraph_range(buffer, current, obj.kind)),
        TextObjectTarget::Sentence => Some(sentence_range(buffer, current, obj.kind)),
    }
}

pub(crate) fn eval_delimited_object_range(
    buffer: &Buffer,
    pos: BytePos,
    obj: TextObject,
    count: usize,
) -> Option<Range<usize>> {
    let (open, close) = match obj.target {
        TextObjectTarget::Parentheses => ('(', ')'),
        TextObjectTarget::Bracket => ('[', ']'),
        TextObjectTarget::Brace => ('{', '}'),
        TextObjectTarget::AngleBracket => ('<', '>'),
        _ => return None,
    };
    find_pair(
        buffer,
        pos.get().min(buffer.byte_len()),
        open,
        close,
        obj.kind,
        count,
    )
}

const MAX_DELIMITER_SEARCH_RADIUS: usize = 64 * 1024;

fn find_pair(
    buffer: &Buffer,
    pos: usize,
    open: char,
    close: char,
    kind: TextObjectKind,
    count: usize,
) -> Option<Range<usize>> {
    let line = buffer.line_of_byte(pos);
    let (start, end) = if open == close {
        (buffer.byte_of_line(line), line_content_end(buffer, line))
    } else {
        let min_start = buffer.floor_char_boundary(pos.saturating_sub(MAX_DELIMITER_SEARCH_RADIUS));
        let max_end = buffer.floor_char_boundary(
            pos.saturating_add(MAX_DELIMITER_SEARCH_RADIUS)
                .min(buffer.byte_len()),
        );
        (min_start, max_end)
    };
    let prefix = buffer.slice(start..pos);
    let mut cursor = pos;
    let mut depth = 0;
    let mut left = None;
    let mut remaining = count.max(1);
    // Include an opening delimiter at the cursor.
    if buffer.slice(pos..end).chars().next() == Some(open) {
        left = Some(pos);
        remaining -= 1;
    }
    if remaining > 0 {
        for c in prefix.chars().rev() {
            cursor -= c.len_utf8();
            if open != close && c == close {
                depth += 1;
            }
            if c == open {
                if depth == 0 {
                    left = Some(cursor);
                    remaining -= 1;
                    if remaining == 0 {
                        break;
                    }
                } else {
                    depth -= 1;
                }
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

fn eval_markdown_object_range(
    buffer: &Buffer,
    pos: usize,
    obj: TextObject,
) -> Option<Range<usize>> {
    let snapshot = crate::markdown::MarkdownSnapshot::from_buffer(buffer);
    match obj.target {
        TextObjectTarget::MarkdownHeading => {
            let (i, h) = snapshot
                .facts
                .headings
                .iter()
                .enumerate()
                .rfind(|(_, h)| h.range.start <= pos)?;
            let next_pos = snapshot
                .facts
                .headings
                .get(i + 1)
                .map_or(buffer.byte_len(), |next| next.range.start);
            let text = buffer.as_str();
            let heading_rest = &text[h.range.end..next_pos];
            let after_heading = if heading_rest.starts_with("\r\n") {
                h.range.end + 2
            } else if heading_rest.starts_with('\n') {
                h.range.end + 1
            } else {
                h.range.end
            };
            let body_start = after_heading.min(next_pos);
            match obj.kind {
                TextObjectKind::Inner => Some(body_start..next_pos),
                TextObjectKind::Around => Some(h.range.start..next_pos),
            }
        }
        TextObjectTarget::MarkdownSection => {
            let sec = snapshot
                .sections
                .iter()
                .filter(|s| s.full_range.contains(&pos) || s.heading_range.contains(&pos))
                .min_by_key(|s| s.full_range.len())
                .or_else(|| {
                    snapshot
                        .sections
                        .iter()
                        .rfind(|s| s.full_range.start <= pos)
                })?;
            match obj.kind {
                TextObjectKind::Inner => Some(sec.body_range.clone()),
                TextObjectKind::Around => Some(sec.full_range.clone()),
            }
        }
        TextObjectTarget::MarkdownListItem => {
            if let Some(task) = snapshot.tasks.iter().find(|t| t.line_range.contains(&pos)) {
                return match obj.kind {
                    TextObjectKind::Inner => Some(task.content_range.clone()),
                    TextObjectKind::Around => Some(task.line_range.clone()),
                };
            }
            let text = buffer.as_str();
            let line_start = text[..pos].rfind('\n').map_or(0, |i| i + 1);
            let line_end = text[pos..].find('\n').map_or(text.len(), |i| pos + i);
            let line = &text[line_start..line_end];
            let trimmed = line.trim_start();
            let marker_len = line.len() - trimmed.len();
            if trimmed.starts_with(['-', '*', '+'])
                || trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
            {
                let content_start = line_start + marker_len + 2.min(trimmed.len());
                match obj.kind {
                    TextObjectKind::Inner => Some(content_start..line_end),
                    TextObjectKind::Around => Some(line_start..line_end),
                }
            } else {
                Some(line_start..line_end)
            }
        }
        TextObjectTarget::MarkdownTableCell => {
            for table in &snapshot.tables {
                for row in &table.rows {
                    for cell in &row.cells {
                        if cell.contains(&pos) || (pos == cell.end && pos == buffer.byte_len()) {
                            let slice = buffer.slice(cell.clone()).to_string();
                            let trimmed = slice.trim();
                            let start_offset = slice.find(trimmed).unwrap_or(0);
                            return match obj.kind {
                                TextObjectKind::Inner => Some(
                                    (cell.start + start_offset)
                                        ..(cell.start + start_offset + trimmed.len()),
                                ),
                                TextObjectKind::Around => Some(cell.clone()),
                            };
                        }
                    }
                }
            }
            None
        }
        TextObjectTarget::MarkdownTableRow => {
            for table in &snapshot.tables {
                for row in &table.rows {
                    if row.range.contains(&pos) {
                        return Some(row.range.clone());
                    }
                }
            }
            None
        }
        TextObjectTarget::MarkdownQuote => {
            if let Some(callout) = snapshot.callouts.iter().find(|c| c.range.contains(&pos)) {
                return match obj.kind {
                    TextObjectKind::Inner => Some(callout.content_range.clone()),
                    TextObjectKind::Around => Some(callout.range.clone()),
                };
            }
            if let Some(quote) = snapshot.quotes.iter().find(|q| q.contains(&pos)) {
                return Some(quote.clone());
            }
            None
        }
        TextObjectTarget::MarkdownCodeBlock => {
            if let Some(cb) = snapshot
                .code_blocks
                .iter()
                .find(|cb| cb.range.contains(&pos))
            {
                return match obj.kind {
                    TextObjectKind::Inner => Some(cb.content_range.clone()),
                    TextObjectKind::Around => Some(cb.range.clone()),
                };
            }
            None
        }
        TextObjectTarget::MarkdownLink => {
            if let Some(link) = snapshot.facts.links.iter().find(|l| l.range.contains(&pos)) {
                return Some(link.range.clone());
            }
            None
        }
        _ => None,
    }
}
