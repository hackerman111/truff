use super::{InputContext, InputResult, VimState};
use crate::text::{Buffer, BytePos, EditError, Transaction};
use crate::vim::motion::{SyntaxObjectRange, eval_text_object_range};
use crate::vim::{Command, Mode};
use crate::workspace::View;

impl VimState {
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
        obj: crate::vim::motion::TextObject,
    ) -> Result<std::ops::Range<usize>, String> {
        if matches!(
            obj.target,
            crate::vim::motion::TextObjectTarget::MarkdownLink
                | crate::vim::motion::TextObjectTarget::MarkdownHeading
                | crate::vim::motion::TextObjectTarget::MarkdownCodeBlock
                | crate::vim::motion::TextObjectTarget::MarkdownSection
                | crate::vim::motion::TextObjectTarget::MarkdownListItem
                | crate::vim::motion::TextObjectTarget::MarkdownQuote
                | crate::vim::motion::TextObjectTarget::MarkdownTableCell
                | crate::vim::motion::TextObjectTarget::MarkdownTableRow
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
                    if obj.kind == crate::vim::motion::TextObjectKind::Inner {
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
    pub(super) fn selection_range(
        buffer: &Buffer,
        view: &View,
        mode: Mode,
    ) -> (std::ops::Range<usize>, bool) {
        let linewise = mode == Mode::VisualLine;
        let range = if linewise {
            buffer.visual_line_range(view.anchor(), view.head())
        } else {
            buffer.visual_char_inclusive_range(view.anchor(), view.head())
        };
        (range, linewise)
    }
    pub(super) fn apply(
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
}
