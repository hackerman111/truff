use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

use crate::text::{Buffer, BytePos, LineEnding, SingleEdit, Transaction, UndoGroup};
use crate::vim::motion::line_content_end;

#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkerKind {
    Bullet(char),
    Ordered { number: usize, delimiter: char },
    Task { bullet: char, checked: bool },
    QuoteOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedLinePrefix {
    line_start: usize,
    line_end: usize,
    quote_prefix: String,
    indent: String,
    marker_range: Range<usize>,
    content_start: usize,
    kind: MarkerKind,
}

impl ParsedLinePrefix {
    fn is_empty_item(&self, line_text: &str) -> bool {
        let rel_content = self.content_start - self.line_start;
        line_text
            .get(rel_content..)
            .is_none_or(|s| s.trim().is_empty())
    }
}

fn parse_line_prefix(buffer: &Buffer, line_idx: usize) -> Option<ParsedLinePrefix> {
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = line_content_end(buffer, line_idx);
    if line_start >= line_end {
        return None;
    }
    let line_str = buffer.slice(line_start..line_end).to_string();
    let bytes = line_str.as_bytes();

    // Ignore table rows
    if line_str.trim_start().starts_with('|') {
        return None;
    }

    // 1. Parse optional leading blockquote `>` prefixes
    let mut quote_end = 0;
    let mut scan = 0;
    while scan < bytes.len() {
        let ws_start = scan;
        while scan < bytes.len() && matches!(bytes[scan], b' ' | b'\t') && scan - ws_start <= 3 {
            scan += 1;
        }
        if scan < bytes.len() && bytes[scan] == b'>' {
            scan += 1;
            if scan < bytes.len() && bytes[scan] == b' ' {
                scan += 1;
            }
            quote_end = scan;
        } else {
            break;
        }
    }
    let quote_prefix = line_str[..quote_end].to_string();
    let mut idx = quote_end;

    // 2. Parse indentation after quote prefix
    let indent_start = idx;
    while idx < bytes.len() && matches!(bytes[idx], b' ' | b'\t') {
        idx += 1;
    }
    let indent = line_str[indent_start..idx].to_string();
    let rest = &line_str[idx..];

    // 3. Check bullet or task list: `- `, `* `, `+ `
    if let Some(first) = rest.chars().next()
        && matches!(first, '-' | '*' | '+')
        && rest.as_bytes().get(1) == Some(&b' ')
    {
        let after_bullet = &rest[2..];
        if after_bullet.len() >= 4
            && after_bullet.as_bytes()[0] == b'['
            && matches!(after_bullet.as_bytes()[1], b' ' | b'x' | b'X' | b'/' | b'-')
            && after_bullet.as_bytes()[2] == b']'
            && (after_bullet.as_bytes().get(3) == Some(&b' ') || after_bullet.len() == 3)
        {
            let checked = matches!(after_bullet.as_bytes()[1], b'x' | b'X');
            let marker_len = if after_bullet.as_bytes().get(3) == Some(&b' ') {
                6
            } else {
                5
            };
            let content_start = line_start + idx + marker_len;
            return Some(ParsedLinePrefix {
                line_start,
                line_end,
                quote_prefix,
                indent,
                marker_range: (line_start + idx)..content_start,
                content_start,
                kind: MarkerKind::Task {
                    bullet: first,
                    checked,
                },
            });
        }
        if after_bullet.starts_with("[] ") || after_bullet == "[]" {
            let marker_len = if after_bullet.starts_with("[] ") {
                5
            } else {
                4
            };
            let content_start = line_start + idx + marker_len;
            return Some(ParsedLinePrefix {
                line_start,
                line_end,
                quote_prefix,
                indent,
                marker_range: (line_start + idx)..content_start,
                content_start,
                kind: MarkerKind::Task {
                    bullet: first,
                    checked: false,
                },
            });
        }
        let content_start = line_start + idx + 2;
        return Some(ParsedLinePrefix {
            line_start,
            line_end,
            quote_prefix,
            indent,
            marker_range: (line_start + idx)..content_start,
            content_start,
            kind: MarkerKind::Bullet(first),
        });
    }

    // 3b. Standalone task checkbox without leading bullet: `[ ] `, `[x] `, `[] `, etc.
    if rest.starts_with("[ ] ")
        || rest.starts_with("[x] ")
        || rest.starts_with("[X] ")
        || rest.starts_with("[/] ")
        || rest.starts_with("[-] ")
        || rest == "[ ]"
    {
        let checked = rest.starts_with("[x] ") || rest.starts_with("[X] ");
        let marker_len = if rest.starts_with('[') && rest.len() >= 4 && rest.as_bytes()[3] == b' ' {
            4
        } else {
            3
        };
        let content_start = line_start + idx + marker_len;
        return Some(ParsedLinePrefix {
            line_start,
            line_end,
            quote_prefix,
            indent,
            marker_range: (line_start + idx)..content_start,
            content_start,
            kind: MarkerKind::Task {
                bullet: '-',
                checked,
            },
        });
    }
    if rest.starts_with("[] ") || rest == "[]" {
        let marker_len = if rest.starts_with("[] ") { 3 } else { 2 };
        let content_start = line_start + idx + marker_len;
        return Some(ParsedLinePrefix {
            line_start,
            line_end,
            quote_prefix,
            indent,
            marker_range: (line_start + idx)..content_start,
            content_start,
            kind: MarkerKind::Task {
                bullet: '-',
                checked: false,
            },
        });
    }

    // 4. Check ordered list: `12. ` or `12) `
    let digits_len = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits_len > 0
        && let Some(&delim) = rest.as_bytes().get(digits_len)
        && matches!(delim, b'.' | b')')
        && rest.as_bytes().get(digits_len + 1) == Some(&b' ')
        && let Ok(number) = rest[..digits_len].parse::<usize>()
    {
        let marker_len = digits_len + 2;
        let content_start = line_start + idx + marker_len;
        return Some(ParsedLinePrefix {
            line_start,
            line_end,
            quote_prefix,
            indent,
            marker_range: (line_start + idx)..content_start,
            content_start,
            kind: MarkerKind::Ordered {
                number,
                delimiter: delim as char,
            },
        });
    }

    // 5. Quote-only line
    if !quote_prefix.is_empty() {
        return Some(ParsedLinePrefix {
            line_start,
            line_end,
            quote_prefix,
            indent,
            marker_range: line_start..(line_start + quote_end),
            content_start: line_start + quote_end,
            kind: MarkerKind::QuoteOnly,
        });
    }

    None
}

