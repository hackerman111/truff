use super::Application;
use std::sync::Arc;
use truff_core::{
    Buffer, BufferId,
    markdown::facts::{DocFacts, MarkdownFacts},
    workspace::BufferChange,
};

pub(super) struct DocumentFacts {
    id: BufferId,
    revision: u64,
    facts: Arc<DocFacts>,
}

impl DocumentFacts {
    pub(crate) fn notify_buffer_change(&mut self, buffer: &Buffer, change: &BufferChange) {
        if self.id != buffer.id() || change.invalidate_suffix {
            return;
        }
        if self.revision + 1 != change.revision {
            return;
        }
        if buffer.byte_len() == 0 {
            return;
        }
        let line_idx = buffer.line_of_byte(change.new_range.start.min(buffer.byte_len().saturating_sub(1)));
        let line_start = buffer.byte_of_line(line_idx);
        let line_end = if line_idx + 1 < buffer.line_len() {
            buffer.byte_of_line(line_idx + 1)
        } else {
            buffer.byte_len()
        };
        let line_slice = buffer.slice(line_start..line_end);

        let touches_heading_syntax = line_slice
            .chars()
            .next()
            .is_some_and(|c| c == '#' || c == '=' || c == '-')
            || self.facts.headings.iter().any(|h| {
                h.range.start <= change.old_range.end && change.old_range.start <= h.range.end
            });

        if !touches_heading_syntax
            && change.new_range.len() + change.old_range.len() < 64
            && !line_slice.chars().any(|c| matches!(c, '[' | ']' | '^' | '\n'))
        {
            let delta = change.new_range.len() as isize - change.old_range.len() as isize;
            if delta != 0 {
                let facts = Arc::make_mut(&mut self.facts);
                for h in &mut facts.headings {
                    if h.range.start >= change.old_range.end {
                        h.range.start = h.range.start.saturating_add_signed(delta);
                        h.range.end = h.range.end.saturating_add_signed(delta);
                    }
                }
                for l in &mut facts.links {
                    if l.range.start >= change.old_range.end {
                        l.range.start = l.range.start.saturating_add_signed(delta);
                        l.range.end = l.range.end.saturating_add_signed(delta);
                    }
                }
                for b in &mut facts.blocks {
                    if b.range.start >= change.old_range.end {
                        b.range.start = b.range.start.saturating_add_signed(delta);
                        b.range.end = b.range.end.saturating_add_signed(delta);
                    }
                }
            }
            self.revision = change.revision;
        }
    }
}

impl Application {
    pub(crate) fn document_facts(&mut self) -> Option<Arc<DocFacts>> {
        let buffer = self.shell.editor_buffer()?;
        if let Some(cache) = &self.document_facts
            && cache.id == buffer.id()
            && cache.revision == buffer.revision()
        {
            return Some(cache.facts.clone());
        }
        let facts = Arc::new(MarkdownFacts::extract(&buffer.as_str()).ok()?);
        self.document_facts = Some(DocumentFacts {
            id: buffer.id(),
            revision: buffer.revision(),
            facts: facts.clone(),
        });
        Some(facts)
    }

    /// Fast update of cached document facts for small edits not affecting headings/links/blocks.
    pub(crate) fn notify_buffer_change_for_facts(&mut self, change: &BufferChange) {
        let Some(cached) = &mut self.document_facts else {
            return;
        };
        let Some(buffer) = self.shell.workspace().buffers().get_buffer(change.buffer_id) else {
            return;
        };
        cached.notify_buffer_change(buffer, change);
    }
}
