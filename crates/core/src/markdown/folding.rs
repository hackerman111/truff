//! Markdown heading fold range calculation and queries.

use std::collections::BTreeSet;
use std::ops::Range;

/// Lightweight summary of a heading's location and level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadingItem {
    pub line: usize,
    pub level: u8,
}

impl HeadingItem {
    pub const fn new(line: usize, level: u8) -> Self {
        Self { line, level }
    }
}

/// Computes the line range hidden by folding the heading at `heading_idx`.
/// The range returned is `(heading.line + 1)..next_heading_line_or_eof`.
pub fn heading_fold_range(
    headings: &[HeadingItem],
    heading_idx: usize,
    total_lines: usize,
) -> Range<usize> {
    if heading_idx >= headings.len() {
        return 0..0;
    }
    let h = &headings[heading_idx];
    let start_line = h.line + 1;
    let mut end_line = total_lines;

    for next in &headings[heading_idx + 1..] {
        if next.level <= h.level {
            end_line = next.line;
            break;
        }
    }

    start_line..end_line.max(start_line)
}

/// Computes all disjoint line ranges hidden by the currently folded headings.
pub fn compute_hidden_line_ranges(
    headings: &[HeadingItem],
    folded_headings: &BTreeSet<usize>,
    total_lines: usize,
) -> Vec<Range<usize>> {
    let mut raw_ranges = Vec::new();
    for (idx, h) in headings.iter().enumerate() {
        if folded_headings.contains(&h.line) {
            let range = heading_fold_range(headings, idx, total_lines);
            if !range.is_empty() {
                raw_ranges.push(range);
            }
        }
    }

    if raw_ranges.is_empty() {
        return Vec::new();
    }

    raw_ranges.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(raw_ranges.len());
    for r in raw_ranges {
        if let Some(last) = merged.last_mut() {
            if r.start <= last.end {
                last.end = last.end.max(r.end);
            } else {
                merged.push(r);
            }
        } else {
            merged.push(r);
        }
    }
    merged
}

/// Checks if a given line is hidden by any folded heading.
pub fn is_line_in_hidden_ranges(line: usize, hidden_ranges: &[Range<usize>]) -> bool {
    hidden_ranges.iter().any(|r| r.contains(&line))
}

/// Finds the heading index applicable to a cursor line (either directly on heading,
/// or the innermost enclosing heading covering this line).
pub fn find_heading_for_line(
    headings: &[HeadingItem],
    line: usize,
    total_lines: usize,
) -> Option<usize> {
    // 1. Direct match
    if let Some(pos) = headings.iter().position(|h| h.line == line) {
        return Some(pos);
    }

    // 2. Innermost enclosing heading above `line`
    for (idx, h) in headings.iter().enumerate().rev() {
        if h.line < line {
            let range = heading_fold_range(headings, idx, total_lines);
            if range.contains(&line) {
                return Some(idx);
            }
        }
    }

    None
}