fn is_in_code_block_or_frontmatter_or_table(buffer: &Buffer, pos: usize) -> bool {
    let snapshot = crate::markdown::snapshot::MarkdownSnapshot::from_buffer(buffer);
    if let Some(fm) = &snapshot.frontmatter
        && (fm.range.contains(&pos) || (pos == fm.range.end && !fm.range.is_empty()))
    {
        return true;
    }
    for cb in &snapshot.code_blocks {
        if cb.range.contains(&pos) || (pos == cb.range.end && !cb.range.is_empty()) {
            if cb
                .language
                .as_deref()
                .is_some_and(|l| l.eq_ignore_ascii_case("tasks") || l.eq_ignore_ascii_case("task"))
            {
                if cb.content_range.contains(&pos)
                    || (pos == cb.content_range.end && !cb.content_range.is_empty())
                {
                    continue;
                }
                return true;
            }
            let slice = buffer.slice(cb.range.clone()).to_string();
            let is_fenced = slice.starts_with("```") || slice.starts_with("~~~");
            if is_fenced {
                return true;
            }
        }
    }
    for t in &snapshot.tables {
        if t.range.contains(&pos) || (pos == t.range.end && !t.range.is_empty()) {
            return true;
        }
    }
    false
}

/// Plans an atomic smart `Enter` edit in Markdown lists, tasks, ordered lists, and blockquotes.
/// Returns `None` on plain paragraphs, fenced code, YAML frontmatter, and tables.
pub fn plan_enter(buffer: &Buffer, cursor: BytePos) -> Option<Transaction> {
    let pos = cursor.get().min(buffer.byte_len());
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = line_content_end(buffer, line_idx);
    let line_str = buffer.slice(line_start..line_end).to_string();

    if super::table::is_table_row(&line_str) {
        if super::table::is_table_delimiter(&line_str) {
            return None;
        }
        if line_idx + 1 < buffer.line_len() {
            let next_start = buffer.byte_of_line(line_idx + 1);
            let next_end = line_content_end(buffer, line_idx + 1);
            let next_str = buffer.slice(next_start..next_end).to_string();
            if super::table::is_table_delimiter(&next_str) {
                return None;
            }
        }
        return plan_table_enter(buffer, pos, line_idx, line_start, line_end, &line_str);
    }
    if is_in_code_block_or_frontmatter_or_table(buffer, pos) {
        return None;
    }
    let parsed = parse_line_prefix(buffer, line_idx)?;
    let nl = if buffer.line_ending() == LineEnding::CrLf {
        "\r\n"
    } else {
        "\n"
    };
    let group = UndoGroup::next();

    // Rule: Empty item lifting and closure (§3.1):
    // "Для вложенного пустого пункта однократный Enter снимает текущий маркер и поднимает
    // вложенность на один уровень; повторный Enter на оставшемся пустом уровне завершает список."
    if parsed.is_empty_item(&line_str) {
        if !parsed.indent.is_empty() {
            let indent_step = if parsed.indent.starts_with('\t') {
                1
            } else if parsed.indent.len() >= 4 {
                4
            } else if parsed.indent.len() >= 2 {
                2
            } else {
                parsed.indent.len()
            };
            let new_indent = &parsed.indent[indent_step..];
            let marker_str = match parsed.kind {
                MarkerKind::Bullet(b) => format!("{b} "),
                MarkerKind::Task { bullet, .. } => format!("{bullet} [ ] "),
                MarkerKind::Ordered { number, delimiter } => format!("{number}{delimiter} "),
                MarkerKind::QuoteOnly => String::new(),
            };
            let replacement = format!("{}{}{}", parsed.quote_prefix, new_indent, marker_str);
            let new_cursor = BytePos::new(parsed.line_start + replacement.len());
            return Some(
                Transaction::replace(parsed.line_start..parsed.line_end, replacement)
                    .with_cursor(new_cursor, new_cursor)
                    .with_undo_group(group),
            );
        }

        let replacement = if !parsed.quote_prefix.is_empty() && parsed.kind != MarkerKind::QuoteOnly
        {
            parsed.quote_prefix.clone()
        } else {
            String::new()
        };
        let new_cursor = BytePos::new(parsed.line_start + replacement.len());
        return Some(
            Transaction::replace(parsed.line_start..parsed.line_end, replacement)
                .with_cursor(new_cursor, new_cursor)
                .with_undo_group(group),
        );
    }

    // If cursor is before content_start, clamp split point to content_start
    let split_pos = pos.max(parsed.content_start);
    let rel_split = split_pos.saturating_sub(parsed.line_start);
    let after_split = line_str.get(rel_split..).unwrap_or("");
    let has_indent = !parsed.indent.is_empty() && after_split.starts_with(&parsed.indent);
    let rem = if has_indent {
        &after_split[parsed.indent.len()..]
    } else {
        after_split
    };
    let needed_indent = if has_indent {
        ""
    } else {
        parsed.indent.as_str()
    };

    let next_prefix = match parsed.kind {
        MarkerKind::Bullet(b) => {
            let marker = format!("{b} ");
            if rem.starts_with(&marker) {
                format!("{}{}", parsed.quote_prefix, needed_indent)
            } else {
                format!("{}{}{b} ", parsed.quote_prefix, parsed.indent)
            }
        }
        MarkerKind::Task { bullet, .. } => {
            let has_full_task = rem.starts_with(&format!("{bullet} [ ] "))
                || rem.starts_with(&format!("{bullet} [x] "))
                || rem.starts_with(&format!("{bullet} [X] "))
                || rem.starts_with(&format!("{bullet} [/] "))
                || rem.starts_with(&format!("{bullet} [-] "))
                || rem.starts_with(&format!("{bullet} [] "));
            let has_checkbox_only = rem.starts_with("[ ] ")
                || rem.starts_with("[x] ")
                || rem.starts_with("[X] ")
                || rem.starts_with("[/] ")
                || rem.starts_with("[-] ")
                || rem.starts_with("[] ");

            if has_full_task {
                format!("{}{}", parsed.quote_prefix, needed_indent)
            } else if has_checkbox_only {
                format!("{}{}{bullet} ", parsed.quote_prefix, needed_indent)
            } else {
                format!("{}{}{bullet} [ ] ", parsed.quote_prefix, parsed.indent)
            }
        }
        MarkerKind::Ordered { number, delimiter } => {
            let digits_len = rem.bytes().take_while(|b| b.is_ascii_digit()).count();
            let has_ordered = digits_len > 0
                && rem.as_bytes().get(digits_len) == Some(&(delimiter as u8))
                && rem.as_bytes().get(digits_len + 1) == Some(&b' ');

            if has_ordered {
                format!("{}{}", parsed.quote_prefix, needed_indent)
            } else {
                format!(
                    "{}{}{}{} ",
                    parsed.quote_prefix,
                    parsed.indent,
                    number + 1,
                    delimiter
                )
            }
        }
        MarkerKind::QuoteOnly => parsed.quote_prefix.clone(),
    };

    let inserted = format!("{nl}{next_prefix}");
    let new_cursor = BytePos::new(split_pos + inserted.len());
    let tx = Transaction::new()
        .with_edit(SingleEdit::insert(split_pos, inserted))
        .with_cursor(new_cursor, new_cursor)
        .with_undo_group(group);

    // Numbering per §3.1: "автоматически увеличивается число только у нового пункта,
    // не производится массовая перенумерация документа."

    Some(tx)
}

