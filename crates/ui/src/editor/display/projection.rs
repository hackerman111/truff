use std::ops::Range;

use super::DisplayMap;
use super::markers::ranges_touch;
use super::types::{DisplayChunk, ProjectionMode};

impl DisplayMap {
    /// Rebuilds projection visibility from cached source markers.
    pub(super) fn rebuild(&mut self) {
        if self.mode == ProjectionMode::Source {
            self.visual_text = None;
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
            chunks.push(DisplayChunk {
                source_range: marker_range.clone(),
                is_marker: true,
                enclosing_span: span_range.clone(),
                hidden,
                visual_len: if hidden {
                    self.replacement(marker_range).map_or(0, str::len)
                } else {
                    marker_range.len()
                },
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

        self.visual_text = if chunks.iter().any(|chunk| chunk.hidden) {
            let len = chunks.iter().map(|chunk| chunk.visual_len).sum();
            let mut visual = String::with_capacity(len);
            for chunk in &chunks {
                if !chunk.hidden {
                    visual.push_str(&self.source_text[chunk.source_range.clone()]);
                } else if let Some(replacement) = self.replacement(&chunk.source_range) {
                    visual.push_str(replacement);
                }
            }
            Some(visual.into_boxed_str())
        } else {
            None
        };

        self.chunks = chunks;
    }
}
