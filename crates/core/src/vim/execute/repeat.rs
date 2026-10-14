use super::{InputContext, InputResult, VimState};
use crate::config::LimitsConfig;
use crate::text::{BytePos, UndoGroup};
use crate::vim::{Command, Mode, Operator};

impl VimState {
    pub(super) fn record(&mut self, cmd: &Command) -> Result<(), String> {
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
    pub fn run_command(&mut self, cmd: Command, ctx: InputContext<'_>) -> InputResult {
        if let InputContext::Editor { buffer, view, .. } = &ctx
            && [view.anchor().get(), view.head().get()]
                .into_iter()
                .any(|p| p > buffer.byte_len() || !buffer.is_char_boundary(p))
        {
            return self.error("Editor cursor is not a valid source boundary");
        }
        if !self.macro_stack.is_empty() {
            self.total_macro_steps += 1;
            if self.total_macro_steps > LimitsConfig::DEFAULT_MAX_MACRO_STEPS {
                return self.error("Macro execution step limit exceeded");
            }
        }
        let resolved_repeat = if let InputContext::Editor { buffer, view, .. } = &ctx {
            let resolved = match &cmd {
                Command::OperatorSelection(op) => {
                    let (range, linewise) = Self::selection_range(buffer, view, self.mode);
                    Some((*op, range, linewise))
                }
                Command::OperatorSelectionLine(op) => {
                    let (range, _) = Self::selection_range(buffer, view, Mode::VisualLine);
                    Some((*op, range, true))
                }
                Command::OperatorRange {
                    op,
                    range,
                    linewise,
                } => Some((*op, range.clone(), *linewise)),
                _ => None,
            };
            resolved.map(|(op, range, linewise)| {
                let mut graphemes = 0;
                if !linewise
                    && range.start < range.end
                    && range.end <= buffer.byte_len()
                    && buffer.is_char_boundary(range.start)
                {
                    // Include the boundary covering a final partial grapheme.
                    graphemes = buffer
                        .grapheme_boundaries_from(BytePos::new(range.start))
                        .take_while(|next| next.get() < range.end)
                        .count()
                        + 1;
                }
                let lines = linewise.then(|| {
                    let end_pos = if range.end > range.start {
                        buffer.floor_char_boundary(range.end.saturating_sub(1))
                    } else {
                        range.start
                    };
                    buffer.line_of_byte(end_pos) - buffer.line_of_byte(range.start) + 1
                });
                Command::OperatorExtent {
                    op,
                    graphemes,
                    lines,
                }
            })
        } else {
            None
        };
        let repeat_command = resolved_repeat.as_ref().unwrap_or(&cmd);
        if !matches!(
            cmd,
            Command::ToggleMacroRecord(_) | Command::StopMacroRecord
        ) && let Err(e) = self.record(repeat_command)
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
            ) | Command::OperatorSelectionLine(
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
                | Command::OperatorSelectionLine(Operator::Change)
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
            self.current_edit_bytes = command_text_bytes(repeat_command);
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
        // TextCommit preserves Insert mode, so its owned repeat entry above
        // suffices. Other edits can leave Insert mode during execution.
        let saved = (is_edit
            && !(self.mode == Mode::Insert && matches!(cmd, Command::TextCommit(_))))
        .then(|| repeat_command.clone());
        let res = self.execute_command(cmd, ctx);
        if is_edit
            && self.mode != Mode::Insert
            && !self.replaying_edit
            && !matches!(res, InputResult::Error(_))
            && let Some(saved) = saved
        {
            self.last_edit = Some(vec![saved]);
        }
        res
    }
}

pub(super) fn command_text_bytes(command: &Command) -> usize {
    match command {
        Command::TextCommit(t) | Command::ColonCommand(t) | Command::Search { query: t, .. } => {
            t.len()
        }
        _ => 0,
    }
}