/// Plans an atomic `Tab` indentation for a Markdown list/task item and its child subtree (§3.2).
pub fn plan_tab(buffer: &Buffer, cursor: BytePos) -> Option<Transaction> {
    let pos = cursor.get().min(buffer.byte_len());
    if is_in_code_block_or_frontmatter_or_table(buffer, pos) {
        return None;
    }
    let line_idx = buffer.line_of_byte(pos);
    let parsed = parse_line_prefix(buffer, line_idx)?;
    if parsed.kind == MarkerKind::QuoteOnly {
        return None;
    }
    let indent_unit = "    "; // 4 spaces default
    let parent_indent_len = parsed.indent.len();
    let mut edits = Vec::new();

    // 1. Indent current item
    let insert_pos = parsed.line_start + parsed.quote_prefix.len();
    edits.push(SingleEdit::insert(insert_pos, indent_unit));

    // 2. Indent child subtree
    for sub_line in (line_idx + 1)..buffer.line_len() {
        let Some(sub_parsed) = parse_line_prefix(buffer, sub_line) else {
            break;
        };
        if sub_parsed.quote_prefix != parsed.quote_prefix
            || sub_parsed.indent.len() <= parent_indent_len
        {
            break;
        }
        let sub_insert_pos = sub_parsed.line_start + sub_parsed.quote_prefix.len();
        edits.push(SingleEdit::insert(sub_insert_pos, indent_unit));
    }

    edits.sort_by_key(|e| e.range.start);

    let new_cursor = BytePos::new(pos + indent_unit.len());
    let mut tx = Transaction::new()
        .with_cursor(new_cursor, new_cursor)
        .with_undo_group(UndoGroup::next());
    tx.edits = edits;

    Some(tx)
}

