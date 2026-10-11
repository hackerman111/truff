use std::ops::Range;

use super::DisplayMap;
use super::markers::ranges_touch;
use super::types::{DisplayChunk, ProjectionMode};

impl DisplayMap {
    /// Rebuilds projection visibility from cached source markers.
    pub(super) fn rebuild(&mut self) {
        if self.mode == ProjectionMode::Source {
            self.visual_text.clear();
            self.visual_text.push_str(&self.source_text);
            self.chunks.clear();
            self.chunks.push(DisplayChunk {
                source_range: 0..self.source_text.len(),
                is_marker: false,
                enclosing_span: None,
                hidden: false,
                visual_len: self.source_text.len(),
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
        for (marker_range, span_range) in &self.markers {
            // Some Markdown constructs share delimiters. One source byte appears once.
            if marker_range.start < last_idx {
                continue;
            }
            if marker_range.start > last_idx {
                chunks.push(DisplayChunk {
                    source_range: last_idx..marker_range.start,
                    is_marker: false,
                    enclosing_span: None,
                    hidden: false,
                    visual_len: marker_range.start - last_idx,
                });
            }
            let caret_touches =
                |range: &Range<usize>| cursor_offset >= range.start && cursor_offset <= range.end;
            let revealed = caret_touches(marker_range)
                || span_range.as_ref().is_some_and(caret_touches)
                || selection_range.as_ref().is_some_and(|selection| {
                    ranges_touch(selection, marker_range)
                        || span_range
                            .as_ref()
                            .is_some_and(|span| ranges_touch(selection, span))
                });
            let hidden = !revealed;
            chunks.push(DisplayChunk {
                source_range: marker_range.clone(),
                is_marker: true,
                enclosing_span: span_range.clone(),
                hidden,
                visual_len: if hidden { 0 } else { marker_range.len() },
            });
            last_idx = marker_range.end;
        }
        if last_idx < self.source_text.len() {
            chunks.push(DisplayChunk {
                source_range: last_idx..self.source_text.len(),
                is_marker: false,
                enclosing_span: None,
                hidden: false,
                visual_len: self.source_text.len() - last_idx,
            });
        }

        // Construct projected visual text from non-hidden chunks
        let mut visual = std::mem::take(&mut self.visual_text);
        visual.clear();
        for chunk in &chunks {
            if !chunk.hidden {
                visual.push_str(&self.source_text[chunk.source_range.clone()]);
            }
        }

        self.chunks = chunks;
        self.visual_text = visual;
    }
}
