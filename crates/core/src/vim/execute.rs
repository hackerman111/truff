use std::collections::HashMap;

use super::motion::{
    FindKind, Motion, eval_motion, eval_operator_motion_range, eval_text_object_range,
};
use super::parser::{Command, InsertEntry, Operator, ParseResult, VimParser};
use super::{Key, Mode};
use crate::commands::Action;
use crate::config::LimitsConfig;
use crate::text::{Buffer, BytePos, Register, RegisterKind, Registers, Transaction};
use crate::workspace::views::{View, ViewId};

/// Context passed to `VimState::feed_key` representing the active target.
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
    None,
}

/// Result returned from feeding a key to `VimState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputResult {
    Consumed,
    Incomplete,
    Action(Action),
    Actions(Vec<Action>),
    Ignored,
    Error(String),
}

/// Central Vim state machine for the application.
pub struct VimState {
    pub mode: Mode,
    parser: VimParser,
    registers: Registers,
    selected_reg: Option<char>,
    macros: HashMap<char, Vec<Key>>,
    recording_macro: Option<char>,
    macro_buffer: Vec<Key>,
    macro_stack: Vec<char>,
    total_macro_steps: usize,
    last_find: Option<(FindKind, char)>,
    last_edit: Option<Vec<Key>>,
    current_edit: Vec<Key>,
    last_error: Option<String>,
}

impl Default for VimState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            parser: VimParser::new(),
            registers: Registers::new(),
            selected_reg: None,
            macros: HashMap::new(),
            recording_macro: None,
            macro_buffer: Vec::new(),
            macro_stack: Vec::new(),
            total_macro_steps: 0,
            last_find: None,
            last_edit: None,
            current_edit: Vec::new(),
            last_error: None,
        }
    }
}

