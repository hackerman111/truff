use std::ops::Range;

use super::SyntaxSnapshot;
use crate::text::{Buffer, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Ordered,
    Task { checked: bool, checkbox: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMarker {
    pub range: Range<usize>,
    pub depth: usize,
    pub kind: ListKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinePresentation {
    pub list: Option<ListMarker>,
    pub code: bool,
    pub task_fence: bool,
    pub in_tasks_block: bool,
    pub table_header: bool,
    pub table_delimiter: bool,
}

/// Builds a one-character task edit; stale or invalid targets are ignored.
pub fn toggle_task(buffer: &Buffer, checkbox: usize) -> Option<Transaction> {
    let end = checkbox
        .checked_add(2)
        .filter(|end| *end <= buffer.byte_len())?;
    if checkbox == 0 || !buffer.is_char_boundary(checkbox - 1) || !buffer.is_char_boundary(end) {
        return None;
    }
    let marker = buffer.slice(checkbox - 1..end);
    let mut chars = marker.chars();
    let replacement = match (chars.next(), chars.next(), chars.next()) {
        (Some('['), Some(' ' | '/' | '-'), Some(']')) => "x",
        (Some('['), Some('x' | 'X'), Some(']')) => " ",
        _ => return None,
    };
    Some(Transaction::replace(checkbox..checkbox + 1, replacement))
}

/// Cycles task checkbox status: `[ ]` -> `[/]` -> `[x]` -> `[ ]`.
pub fn cycle_task_status(buffer: &Buffer, checkbox: usize) -> Option<Transaction> {
    let end = checkbox
        .checked_add(2)
        .filter(|end| *end <= buffer.byte_len())?;
    if checkbox == 0 || !buffer.is_char_boundary(checkbox - 1) || !buffer.is_char_boundary(end) {
        return None;
    }
    let marker = buffer.slice(checkbox - 1..end);
    let mut chars = marker.chars();
    let replacement = match (chars.next(), chars.next(), chars.next()) {
        (Some('['), Some(' '), Some(']')) => "/",
        (Some('['), Some('/' | '-'), Some(']')) => "x",
        (Some('['), Some('x' | 'X'), Some(']')) => " ",
        _ => return None,
    };
    Some(Transaction::replace(checkbox..checkbox + 1, replacement))
}

fn parse_line_list_marker(line_str: &str, line_start: usize, depth: usize) -> Option<ListMarker> {
    let ws_len = line_str
        .bytes()
        .take_while(|b| matches!(*b, b' ' | b'\t'))
        .count();
    let rest = &line_str[ws_len..];
    let bytes = rest.as_bytes();
    if bytes.len() >= 2
        && matches!(bytes[0], b'-' | b'*' | b'+')
        && matches!(bytes[1], b' ' | b'\t')
    {
        if bytes.len() >= 6
            && bytes[2] == b'['
            && matches!(bytes[3], b' ' | b'x' | b'X' | b'/' | b'-')
            && bytes[4] == b']'
            && matches!(bytes[5], b' ' | b'\t')
        {
            return Some(ListMarker {
                range: (line_start + ws_len)..(line_start + ws_len + 6),
                depth,
                kind: ListKind::Task {
                    checked: matches!(bytes[3], b'x' | b'X'),
                    checkbox: line_start + ws_len + 3,
                },
            });
        }
        return Some(ListMarker {
            range: (line_start + ws_len)..(line_start + ws_len + 2),
            depth,
            kind: ListKind::Bullet,
        });
    }
    let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0
        && bytes.get(digits).is_some_and(|b| matches!(*b, b'.' | b')'))
        && bytes
            .get(digits + 1)
            .is_some_and(|b| matches!(*b, b' ' | b'\t'))
    {
        return Some(ListMarker {
            range: (line_start + ws_len)..(line_start + ws_len + digits + 2),
            depth,
            kind: ListKind::Ordered,
        });
    }
    None
}

fn indent_columns(line_str: &str) -> usize {
    line_str
        .bytes()
        .take_while(|b| matches!(*b, b' ' | b'\t'))
        .map(|b| if b == b'\t' { 4 } else { 1 })
        .sum()
}

fn is_tasks_fence_header(header_line: &str) -> bool {
    let trimmed = header_line.trim_start();
    let after_fence = if let Some(rest) = trimmed.strip_prefix("```") {
        rest.trim_start_matches('`')
    } else if let Some(rest) = trimmed.strip_prefix("~~~") {
        rest.trim_start_matches('~')
    } else {
        return false;
    };
    after_fence
        .split_whitespace()
        .next()
        .is_some_and(|lang| lang.eq_ignore_ascii_case("tasks"))
}

impl SyntaxSnapshot {
    /// Block context for one source line, including ancestors outside the viewport.
    pub fn line_presentation(&self, range: Range<usize>) -> LinePresentation {
        self.line_presentation_with_slice(range, None, None)
    }

    /// Block context for one source line backed by a `Rope`, resolving over-indented list items
    /// and `tasks` fenced environments.
    pub fn line_presentation_in_rope(
        &self,
        rope: &crop::Rope,
        range: Range<usize>,
    ) -> LinePresentation {
        let clamped_start = range.start.min(rope.byte_len());
        let clamped_end = range.end.min(rope.byte_len()).max(clamped_start);
        let line_str = rope.byte_slice(clamped_start..clamped_end).to_string();
        self.line_presentation_with_slice(
            clamped_start..clamped_end,
            Some(&line_str),
            Some(&|fence_start| {
                let start = fence_start.min(rope.byte_len());
                let end = (start + 32).min(rope.byte_len());
                let mut valid_end = end;
                while valid_end > start && !rope.is_char_boundary(valid_end) {
                    valid_end -= 1;
                }
                rope.byte_slice(start..valid_end).to_string()
            }),
        )
    }

    /// Block context for one source line backed by `&str`.
    pub fn line_presentation_in_str(&self, source: &str, range: Range<usize>) -> LinePresentation {
        let clamped_start = range.start.min(source.len());
        let clamped_end = range.end.min(source.len()).max(clamped_start);
        let line_str = &source[clamped_start..clamped_end];
        self.line_presentation_with_slice(
            clamped_start..clamped_end,
            Some(line_str),
            Some(&|fence_start| {
                let start = fence_start.min(source.len());
                let end = (start + 32).min(source.len());
                let mut valid_end = end;
                while valid_end > start && !source.is_char_boundary(valid_end) {
                    valid_end -= 1;
                }
                source[start..valid_end].to_string()
            }),
        )
    }

    fn line_presentation_with_slice(
        &self,
        range: Range<usize>,
        line_str: Option<&str>,
        fence_header: Option<&dyn Fn(usize) -> String>,
    ) -> LinePresentation {
        let mut result = LinePresentation::default();
        let Some(mut node) = self.find_node_at(range.end.saturating_sub(1).max(range.start)) else {
            return result;
        };
        let mut fenced_range: Option<Range<usize>> = None;
        let mut list_item_ancestors = 0usize;
        loop {
            match node.kind() {
                "fenced_code_block" => {
                    fenced_range = Some(node.byte_range());
                    result.code = true;
                }
                "indented_code_block" => {
                    result.code = true;
                }
                "list_item" => {
                    list_item_ancestors += 1;
                    if result.list.is_none() {
                        let mut marker = None;
                        let mut task = None;
                        let mut walk = node.walk();
                        for child in node.named_children(&mut walk) {
                            if child.start_byte() < range.start || child.start_byte() >= range.end {
                                continue;
                            }
                            match child.kind() {
                                "list_marker_minus" | "list_marker_plus" | "list_marker_star" => {
                                    marker = Some((child.byte_range(), ListKind::Bullet));
                                }
                                "list_marker_dot" | "list_marker_parenthesis" => {
                                    marker = Some((child.byte_range(), ListKind::Ordered));
                                }
                                "task_list_marker_checked" | "task_list_marker_unchecked" => {
                                    task = Some((
                                        child.end_byte(),
                                        ListKind::Task {
                                            checked: child.kind() == "task_list_marker_checked",
                                            checkbox: child.start_byte() + 1,
                                        },
                                    ));
                                }
                                _ => {}
                            }
                        }
                        if let Some((mut marker_range, mut kind)) = marker {
                            if let Some((end, task_kind)) = task {
                                marker_range.end = end;
                                kind = task_kind;
                            }
                            let mut depth = 0;
                            let mut parent = node.parent();
                            while let Some(ancestor) = parent {
                                depth += usize::from(ancestor.kind() == "list_item");
                                parent = ancestor.parent();
                            }
                            result.list = Some(ListMarker {
                                range: marker_range,
                                depth,
                                kind,
                            });
                        }
                    }
                }
                _ => {}
            }
            let Some(parent) = node.parent() else { break };
            node = parent;
        }

        if let Some(line) = line_str {
            if let Some(fence) = fenced_range {
                let is_tasks_fence = fence_header.is_some_and(|f| {
                    let hdr = f(fence.start);
                    let first_line = hdr.lines().next().unwrap_or("");
                    is_tasks_fence_header(first_line)
                });
                if is_tasks_fence {
                    result.code = false;
                    let is_fence_delimiter = {
                        let t = line.trim_start();
                        t.starts_with("```") || t.starts_with("~~~")
                    };
                    if is_fence_delimiter || range.start <= fence.start {
                        result.task_fence = true;
                    } else {
                        result.in_tasks_block = true;
                        let depth = indent_columns(line) / 4;
                        result.list = parse_line_list_marker(line, range.start, depth);
                    }
                }
            } else if result.list.is_none() && list_item_ancestors > 0 {
                if let Some(marker) = parse_line_list_marker(line, range.start, list_item_ancestors)
                {
                    result.code = false;
                    result.list = Some(marker);
                }
            } else if let Some(ref mut list) = result.list
                && list.kind == ListKind::Bullet
                && let Some(parsed) = parse_line_list_marker(line, range.start, list.depth)
                && matches!(parsed.kind, ListKind::Task { .. })
            {
                *list = parsed;
            }
        }
        result
    }
}
