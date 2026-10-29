//! Resolve loaded note destinations and apply them to the active workspace view.
use super::{AppResult, Application};
use truff_core::markdown::facts::MarkdownFacts;
use truff_core::text::BytePos;

pub(crate) enum Location {
    Byte(usize),
    Anchor(String),
}

impl Application {
    pub fn set_backlinks(
        &mut self,
        occurrences: Vec<truff_store::index::backlinks::BacklinkOccurrence>,
    ) {
        use truff_core::navigation::{RowId, RowItem};
        self.shell.right_model_mut().set_items(
            occurrences
                .iter()
                .enumerate()
                .map(|(index, link)| RowItem {
                    id: RowId(index as u64 + 1),
                    title: link.source_path.clone(),
                    depth: 0,
                    is_folder: false,
                    is_collapsed: false,
                    data: None,
                })
                .collect(),
        );
        self.backlinks = occurrences;
        self.shell
            .set_right_panel_available(!self.backlinks.is_empty());
    }

    pub(crate) fn open_location(
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

    pub fn jump_to_line(&mut self, line: usize) {
        if let Some(view_id) = self.shell.workspace().active_view()
            && let Some((buffer, view)) = self.shell.workspace_mut().editor_parts_mut(view_id)
        {
            let line_idx = line.saturating_sub(1);
            if line_idx < buffer.line_len() {
                let byte_offset = buffer.byte_of_line(line_idx);
                let byte_pos = truff_core::text::BytePos::new(byte_offset);
                view.set_cursor_with_jump(byte_pos, byte_pos);
            }
        }
    }

    pub fn navigate_to_link(&mut self, target: &str) {
        let trimmed = target.trim();
        if trimmed.starts_with('#') && self.jump_to_heading(trimmed) {
            return;
        }
        if let Err(e) = self.execute_command(&format!("follow {trimmed}")) {
            let (note_part, heading_part) = trimmed
                .split_once('#')
                .map(|(n, h)| (n.trim(), Some(h.trim())))
                .unwrap_or((trimmed, None));
            let path_with_md = if note_part.ends_with(".md") {
                note_part.to_string()
            } else {
                format!("{note_part}.md")
            };
            if let Err(err) = self.open_note(&path_with_md) {
                self.status = format!("{e}; {err}");
            } else if let Some(heading) = heading_part {
                self.jump_to_heading(heading);
            }
        }
    }

    pub fn jump_to_heading(&mut self, anchor: &str) -> bool {
        let raw = anchor.trim_start_matches('#').trim();
        if raw.is_empty() {
            return false;
        }
        let clean = decode_percent(raw);
        let Some(buffer) = self.shell.editor_buffer() else {
            return false;
        };
        let text = buffer.as_str();
        let Ok(facts) = MarkdownFacts::extract(&text) else {
            return false;
        };
        let clean_slug = truff_core::markdown::facts::slugify(&clean);
        let raw_slug = truff_core::markdown::facts::slugify(raw);
        let found_offset = facts
            .headings
            .iter()
            .find(|h| {
                h.anchor == clean
                    || h.text == clean
                    || h.anchor == clean_slug
                    || h.anchor == raw
                    || h.anchor == raw_slug
                    || truff_core::markdown::facts::slugify(&h.text) == clean_slug
                    || h.text.eq_ignore_ascii_case(&clean)
                    || h.anchor.eq_ignore_ascii_case(&clean_slug)
            })
            .map(|h| h.range.start)
            .or_else(|| {
                facts
                    .blocks
                    .iter()
                    .find(|b| b.id == clean.trim_start_matches('^'))
                    .map(|b| b.range.start)
            });
        if let Some(offset) = found_offset {
            let pos = buffer.snap_to_grapheme_boundary(BytePos::new(offset));
            if let Some(view_id) = self.shell.workspace().active_view()
                && let Some(view) = self.shell.workspace_mut().get_view_mut(view_id)
            {
                view.set_cursor_with_jump(pos, pos);
                return true;
            }
        }
        false
    }

    pub fn open_backlink(&mut self) -> AppResult<()> {
        let index = self
            .shell
            .right_model()
            .selected_row()
            .map(|row| row.id.get().saturating_sub(1) as usize)
            .ok_or("No backlink selected")?;
        let link = self.backlinks.get(index).ok_or("No backlink selected")?;
        let path = link.source_path.clone();
        let start = link.range.start;
        self.open_location(&path, Some(Location::Byte(start)))
    }

    pub(super) fn apply_pending_location(&mut self) -> AppResult<()> {
        let Some((id, _)) = &self.pending_location else {
            return Ok(());
        };
        let Some(buffer) = self.shell.editor_buffer() else {
            return Ok(());
        };
        if self
            .shell
            .editor_view()
            .is_none_or(|view| view.buffer_id() != *id)
        {
            self.pending_location = None;
            return Ok(());
        }
        let (_, location) = self.pending_location.take().expect("checked destination");
        let offset = match location {
            Location::Byte(offset) => offset,
            Location::Anchor(anchor) => {
                let text = buffer.as_str();
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
        let pos = buffer.snap_to_grapheme_boundary(BytePos::new(offset));
        if let Some(view_id) = self.shell.workspace().active_view()
            && let Some(view) = self.shell.workspace_mut().get_view_mut(view_id)
        {
            view.set_cursor_with_jump(pos, pos);
        }
        Ok(())
    }
}

fn decode_percent(s: &str) -> String {
    if !s.contains('%') {
        return s.to_string();
    }
    let mut bytes = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16)
        {
            bytes.push(byte);
            i += 3;
            continue;
        }
        bytes.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