impl VimState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the length of recorded commands in macro register `reg`.
    pub fn macro_length(&self, reg: char) -> usize {
        self.macros.get(&reg).map_or(0, |v| v.len())
    }

    /// Returns the last error message, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Feeds a key with the given input context.
    pub fn feed_key(&mut self, key: Key, ctx: InputContext<'_>) -> InputResult {
        self.last_error = None;

        // In Insert mode, handle text insertion directly
        if self.mode == Mode::Insert {
            return self.handle_insert_key(key, ctx);
        }

        // Handle Esc in Normal or Visual mode
        if key == Key::Esc {
            self.mode = Mode::Normal;
            self.parser.reset();
            if let InputContext::Editor { view, .. } = ctx {
                view.set_cursor(view.head(), view.head());
            }
            return InputResult::Consumed;
        }

        // Handle macro recording key capture
        if let Some(rec_reg) = self.recording_macro {
            if key == Key::Char('q') && self.macro_buffer.is_empty() {
                // Initial 'q' was start; second 'q' handled in parser
            } else if key == Key::Char('q') {
                // Stop recording
                let recorded = std::mem::take(&mut self.macro_buffer);
                self.macros.insert(rec_reg, recorded);
                self.recording_macro = None;
                return InputResult::Consumed;
            } else if self.macro_buffer.len() < LimitsConfig::DEFAULT_MAX_MACRO_COMMANDS {
                self.macro_buffer.push(key);
            }
        }

        match self.parser.feed(key, self.mode) {
            ParseResult::Incomplete => InputResult::Incomplete,
            ParseResult::Invalid => InputResult::Ignored,
            ParseResult::Complete(cmd) => self.execute_command(cmd, ctx),
        }
    }

    fn handle_insert_key(&mut self, key: Key, ctx: InputContext<'_>) -> InputResult {
        if self.recording_macro.is_some()
            && self.macro_buffer.len() < LimitsConfig::DEFAULT_MAX_MACRO_COMMANDS
        {
            self.macro_buffer.push(key);
        }

        self.current_edit.push(key);

        match key {
            Key::Esc => {
                self.mode = Mode::Normal;
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    // In Vim, moving out of Insert mode moves cursor left one grapheme
                    let head = view.head();
                    let line_start = buffer.byte_of_line(buffer.line_of_byte(head.get()));
                    if head.get() > line_start
                        && let Some(prev) = buffer.grapheme_prev(head)
                    {
                        view.set_cursor(prev, prev);
                    }
                }
                self.last_edit = Some(std::mem::take(&mut self.current_edit));
                InputResult::Consumed
            }
            Key::Backspace => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let head = view.head();
                    if let Some(prev) = buffer.grapheme_prev(head) {
                        let _ = buffer.apply(Transaction::delete(prev.get()..head.get()));
                        view.set_cursor(prev, prev);
                    }
                }
                InputResult::Consumed
            }
            Key::Enter => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let head = view.head();
                    let nl = match buffer.line_ending() {
                        crate::text::LineEnding::Lf => "\n",
                        crate::text::LineEnding::CrLf => "\r\n",
                    };
                    let _ = buffer.apply(Transaction::insert(head.get(), nl));
                    let new_head = head + nl.len();
                    view.set_cursor(new_head, new_head);
                }
                InputResult::Consumed
            }
            Key::Char(c) => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let head = view.head();
                    let mut s = String::new();
                    s.push(c);
                    let _ = buffer.apply(Transaction::insert(head.get(), &s));
                    let new_head = head + s.len();
                    view.set_cursor(new_head, new_head);
                }
                InputResult::Consumed
            }
            _ => InputResult::Ignored,
        }
    }

    fn execute_command(&mut self, cmd: Command, mut ctx: InputContext<'_>) -> InputResult {
        match cmd {
            Command::Escape => {
                self.mode = Mode::Normal;
                if let InputContext::Editor { view, .. } = ctx {
                    view.set_cursor(view.head(), view.head());
                }
                InputResult::Consumed
            }
            Command::Visual(new_mode) => {
                self.mode = new_mode;
                InputResult::Consumed
            }
            Command::SelectRegister(reg) => {
                self.selected_reg = Some(reg);
                InputResult::Consumed
            }
            Command::ToggleMacroRecord(reg) => {
                if self.recording_macro.is_some() {
                    let prev_reg = self.recording_macro.take().unwrap();
                    let recorded = std::mem::take(&mut self.macro_buffer);
                    self.macros.insert(prev_reg, recorded);
                } else {
                    self.recording_macro = Some(reg);
                    self.macro_buffer.clear();
                }
                InputResult::Consumed
            }
            Command::StopMacroRecord => {
                if let Some(reg) = self.recording_macro.take() {
                    let recorded = std::mem::take(&mut self.macro_buffer);
                    self.macros.insert(reg, recorded);
                }
                InputResult::Consumed
            }
            Command::ReplayMacro { reg, count } => {
                // Recursion check: reject if reg is currently replaying
                if self.macro_stack.contains(&reg) {
                    let err = format!("Recursive macro execution detected for register '{reg}'");
                    self.last_error = Some(err.clone());
                    return InputResult::Error(err);
                }

                let keys = match self.macros.get(&reg) {
                    Some(k) => k.clone(),
                    None => return InputResult::Consumed,
                };

                if keys.len() > LimitsConfig::DEFAULT_MAX_MACRO_COMMANDS {
                    let err = format!(
                        "Macro in register '{reg}' exceeds limit of {} commands",
                        LimitsConfig::DEFAULT_MAX_MACRO_COMMANDS
                    );
                    self.last_error = Some(err.clone());
                    return InputResult::Error(err);
                }

                self.macro_stack.push(reg);
                for _ in 0..count {
                    for &k in &keys {
                        self.total_macro_steps += 1;
                        if self.total_macro_steps > LimitsConfig::DEFAULT_MAX_MACRO_STEPS {
                            self.macro_stack.pop();
                            let err = "Macro execution halted: step limit exceeded".to_string();
                            self.last_error = Some(err.clone());
                            return InputResult::Error(err);
                        }
                        // Re-bind context for each replay step
                        // (if ctx is editor, we borrow it)
                        if let InputContext::Editor {
                            view_id,
                            buffer,
                            view,
                        } = &mut ctx
                        {
                            let sub_ctx = InputContext::Editor {
                                view_id: *view_id,
                                buffer,
                                view,
                            };
                            let res = self.feed_key(k, sub_ctx);
                            if let InputResult::Error(_) = res {
                                self.macro_stack.pop();
                                return res;
                            }
                        }
                    }
                }
                self.macro_stack.pop();
                InputResult::Consumed
            }
            Command::Undo => {
                if let InputContext::Editor { buffer, view, .. } = ctx
                    && let Ok(Some(cs)) = buffer.undo()
                {
                    if let Some(first_change) = cs.changes.first() {
                        let pos = BytePos::new(first_change.old_range.start);
                        view.set_cursor(pos, pos);
                    } else {
                        view.apply_change_set(&cs);
                    }
                }
                InputResult::Consumed
            }
            Command::Redo => {
                if let InputContext::Editor { buffer, view, .. } = ctx
                    && let Ok(Some(cs)) = buffer.redo()
                {
                    view.apply_change_set(&cs);
                }
                InputResult::Consumed
            }
            Command::RepeatLastEdit => {
                if let Some(keys) = self.last_edit.clone() {
                    for k in keys {
                        if let InputContext::Editor {
                            view_id,
                            buffer,
                            view,
                        } = &mut ctx
                        {
                            let sub_ctx = InputContext::Editor {
                                view_id: *view_id,
                                buffer,
                                view,
                            };
                            self.feed_key(k, sub_ctx);
                        }
                    }
                }
                InputResult::Consumed
            }
            Command::Motion { motion, count } => {
                match ctx {
                    InputContext::RowModel {
                        selected_index,
                        total_rows,
                        ..
                    } => {
                        // Route shared motions to row navigation
                        let _new_idx = match motion {
                            Motion::Down => {
                                (selected_index + count).min(total_rows.saturating_sub(1))
                            }
                            Motion::Up => selected_index.saturating_sub(count),
                            Motion::FirstLine => 0,
                            Motion::LastLine => total_rows.saturating_sub(1),
                            _ => selected_index,
                        };
                        InputResult::Consumed
                    }
                    InputContext::Editor { buffer, view, .. } => {
                        let (new_pos, updated_find) =
                            eval_motion(buffer, view.head(), &motion, count, self.last_find);
                        self.last_find = updated_find;

                        if self.mode == Mode::VisualChar || self.mode == Mode::VisualLine {
                            view.set_cursor(view.anchor(), new_pos);
                        } else {
                            view.set_cursor(new_pos, new_pos);
                        }
                        InputResult::Consumed
                    }
                    InputContext::None => InputResult::Consumed,
                }
            }
            Command::OperatorMotion { op, motion, count } => {
                match ctx {
                    InputContext::RowModel { panel, .. } => {
                        // 'd' in file tree row model deletes / trashes row
                        if op == Operator::Delete {
                            return InputResult::Action(Action::TrashPath(
                                std::path::PathBuf::from(panel),
                            ));
                        }
                        InputResult::Consumed
                    }
                    InputContext::Editor { buffer, view, .. } => {
                        let (range, is_linewise, updated_find) = eval_operator_motion_range(
                            buffer,
                            view.head(),
                            &motion,
                            count,
                            self.last_find,
                        );
                        self.last_find = updated_find;
                        self.execute_operator_on_range(op, range, is_linewise, buffer, view)
                    }
                    InputContext::None => InputResult::Consumed,
                }
            }
            Command::OperatorTextObject { op, obj, count: _ } => {
                if let InputContext::Editor { buffer, view, .. } = ctx
                    && let Some(range) = eval_text_object_range(buffer, view.head(), obj)
                {
                    return self.execute_operator_on_range(op, range, false, buffer, view);
                }
                InputResult::Consumed
            }
            Command::DeleteChar(count) => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let head = view.head();
                    let mut end = head;
                    for _ in 0..count {
                        if let Some(next) = buffer.grapheme_next(end) {
                            end = next;
                        } else {
                            break;
                        }
                    }
                    let range = head.get()..end.get();
                    if !range.is_empty() {
                        let text = buffer.slice(range.clone()).to_string();
                        self.registers.set(
                            self.selected_reg.take(),
                            Register::new(text, RegisterKind::Characterwise),
                        );
                        let _ = buffer.apply(Transaction::delete(range));
                        view.set_cursor(head, head);
                        self.last_edit = Some(vec![Key::Char('x')]);
                    }
                }
                InputResult::Consumed
            }
            Command::Paste { after, count } => {
                if let InputContext::Editor { buffer, view, .. } = ctx
                    && let Some(reg) = self.registers.get(self.selected_reg.take()).cloned()
                {
                    for _ in 0..count {
                        let head = view.head();
                        match reg.kind {
                            RegisterKind::Linewise => {
                                let line_idx = buffer.line_of_byte(head.get());
                                let insert_pos = if after {
                                    if line_idx + 1 < buffer.line_len() {
                                        buffer.byte_of_line(line_idx + 1)
                                    } else {
                                        buffer.byte_len()
                                    }
                                } else {
                                    buffer.byte_of_line(line_idx)
                                };
                                let _ = buffer.apply(Transaction::insert(insert_pos, &reg.text));
                                view.set_cursor(BytePos::new(insert_pos), BytePos::new(insert_pos));
                            }
                            RegisterKind::Characterwise | RegisterKind::Blockwise => {
                                let insert_pos = if after {
                                    if let Some(next) = buffer.grapheme_next(head) {
                                        next.get()
                                    } else {
                                        head.get()
                                    }
                                } else {
                                    head.get()
                                };
                                let _ = buffer.apply(Transaction::insert(insert_pos, &reg.text));
                                let new_pos = BytePos::new(insert_pos + reg.text.len());
                                view.set_cursor(new_pos, new_pos);
                            }
                        }
                    }
                    let key_used = if after {
                        Key::Char('p')
                    } else {
                        Key::Char('P')
                    };
                    self.last_edit = Some(vec![key_used]);
                }
                InputResult::Consumed
            }
            Command::Insert(entry) => {
                self.mode = Mode::Insert;
                self.current_edit.clear();

                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let head = view.head();
                    match entry {
                        InsertEntry::AtCursor => {
                            self.current_edit.push(Key::Char('i'));
                        }
                        InsertEntry::AfterCursor => {
                            self.current_edit.push(Key::Char('a'));
                            if let Some(next) = buffer.grapheme_next(head) {
                                let line_idx = buffer.line_of_byte(head.get());
                                let next_line_start = if line_idx + 1 < buffer.line_len() {
                                    buffer.byte_of_line(line_idx + 1)
                                } else {
                                    buffer.byte_len()
                                };
                                if next.get() < next_line_start {
                                    view.set_cursor(next, next);
                                }
                            }
                        }
                        InsertEntry::LineStart => {
                            self.current_edit.push(Key::Char('I'));
                            let (pos, _) =
                                eval_motion(buffer, head, &Motion::LineFirstNonBlank, 1, None);
                            view.set_cursor(pos, pos);
                        }
                        InsertEntry::LineEnd => {
                            self.current_edit.push(Key::Char('A'));
                            let line_idx = buffer.line_of_byte(head.get());
                            let next_line_start = if line_idx + 1 < buffer.line_len() {
                                buffer.byte_of_line(line_idx + 1)
                            } else {
                                buffer.byte_len()
                            };
                            let slice = buffer.slice(head.get()..next_line_start).to_string();
                            let mut trim = 0;
                            if slice.ends_with("\r\n") {
                                trim = 2;
                            } else if slice.ends_with('\n') {
                                trim = 1;
                            }
                            let pos = BytePos::new(next_line_start.saturating_sub(trim));
                            view.set_cursor(pos, pos);
                        }
                        InsertEntry::NewLineBelow => {
                            self.current_edit.push(Key::Char('o'));
                            let line_idx = buffer.line_of_byte(head.get());
                            let next_line_start = if line_idx + 1 < buffer.line_len() {
                                buffer.byte_of_line(line_idx + 1)
                            } else {
                                buffer.byte_len()
                            };
                            let _ = buffer.apply(Transaction::insert(next_line_start, "\n"));
                            let pos = BytePos::new(next_line_start);
                            view.set_cursor(pos, pos);
                        }
                        InsertEntry::NewLineAbove => {
                            self.current_edit.push(Key::Char('O'));
                            let line_idx = buffer.line_of_byte(head.get());
                            let line_start = buffer.byte_of_line(line_idx);
                            let _ = buffer.apply(Transaction::insert(line_start, "\n"));
                            let pos = BytePos::new(line_start);
                            view.set_cursor(pos, pos);
                        }
                    }
                }
                InputResult::Consumed
            }
            Command::ColonCommand(cmd) => {
                let trimmed = cmd.trim();
                match trimmed {
                    "w" => {
                        if let InputContext::Editor { buffer, .. } = ctx {
                            buffer.set_savepoint();
                            return InputResult::Action(Action::SaveBuffer(buffer.id()));
                        }
                        InputResult::Consumed
                    }
                    "q" => {
                        if let InputContext::Editor { buffer, view, .. } = ctx {
                            if buffer.is_dirty() {
                                let err = "No write since last change (use ! to override)";
                                self.last_error = Some(err.to_string());
                                return InputResult::Error(err.to_string());
                            }
                            return InputResult::Action(Action::CloseView(view.id()));
                        }
                        InputResult::Consumed
                    }
                    "q!" => {
                        if let InputContext::Editor { view, .. } = ctx {
                            return InputResult::Action(Action::CloseView(view.id()));
                        }
                        InputResult::Consumed
                    }
                    "wq" => {
                        if let InputContext::Editor { buffer, view, .. } = ctx {
                            buffer.set_savepoint();
                            return InputResult::Actions(vec![
                                Action::SaveBuffer(buffer.id()),
                                Action::CloseView(view.id()),
                            ]);
                        }
                        InputResult::Consumed
                    }
                    "bd" => {
                        if let InputContext::Editor { buffer, view, .. } = ctx {
                            if buffer.is_dirty() {
                                let err = "No write since last change (use ! to override)";
                                self.last_error = Some(err.to_string());
                                return InputResult::Error(err.to_string());
                            }
                            return InputResult::Action(Action::CloseView(view.id()));
                        }
                        InputResult::Consumed
                    }
                    _ => InputResult::Ignored,
                }
            }
        }
    }

    fn execute_operator_on_range(
        &mut self,
        op: Operator,
        range: std::ops::Range<usize>,
        is_linewise: bool,
        buffer: &mut Buffer,
        view: &mut View,
    ) -> InputResult {
        if range.is_empty() {
            return InputResult::Consumed;
        }

        let kind = if is_linewise {
            RegisterKind::Linewise
        } else {
            RegisterKind::Characterwise
        };
        let text = buffer.slice(range.clone()).to_string();
        self.registers
            .set(self.selected_reg.take(), Register::new(text, kind));

        match op {
            Operator::Delete => {
                let _ = buffer.apply(Transaction::delete(range.clone()));
                let pos = BytePos::new(range.start);
                view.set_cursor(pos, pos);
                let edit_keys = if is_linewise {
                    vec![Key::Char('d'), Key::Char('d')]
                } else {
                    vec![Key::Char('d'), Key::Char('w')]
                };
                self.last_edit = Some(edit_keys);
            }
            Operator::Change => {
                let _ = buffer.apply(Transaction::delete(range.clone()));
                let pos = BytePos::new(range.start);
                view.set_cursor(pos, pos);
                self.mode = Mode::Insert;
                self.current_edit = vec![Key::Char('c'), Key::Char('i'), Key::Char('w')];
            }
            Operator::Yank => {
                // Yank leaves cursor and buffer intact
            }
            Operator::Indent => {
                let _ = buffer.apply(Transaction::insert(range.start, "    "));
            }
            Operator::Dedent => {
                let slice = buffer
                    .slice(range.start..(range.start + 4.min(buffer.byte_len() - range.start)))
                    .to_string();
                if slice.starts_with("    ") {
                    let _ = buffer.apply(Transaction::delete(range.start..(range.start + 4)));
                }
            }
        }

        InputResult::Consumed
    }
}
