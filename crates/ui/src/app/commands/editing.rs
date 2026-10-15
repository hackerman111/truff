use super::super::{AppResult, Application};
use truff_core::vim::Mode;

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
        let start = view.anchor().get().min(view.head().get());
        let mut end = view.anchor().get().max(view.head().get());
        match self.shell.mode() {
            Mode::VisualChar => {
                end = buffer
                    .grapheme_next(truff_core::text::BytePos::new(end))
                    .map(|pos| pos.get())
                    .unwrap_or(end)
            }
            Mode::VisualLine => {
                let first = buffer.byte_of_line(buffer.line_of_byte(start));
                let last = buffer.line_of_byte(end) + 1;
                return Ok(first..if last < buffer.line_len() {
                    buffer.byte_of_line(last)
                } else {
                    buffer.byte_len()
                });
            }
            _ if start == end => return Err("Select text with v or V first".into()),
            _ => {}
        }
        Ok(start..end)
    }
}
