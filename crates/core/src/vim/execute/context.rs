use crate::commands::{Action, CommandContext, CommandId};
use crate::navigation::RowModel;
use crate::text::Buffer;
use crate::workspace::{View, ViewId};

pub enum InputContext<'a> {
    Editor {
        view_id: ViewId,
        buffer: &'a mut Buffer,
        view: &'a mut View,
    },
    TextField {
        field_id: &'a str,
        buffer: &'a mut Buffer,
        view: &'a mut View,
    },
    CommandLine {
        buffer: &'a mut Buffer,
        view: &'a mut View,
    },
    RowModel {
        panel: &'a str,
        selected_index: usize,
        total_rows: usize,
    },
    Rows(&'a mut RowModel),
    Navigator {
        panel: &'a str,
        rows: &'a mut RowModel,
    },
    None,
}

impl InputContext<'_> {
    /// Maps the runtime borrow context to its typed `CommandContext`.
    pub fn command_context(&self) -> CommandContext {
        match self {
            Self::Editor { .. } => CommandContext::Editor,
            Self::TextField { .. } => CommandContext::TextField,
            Self::CommandLine { .. } => CommandContext::CommandLine,
            Self::RowModel { .. } | Self::Rows(_) => CommandContext::Rows,
            Self::Navigator { .. } => CommandContext::Navigator,
            Self::None => CommandContext::Global,
        }
    }
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
    ScrollCursor(crate::vim::parser::ScrollPosition),
    FollowLink,
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
        InputContext::TextField {
            field_id,
            buffer,
            view,
        } => InputContext::TextField {
            field_id,
            buffer,
            view,
        },
        InputContext::CommandLine { buffer, view } => InputContext::CommandLine { buffer, view },
        InputContext::Rows(model) => InputContext::Rows(model),
        InputContext::Navigator { panel, rows } => InputContext::Navigator { panel, rows },
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
