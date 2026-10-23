use std::ops::Range;

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
    MatchingBracket,
    NextHeading,
    PrevHeading,
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
    MarkdownSection,
    MarkdownListItem,
    MarkdownQuote,
    MarkdownTableCell,
    MarkdownTableRow,
}

/// Structured text object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextObject {
    pub kind: TextObjectKind,
    pub target: TextObjectTarget,
}

/// Syntax producer supplies source coordinates at the current buffer revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxObjectRange {
    pub target: TextObjectTarget,
    pub inner: Range<usize>,
    pub around: Range<usize>,
}