/// Plans an atomic `Shift+Tab` outdentation for a Markdown list/task item and its child subtree (§3.2).
pub fn plan_shift_tab(buffer: &Buffer, cursor: BytePos) -> Option<Transaction> {
    let pos = cursor.get().min(buffer.byte_len());
    if is_in_code_block_or_frontmatter_or_table(buffer, pos) {
        return None;
    }
    let line_idx = buffer.line_of_byte(pos);
    let parsed = parse_line_prefix(buffer, line_idx)?;
    if parsed.kind == MarkerKind::QuoteOnly || parsed.indent.is_empty() {
        return None;
    }
    let remove_len = if parsed.indent.starts_with('\t') {
        1
    } else {
        let spaces = parsed.indent.bytes().take_while(|&b| b == b' ').count();
        if spaces >= 4 && spaces % 4 == 0 {
            4
        } else if spaces > 4 && spaces % 4 != 0 {
            spaces % 4
        } else if spaces >= 4 {
            4
        } else {
            spaces
        }
    };
    if remove_len == 0 {
        return None;
    }
    let parent_indent_len = parsed.indent.len();
    let mut edits = Vec::new();

    // 1. Outdent current item
    let indent_start = parsed.line_start + parsed.quote_prefix.len();
    edits.push(SingleEdit::delete(
        indent_start..(indent_start + remove_len),
    ));

    // 2. Outdent child subtree
    for sub_line in (line_idx + 1)..buffer.line_len() {
        let Some(sub_parsed) = parse_line_prefix(buffer, sub_line) else {
            break;
        };
        if sub_parsed.quote_prefix != parsed.quote_prefix
            || sub_parsed.indent.len() <= parent_indent_len
        {
            break;
        }
        let child_remove = if sub_parsed.indent.starts_with('\t') {
            1
        } else {
            sub_parsed
                .indent
                .bytes()
                .take_while(|&b| b == b' ')
                .count()
                .min(remove_len)
        };
        if child_remove > 0 {
            let child_start = sub_parsed.line_start + sub_parsed.quote_prefix.len();
            edits.push(SingleEdit::delete(
                child_start..(child_start + child_remove),
            ));
        }
    }

    edits.sort_by_key(|e| e.range.start);

    let new_cursor = BytePos::new(pos.saturating_sub(remove_len).max(indent_start));
    let mut tx = Transaction::new()
        .with_cursor(new_cursor, new_cursor)
        .with_undo_group(UndoGroup::next());
    tx.edits = edits;

    Some(tx)
}

