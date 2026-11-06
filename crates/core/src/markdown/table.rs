use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

use crate::text::{Buffer, SingleEdit, Transaction, UndoGroup};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnAlignment {
    Left,
    Center,
    Right,
}

/// Checks whether a single line string matches Markdown table row syntax.
pub fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.ends_with('|') && t.len() > 1
}

/// Checks whether a cell string consists only of Markdown delimiter characters (`-`, `:`).
fn is_cell_delimiter(cell: &str) -> bool {
    let t = cell.trim();
    if t.is_empty() || !t.contains('-') {
        return false;
    }
    t.chars().all(|c| c == '-' || c == ':')
}

/// Checks whether a line string is a Markdown table delimiter row (`|---|:---:|---:|`).
pub fn is_table_delimiter(line: &str) -> bool {
    if !is_table_row(line) {
        return false;
    }
    let cells = split_row_cells(line);
    !cells.is_empty() && cells.iter().all(|c| is_cell_delimiter(c))
}

/// Splits a table row line into trimmed cell contents, respecting escaped pipes (`\|`).
pub fn split_row_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    if !t.starts_with('|') || !t.ends_with('|') || t.len() <= 1 {
        return Vec::new();
    }
    // Strip outer '|'
    let inner = &t[1..t.len() - 1];

    let mut cells = Vec::new();
    let mut current = String::new();
    let mut is_escaped = false;

    for c in inner.chars() {
        if is_escaped {
            current.push(c);
            is_escaped = false;
        } else if c == '\\' {
            current.push(c);
            is_escaped = true;
        } else if c == '|' {
            cells.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

/// Parses the column alignment from a delimiter cell.
fn parse_alignment(cell: &str) -> ColumnAlignment {
    let t = cell.trim();
    let starts = t.starts_with(':');
    let ends = t.ends_with(':');
    if starts && ends {
        ColumnAlignment::Center
    } else if ends {
        ColumnAlignment::Right
    } else {
        ColumnAlignment::Left
    }
}

/// Returns the visible width of a cell in Unicode graphemes.
fn cell_visible_width(cell: &str) -> usize {
    cell.graphemes(true).count()
}

/// Formats a raw Markdown table text into an aligned table with uniform column widths.
pub fn format_table_str(raw: &str) -> Option<String> {
    let raw_lines: Vec<&str> = raw.lines().collect();
    if raw_lines.len() < 2 {
        return None;
    }

    let parsed_rows: Vec<Vec<String>> = raw_lines.iter().map(|line| split_row_cells(line)).collect();
    if parsed_rows.iter().any(|r| r.is_empty()) {
        return None;
    }

    // Locate delimiter row
    let delimiter_idx = raw_lines
        .iter()
        .position(|line| is_table_delimiter(line))?;

    let num_columns = parsed_rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if num_columns == 0 {
        return None;
    }

    // Extract alignments from the delimiter row
    let mut alignments = Vec::with_capacity(num_columns);
    for col in 0..num_columns {
        let align = parsed_rows[delimiter_idx]
            .get(col)
            .map(|cell| parse_alignment(cell))
            .unwrap_or(ColumnAlignment::Left);
        alignments.push(align);
    }

    // Calculate maximum content width for each column (excluding delimiter row)
    let mut content_widths = vec![1usize; num_columns];
    for (row_idx, row) in parsed_rows.iter().enumerate() {
        if row_idx == delimiter_idx {
            continue;
        }
        for (col, cell) in row.iter().enumerate() {
            content_widths[col] = content_widths[col].max(cell_visible_width(cell));
        }
    }

    // Each cell in content row has 1 leading space and 1 trailing space, so total inner width is content_width + 2.
    // Minimum inner width is 3 (to fit at least 3 dashes `---` or `:--` in the delimiter row).
    let mut dash_widths = Vec::with_capacity(num_columns);
    for &cw in &content_widths {
        dash_widths.push((cw + 2).max(3));
    }

    let mut output = String::new();
    let has_trailing_newline = raw.ends_with('\n');

    for (row_idx, row) in parsed_rows.iter().enumerate() {
        if row_idx == delimiter_idx {
            output.push('|');
            for (col, &dash_w) in dash_widths.iter().enumerate() {
                let align = alignments[col];
                let dashes = match align {
                    ColumnAlignment::Center => {
                        format!(":{}:", "-".repeat(dash_w.saturating_sub(2)))
                    }
                    ColumnAlignment::Right => {
                        format!("{}:", "-".repeat(dash_w.saturating_sub(1)))
                    }
                    ColumnAlignment::Left => {
                        if parsed_rows[delimiter_idx]
                            .get(col)
                            .is_some_and(|c| c.starts_with(':'))
                        {
                            format!(":{}", "-".repeat(dash_w.saturating_sub(1)))
                        } else {
                            "-".repeat(dash_w)
                        }
                    }
                };
                output.push_str(&dashes);
                output.push('|');
            }
        } else {
            output.push('|');
            for (col, &dash_w) in dash_widths.iter().enumerate() {
                let cell_text = row.get(col).map(String::as_str).unwrap_or("");
                let cell_w = cell_visible_width(cell_text);
                // Total available space inside cell is dash_w.
                // Subtract 2 for the outer padding spaces.
                let target_content_w = dash_w - 2;
                let padding = target_content_w.saturating_sub(cell_w);
                let align = alignments[col];

                output.push(' ');
                match align {
                    ColumnAlignment::Left => {
                        output.push_str(cell_text);
                        for _ in 0..padding {
                            output.push(' ');
                        }
                    }
                    ColumnAlignment::Right => {
                        for _ in 0..padding {
                            output.push(' ');
                        }
                        output.push_str(cell_text);
                    }
                    ColumnAlignment::Center => {
                        let left_pad = padding / 2;
                        let right_pad = padding - left_pad;
                        for _ in 0..left_pad {
                            output.push(' ');
                        }
                        output.push_str(cell_text);
                        for _ in 0..right_pad {
                            output.push(' ');
                        }
                    }
                }
                output.push_str(" |");
            }
        }
        output.push('\n');
    }

    if !has_trailing_newline && output.ends_with('\n') {
        output.pop();
    }

    Some(output)
}

/// Finds the continuous table range containing or touching `pos`.
pub fn find_table_range_at(buffer: &Buffer, pos: usize) -> Option<Range<usize>> {
    if buffer.is_empty() {
        return None;
    }
    let pos = pos.min(buffer.byte_len());
    let current_line = buffer.line_of_byte(pos);
    let line_str = buffer
        .slice(buffer.byte_of_line(current_line)..buffer.byte_of_line((current_line + 1).min(buffer.line_len())))
        .to_string();

    if !is_table_row(&line_str) {
        return None;
    }

    // Scan backwards to find the first table row
    let mut start_line = current_line;
    while start_line > 0 {
        let prev_line = start_line - 1;
        let prev_str = buffer
            .slice(buffer.byte_of_line(prev_line)..buffer.byte_of_line(start_line))
            .to_string();
        if is_table_row(&prev_str) {
            start_line = prev_line;
        } else {
            break;
        }
    }

    // Scan forwards to find the last table row
    let mut end_line = current_line;
    while end_line + 1 < buffer.line_len() {
        let next_line = end_line + 1;
        let next_str = buffer
            .slice(buffer.byte_of_line(next_line)..buffer.byte_of_line((next_line + 1).min(buffer.line_len())))
            .to_string();
        if is_table_row(&next_str) {
            end_line = next_line;
        } else {
            break;
        }
    }

    let start_byte = buffer.byte_of_line(start_line);
    let end_byte = if end_line + 1 < buffer.line_len() {
        buffer.byte_of_line(end_line + 1)
    } else {
        buffer.byte_len()
    };

    let table_text = buffer.slice(start_byte..end_byte).to_string();
    if table_text.lines().count() >= 2 && table_text.lines().any(is_table_delimiter) {
        Some(start_byte..end_byte)
    } else {
        None
    }
}

/// Creates an atomic `Transaction` formatting the table at `pos`.
pub fn format_table_transaction(buffer: &Buffer, pos: usize) -> Option<Transaction> {
    let range = find_table_range_at(buffer, pos)?;
    let raw = buffer.slice(range.clone()).to_string();
    let formatted = format_table_str(&raw)?;

    if formatted == raw {
        return None;
    }

    let tx = Transaction::new()
        .with_edit(SingleEdit::replace(range, formatted))
        .with_undo_group(UndoGroup::next());
    Some(tx)
}
