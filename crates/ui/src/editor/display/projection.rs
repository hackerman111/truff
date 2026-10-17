use std::ops::Range;

use super::DisplayMap;
use super::markers::ranges_touch;
use super::types::{DisplayChunk, ProjectionMode};

impl DisplayMap {
    /// Rebuilds projection visibility from cached source markers.
    pub(super) fn rebuild(&mut self) {
        if self.mode == ProjectionMode::Source {
            if self.visual_text.is_some() || self.chunks.len() != 1 {
                self.visual_revision = self.visual_revision.wrapping_add(1);
            }
            self.visual_text = None;
            self.chunks.clear();
            self.chunks.push(DisplayChunk {
                source_start: 0,
                source_end: self.source_text.len() as u32,
                visual_start: 0,
                visual_len: self.source_text.len() as u32,
                enclosing_start: u32::MAX,
                enclosing_end: u32::MAX,
                is_marker: false,
                hidden: false,
            });
            return;
        }
        let cursor_offset = self.cursor.get();
        let selection_range = self
            .selection
            .map(|(a, h)| a.get().min(h.get())..a.get().max(h.get()));
        let mut chunks = std::mem::take(&mut self.chunks);
        chunks.clear();
        let mut last_idx = 0;
        let mut running_visual_pos = 0u32;
        for (marker_range, span_range) in &self.markers {
            // Some Markdown constructs share delimiters. One source byte appears once.
            if marker_range.start < last_idx {
                continue;
            }
            if marker_range.start > last_idx {
                let v_len = (marker_range.start - last_idx) as u32;
                chunks.push(DisplayChunk {
                    source_start: last_idx as u32,
                    source_end: marker_range.start as u32,
                    visual_start: running_visual_pos,
                    visual_len: v_len,
                    enclosing_start: u32::MAX,
                    enclosing_end: u32::MAX,
                    is_marker: false,
                    hidden: false,
                });
                running_visual_pos += v_len;
            }
            let caret_touches =
                |range: &Range<usize>| cursor_offset >= range.start && cursor_offset <= range.end;
            let revealed = if self.mode == ProjectionMode::Reading {
                false
            } else {
                caret_touches(marker_range)
                    || span_range.as_ref().is_some_and(caret_touches)
                    || selection_range.as_ref().is_some_and(|selection| {
                        ranges_touch(selection, marker_range)
                            || span_range
                                .as_ref()
                                .is_some_and(|span| ranges_touch(selection, span))
                    })
            };
            let hidden = !revealed
                && !self.list_at_marker(marker_range).is_some_and(|list| {
                    matches!(
                        list.kind,
                        truff_core::markdown::presentation::ListKind::Ordered
                    )
                });
            let v_len = if hidden {
                self.replacement(marker_range).map_or(0, str::len) as u32
            } else {
                marker_range.len() as u32
            };
            chunks.push(DisplayChunk {
                source_start: marker_range.start as u32,
                source_end: marker_range.end as u32,
                visual_start: running_visual_pos,
                visual_len: v_len,
                enclosing_start: span_range.as_ref().map_or(u32::MAX, |r| r.start as u32),
                enclosing_end: span_range.as_ref().map_or(u32::MAX, |r| r.end as u32),
                is_marker: true,
                hidden,
            });
            running_visual_pos += v_len;
            last_idx = marker_range.end;
        }
        if last_idx < self.source_text.len() {
            let v_len = (self.source_text.len() - last_idx) as u32;
            chunks.push(DisplayChunk {
                source_start: last_idx as u32,
                source_end: self.source_text.len() as u32,
                visual_start: running_visual_pos,
                visual_len: v_len,
                enclosing_start: u32::MAX,
                enclosing_end: u32::MAX,
                is_marker: false,
                hidden: false,
            });
        }

        let new_visual_text = if chunks.iter().any(|chunk| chunk.hidden) {
            let len = chunks.iter().map(|chunk| chunk.visual_len as usize).sum();
            let mut visual = String::with_capacity(len);
            for chunk in &chunks {
                if !chunk.hidden {
                    visual.push_str(&self.source_text[chunk.source_range()]);
                } else if let Some(replacement) = self.replacement(&chunk.source_range()) {
                    visual.push_str(replacement);
                }
            }
            Some(visual.into_boxed_str())
        } else {
            None
        };

        if self.visual_text != new_visual_text {
            self.visual_text = new_visual_text;
            self.visual_revision = self.visual_revision.wrapping_add(1);
        }

        self.chunks = chunks;
    }
}