/// Finds the task checkbox position on the line containing `cursor`, if present.
pub fn find_task_checkbox_on_line(buffer: &Buffer, cursor: BytePos) -> Option<usize> {
    let line_idx = buffer.line_of_byte(cursor.get().min(buffer.byte_len()));
    let prefix = parse_line_prefix(buffer, line_idx)?;
    if let MarkerKind::Task { .. } = prefix.kind {
        let marker_str = buffer.slice(prefix.marker_range.clone()).to_string();
        let bracket = marker_str.find('[')?;
        Some(prefix.marker_range.start + bracket + 1)
    } else {
        None
    }
}

/// Plans toggling the task checkbox on the line containing `cursor`.
pub fn plan_toggle_task(buffer: &Buffer, cursor: BytePos) -> Option<Transaction> {
    let checkbox = find_task_checkbox_on_line(buffer, cursor)?;
    super::presentation::toggle_task(buffer, checkbox)
}

/// Plans cycling the task checkbox status (`[ ]` -> `[/]` -> `[x]` -> `[ ]`) on the line containing `cursor`.
pub fn plan_cycle_task(buffer: &Buffer, cursor: BytePos) -> Option<Transaction> {
    let checkbox = find_task_checkbox_on_line(buffer, cursor)?;
    super::presentation::cycle_task_status(buffer, checkbox)
}

