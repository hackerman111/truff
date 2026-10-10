use std::collections::HashMap;
mod editing;
use super::motion::{
    FindKind, Motion, SyntaxObjectRange, eval_motion, eval_operator_motion_range,
    eval_text_object_range, line_content_end, literal_search,
};
use super::parser::{Command, InsertEntry, Operator, ParseResult, VimParser};
use super::{Key, Mode};
use crate::commands::{Action, CommandId};
use crate::config::LimitsConfig;
use crate::navigation::{Move, RowModel};
use crate::text::{
    Buffer, BytePos, ChangeSet, EditError, Register, RegisterKind, Registers, Transaction,
    UndoGroup,
};
use crate::workspace::views::{View, ViewId};

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

/// One parsed command or committed text event is one macro instruction.
pub struct VimState {
    pub mode: Mode,
    parser: VimParser,
    registers: Registers,
    selected_reg: Option<char>,
    macros: HashMap<char, Vec<Command>>,
    recording_macro: Option<char>,
    macro_buffer: Vec<Command>,
    macro_bytes: usize,
    macro_text_budget: usize,
    macro_stack: Vec<char>,
    total_macro_steps: usize,
    last_find: Option<(FindKind, char)>,
    last_search: Option<(String, bool)>,
    last_edit: Option<Vec<Command>>,
    current_edit: Vec<Command>,
    current_edit_bytes: usize,
    insert_group: Option<UndoGroup>,
    last_error: Option<String>,
    changes: Vec<ChangeSet>,
    syntax_revision: Option<u64>,
    syntax_objects: Vec<SyntaxObjectRange>,
    replaying_edit: bool,
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
            macro_bytes: 0,
            macro_text_budget: LimitsConfig::DEFAULT_MAX_HISTORY_BYTES,
            macro_stack: Vec::new(),
            total_macro_steps: 0,
            last_find: None,
            last_search: None,
            last_edit: None,
            current_edit: Vec::new(),
            current_edit_bytes: 0,
            insert_group: None,
            last_error: None,
            changes: Vec::new(),
            syntax_revision: None,
            syntax_objects: Vec::new(),
            replaying_edit: false,
        }
    }
}
impl VimState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel_input(&mut self) {
        if !self.current_edit.is_empty() {
            let edit = std::mem::take(&mut self.current_edit);
            if !self.replaying_edit {
                self.last_edit = Some(edit);
            }
        }
        self.current_edit_bytes = 0;
        self.mode = Mode::Normal;
        self.parser.reset();
        self.selected_reg = None;
        self.insert_group = None;
        self.last_error = None;
    }
    pub fn macro_length(&self, reg: char) -> usize {
        self.macros.get(&reg).map_or(0, Vec::len)
    }
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
    pub fn search_query(&self) -> Option<&str> {
        self.parser.search_query()
    }
    pub fn take_changes(&mut self) -> Vec<ChangeSet> {
        std::mem::take(&mut self.changes)
    }
    pub fn set_syntax_objects(&mut self, revision: u64, objects: Vec<SyntaxObjectRange>) {
        self.syntax_revision = Some(revision);
        self.syntax_objects = objects;
    }
    pub fn clear_syntax_objects(&mut self) {
        self.syntax_revision = None;
        self.syntax_objects.clear();
    }
    pub fn syntax_object_ranges(&self, revision: u64) -> Option<&[SyntaxObjectRange]> {
        (self.syntax_revision == Some(revision)).then_some(self.syntax_objects.as_slice())
    }
    pub fn text_object_range(
        &self,
        buffer: &Buffer,
        position: BytePos,
        obj: super::motion::TextObject,
    ) -> Result<std::ops::Range<usize>, String> {
        if matches!(
            obj.target,
            super::motion::TextObjectTarget::MarkdownLink
                | super::motion::TextObjectTarget::MarkdownHeading
                | super::motion::TextObjectTarget::MarkdownCodeBlock
        ) {
            if self.syntax_revision != Some(buffer.revision()) {
                return Err("Syntax object ranges unavailable at this revision".into());
            }
            self.syntax_objects
                .iter()
                .filter(|r| {
                    r.target == obj.target
                        && r.around.start <= position.get()
                        && position.get() < r.around.end
                })
                .min_by_key(|r| r.around.len())
                .map(|r| {
                    if obj.kind == super::motion::TextObjectKind::Inner {
                        r.inner.clone()
                    } else {
                        r.around.clone()
                    }
                })
                .ok_or_else(|| "Syntax object target unavailable".into())
        } else {
            eval_text_object_range(buffer, position, obj)
                .ok_or_else(|| "Text object target unavailable".into())
        }
    }
    pub fn apply_visual_row(&mut self, view: &mut View, target: BytePos) {
        let anchor = if matches!(self.mode, Mode::VisualChar | Mode::VisualLine) {
            view.anchor()
        } else {
            target
        };
        view.set_cursor(anchor, target);
    }
    pub fn target_range(
        &mut self,
        range: std::ops::Range<usize>,
        ctx: InputContext<'_>,
    ) -> InputResult {
        if let Some(op) = self.parser.take_operator() {
            return self.run_command(
                Command::OperatorRange {
                    op,
                    range,
                    linewise: false,
                },
                ctx,
            );
        }
        if let InputContext::Editor { buffer, view, .. } = ctx {
            if range.start > range.end
                || range.end > buffer.byte_len()
                || !buffer.is_char_boundary(range.start)
                || !buffer.is_char_boundary(range.end)
            {
                return self.error("Invalid source target range");
            }
            if matches!(self.mode, Mode::VisualChar | Mode::VisualLine) {
                let end = buffer
                    .grapheme_prev(BytePos::new(range.end))
                    .unwrap_or(BytePos::new(range.start));
                view.set_cursor(BytePos::new(range.start), end);
            } else {
                let p = BytePos::new(range.start);
                view.set_cursor_with_jump(p, p);
            }
            InputResult::Consumed
        } else {
            self.error("Editor target unavailable")
        }
    }
    fn error(&mut self, message: impl Into<String>) -> InputResult {
        let message = message.into();
        self.last_error = Some(message.clone());
        InputResult::Error(message)
    }
    fn record(&mut self, cmd: &Command) -> Result<(), String> {
        if self.recording_macro.is_none() || !self.macro_stack.is_empty() {
            return Ok(());
        }
        let bytes = match cmd {
            Command::TextCommit(t)
            | Command::ColonCommand(t)
            | Command::Search { query: t, .. } => t.len(),
            _ => 0,
        };
        if self.macro_buffer.len() >= LimitsConfig::DEFAULT_MAX_MACRO_COMMANDS
            || self.macro_bytes.saturating_add(bytes) > self.macro_text_budget
        {
            self.recording_macro = None;
            self.macro_buffer.clear();
            self.macro_bytes = 0;
            return Err(
                "Macro definition exceeds command or text memory limit; recording discarded".into(),
            );
        }
        self.macro_bytes += bytes;
        self.macro_buffer.push(cmd.clone());
        Ok(())
    }
    pub fn feed_key(&mut self, key: Key, ctx: InputContext<'_>) -> InputResult {
        self.last_error = None;
        if self.mode == Mode::Insert {
            let cmd = if let Key::Char(c) = key {
                Command::TextCommit(c.to_string())
            } else {
                Command::InsertKey(key)
            };
            return self.run_command(cmd, ctx);
        }
        if key == Key::Char('q') && self.recording_macro.is_some() && self.parser.is_idle() {
            let reg = self.recording_macro.take().unwrap();
            self.macros
                .insert(reg, std::mem::take(&mut self.macro_buffer));
            self.macro_bytes = 0;
            return InputResult::Consumed;
        }
        match self.parser.feed(key, self.mode) {
            ParseResult::Incomplete => InputResult::Incomplete,
            ParseResult::Invalid => InputResult::Ignored,
            ParseResult::Overflow => self.error("Vim count arithmetic overflow"),
            ParseResult::TextLimit => self.error("Vim query text limit exceeded"),
            ParseResult::Complete(cmd) => self.run_command(cmd, ctx),
        }
    }
    /// Canonical Text/IME/Paste event. This never splits a multi-character commit into keys.
    pub fn commit_text(&mut self, text: &str, ctx: InputContext<'_>) -> InputResult {
        if text.is_empty() {
            return InputResult::Consumed;
        }
        if text.len() > LimitsConfig::DEFAULT_MAX_HISTORY_BYTES {
            return self.error("Committed text repeat memory limit exceeded");
        }
        self.run_command(Command::TextCommit(text.into()), ctx)
    }
    pub fn run_binding(&mut self, mut cmd: Command, ctx: InputContext<'_>) -> InputResult {
        let count = self.parser.take_count();
        match &mut cmd {
            Command::Motion { count: n, .. }
            | Command::OperatorMotion { count: n, .. }
            | Command::OperatorTextObject { count: n, .. }
            | Command::DeleteChar(n)
            | Command::Paste { count: n, .. } => *n = count,
            _ => {}
        }
        self.run_command(cmd, ctx)
    }
    pub fn run_command(&mut self, cmd: Command, ctx: InputContext<'_>) -> InputResult {
        if let InputContext::Editor { buffer, view, .. } = &ctx {
            if [view.anchor().get(), view.head().get()]
                .into_iter()
                .any(|p| p > buffer.byte_len() || !buffer.is_char_boundary(p))
            {
                return self.error("Editor cursor is not a valid source boundary");
            }
        }
        if !self.macro_stack.is_empty() {
            self.total_macro_steps += 1;
            if self.total_macro_steps > LimitsConfig::DEFAULT_MAX_MACRO_STEPS {
                return self.error("Macro execution step limit exceeded");
            }
        }
        let repeat_command = if let InputContext::Editor { buffer, view, .. } = &ctx {
            let resolved = match &cmd {
                Command::OperatorSelection(op) => {
                    let (range, linewise) = Self::selection_range(buffer, view, self.mode);
                    Some((*op, range, linewise))
                }
                Command::OperatorRange {
                    op,
                    range,
                    linewise,
                } => Some((*op, range.clone(), *linewise)),
                _ => None,
            };
            resolved
                .map(|(op, range, linewise)| {
                    let mut graphemes = 0;
                    let mut cursor = BytePos::new(range.start);
                    if range.start <= range.end
                        && range.end <= buffer.byte_len()
                        && buffer.is_char_boundary(range.start)
                    {
                        while cursor.get() < range.end {
                            let Some(next) = buffer.grapheme_next(cursor) else {
                                break;
                            };
                            graphemes += 1;
                            cursor = next;
                        }
                    }
                    let lines = linewise.then(|| {
                        buffer.line_of_byte(range.end.saturating_sub(1).max(range.start))
                            - buffer.line_of_byte(range.start)
                            + 1
                    });
                    Command::OperatorExtent {
                        op,
                        graphemes,
                        lines,
                    }
                })
                .unwrap_or_else(|| cmd.clone())
        } else {
            cmd.clone()
        };
        if !matches!(
            cmd,
            Command::ToggleMacroRecord(_) | Command::StopMacroRecord
        ) && let Err(e) = self.record(&repeat_command)
        {
            return self.error(e);
        }
        let is_edit = matches!(
            cmd,
            Command::OperatorMotion {
                op: Operator::Delete | Operator::Change | Operator::Indent | Operator::Dedent,
                ..
            } | Command::OperatorTextObject {
                op: Operator::Delete | Operator::Change | Operator::Indent | Operator::Dedent,
                ..
            } | Command::OperatorSelection(
                Operator::Delete | Operator::Change | Operator::Indent | Operator::Dedent
            ) | Command::OperatorRange {
                op: Operator::Delete | Operator::Change | Operator::Indent | Operator::Dedent,
                ..
            } | Command::OperatorExtent {
                op: Operator::Delete | Operator::Change | Operator::Indent | Operator::Dedent,
                ..
            } | Command::DeleteChar(_)
                | Command::Paste { .. }
                | Command::Insert(_)
                | Command::TextCommit(_)
                | Command::DeleteSurrounding { .. }
        );
        let starts_insert = matches!(
            cmd,
            Command::Insert(_)
                | Command::OperatorMotion {
                    op: Operator::Change,
                    ..
                }
                | Command::OperatorTextObject {
                    op: Operator::Change,
                    ..
                }
                | Command::OperatorSelection(Operator::Change)
                | Command::OperatorRange {
                    op: Operator::Change,
                    ..
                }
                | Command::OperatorExtent {
                    op: Operator::Change,
                    ..
                }
        );
        if starts_insert {
            self.insert_group = Some(UndoGroup::next());
            self.current_edit = vec![repeat_command.clone()];
            self.current_edit_bytes = command_text_bytes(&repeat_command);
        } else if self.mode == Mode::Insert && !self.replaying_edit {
            let added = command_text_bytes(&cmd);
            if self.current_edit_bytes.saturating_add(added)
                > LimitsConfig::DEFAULT_MAX_HISTORY_BYTES
                || self.current_edit.len() >= 10000
            {
                return self.error("Insert repeat memory limit exceeded");
            }
            self.current_edit_bytes += added;
            self.current_edit.push(cmd.clone());
        }
        let saved = repeat_command;
        let res = self.execute_command(cmd, ctx);
        if is_edit
            && self.mode != Mode::Insert
            && !self.replaying_edit
            && !matches!(res, InputResult::Error(_))
        {
            self.last_edit = Some(vec![saved]);
        }
        res
    }
    fn selection_range(buffer: &Buffer, view: &View, mode: Mode) -> (std::ops::Range<usize>, bool) {
        let start = view.anchor().get().min(view.head().get());
        let end = view.anchor().get().max(view.head().get());
        let linewise = mode == Mode::VisualLine;
        let range = if linewise {
            let first = buffer.line_of_byte(start);
            let last = buffer.line_of_byte(end);
            let end = if last + 1 < buffer.line_len() {
                buffer.byte_of_line(last + 1)
            } else {
                buffer.byte_len()
            };
            buffer.byte_of_line(first)..end
        } else {
            start
                ..buffer
                    .grapheme_next(BytePos::new(end))
                    .map_or(end, BytePos::get)
        };
        (range, linewise)
    }
    fn apply(
        &mut self,
        buffer: &mut Buffer,
        view: &mut View,
        mut tx: Transaction,
    ) -> Result<(), EditError> {
        if let Some(group) = self.insert_group {
            tx.undo_group = Some(group);
        }
        let explicit = tx.cursor;
        let changes = buffer.apply(tx)?;
        view.apply_change_set(&changes);
        if let Some(cursor) = explicit {
            view.set_cursor(cursor.anchor, cursor.head);
        }
        self.changes.push(changes);
        Ok(())
    }
    fn execute_command(&mut self, cmd: Command, mut ctx: InputContext<'_>) -> InputResult {
        match cmd {
            Command::Escape => {
                self.cancel_input();
                if let InputContext::Editor { view, .. } = ctx {
                    view.set_cursor(view.head(), view.head());
                }
                InputResult::Consumed
            }
            Command::Visual(mode) => {
                self.mode = mode;
                InputResult::Consumed
            }
            Command::SelectRegister(reg) => {
                if !reg.is_ascii_alphabetic() && !matches!(reg, '"' | '+' | '*') {
                    return self.error("Unsupported register name");
                }
                self.selected_reg = Some(reg);
                InputResult::Consumed
            }
            Command::ToggleMacroRecord(reg) => {
                if !reg.is_ascii_lowercase() {
                    return self.error("Macro register must be a-z");
                }
                let used = self
                    .macros
                    .iter()
                    .filter(|(name, _)| **name != reg)
                    .flat_map(|(_, commands)| commands)
                    .map(command_text_bytes)
                    .sum::<usize>();
                self.macro_text_budget =
                    LimitsConfig::DEFAULT_MAX_HISTORY_BYTES.saturating_sub(used);
                self.recording_macro = Some(reg);
                self.macro_buffer.clear();
                self.macro_bytes = 0;
                InputResult::Consumed
            }
            Command::StopMacroRecord => {
                if let Some(reg) = self.recording_macro.take() {
                    self.macros
                        .insert(reg, std::mem::take(&mut self.macro_buffer));
                }
                InputResult::Consumed
            }
            Command::ReplayMacro { reg, count } => {
                if self.macro_stack.contains(&reg) {
                    return self.error("Recursive macro execution rejected");
                }
                if self.macro_stack.is_empty() {
                    self.total_macro_steps = 0;
                }
                let Some(commands) = self.macros.get(&reg).cloned() else {
                    return self.error(format!("Macro register '{reg}' unavailable"));
                };
                if commands
                    .len()
                    .checked_mul(count)
                    .is_none_or(|steps| steps > LimitsConfig::DEFAULT_MAX_MACRO_STEPS)
                {
                    return self.error("Macro execution step limit exceeded");
                }
                self.macro_stack.push(reg);
                for _ in 0..count {
                    for command in &commands {
                        let result = self.run_command(command.clone(), reborrow(&mut ctx));
                        if matches!(
                            result,
                            InputResult::Error(_)
                                | InputResult::Action(_)
                                | InputResult::Actions(_)
                                | InputResult::Command { .. }
                                | InputResult::VisualRow { .. }
                        ) {
                            self.macro_stack.pop();
                            return result;
                        }
                    }
                }
                self.macro_stack.pop();
                InputResult::Consumed
            }
            Command::RepeatLastEdit => {
                let commands = self.last_edit.clone().unwrap_or_default();
                self.replaying_edit = true;
                for command in commands {
                    let result = self.run_command(command, reborrow(&mut ctx));
                    if matches!(result, InputResult::Error(_)) {
                        self.replaying_edit = false;
                        return result;
                    }
                }
                self.replaying_edit = false;
                InputResult::Consumed
            }
            Command::Undo | Command::Redo => {
                let redo = cmd == Command::Redo;
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    match if redo { buffer.redo() } else { buffer.undo() } {
                        Ok(Some(cs)) => {
                            view.apply_change_set(&cs);
                            if !redo
                                && let Some(start) =
                                    cs.changes().iter().map(|c| c.old_range.start).min()
                            {
                                let pos = BytePos::new(start.min(buffer.byte_len()));
                                view.set_cursor(pos, pos);
                            }
                            self.changes.push(cs);
                        }
                        Ok(None) => {}
                        Err(e) => return self.error(e.to_string()),
                    }
                    InputResult::Consumed
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::Search {
                query,
                forward,
                count,
            } => {
                let query = if query.is_empty() {
                    self.last_search
                        .as_ref()
                        .map(|(q, _)| q.clone())
                        .unwrap_or_default()
                } else {
                    query
                };
                self.last_search = Some((query.clone(), forward));
                self.search(&query, forward, count, ctx)
            }
            Command::RepeatSearch { reverse, count } => {
                let Some((query, forward)) = self.last_search.clone() else {
                    return self.error("No previous search");
                };
                self.search(&query, if reverse { !forward } else { forward }, count, ctx)
            }
            Command::Motion { motion, count } => {
                if let InputContext::Rows(model) = ctx {
                    let movement = match motion {
                        Motion::Down => Move::Down(count),
                        Motion::Up => Move::Up(count),
                        Motion::FirstLine => Move::Top,
                        Motion::LastLine => Move::Bottom,
                        Motion::Left => Move::Parent,
                        Motion::Right => Move::Child,
                        _ => return InputResult::Ignored,
                    };
                    model.move_selection(movement);
                    return InputResult::Consumed;
                }
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    if matches!(motion, Motion::ScreenDown | Motion::ScreenUp) {
                        return InputResult::VisualRow {
                            down: motion == Motion::ScreenDown,
                            count,
                        };
                    }
                    let (pos, find) =
                        eval_motion(buffer, view.head(), &motion, count, self.last_find);
                    self.last_find = find;
                    let anchor = if matches!(self.mode, Mode::VisualChar | Mode::VisualLine) {
                        view.anchor()
                    } else {
                        pos
                    };
                    view.set_cursor(anchor, pos);
                }
                InputResult::Consumed
            }
            Command::OperatorExtent {
                op,
                graphemes,
                lines,
            } => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let (range, linewise) = if let Some(lines) = lines {
                        let (r, l, _) = eval_operator_motion_range(
                            buffer,
                            view.head(),
                            &Motion::CurrentLine,
                            lines,
                            None,
                        );
                        (r, l)
                    } else {
                        let start = view.head();
                        let mut end = start;
                        for _ in 0..graphemes {
                            let Some(next) = buffer.grapheme_next(end) else {
                                break;
                            };
                            end = next;
                        }
                        (start.get()..end.get(), false)
                    };
                    self.operator(op, range, linewise, buffer, view)
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::OperatorSelection(op) => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let (range, linewise) = Self::selection_range(buffer, view, self.mode);
                    self.operator(op, range, linewise, buffer, view)
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::OperatorRange {
                op,
                range,
                linewise,
            } => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    self.operator(op, range, linewise, buffer, view)
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::OperatorMotion { op, motion, count } => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let (range, linewise, find) = eval_operator_motion_range(
                        buffer,
                        view.head(),
                        &motion,
                        count,
                        self.last_find,
                    );
                    self.last_find = find;
                    self.operator(op, range, linewise, buffer, view)
                } else {
                    InputResult::Ignored
                }
            }
            Command::OperatorTextObject { op, obj, count } => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let mut range = match self.text_object_range(buffer, view.head(), obj) {
                        Ok(range) => Some(range),
                        Err(e) => return self.error(e),
                    };
                    for _ in 1..count {
                        if let Some(r) = &mut range
                            && let Some(next) =
                                eval_text_object_range(buffer, BytePos::new(r.end), obj)
                        {
                            if next.end <= r.end {
                                break;
                            }
                            r.end = next.end;
                        }
                    }
                    if let Some(range) = range {
                        self.operator(op, range, false, buffer, view)
                    } else {
                        self.error("Text object target unavailable")
                    }
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::DeleteChar(count) => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let mut end = view.head();
                    for _ in 0..count {
                        let Some(next) = buffer.grapheme_next(end) else {
                            break;
                        };
                        end = next;
                    }
                    self.operator(
                        Operator::Delete,
                        view.head().get()..end.get(),
                        false,
                        buffer,
                        view,
                    )
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::Paste { after, count } => self.paste(after, count, ctx),
            Command::Insert(entry) => self.enter_insert(entry, ctx),
            Command::TextCommit(text) => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let start = view.anchor().get().min(view.head().get());
                    let end = view.anchor().get().max(view.head().get());
                    let pos = BytePos::new(start + text.len());
                    let tx = Transaction::replace(start..end, text).with_cursor(pos, pos);
                    match self.apply(buffer, view, tx) {
                        Ok(()) => InputResult::Consumed,
                        Err(e) => self.error(e.to_string()),
                    }
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::InsertKey(key) => self.insert_key(key, ctx),
            Command::DeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let range = surrounding_char_range(
                        buffer,
                        view.head().get(),
                        before_chars,
                        after_chars,
                    );
                    if range.is_empty() {
                        return InputResult::Consumed;
                    }
                    let p = BytePos::new(range.start);
                    match self.apply(buffer, view, Transaction::delete(range).with_cursor(p, p)) {
                        Ok(()) => InputResult::Consumed,
                        Err(e) => self.error(e.to_string()),
                    }
                } else {
                    self.error("Editor target unavailable")
                }
            }
            Command::ColonCommand(cmd) => {
                let trimmed = cmd.trim();
                match trimmed {
                    "w" => {
                        if let InputContext::Editor { buffer, .. } = ctx {
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
                    _ => {
                        let (name, argument) = trimmed.split_once(' ').unwrap_or((trimmed, ""));
                        let id = match name {
                            "e" => "buffer.open",
                            "b" => "buffer.select",
                            "bn" => "buffer.next",
                            "bp" => "buffer.previous",
                            _ => return self.error(format!("Unknown command: {name}")),
                        };
                        InputResult::Command {
                            id: CommandId::new(id),
                            argument: argument.into(),
                        }
                    }
                }
            }
        }
    }
}

fn reborrow<'a>(ctx: &'a mut InputContext<'_>) -> InputContext<'a> {
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

fn command_text_bytes(command: &Command) -> usize {
    match command {
        Command::TextCommit(t) | Command::ColonCommand(t) | Command::Search { query: t, .. } => {
            t.len()
        }
        _ => 0,
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
