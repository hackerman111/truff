use super::super::motion::{Motion, TextObject};
use super::super::{Key, Mode};

/// Operations that act on a range of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    Delete,
    Change,
    Yank,
    Indent,
    Dedent,
}

/// Entry point into Insert mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertEntry {
    AtCursor,     // i
    AfterCursor,  // a
    LineStart,    // I
    LineEnd,      // A
    NewLineBelow, // o
    NewLineAbove, // O
}

/// Parsed Vim command ready for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Motion {
        motion: Motion,
        count: usize,
    },
    OperatorMotion {
        op: Operator,
        motion: Motion,
        count: usize,
    },
    OperatorTextObject {
        op: Operator,
        obj: TextObject,
        count: usize,
    },
    Insert(InsertEntry),
    Visual(Mode),
    Escape,
    DeleteChar(usize),
    Paste {
        after: bool,
        count: usize,
    },
    Undo,
    Redo,
    RepeatLastEdit,
    SelectRegister(char),
    ToggleMacroRecord(char),
    StopMacroRecord,
    ReplayMacro {
        reg: char,
        count: usize,
    },
    ColonCommand(String),
    TextCommit(String),
    InsertKey(Key),
    DeleteSurrounding {
        before_chars: usize,
        after_chars: usize,
    },
    OperatorSelection(Operator),
    OperatorSelectionLine(Operator),
    OperatorExtent {
        op: Operator,
        graphemes: usize,
        lines: Option<usize>,
    },
    OperatorRange {
        op: Operator,
        range: std::ops::Range<usize>,
        linewise: bool,
    },
    Search {
        query: String,
        forward: bool,
        count: usize,
    },
    RepeatSearch {
        reverse: bool,
        count: usize,
    },
}

/// Result of parsing a sequence of keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseResult {
    Complete(Command),
    Incomplete,
    Invalid,
    Overflow,
    TextLimit,
}
