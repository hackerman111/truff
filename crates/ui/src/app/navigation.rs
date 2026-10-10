//! Resolve loaded note destinations and apply them to the active workspace view.
use super::{AppResult, Application};
use truff_core::markdown::facts::MarkdownFacts;
use truff_core::text::BytePos;

pub(super) enum Location {
    Byte(usize),
    Anchor(String),
}

impl Application {
    pub(super) fn open_location(
        &mut self,
        path: &str,
        location: Option<Location>,
    ) -> AppResult<()> {
        self.open_note(path)?;
        if let Some(location) = location
            && let Some(view) = self.shell.editor_view()
        {
            self.pending_location = Some((view.buffer_id(), location));
            self.apply_pending_location()?;
        }
        Ok(())
    }

    pub fn open_backlink(&mut self) -> AppResult<()> {
        let link = self
            .backlinks
            .get(self.backlink_selection)
            .ok_or("No backlink selected")?;
        let path = link.source_path.clone();
        let start = link.range.start;
        self.open_location(&path, Some(Location::Byte(start)))
    }

    pub(super) fn apply_pending_location(&mut self) -> AppResult<()> {
        let Some((id, _)) = &self.pending_location else {
            return Ok(());
        };
        if self.shell.editor_buffer().is_none() {
            return Ok(());
        }
        if self
            .shell
            .editor_view()
            .is_none_or(|view| view.buffer_id() != *id)
        {
            self.pending_location = None;
            return Ok(());
        }
        let (_, location) = self.pending_location.take().expect("checked destination");
        let text = self.shell.editor_text();
        let offset = match location {
            Location::Byte(offset) => offset,
            Location::Anchor(anchor) => {
                let facts = MarkdownFacts::extract(&text)?;
                facts
                    .headings
                    .iter()
                    .find(|heading| heading.anchor == anchor || heading.text == anchor)
                    .map(|heading| heading.range.start)
                    .or_else(|| {
                        facts
                            .blocks
                            .iter()
                            .find(|block| block.id == anchor.trim_start_matches('^'))
                            .map(|block| block.range.start)
                    })
                    .ok_or("Link anchor is missing")?
            }
        };
        let pos = BytePos::new(crate::editor::snap_to_grapheme_boundary(&text, offset));
        if let Some(view_id) = self.shell.workspace().active_view()
            && let Some(view) = self.shell.workspace_mut().get_view_mut(view_id)
        {
            view.set_cursor_with_jump(pos, pos);
        }
        Ok(())
    }
}
