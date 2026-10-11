mod command;

pub use command::{Command, InsertEntry, Operator, ParseResult};

use super::motion::{Motion, TextObject, TextObjectKind, TextObjectTarget};
use super::{Key, Mode};

/// Key sequence parser for Vim commands.
#[derive(Debug, Clone, Default)]
pub struct VimParser {
    count_prefix: Option<usize>,
    operator: Option<(Operator, usize)>,
    buffer: Vec<Key>,
    in_colon: bool,
    colon_buffer: String,
    search: Option<(bool, usize, String)>,
}

impl VimParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets all pending parse state (e.g. on Esc).
    pub fn reset(&mut self) {
        self.count_prefix = None;
        self.operator = None;
        self.buffer.clear();
        self.in_colon = false;
        self.colon_buffer.clear();
        self.search = None;
    }

    /// Feeds a key and attempts to produce a complete Vim `Command`.
    pub fn feed(&mut self, key: Key, mode: Mode) -> ParseResult {
        // Esc always resets parser and returns Escape
        if key == Key::Esc {
            self.reset();
            return ParseResult::Complete(Command::Escape);
        }

        if let Some((forward, count, query)) = &mut self.search {
            match key {
                Key::Enter => {
                    let cmd = Command::Search {
                        query: std::mem::take(query),
                        forward: *forward,
                        count: *count,
                    };
                    self.reset();
                    return ParseResult::Complete(cmd);
                }
                Key::Backspace => {
                    query.pop();
                }
                Key::Char(c) => {
                    if query.len() + c.len_utf8() > 65536 {
                        self.reset();
                        return ParseResult::TextLimit;
                    }
                    query.push(c);
                }
                _ => {}
            }
            return ParseResult::Incomplete;
        }
        if self.buffer.is_empty() && self.operator.is_none() && matches!(key, Key::Char('/' | '?'))
        {
            self.search = Some((
                key == Key::Char('/'),
                self.count_prefix.take().unwrap_or(1),
                String::new(),
            ));
            return ParseResult::Incomplete;
        }
        // Colon command mode
        if self.in_colon {
            match key {
                Key::Enter => {
                    let cmd = std::mem::take(&mut self.colon_buffer);
                    self.in_colon = false;
                    return ParseResult::Complete(Command::ColonCommand(cmd));
                }
                Key::Backspace => {
                    self.colon_buffer.pop();
                    if self.colon_buffer.is_empty() {
                        self.in_colon = false;
                        return ParseResult::Complete(Command::Escape);
                    }
                    return ParseResult::Incomplete;
                }
                Key::Char(c) => {
                    if self.colon_buffer.len() + c.len_utf8() > 65536 {
                        self.reset();
                        return ParseResult::TextLimit;
                    }
                    self.colon_buffer.push(c);
                    return ParseResult::Incomplete;
                }
                _ => return ParseResult::Incomplete,
            }
        }

        if mode == Mode::Insert {
            return ParseResult::Invalid;
        }

        // Handle ':' to start colon command in normal/visual mode
        if self.buffer.is_empty() && self.operator.is_none() && key == Key::Char(':') {
            self.in_colon = true;
            self.colon_buffer.clear();
            return ParseResult::Incomplete;
        }

        // Handle digits for counts
        if let Key::Char(c @ '0'..='9') = key {
            let is_first_digit = self.count_prefix.is_none() && self.buffer.is_empty();
            // '0' as first key is line-start motion, not count 0
            if !(c == '0' && is_first_digit) {
                let digit = c.to_digit(10).unwrap() as usize;
                let Some(new_count) = self
                    .count_prefix
                    .unwrap_or(0)
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(digit))
                else {
                    self.reset();
                    return ParseResult::Overflow;
                };
                self.count_prefix = Some(new_count);
                return ParseResult::Incomplete;
            }
        }

        self.buffer.push(key);
        self.parse_buffer(mode)
    }

    pub fn is_idle(&self) -> bool {
        self.count_prefix.is_none()
            && self.operator.is_none()
            && self.buffer.is_empty()
            && !self.in_colon
            && self.search.is_none()
    }
    pub fn take_count(&mut self) -> usize {
        let count = self.count_prefix.take().unwrap_or(1);
        self.reset();
        count
    }
    pub fn take_operator(&mut self) -> Option<Operator> {
        let op = self.operator.map(|(op, _)| op);
        self.reset();
        op
    }

    pub fn search_query(&self) -> Option<&str> {
        self.search.as_ref().map(|(_, _, q)| q.as_str())
    }

    fn parse_buffer(&mut self, mode: Mode) -> ParseResult {
        let first = self.buffer[0];

        // 1. Register selection: "{reg}
        if first == Key::Char('"') {
            if self.buffer.len() < 2 {
                return ParseResult::Incomplete;
            }
            if let Key::Char(reg) = self.buffer[1] {
                self.buffer.clear();
                return ParseResult::Complete(Command::SelectRegister(reg));
            }
            self.reset();
            return ParseResult::Invalid;
        }

        // 2. Macro recording: q{reg} or q to stop
        if first == Key::Char('q') && self.operator.is_none() {
            if self.buffer.len() < 2 {
                return ParseResult::Incomplete;
            }
            if let Key::Char(reg) = self.buffer[1] {
                self.buffer.clear();
                return ParseResult::Complete(Command::ToggleMacroRecord(reg));
            }
            self.reset();
            return ParseResult::Invalid;
        }

        // 3. Macro replay: @{reg}
        if first == Key::Char('@') && self.operator.is_none() {
            if self.buffer.len() < 2 {
                return ParseResult::Incomplete;
            }
            if let Key::Char(reg) = self.buffer[1] {
                let count = self.count_prefix.take().unwrap_or(1);
                self.buffer.clear();
                return ParseResult::Complete(Command::ReplayMacro { reg, count });
            }
            self.reset();
            return ParseResult::Invalid;
        }

        // 4. Operators pending (d, c, y, >, <)
        if self.operator.is_none() {
            let op = match first {
                Key::Char('d') => Some(Operator::Delete),
                Key::Char('c') => Some(Operator::Change),
                Key::Char('y') => Some(Operator::Yank),
                Key::Char('>') => Some(Operator::Indent),
                Key::Char('<') => Some(Operator::Dedent),
                _ => None,
            };

            if let Some(op) = op {
                let count = self.count_prefix.take().unwrap_or(1);
                if matches!(mode, Mode::VisualChar | Mode::VisualLine) {
                    self.reset();
                    return ParseResult::Complete(Command::OperatorSelection(op));
                }
                self.operator = Some((op, count));
                self.buffer.clear();
                return ParseResult::Incomplete;
            }
        }

        // 5. Operator-pending target resolution
        if let Some((op, op_count)) = self.operator {
            // Check linewise double-operator: dd, cc, yy, >>, <<
            let double_match = matches!(
                (op, first),
                (Operator::Delete, Key::Char('d'))
                    | (Operator::Change, Key::Char('c'))
                    | (Operator::Yank, Key::Char('y'))
                    | (Operator::Indent, Key::Char('>'))
                    | (Operator::Dedent, Key::Char('<'))
            );

            if double_match {
                let motion_count = self.count_prefix.take().unwrap_or(1);
                let Some(total_count) = op_count.checked_mul(motion_count) else {
                    self.reset();
                    return ParseResult::Overflow;
                };
                self.reset();
                return ParseResult::Complete(Command::OperatorMotion {
                    op,
                    motion: Motion::CurrentLine,
                    count: total_count,
                });
            }

            // Check text objects: i{target} or a{target}
            if first == Key::Char('i') || first == Key::Char('a') {
                if self.buffer.len() < 2 {
                    return ParseResult::Incomplete;
                }
                let kind = if first == Key::Char('i') {
                    TextObjectKind::Inner
                } else {
                    TextObjectKind::Around
                };
                let target_key = self.buffer[1];
                let target = match target_key {
                    Key::Char('w') => Some(TextObjectTarget::Word),
                    Key::Char('\'') => Some(TextObjectTarget::SingleQuote),
                    Key::Char('"') => Some(TextObjectTarget::DoubleQuote),
                    Key::Char('`') => Some(TextObjectTarget::Backtick),
                    Key::Char('(') | Key::Char(')') | Key::Char('b') => {
                        Some(TextObjectTarget::Parentheses)
                    }
                    Key::Char('[') | Key::Char(']') => Some(TextObjectTarget::Bracket),
                    Key::Char('{') | Key::Char('}') | Key::Char('B') => {
                        Some(TextObjectTarget::Brace)
                    }
                    Key::Char('<') | Key::Char('>') => Some(TextObjectTarget::AngleBracket),
                    Key::Char('p') => Some(TextObjectTarget::Paragraph),
                    Key::Char('s') => Some(TextObjectTarget::Sentence),
                    Key::Char('l') => Some(TextObjectTarget::MarkdownLink),
                    Key::Char('h') => Some(TextObjectTarget::MarkdownHeading),
                    Key::Char('c') => Some(TextObjectTarget::MarkdownCodeBlock),
                    _ => None,
                };

                if let Some(target) = target {
                    let motion_count = self.count_prefix.take().unwrap_or(1);
                    let Some(total_count) = op_count.checked_mul(motion_count) else {
                        self.reset();
                        return ParseResult::Overflow;
                    };
                    self.reset();
                    return ParseResult::Complete(Command::OperatorTextObject {
                        op,
                        obj: TextObject { kind, target },
                        count: total_count,
                    });
                } else {
                    self.reset();
                    return ParseResult::Invalid;
                }
            }

            // Otherwise, attempt to parse motion
            match self.parse_motion() {
                ParseMotionResult::Complete(motion) => {
                    let motion_count = self.count_prefix.take().unwrap_or(1);
                    let Some(total_count) = op_count.checked_mul(motion_count) else {
                        self.reset();
                        return ParseResult::Overflow;
                    };
                    self.reset();
                    return ParseResult::Complete(Command::OperatorMotion {
                        op,
                        motion,
                        count: total_count,
                    });
                }
                ParseMotionResult::Incomplete => return ParseResult::Incomplete,
                ParseMotionResult::Invalid => {
                    self.reset();
                    return ParseResult::Invalid;
                }
            }
        }

        // 6. Direct commands in Normal / Visual mode
        let count = self.count_prefix.take().unwrap_or(1);
        match first {
            Key::Char('n') | Key::Char('N') => {
                self.reset();
                ParseResult::Complete(Command::RepeatSearch {
                    reverse: first == Key::Char('N'),
                    count,
                })
            }
            Key::Char('i') => {
                self.reset();
                ParseResult::Complete(Command::Insert(InsertEntry::AtCursor))
            }
            Key::Char('a') => {
                self.reset();
                ParseResult::Complete(Command::Insert(InsertEntry::AfterCursor))
            }
            Key::Char('I') => {
                self.reset();
                ParseResult::Complete(Command::Insert(InsertEntry::LineStart))
            }
            Key::Char('A') => {
                self.reset();
                ParseResult::Complete(Command::Insert(InsertEntry::LineEnd))
            }
            Key::Char('o') => {
                self.reset();
                ParseResult::Complete(Command::Insert(InsertEntry::NewLineBelow))
            }
            Key::Char('O') => {
                self.reset();
                ParseResult::Complete(Command::Insert(InsertEntry::NewLineAbove))
            }
            Key::Char('v') => {
                self.reset();
                let next_mode = if mode == Mode::VisualChar {
                    Mode::Normal
                } else {
                    Mode::VisualChar
                };
                ParseResult::Complete(Command::Visual(next_mode))
            }
            Key::Char('V') => {
                self.reset();
                let next_mode = if mode == Mode::VisualLine {
                    Mode::Normal
                } else {
                    Mode::VisualLine
                };
                ParseResult::Complete(Command::Visual(next_mode))
            }
            Key::Char('x') => {
                self.reset();
                if matches!(mode, Mode::VisualChar | Mode::VisualLine) {
                    ParseResult::Complete(Command::OperatorSelection(Operator::Delete))
                } else {
                    ParseResult::Complete(Command::DeleteChar(count))
                }
            }
            Key::Char('X') => {
                self.reset();
                if matches!(mode, Mode::VisualChar | Mode::VisualLine) {
                    ParseResult::Complete(Command::OperatorSelectionLine(Operator::Delete))
                } else {
                    ParseResult::Invalid
                }
            }
            Key::Char('p') => {
                self.reset();
                ParseResult::Complete(Command::Paste { after: true, count })
            }
            Key::Char('P') => {
                self.reset();
                ParseResult::Complete(Command::Paste {
                    after: false,
                    count,
                })
            }
            Key::Char('u') => {
                self.reset();
                ParseResult::Complete(Command::Undo)
            }
            Key::Ctrl('r') => {
                self.reset();
                ParseResult::Complete(Command::Redo)
            }
            Key::Char('.') => {
                self.reset();
                ParseResult::Complete(Command::RepeatLastEdit)
            }
            _ => match self.parse_motion() {
                ParseMotionResult::Complete(motion) => {
                    self.reset();
                    ParseResult::Complete(Command::Motion { motion, count })
                }
                ParseMotionResult::Incomplete => {
                    self.count_prefix = Some(count);
                    ParseResult::Incomplete
                }
                ParseMotionResult::Invalid => {
                    self.reset();
                    ParseResult::Invalid
                }
            },
        }
    }

    fn parse_motion(&self) -> ParseMotionResult {
        if self.buffer.is_empty() {
            return ParseMotionResult::Incomplete;
        }

        match self.buffer[0] {
            Key::Char('h') | Key::Left => ParseMotionResult::Complete(Motion::Left),
            Key::Char('j') | Key::Down => ParseMotionResult::Complete(Motion::Down),
            Key::Char('k') | Key::Up => ParseMotionResult::Complete(Motion::Up),
            Key::Char('l') | Key::Right => ParseMotionResult::Complete(Motion::Right),
            Key::Char('w') => ParseMotionResult::Complete(Motion::WordForward),
            Key::Char('b') => ParseMotionResult::Complete(Motion::WordBackward),
            Key::Char('e') => ParseMotionResult::Complete(Motion::WordEnd),
            Key::Char('0') => ParseMotionResult::Complete(Motion::LineStart),
            Key::Char('^') => ParseMotionResult::Complete(Motion::LineFirstNonBlank),
            Key::Char('$') => ParseMotionResult::Complete(Motion::LineEnd),
            Key::Char('{') => ParseMotionResult::Complete(Motion::ParagraphBackward),
            Key::Char('}') => ParseMotionResult::Complete(Motion::ParagraphForward),
            Key::Char(';') => ParseMotionResult::Complete(Motion::RepeatFind),
            Key::Char(',') => ParseMotionResult::Complete(Motion::ReverseRepeatFind),
            Key::Char('G') => ParseMotionResult::Complete(Motion::LastLine),
            Key::Char('g') => {
                if self.buffer.len() < 2 {
                    return ParseMotionResult::Incomplete;
                }
                match self.buffer[1] {
                    Key::Char('g') => ParseMotionResult::Complete(Motion::FirstLine),
                    Key::Char('j') => ParseMotionResult::Complete(Motion::ScreenDown),
                    Key::Char('k') => ParseMotionResult::Complete(Motion::ScreenUp),
                    _ => ParseMotionResult::Invalid,
                }
            }
            Key::Char(c @ ('f' | 'F' | 't' | 'T')) => {
                if self.buffer.len() < 2 {
                    return ParseMotionResult::Incomplete;
                }
                if let Key::Char(target_char) = self.buffer[1] {
                    match c {
                        'f' => ParseMotionResult::Complete(Motion::FindCharForward(target_char)),
                        'F' => ParseMotionResult::Complete(Motion::FindCharBackward(target_char)),
                        't' => ParseMotionResult::Complete(Motion::TillCharForward(target_char)),
                        'T' => ParseMotionResult::Complete(Motion::TillCharBackward(target_char)),
                        _ => unreachable!(),
                    }
                } else {
                    ParseMotionResult::Invalid
                }
            }
            _ => ParseMotionResult::Invalid,
        }
    }
}

enum ParseMotionResult {
    Complete(Motion),
    Incomplete,
    Invalid,
}
