mod colon;
mod command;
mod context;
mod editing;
mod repeat;
mod targets;

use crate::config::LimitsConfig;
use crate::text::{ChangeSet, Registers, UndoGroup};
use crate::vim::motion::{FindKind, SyntaxObjectRange};
use crate::vim::parser::{ParseResult, VimParser};
use crate::vim::{Command, Key, Mode};
pub use context::{InputContext, InputResult, surrounding_char_range};
use std::collections::HashMap;

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
    fn error(&mut self, message: impl Into<String>) -> InputResult {
        let message = message.into();
        self.last_error = Some(message.clone());
        InputResult::Error(message)
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

    /// Returns `true` if Vim parser has a pending count, operator, motion prefix, colon, or search.
    pub fn has_pending_input(&self) -> bool {
        !self.parser.is_idle() || self.selected_reg.is_some()
    }

    /// Returns `true` when printable characters must be treated strictly as literal text input.
    pub fn accepts_literal_text(
        &self,
        context: crate::commands::CommandContext,
        ime_composing: bool,
    ) -> bool {
        ime_composing || (self.mode == Mode::Insert && context.is_editable_text())
    }

    /// Pure resolver for UI input routing: determines whether a key is literal text, IME,
    /// field-local Esc, pending Vim state, or a contextual keymap command/prefix.
    pub fn resolve_key_disposition(
        &self,
        key: Key,
        prefix: &[Key],
        context: crate::commands::CommandContext,
        ime_composing: bool,
        keymap: &crate::config::CompiledKeymap,
    ) -> KeyDisposition {
        if ime_composing {
            return KeyDisposition::ImeComposing;
        }
        if self.mode == Mode::Insert && context.is_editable_text() {
            return match key {
                Key::Esc => KeyDisposition::ExitInsertInField,
                Key::Char(c) => KeyDisposition::LiteralText(c),
                other => KeyDisposition::InsertEditingKey(other),
            };
        }
        if key == Key::Esc {
            if !prefix.is_empty()
                || self.has_pending_input()
                || matches!(self.mode, Mode::VisualChar | Mode::VisualLine)
            {
                return KeyDisposition::CancelPendingOrVisual;
            }
            return KeyDisposition::CloseModalOrCancel;
        }
        if prefix.is_empty() && self.has_pending_input() {
            return KeyDisposition::VimPending;
        }
        let mut seq = Vec::with_capacity(prefix.len() + 1);
        seq.extend_from_slice(prefix);
        seq.push(key);
        match keymap.lookup(self.mode, &seq, context) {
            crate::config::BindingLookup::Complete(id) => KeyDisposition::KeymapCommand(id),
            crate::config::BindingLookup::Prefix => KeyDisposition::KeymapPrefix,
            crate::config::BindingLookup::Missing => KeyDisposition::VimNormal,
        }
    }
}

/// Pure, side-effect-free decision describing how UI/input routing must treat a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyDisposition {
    /// Active IME composition owns the key event (including Esc to cancel composition).
    ImeComposing,
    /// Printable character in an active `Insert` field must be committed as literal text.
    LiteralText(char),
    /// Non-character editing key (`Backspace`, `Enter`, `Tab`, `ShiftTab`, arrows) in `Insert` mode.
    InsertEditingKey(Key),
    /// `Esc` inside an `Insert` text field exits `Insert` -> `Normal` locally without closing outer modals.
    ExitInsertInField,
    /// `Esc` cancels pending Vim state/prefix or Visual mode without closing outer modals.
    CancelPendingOrVisual,
    /// `Esc` when already idle in `Normal` mode may be used by outer shell/modal to close.
    CloseModalOrCancel,
    /// Mid-sequence Vim state (pending operator, count, `f/t`, register, `:` or `/`) owns the key.
    VimPending,
    /// Matches a prefix in `CompiledKeymap` (triggers Which-Key / prefix buffering).
    KeymapPrefix,
    /// Matches a complete `CommandId` in `CompiledKeymap` for the current context.
    KeymapCommand(crate::commands::CommandId),
    /// Standard Normal/Visual Vim parser handling.
    VimNormal,
}
