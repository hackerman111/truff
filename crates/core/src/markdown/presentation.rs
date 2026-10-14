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
        (Some('['), Some(' '), Some(']')) => "x",
        (Some('['), Some('x' | 'X'), Some(']')) => " ",
        _ => return None,
    };
    Some(Transaction::replace(checkbox..checkbox + 1, replacement))
}

impl SyntaxSnapshot {
    /// Block context for one source line, including ancestors outside the viewport.
    pub fn line_presentation(&self, range: Range<usize>) -> LinePresentation {
        let mut result = LinePresentation::default();
        let Some(mut node) = self.find_node_at(range.end.saturating_sub(1).max(range.start)) else {
            return result;
        };
        loop {
            match node.kind() {
                "fenced_code_block" | "indented_code_block" => result.code = true,
                "list_item" if result.list.is_none() => {
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
                    if let Some((mut range, mut kind)) = marker {
                        if let Some((end, task_kind)) = task {
                            range.end = end;
                            kind = task_kind;
                        }
                        let mut depth = 0;
                        let mut parent = node.parent();
                        while let Some(ancestor) = parent {
                            depth += usize::from(ancestor.kind() == "list_item");
                            parent = ancestor.parent();
                        }
                        result.list = Some(ListMarker { range, depth, kind });
                    }
                }
                _ => {}
            }
            let Some(parent) = node.parent() else { break };
            node = parent;
        }
        result
    }
}
