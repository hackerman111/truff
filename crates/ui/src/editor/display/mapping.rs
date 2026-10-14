use truff_core::text::BytePos;

use super::DisplayMap;
use super::types::{Affinity, ProjectionMode, SourceHit, VisualPos};

impl DisplayMap {
    /// Maps a source `BytePos` into a `VisualPos` with the specified affinity.
    pub fn source_to_visual(&self, pos: BytePos, affinity: Affinity) -> VisualPos {
        let source_offset = pos.get().min(self.source_text.len());
        if self.mode == ProjectionMode::Source {
            return VisualPos(source_offset);
        }

        let mut current_visual = 0;
        for chunk in &self.chunks {
            if source_offset < chunk.source_range.start {
                break;
            }

            if chunk.source_range.contains(&source_offset) {
                if chunk.hidden {
                    return match affinity {
                        Affinity::Left => VisualPos(current_visual),
                        Affinity::Right => VisualPos(current_visual + chunk.visual_len),
                    };
                } else {
                    let offset_in_chunk = source_offset - chunk.source_range.start;
                    return VisualPos(current_visual + offset_in_chunk);
                }
            }

            if source_offset == chunk.source_range.end {
                if chunk.hidden {
                    // Pos is at end of hidden chunk
                    match affinity {
                        Affinity::Left => return VisualPos(current_visual + chunk.visual_len),
                        Affinity::Right => {} // continue to see if next chunk is visible
                    }
                } else {
                    // Pos is at end of visible chunk
                    if affinity == Affinity::Left {
                        return VisualPos(current_visual + chunk.visual_len);
                    }
                }
            }

            current_visual += chunk.visual_len;
        }

        VisualPos(current_visual)
    }

    /// Maps a `VisualPos` back to a `SourceHit`.
    pub fn visual_to_source(&self, pos: VisualPos) -> SourceHit {
        let visual_offset = pos.get().min(self.visual_text().len());
        if self.mode == ProjectionMode::Source {
            return SourceHit {
                pos: BytePos(visual_offset),
                affinity: Affinity::Left,
                in_marker: false,
            };
        }

        let mut current_visual = 0;
        for chunk in &self.chunks {
            if chunk.hidden {
                if visual_offset >= current_visual
                    && visual_offset < current_visual + chunk.visual_len
                {
                    return SourceHit {
                        pos: BytePos(chunk.source_range.start),
                        affinity: Affinity::Left,
                        in_marker: true,
                    };
                }
                current_visual += chunk.visual_len;
                continue;
            }

            let chunk_visual_end = current_visual + chunk.visual_len;
            if visual_offset >= current_visual && visual_offset < chunk_visual_end {
                let offset_in_chunk = visual_offset - current_visual;
                return SourceHit {
                    pos: BytePos(chunk.source_range.start + offset_in_chunk),
                    affinity: Affinity::Left,
                    in_marker: chunk.is_marker,
                };
            }

            if visual_offset == chunk_visual_end {
                // At the boundary of this chunk
                current_visual = chunk_visual_end;
                continue;
            }

            current_visual = chunk_visual_end;
        }

        // At end of text
        SourceHit {
            pos: BytePos(self.source_text.len()),
            affinity: Affinity::Right,
            in_marker: false,
        }
    }
}
