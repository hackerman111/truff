use super::context::reborrow;
use super::repeat::command_text_bytes;
use super::{InputContext, InputResult, VimState, surrounding_char_range};
use crate::config::LimitsConfig;
use crate::navigation::Move;
use crate::text::{BytePos, Transaction};
use crate::vim::motion::{
    Motion, TextObjectTarget, eval_delimited_object_range, eval_motion, eval_operator_motion_range,
    eval_text_object_range,
};
use crate::vim::{Command, Mode, Operator};

impl VimState {
    pub(super) fn execute_command(
        &mut self,
        cmd: Command,
        mut ctx: InputContext<'_>,
    ) -> InputResult {
        match cmd {
            Command::Escape => {
                self.cancel_input();
                match ctx {
                    InputContext::Editor { view, .. }
                    | InputContext::TextField { view, .. }
                    | InputContext::CommandLine { view, .. } => {
                        view.set_cursor(view.head(), view.head());
                    }
                    InputContext::Rows(model) | InputContext::Navigator { rows: model, .. } => {
                        model.clear_visual_selection();
                    }
                    _ => {}
                }
                InputResult::Consumed
            }
            Command::Visual(mode) => {
                self.mode = mode;
                if let InputContext::Rows(model) | InputContext::Navigator { rows: model, .. } = ctx
                {
                    model.start_visual_selection();
                }
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
                if let InputContext::Rows(model) | InputContext::Navigator { rows: model, .. } = ctx
                {
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
                if let InputContext::Editor { buffer, view, .. }
                | InputContext::TextField { buffer, view, .. }
                | InputContext::CommandLine { buffer, view, .. } = ctx
                {
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
                        let end = buffer
                            .grapheme_boundaries_from(start)
                            .take(graphemes)
                            .last()
                            .unwrap_or(start);
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
            Command::OperatorSelectionLine(op) => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    let (range, _) = Self::selection_range(buffer, view, Mode::VisualLine);
                    self.operator(op, range, true, buffer, view)
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
                    if matches!(
                        obj.target,
                        TextObjectTarget::Parentheses
                            | TextObjectTarget::Bracket
                            | TextObjectTarget::Brace
                            | TextObjectTarget::AngleBracket
                    ) {
                        return match eval_delimited_object_range(buffer, view.head(), obj, count) {
                            Some(range) => self.operator(op, range, false, buffer, view),
                            None => self.error("Text object target unavailable"),
                        };
                    }
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
                    let end = buffer
                        .grapheme_boundaries_from(view.head())
                        .take(count)
                        .last()
                        .unwrap_or(view.head());
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
            Command::ColonCommand(cmd) => self.colon_command(&cmd, ctx),
            Command::ScrollCursor(pos) => InputResult::ScrollCursor(pos),
            Command::FollowLink => InputResult::FollowLink,
            Command::SearchWordUnderCursor { forward } => {
                let maybe_query = if let InputContext::Editor { buffer, view, .. } = &mut ctx {
                    let pos = view.head();
                    self.text_object_range(
                        buffer,
                        pos,
                        crate::vim::motion::TextObject {
                            kind: crate::vim::motion::TextObjectKind::Inner,
                            target: TextObjectTarget::Word,
                        },
                    )
                    .ok()
                    .map(|range| buffer.slice(range).to_string())
                } else {
                    return self.error("Editor target unavailable");
                };

                if let Some(query) = maybe_query
                    && !query.is_empty()
                {
                    self.last_search = Some((query.clone(), forward));
                    return self.search(&query, forward, 1, ctx);
                }
                InputResult::Consumed
            }
        }
    }
}