/// Plans smart Backspace: deletes an entire indentation step (tab / spaces), deletes task marker in 1 backspace, or outdents empty list item.
pub fn plan_backspace(buffer: &Buffer, cursor: BytePos) -> Option<Transaction> {
    let pos = cursor.get().min(buffer.byte_len());
    if pos == 0 {
        return None;
    }
    if is_in_code_block_or_frontmatter_or_table(buffer, pos) {
        return None;
    }
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = line_content_end(buffer, line_idx);
    if pos == line_start {
        return None;
    }
    let line_str = buffer.slice(line_start..line_end).to_string();
    let before_cursor = buffer.slice(line_start..pos).to_string();

    if let Some(parsed) = parse_line_prefix(buffer, line_idx) {
        let is_empty = parsed.is_empty_item(&line_str);
        if matches!(parsed.kind, MarkerKind::Task { .. }) {
            let in_empty_task = is_empty
                && pos >= (parsed.line_start + parsed.quote_prefix.len())
                && pos <= line_end;
            let at_content_start = pos == parsed.content_start;
            if in_empty_task || at_content_start {
                if !parsed.indent.is_empty()
                    && !is_empty
                    && let Some(tx) = plan_shift_tab(buffer, cursor)
                {
                    return Some(tx);
                }
                let delete_range = if is_empty {
                    (parsed.line_start + parsed.quote_prefix.len())..line_end
                } else {
                    parsed.marker_range.clone()
                };
                let new_pos = BytePos::new(delete_range.start);
                let tx = Transaction::new()
                    .with_edit(SingleEdit::delete(delete_range))
                    .with_cursor(new_pos, new_pos)
                    .with_undo_group(UndoGroup::next());
                return Some(tx);
            }
        } else if matches!(parsed.kind, MarkerKind::Bullet(_)) {
            let in_empty_bullet = is_empty
                && pos >= (parsed.line_start + parsed.quote_prefix.len())
                && pos <= line_end;
            let at_content_start = pos == parsed.content_start;
            if (in_empty_bullet || at_content_start) && parsed.indent.is_empty() {
                let delete_range = if is_empty {
                    (parsed.line_start + parsed.quote_prefix.len())..line_end
                } else {
                    parsed.marker_range.clone()
                };
                let new_pos = BytePos::new(delete_range.start);
                let tx = Transaction::new()
                    .with_edit(SingleEdit::delete(delete_range))
                    .with_cursor(new_pos, new_pos)
                    .with_undo_group(UndoGroup::next());
                return Some(tx);
            }
        }
    }

    let trailing_spaces = before_cursor
        .chars()
        .rev()
        .take_while(|&c| c == ' ')
        .count();
    if trailing_spaces >= 2 {
        let delete_len = if trailing_spaces >= 4
            && (trailing_spaces % 4 == 0 || before_cursor.trim().is_empty() && trailing_spaces == 4)
        {
            4
        } else if trailing_spaces % 2 == 0 {
            2
        } else {
            2.min(trailing_spaces)
        };
        let new_pos = BytePos::new(pos - delete_len);
        let tx = Transaction::new()
            .with_edit(SingleEdit::delete((pos - delete_len)..pos))
            .with_cursor(new_pos, new_pos)
            .with_undo_group(UndoGroup::next());
        return Some(tx);
    }
    if let Some(parsed) = parse_line_prefix(buffer, line_idx)
        && pos == parsed.content_start
        && parsed.kind != MarkerKind::QuoteOnly
    {
        return plan_shift_tab(buffer, cursor);
    }
    None
}

