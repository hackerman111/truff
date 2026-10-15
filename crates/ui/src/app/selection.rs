use super::{AppResult, Application};
use std::ops::Range;
use truff_core::{
    BufferId, ViewId,
    markdown::{
        CalloutKind, LinkSelection,
        style::{apply_highlight, apply_text_color, remove_style, wrap_callout},
    },
};

pub(crate) struct SelectionTarget {
    view: ViewId,
    buffer: BufferId,
    revision: u64,
    range: Range<usize>,
}

impl Application {
    pub(crate) fn capture_selection(&self) -> AppResult<SelectionTarget> {
        let range = self.selection_range()?;
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self
            .shell
            .editor_buffer()
            .ok_or("Selection buffer unavailable")?;
        Ok(SelectionTarget {
            view: view.id(),
            buffer: buffer.id(),
            revision: buffer.revision(),
            range,
        })
    }

    fn selection_buffer(&self, target: &SelectionTarget) -> AppResult<&truff_core::Buffer> {
        let view = self
            .shell
            .workspace()
            .get_view(target.view)
            .ok_or("Selection View was closed")?;
        if view.buffer_id() != target.buffer {
            return Err("Selection target changed".into());
        }
        let buffer = self
            .shell
            .workspace()
            .buffers()
            .get_buffer(target.buffer)
            .ok_or("Selection buffer unavailable")?;
        if buffer.revision() != target.revision {
            return Err("Selection changed; open the picker again".into());
        }
        Ok(buffer)
    }

    pub(crate) fn color_target(&mut self, target: SelectionTarget, color: &str) -> AppResult<()> {
        let hex = color.strip_prefix('#').unwrap_or(color);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Usage: :color #RRGGBB".into());
        }
        let buffer = self.selection_buffer(&target)?;
        let transaction = apply_text_color(&buffer.as_str(), target.range.clone(), color)?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn highlight_target(
        &mut self,
        target: SelectionTarget,
        color: &str,
    ) -> AppResult<()> {
        let hex = color.strip_prefix('#').unwrap_or(color);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Usage: :highlight #RRGGBB".into());
        }
        let buffer = self.selection_buffer(&target)?;
        let transaction = apply_highlight(&buffer.as_str(), target.range.clone(), color)?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn callout_target(
        &mut self,
        target: SelectionTarget,
        kind: CalloutKind,
    ) -> AppResult<()> {
        let buffer = self.selection_buffer(&target)?;
        let transaction = wrap_callout(&buffer.as_str(), target.range.clone(), kind)?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn remove_style_target(&mut self, target: SelectionTarget) -> AppResult<()> {
        let buffer = self.selection_buffer(&target)?;
        let transaction = remove_style(&buffer.as_str(), target.range.clone())?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn link_target(&mut self, target: &SelectionTarget, note: &str) -> AppResult<()> {
        if note.is_empty() || note.contains(['[', ']', '\n', '\r']) {
            return Err("Usage: :link <note or note#heading>".into());
        }
        let buffer = self.selection_buffer(target)?;
        let label = buffer.slice(target.range.clone()).to_string();
        let label_opt = (label != note).then_some(label.as_str());
        let transaction = LinkSelection::wikilink(target.range.clone(), note, label_opt);
        self.shell
            .workspace_mut()
            .set_active_view(Some(target.view));
        self.shell.set_focus(crate::shell::Focus::Editor);
        self.shell.apply_transaction(transaction)?;
        self.handle_shell_action(crate::shell::ShellAction::EditorEdit);
        Ok(())
    }

    fn apply_selection_edit(
        &mut self,
        target: SelectionTarget,
        transaction: truff_core::Transaction,
    ) -> AppResult<()> {
        self.shell
            .workspace_mut()
            .set_active_view(Some(target.view));
        self.shell.set_focus(crate::shell::Focus::Editor);
        self.shell.apply_transaction(transaction)?;
        self.handle_shell_action(crate::shell::ShellAction::EditorEdit);
        Ok(())
    }
}
