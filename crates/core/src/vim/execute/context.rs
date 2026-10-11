use crate::commands::{Action, CommandId};
use crate::navigation::RowModel;
use crate::text::Buffer;
use crate::workspace::{View, ViewId};

pub enum InputContext<'a> {
    Editor {
        view_id: ViewId,
        buffer: &'a mut Buffer,
        view: &'a mut View,
    },
    RowModel {
        panel: &'a str,
        selected_index: usize,
        total_rows: usize,
    },
    Rows(&'a mut RowModel),
    None,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputResult {
    Consumed,
    Incomplete,
    Action(Action),
    Actions(Vec<Action>),
    Ignored,
    Error(String),
    /// Renderer resolves wrapped row movement in source coordinates.
    VisualRow {
        down: bool,
        count: usize,
    },
    Command {
        id: CommandId,
        argument: String,
    },
}

pub(super) fn reborrow<'a>(ctx: &'a mut InputContext<'_>) -> InputContext<'a> {
    match ctx {
        InputContext::Editor {
            view_id,
            buffer,
            view,
        } => InputContext::Editor {
            view_id: *view_id,
            buffer,
            view,
        },
        InputContext::Rows(model) => InputContext::Rows(model),
        InputContext::RowModel {
            panel,
            selected_index,
            total_rows,
        } => InputContext::RowModel {
            panel,
            selected_index: *selected_index,
            total_rows: *total_rows,
        },
        InputContext::None => InputContext::None,
    }
}

/// IME deletion uses Unicode scalar counts supplied by the OS, not byte or grapheme counts.
pub fn surrounding_char_range(
    buffer: &Buffer,
    cursor: usize,
    before: usize,
    after: usize,
) -> std::ops::Range<usize> {
    let mut start = cursor;
    let mut end = cursor;
    for c in buffer.slice(0..cursor).chars().rev().take(before) {
        start -= c.len_utf8();
    }
    for c in buffer.slice(cursor..buffer.byte_len()).chars().take(after) {
        end += c.len_utf8();
    }
    start..end
}