fn plan_table_enter(
    buffer: &Buffer,
    pos: usize,
    _line_idx: usize,
    line_start: usize,
    line_end: usize,
    line_str: &str,
) -> Option<Transaction> {
    let nl = if buffer.line_ending() == LineEnding::CrLf {
        "\r\n"
    } else {
        "\n"
    };
    let cells = super::table::split_row_cells(line_str);
    let is_empty = cells.iter().all(|c| c.trim().is_empty());

    if is_empty {
        // Exit table: delete empty table row leaving a blank line after the table
        let delete_range = line_start..line_end;
        let new_pos = BytePos::new(line_start);
        return Some(
            Transaction::new()
                .with_edit(SingleEdit::delete(delete_range))
                .with_cursor(new_pos, new_pos)
                .with_undo_group(UndoGroup::next()),
        );
    }

    let table_range = super::table::find_table_range_at(buffer, pos)?;
    let raw_table = buffer.slice(table_range.clone()).to_string();
    let num_cols = cells.len();

    let mut col_widths = vec![1usize; num_cols];
    for line in raw_table.lines() {
        if super::table::is_table_delimiter(line) {
            let row_cells = super::table::split_row_cells(line);
            for (i, c) in row_cells.iter().enumerate() {
                if i < num_cols {
                    col_widths[i] = col_widths[i].max(c.trim().len().saturating_sub(2));
                }
            }
        } else {
            let row_cells = super::table::split_row_cells(line);
            for (i, c) in row_cells.iter().enumerate() {
                if i < num_cols {
                    col_widths[i] = col_widths[i].max(c.trim().graphemes(true).count());
                }
            }
        }
    }

    let mut new_row = String::from("|");
    for &w in &col_widths {
        let space_count = w + 2;
        new_row.push_str(&" ".repeat(space_count));
        new_row.push('|');
    }
    let insertion = format!("{nl}{new_row}");
    let insert_pos = line_end;
    let target_cursor_pos = insert_pos + nl.len() + 2;
    let new_pos = BytePos::new(target_cursor_pos);

    Some(
        Transaction::new()
            .with_edit(SingleEdit::insert(insert_pos, insertion))
            .with_cursor(new_pos, new_pos)
            .with_undo_group(UndoGroup::next()),
    )
}

fn find_pipe_offsets(line_str: &str) -> Vec<usize> {
    let mut pipe_offsets = Vec::new();
    let mut is_escaped = false;
    for (i, b) in line_str.bytes().enumerate() {
        if is_escaped {
            is_escaped = false;
        } else if b == b'\\' {
            is_escaped = true;
        } else if b == b'|' {
            pipe_offsets.push(i);
        }
    }
    pipe_offsets
}

fn find_cell_text_start(buffer: &Buffer, cell_start: usize, cell_end: usize) -> usize {
    let s = buffer.slice(cell_start..cell_end).to_string();
    let ws = s.bytes().take_while(|b| *b == b' ').count();
    let target = cell_start + ws;
    if ws == s.len() && ws > 1 {
        cell_start + 1
    } else {
        target.min(cell_end)
    }
}

