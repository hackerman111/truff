use truff_core::text::BytePos;

use super::DisplayMap;
use super::types::{Affinity, ProjectionMode, SourceHit, VisualPos};

impl DisplayMap {
    /// Maps a source `BytePos` into a `VisualPos` with the specified affinity.
    pub fn source_to_visual(&self, pos: BytePos, affinity: Affinity) -> VisualPos {
        let source_offset = pos.get().min(self.source_text.len());
        if self.mode == ProjectionMode::Source
            || (self.chunks.len() == 1 && !self.chunks[0].hidden)
        {
            return VisualPos(source_offset);
        }
        if self.chunks.is_empty() {
            return VisualPos(0);
        }

        let mut current_visual = 0;
        for chunk in &self.chunks {
            let chunk_source_start = chunk.source_start as usize;
            let chunk_source_end = chunk.source_end as usize;
            let chunk_visual_len = chunk.visual_len as usize;
            let chunk_visual_start = chunk.visual_start as usize;

            if source_offset < chunk_source_start {
                break;
            }

            if source_offset >= chunk_source_start && source_offset < chunk_source_end {
                if chunk.hidden {
                    return match affinity {
                        Affinity::Left => VisualPos(chunk_visual_start),
                        Affinity::Right => VisualPos(chunk_visual_start + chunk_visual_len),
                    };
                } else {
                    let offset_in_chunk = source_offset - chunk_source_start;
                    return VisualPos(chunk_visual_start + offset_in_chunk);
                }
            }

            if source_offset == chunk_source_end {
                if chunk.hidden {
                    // Pos is at end of hidden chunk
                    match affinity {
                        Affinity::Left => return VisualPos(chunk_visual_start + chunk_visual_len),
                        Affinity::Right => {} // continue to see if next chunk is visible
                    }
                } else {
                    // Pos is at end of visible chunk
                    if affinity == Affinity::Left {
                        return VisualPos(chunk_visual_start + chunk_visual_len);
                    }
                }
            }

            current_visual = chunk_visual_start + chunk_visual_len;
        }

        VisualPos(current_visual)
    }

    /// Maps a `VisualPos` back to a `SourceHit`.
    pub fn visual_to_source(&self, pos: VisualPos) -> SourceHit {
        let visual_offset = pos.get().min(self.visual_text().len());
        if self.mode == ProjectionMode::Source
            || (self.chunks.len() == 1 && !self.chunks[0].hidden)
        {
            return SourceHit {
                pos: BytePos(visual_offset),
                affinity: Affinity::Left,
                in_marker: false,
            };
        }
        if self.chunks.is_empty() {
            return SourceHit {
                pos: BytePos(self.source_text.len()),
                affinity: Affinity::Right,
                in_marker: false,
            };
        }

        let idx = self.chunks.partition_point(|chunk| {
            (chunk.visual_start + chunk.visual_len) as usize <= visual_offset
        });

        if idx < self.chunks.len() {
            let chunk = &self.chunks[idx];
            let chunk_v_start = chunk.visual_start as usize;
            let chunk_v_len = chunk.visual_len as usize;
            let chunk_v_end = chunk_v_start + chunk_v_len;

            if chunk.hidden {
                if visual_offset >= chunk_v_start && visual_offset < chunk_v_end {
                    return SourceHit {
                        pos: BytePos(chunk.source_start as usize),
                        affinity: Affinity::Left,
                        in_marker: true,
                    };
                }
            } else if visual_offset >= chunk_v_start && visual_offset < chunk_v_end {
                let offset_in_chunk = visual_offset - chunk_v_start;
                return SourceHit {
                    pos: BytePos(chunk.source_start as usize + offset_in_chunk),
                    affinity: Affinity::Left,
                    in_marker: chunk.is_marker,
                };
            }
        }

        // At end of text
        SourceHit {
            pos: BytePos(self.source_text.len()),
            affinity: Affinity::Right,
            in_marker: false,
        }
    }
}
