use super::{InputContext, InputResult, VimState};
use crate::config::LimitsConfig;
use crate::text::{Buffer, BytePos, Register, RegisterKind, Transaction};
use crate::text::{LineEnding, SingleEdit};
use crate::vim::motion::{Motion, eval_motion, line_content_end, literal_search};
use crate::vim::parser::InsertEntry;
use crate::vim::{Command, Key, Mode, Operator};
use crate::workspace::View;

impl VimState {
    pub(super) fn search(
        &mut self,
        query: &str,
        forward: bool,
        count: usize,
        ctx: InputContext<'_>,
    ) -> InputResult {
        if let InputContext::Editor { buffer, view, .. } = ctx {
            if let Some(pos) = literal_search(buffer, view.head().get(), query, forward, count) {
                let p = BytePos::new(pos);
                let anchor = if matches!(self.mode, Mode::VisualChar | Mode::VisualLine) {
                    view.anchor()
                } else {
                    p
                };
                view.set_cursor_with_jump(anchor, p);
                InputResult::Consumed
            } else {
                self.error(format!("Search pattern not found: {query}"))
            }
        } else if let InputContext::Rows(model) = ctx {
            model.set_filter(query);
            InputResult::Consumed
        } else {
            self.error("Search target unavailable")
        }
    }
    pub(super) fn operator(
        &mut self,
        op: Operator,
        range: std::ops::Range<usize>,
        linewise: bool,
        buffer: &mut Buffer,
        view: &mut View,
    ) -> InputResult {
        if range.end > buffer.byte_len() || range.start > range.end {
            return self.error("Invalid operator source range");
        }
        if !buffer.is_char_boundary(range.start) || !buffer.is_char_boundary(range.end) {
            return self.error("Operator target is not a UTF-8 boundary");
        }
        if matches!(op, Operator::Delete | Operator::Change | Operator::Yank) {
            let name = self.selected_reg.unwrap_or('"');
            let previous = self.registers.get(Some(name)).map_or(0, |r| r.text.len());
            let new_len = range.len()
                + if name.is_ascii_uppercase() {
                    previous
                } else {
                    0
                };
            let named = (('a'..='z').chain(['+', '*']))
                .filter(|c| *c != name.to_ascii_lowercase())
                .map(|c| self.registers.get(Some(c)).map_or(0, |r| r.text.len()))
                .sum::<usize>();
            let total = named + new_len + if name == '"' { 0 } else { new_len };
            if total > LimitsConfig::DEFAULT_MAX_HISTORY_BYTES {
                return self.error("Register text memory limit exceeded");
            }
        }
        let pos = BytePos::new(range.start);
        let tx = match op {
            Operator::Delete | Operator::Change => {
                Some(Transaction::delete(range.clone()).with_cursor(pos, pos))
            }
            Operator::Yank => None,
            Operator::Indent | Operator::Dedent => {
                let first = buffer.line_of_byte(range.start);
                let end_pos = if range.end > range.start {
                    buffer.floor_char_boundary(range.end.saturating_sub(1))
                } else {
                    range.start
                };
                let last = buffer.line_of_byte(end_pos);
                let mut tx = Transaction::new();
                for line in first..=last {
                    let start = buffer.byte_of_line(line);
                    if op == Operator::Indent {
                        tx.edits.push(SingleEdit::insert(start, "    "));
                    } else {
                        let end = line_content_end(buffer, line);
                        let mut width = 0;
                        for c in buffer.slice(start..end).chars().take(4) {
                            if c == '\t' && width == 0 {
                                width = 1;
                                break;
                            }
                            if c != ' ' {
                                break;
                            }
                            width += 1;
                        }
                        if width > 0 {
                            tx.edits.push(SingleEdit::delete(start..start + width));
                        }
                    }
                }
                Some(tx)
            }
        };
        let register =
            matches!(op, Operator::Delete | Operator::Change | Operator::Yank).then(|| {
                Register::new(
                    buffer.slice(range.clone()).to_string(),
                    if linewise {
                        RegisterKind::Linewise
                    } else {
                        RegisterKind::Characterwise
                    },
                )
            });
        if let Some(tx) = tx
            && !tx.edits.is_empty()
            && let Err(e) = self.apply(buffer, view, tx)
        {
            return self.error(e.to_string());
        }
        if let Some(register) = register {
            self.registers.set(self.selected_reg.take(), register);
        }
        if op == Operator::Change {
            self.mode = Mode::Insert;
        } else {
            self.mode = Mode::Normal;
            self.insert_group = None;
            view.set_cursor(view.head(), view.head());
        }
        InputResult::Consumed
    }
    pub(super) fn enter_insert(
        &mut self,
        entry: InsertEntry,
        ctx: InputContext<'_>,
    ) -> InputResult {
        let InputContext::Editor { buffer, view, .. } = ctx else {
            return self.error("Editor target unavailable");
        };
        let head = view.head();
        let line = buffer.line_of_byte(head.get());
        let start = buffer.byte_of_line(line);
        let end = line_content_end(buffer, line);
        let nl = if buffer.line_ending() == LineEnding::CrLf {
            "\r\n"
        } else {
            "\n"
        };
        let pos = match entry {
            InsertEntry::AtCursor => head.get(),
            InsertEntry::AfterCursor => buffer
                .grapheme_next(head)
                .map_or(head.get(), BytePos::get)
                .min(end),
            InsertEntry::LineStart => {
                eval_motion(buffer, head, &Motion::LineFirstNonBlank, 1, None)
                    .0
                    .get()
            }
            InsertEntry::LineEnd => end,
            InsertEntry::NewLineAbove => {
                let p = BytePos::new(start);
                if let Err(e) = self.apply(
                    buffer,
                    view,
                    Transaction::insert(start, nl).with_cursor(p, p),
                ) {
                    return self.error(e.to_string());
                }
                start
            }
            InsertEntry::NewLineBelow => {
                let next = if line + 1 < buffer.line_len() {
                    buffer.byte_of_line(line + 1)
                } else {
                    buffer.byte_len()
                };
                // An unterminated final line needs a separator before the new text.
                let p = BytePos::new(if next == end { next + nl.len() } else { next });
                if let Err(e) = self.apply(
                    buffer,
                    view,
                    Transaction::insert(next, nl).with_cursor(p, p),
                ) {
                    return self.error(e.to_string());
                }
                p.get()
            }
        };
        let p = BytePos::new(pos);
        view.set_cursor(p, p);
        self.mode = Mode::Insert;
        InputResult::Consumed
    }
    pub(super) fn insert_key(&mut self, key: Key, ctx: InputContext<'_>) -> InputResult {
        let InputContext::Editor { buffer, view, .. } = ctx else {
            return self.error("Editor target unavailable");
        };
        let head = view.head();
        match key {
            Key::Esc => {
                self.mode = Mode::Normal;
                self.insert_group = None;
                if head.get() > buffer.byte_of_line(buffer.line_of_byte(head.get()))
                    && let Some(prev) = buffer.grapheme_prev(head)
                {
                    view.set_cursor(prev, prev);
                }
                if !self.replaying_edit {
                    self.last_edit = Some(std::mem::take(&mut self.current_edit));
                }
            }
            Key::Backspace | Key::Delete => {
                let start = view.anchor().get().min(head.get());
                let end = view.anchor().get().max(head.get());
                let range = if start != end {
                    start..end
                } else if key == Key::Backspace {
                    buffer.grapheme_prev(head).map_or(start, BytePos::get)..end
                } else {
                    start..buffer.grapheme_next(head).map_or(end, BytePos::get)
                };
                if !range.is_empty() {
                    let p = BytePos::new(range.start);
                    if let Err(e) =
                        self.apply(buffer, view, Transaction::delete(range).with_cursor(p, p))
                    {
                        return self.error(e.to_string());
                    }
                }
            }
            Key::Tab | Key::Enter => {
                let text = if key == Key::Tab {
                    "\t"
                } else if buffer.line_ending() == LineEnding::CrLf {
                    "\r\n"
                } else {
                    "\n"
                };
                return self.execute_command(
                    Command::TextCommit(text.into()),
                    InputContext::Editor {
                        view_id: view.id(),
                        buffer,
                        view,
                    },
                );
            }
            Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End => {
                let pos = match key {
                    Key::Left => buffer.grapheme_prev(head).unwrap_or(head),
                    Key::Right => buffer.grapheme_next(head).unwrap_or(head),
                    Key::Home => BytePos::new(buffer.byte_of_line(buffer.line_of_byte(head.get()))),
                    Key::End => {
                        BytePos::new(line_content_end(buffer, buffer.line_of_byte(head.get())))
                    }
                    Key::Up => eval_motion(buffer, head, &Motion::Up, 1, None).0,
                    Key::Down => eval_motion(buffer, head, &Motion::Down, 1, None).0,
                    _ => head,
                };
                view.set_cursor(pos, pos);
            }
            _ => return InputResult::Ignored,
        }
        InputResult::Consumed
    }
    pub(super) fn paste(
        &mut self,
        after: bool,
        count: usize,
        ctx: InputContext<'_>,
    ) -> InputResult {
        let InputContext::Editor { buffer, view, .. } = ctx else {
            return self.error("Editor target unavailable");
        };
        let Some(reg) = self.registers.get(self.selected_reg.take()).cloned() else {
            return self.error("Register unavailable");
        };
        let Some(bytes) = reg.text.len().checked_mul(count) else {
            return self.error("Paste count arithmetic overflow");
        };
        if bytes > buffer.max_file_bytes().saturating_sub(buffer.byte_len()) {
            return self.error("Paste exceeds buffer size limit");
        }
        let head = view.head();
        let line = buffer.line_of_byte(head.get());
        let nl = if buffer.line_ending() == LineEnding::CrLf {
            "\r\n"
        } else {
            "\n"
        };
        let mut text = String::new();
        for index in 0..count {
            text.push_str(&reg.text);
            if reg.kind == RegisterKind::Linewise && !reg.text.ends_with('\n') && index + 1 < count
            {
                text.push_str(nl);
            }
        }
        let mut prefix_len = 0;
        let pos = if reg.kind == RegisterKind::Linewise {
            let insert = if after {
                if line + 1 < buffer.line_len() {
                    buffer.byte_of_line(line + 1)
                } else {
                    buffer.byte_len()
                }
            } else {
                buffer.byte_of_line(line)
            };
            if insert == buffer.byte_len() && insert > 0 && line_content_end(buffer, line) == insert
            {
                text.insert_str(0, nl);
                prefix_len = nl.len();
            }
            if insert < buffer.byte_len() && !text.ends_with('\n') {
                text.push_str(nl);
            }
            insert
        } else if after {
            buffer
                .grapheme_next(head)
                .map_or(head.get(), BytePos::get)
                .min(line_content_end(buffer, line))
        } else {
            head.get()
        };
        if text.len() > buffer.max_file_bytes().saturating_sub(buffer.byte_len()) {
            return self.error("Paste exceeds buffer size limit");
        }
        let cursor = if reg.kind == RegisterKind::Linewise {
            pos + prefix_len
        } else {
            pos + text.len()
        };
        let cursor = BytePos::new(cursor);
        match self.apply(
            buffer,
            view,
            Transaction::insert(pos, text).with_cursor(cursor, cursor),
        ) {
            Ok(()) => InputResult::Consumed,
            Err(e) => self.error(e.to_string()),
        }
    }
}