/// Plans Tab / Shift+Tab cell navigation inside a Markdown table.
pub fn plan_table_tab(buffer: &Buffer, cursor: BytePos, backward: bool) -> Option<Transaction> {
    let pos = cursor.get().min(buffer.byte_len());
    let line_idx = buffer.line_of_byte(pos);
    let line_start = buffer.byte_of_line(line_idx);
    let line_end = line_content_end(buffer, line_idx);
    let line_str = buffer.slice(line_start..line_end).to_string();

    if !super::table::is_table_row(&line_str) {
        return None;
    }

    let table_range = super::table::find_table_range_at(buffer, pos)?;
    let table_start_line = buffer.line_of_byte(table_range.start);
    let table_end_line = buffer.line_of_byte(table_range.end.saturating_sub(1));

    let pipe_offsets = find_pipe_offsets(&line_str);
    if pipe_offsets.len() < 2 {
        return None;
    }

    let rel_pos = pos.saturating_sub(line_start);
    let mut current_cell_idx = 0;
    for (i, window) in pipe_offsets.windows(2).enumerate() {
        if rel_pos >= window[0] && rel_pos <= window[1] {
            current_cell_idx = i;
            break;
        }
    }
    let num_cells = pipe_offsets.len() - 1;

    if !backward {
        if current_cell_idx + 1 < num_cells {
            let next_pipe = pipe_offsets[current_cell_idx + 1];
            let cell_start = line_start + next_pipe + 1;
            let target = find_cell_text_start(
                buffer,
                cell_start,
                line_start + pipe_offsets[current_cell_idx + 2],
            );
            let new_pos = BytePos::new(target);
            Some(Transaction::new().with_cursor(new_pos, new_pos))
        } else if line_idx < table_end_line {
            let next_line_idx = line_idx + 1;
            let next_line_start = buffer.byte_of_line(next_line_idx);
            let next_line_end = line_content_end(buffer, next_line_idx);
            let next_line_str = buffer.slice(next_line_start..next_line_end).to_string();
            let next_pipes = find_pipe_offsets(&next_line_str);
            if next_pipes.len() >= 2 {
                let cell_start = next_line_start + next_pipes[0] + 1;
                let target =
                    find_cell_text_start(buffer, cell_start, next_line_start + next_pipes[1]);
                let new_pos = BytePos::new(target);
                Some(Transaction::new().with_cursor(new_pos, new_pos))
            } else {
                None
            }
        } else {
            let nl = if buffer.line_ending() == LineEnding::CrLf {
                "\r\n"
            } else {
                "\n"
            };
            let raw_table = buffer.slice(table_range.clone()).to_string();
            let mut col_widths = vec![1usize; num_cells];
            for line in raw_table.lines() {
                if super::table::is_table_delimiter(line) {
                    let row_cells = super::table::split_row_cells(line);
                    for (i, c) in row_cells.iter().enumerate() {
                        if i < num_cells {
                            col_widths[i] = col_widths[i].max(c.trim().len().saturating_sub(2));
                        }
                    }
                } else {
                    let row_cells = super::table::split_row_cells(line);
                    for (i, c) in row_cells.iter().enumerate() {
                        if i < num_cells {
                            col_widths[i] = col_widths[i].max(c.trim().graphemes(true).count());
                        }
                    }
                }
            }
            let mut new_row = String::from("|");
            for &w in &col_widths {
                let space_count = w + 2;
                new_row.push_str(&" ".repeat(space_count));
                new_row.push('|');
            }
            let insertion = format!("{nl}{new_row}");
            let insert_pos = line_end;
            let target_cursor_pos = insert_pos + nl.len() + 2;
            let new_pos = BytePos::new(target_cursor_pos);
            Some(
                Transaction::new()
                    .with_edit(SingleEdit::insert(insert_pos, insertion))
                    .with_cursor(new_pos, new_pos)
                    .with_undo_group(UndoGroup::next()),
            )
        }
    } else if current_cell_idx > 0 {
        let prev_pipe = pipe_offsets[current_cell_idx - 1];
        let cell_start = line_start + prev_pipe + 1;
        let target = find_cell_text_start(
            buffer,
            cell_start,
            line_start + pipe_offsets[current_cell_idx],
        );
        let new_pos = BytePos::new(target);
        Some(Transaction::new().with_cursor(new_pos, new_pos))
    } else if line_idx > table_start_line {
        let prev_line_idx = line_idx - 1;
        let prev_line_start = buffer.byte_of_line(prev_line_idx);
        let prev_line_end = line_content_end(buffer, prev_line_idx);
        let prev_line_str = buffer.slice(prev_line_start..prev_line_end).to_string();
        let prev_pipes = find_pipe_offsets(&prev_line_str);
        if prev_pipes.len() >= 2 {
            let last_pipe_idx = prev_pipes.len() - 2;
            let cell_start = prev_line_start + prev_pipes[last_pipe_idx] + 1;
            let target = find_cell_text_start(
                buffer,
                cell_start,
                prev_line_start + prev_pipes[last_pipe_idx + 1],
            );
            let new_pos = BytePos::new(target);
            Some(Transaction::new().with_cursor(new_pos, new_pos))
        } else {
            None
        }
    } else {
        None
    }
}
