use super::super::{AppResult, Application};

impl Application {
    pub(super) fn color_selection(&mut self, argument: &str) -> AppResult<()> {
        let target = self.capture_selection()?;
        self.color_target(target, argument)?;
        self.shell.feed_key(truff_core::vim::Key::Esc);
        self.status = "Color applied; u to undo".into();
        Ok(())
    }
    pub fn link_selection(&mut self, argument: &str) -> AppResult<()> {
        let target = self.capture_selection()?;
        self.link_target(&target, argument)?;
        self.shell.feed_key(truff_core::vim::Key::Esc);
        self.status = "Link created; :follow to open".into();
        Ok(())
    }
    pub(crate) fn selection_range(&self) -> AppResult<std::ops::Range<usize>> {
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self.shell.editor_buffer().ok_or("Note is still loading")?;
        buffer
            .selection_range(view.anchor(), view.head(), self.shell.mode())
            .ok_or_else(|| "Select text with v or V first".into())
    }

    pub(super) fn format_table_command(&mut self) -> AppResult<()> {
        let (view_id, cursor) = {
            let view = self.shell.editor_view().ok_or("Open a note first")?;
            (view.id(), view.head().get())
        };
        let buffer = self.shell.editor_buffer().ok_or("Note is still loading")?;
        let tx = truff_core::markdown::table::format_table_transaction(buffer, cursor)
            .ok_or("No markdown table found under cursor")?;
        self.dispatch(truff_core::commands::Action::ApplyEdit {
            view_id,
            transaction: tx,
        })?;
        self.status = "Table formatted".into();
        Ok(())
    }
}
