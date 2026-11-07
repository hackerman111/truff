use super::{AppResult, Application};
use std::ops::Range;
use truff_core::{
    BufferId, ViewId,
    markdown::{
        CalloutKind, LinkSelection,
        style::{apply_highlight, apply_text_color, remove_style, wrap_callout},
    },
};

#[derive(Clone, Debug)]
pub(crate) struct SelectionTarget {
    pub(crate) view: ViewId,
    pub(crate) buffer: BufferId,
    pub(crate) revision: u64,
    pub(crate) range: Range<usize>,
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

    pub(crate) fn capture_selection_or_cursor(&self) -> AppResult<(SelectionTarget, bool)> {
        if let Ok(target) = self.capture_selection() {
            return Ok((target, true));
        }
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self
            .shell
            .editor_buffer()
            .ok_or("Selection buffer unavailable")?;
        let cursor = view.head().get();
        if let Some(range) = crate::overlays::word_boundary_at(buffer, cursor) {
            return Ok((
                SelectionTarget {
                    view: view.id(),
                    buffer: buffer.id(),
                    revision: buffer.revision(),
                    range,
                },
                true,
            ));
        }
        Ok((
            SelectionTarget {
                view: view.id(),
                buffer: buffer.id(),
                revision: buffer.revision(),
                range: cursor..cursor,
            },
            false,
        ))
    }

    pub(crate) fn target_text(&self, target: &SelectionTarget) -> AppResult<String> {
        let buffer = self.selection_buffer(target)?;
        Ok(buffer.slice(target.range.clone()).to_string())
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
        self.apply_note_link_target(target, note, None)
    }

    pub(crate) fn apply_note_link_target(
        &mut self,
        target: &SelectionTarget,
        note: &str,
        custom_title: Option<&str>,
    ) -> AppResult<()> {
        if note.is_empty() || note.contains(['[', ']', '\n', '\r']) {
            return Err("Usage: :link <note or note#heading>".into());
        }
        let buffer = self.selection_buffer(target)?;
        let label = match custom_title {
            Some(t) => t.to_string(),
            None => {
                if target.range.is_empty() {
                    String::new()
                } else {
                    buffer.slice(target.range.clone()).to_string()
                }
            }
        };
        let label_opt = (!label.is_empty() && label != note).then_some(label.as_str());
        let transaction = LinkSelection::wikilink(target.range.clone(), note, label_opt);
        self.apply_selection_edit(target.clone(), transaction)
    }

    pub(crate) fn apply_url_link_target(
        &mut self,
        target: &SelectionTarget,
        url: &str,
        custom_title: Option<&str>,
    ) -> AppResult<()> {
        if url.is_empty() {
            return Err("URL cannot be empty".into());
        }
        let buffer = self.selection_buffer(target)?;
        let label = match custom_title {
            Some(t) => t.to_string(),
            None => {
                if target.range.is_empty() {
                    url.to_string()
                } else {
                    buffer.slice(target.range.clone()).to_string()
                }
            }
        };
        let transaction = LinkSelection::markdown_link(target.range.clone(), url, &label);
        self.apply_selection_edit(target.clone(), transaction)
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
        self.shell.set_mode(truff_core::vim::Mode::Normal);
        self.handle_shell_action(crate::shell::ShellAction::EditorEdit);
        Ok(())
    }
}
