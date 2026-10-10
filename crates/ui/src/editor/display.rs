//! Visual projection map (`DisplayMap`), marker hiding/unhiding, and coordinate mapping.
//!
//! Enforces:
//! - Source projection: 1:1 mapping of raw Markdown source to visual layout.
//! - LivePreview projection: Markdown markers (e.g. `**`, `*`, `~~`, `# `, `[[`, `]]`)
//!   are hidden, BUT if the cursor or selection touches the marker or its span,
//!   the markers are unhidden into raw Markdown for live editing.
//! - Stable roundtrip mapping between source `BytePos` and `VisualPos` with `Affinity`.

use std::ops::Range;

use truff_core::text::BytePos;

/// Directional affinity when mapping coordinates at fold or line boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    /// Left / upstream boundary.
    Left,
    /// Right / downstream boundary.
    Right,
}

/// Mode of visual display projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectionMode {
    /// Show raw Markdown source text as-is.
    Source,
    /// Live Preview: hide syntax markers unless touched by caret/selection.
    #[default]
    LivePreview,
}

/// A position in the visual projection space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VisualPos(pub usize);

impl VisualPos {
    pub const ZERO: Self = Self(0);

    pub const fn new(offset: usize) -> Self {
        Self(offset)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for VisualPos {
    fn from(p: usize) -> Self {
        Self(p)
    }
}

impl From<VisualPos> for usize {
    fn from(p: VisualPos) -> Self {
        p.0
    }
}

/// Result of mapping a visual position back to a source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceHit {
    /// The mapped source byte position.
    pub pos: BytePos,
    /// Affinity at the hit point.
    pub affinity: Affinity,
    /// Whether the hit lands inside or on a hidden syntax marker.
    pub in_marker: bool,
}

/// An atomic chunk of text in the display projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayChunk {
    /// Byte range in the source text.
    pub source_range: Range<usize>,
    /// Whether this chunk represents a Markdown syntax marker.
    pub is_marker: bool,
    /// The associated span range that owns this marker (e.g. the bold span `6..15`).
    pub enclosing_span: Option<Range<usize>>,
    /// Whether this chunk is currently hidden in the visual projection.
    pub hidden: bool,
    /// Length of this chunk in visual characters/bytes.
    pub visual_len: usize,
}

/// The visual projection map translating between source buffer and layout coordinates.
pub struct DisplayMap {
    source_text: String,
    mode: ProjectionMode,
    cursor: BytePos,
    selection: Option<(BytePos, BytePos)>,
    chunks: Vec<DisplayChunk>,
    visual_text: String,
}

impl DisplayMap {
    /// Creates a new `DisplayMap` for the given text and projection mode.
    pub fn new(text: &str, mode: ProjectionMode) -> Self {
        let mut map = Self {
            source_text: text.to_string(),
            mode,
            cursor: BytePos::ZERO,
            selection: None,
            chunks: Vec::new(),
            visual_text: String::new(),
        };
        map.rebuild();
        map
    }

    /// Returns the current projection mode.
    pub fn mode(&self) -> ProjectionMode {
        self.mode
    }

    /// Sets the projection mode and rebuilds mapping.
    pub fn set_mode(&mut self, mode: ProjectionMode) {
        if self.mode != mode {
            self.mode = mode;
            self.rebuild();
        }
    }

    /// Updates the source text.
    pub fn set_text(&mut self, text: &str) {
        self.source_text = text.to_string();
        self.rebuild();
    }

    /// Current source text.
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    /// Current projected visual text.
    pub fn visual_text(&self) -> &str {
        &self.visual_text
    }

    /// Updates the active cursor position.
    ///
    /// If in `LivePreview`, placing the cursor on or near a hidden marker reveals it!
    pub fn set_cursor(&mut self, cursor: BytePos) {
        if self.cursor != cursor {
            self.cursor = cursor;
            self.rebuild();
        }
    }

    /// Updates active selection range `(anchor, head)`.
    pub fn set_selection(&mut self, selection: Option<(BytePos, BytePos)>) {
        if self.selection != selection {
            self.selection = selection;
            self.rebuild();
        }
    }

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
                        Affinity::Right => VisualPos(current_visual),
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
                        Affinity::Left => return VisualPos(current_visual),
                        Affinity::Right => {} // continue to see if next chunk is visible
                    }
                } else {
                    // Pos is at end of visible chunk
                    if affinity == Affinity::Left {
                        return VisualPos(current_visual + chunk.visual_len);
                    }
                }
            }

            if !chunk.hidden {
                current_visual += chunk.visual_len;
            }
        }

        VisualPos(current_visual)
    }

    /// Maps a `VisualPos` back to a `SourceHit`.
    pub fn visual_to_source(&self, pos: VisualPos) -> SourceHit {
        let visual_offset = pos.get().min(self.visual_text.len());
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

    /// Rebuilds the chunk partitioning and projected visual text.
    fn rebuild(&mut self) {
        if self.mode == ProjectionMode::Source {
            self.visual_text = self.source_text.clone();
            self.chunks = vec![DisplayChunk {
                source_range: 0..self.source_text.len(),
                is_marker: false,
                enclosing_span: None,
                hidden: false,
                visual_len: self.source_text.len(),
            }];
            return;
        }

        // LivePreview: scan markers and compute visibility
        let markers = scan_markdown_markers(&self.source_text);
        let cursor_offset = self.cursor.get();
        let selection_range = self.selection.map(|(a, h)| {
            let start = a.get().min(h.get());
            let end = a.get().max(h.get());
            start..end
        });

        // Determine which markers are revealed by caret or selection
        let mut revealed_markers = Vec::new();
        for (marker_range, span_range) in &markers {
            let mut reveal = false;

            // 1. Caret touches marker directly (inclusive)
            if cursor_offset >= marker_range.start && cursor_offset <= marker_range.end {
                reveal = true;
            }

            // 2. Caret is inside the enclosing span
            if let Some(span) = span_range
                && cursor_offset >= span.start
                && cursor_offset <= span.end
            {
                reveal = true;
            }

            // 3. Selection touches marker or span
            if let Some(ref sel) = selection_range {
                if ranges_touch(sel, marker_range) {
                    reveal = true;
                }
                if let Some(span) = span_range
                    && ranges_touch(sel, span)
                {
                    reveal = true;
                }
            }

            if reveal {
                revealed_markers.push(marker_range.clone());
            }
        }

        // Partition source text into chunks (alternating markers and text)
        let mut chunks = Vec::new();
        let mut last_idx = 0;

        // Sort markers by start offset
        let mut sorted_markers = markers.clone();
        sorted_markers.sort_by_key(|(m, _)| m.start);

        for (marker_range, span_range) in sorted_markers {
            if marker_range.start > last_idx {
                chunks.push(DisplayChunk {
                    source_range: last_idx..marker_range.start,
                    is_marker: false,
                    enclosing_span: None,
                    hidden: false,
                    visual_len: marker_range.start - last_idx,
                });
            }

            let hidden = !revealed_markers.contains(&marker_range);
            let visual_len = if hidden { 0 } else { marker_range.len() };

            chunks.push(DisplayChunk {
                source_range: marker_range.clone(),
                is_marker: true,
                enclosing_span: span_range,
                hidden,
                visual_len,
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
        let mut visual = String::with_capacity(self.source_text.len());
        for chunk in &chunks {
            if !chunk.hidden {
                visual.push_str(&self.source_text[chunk.source_range.clone()]);
            }
        }

        self.chunks = chunks;
        self.visual_text = visual;
    }
}

fn ranges_touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && a.end >= b.start
}

/// Scans source text for standard Markdown syntax markers:
/// - Bold: `**`
/// - Italic: `*`
/// - Strikethrough: `~~`
/// - Inline code: `` ` ``
/// - Headings: `# `, `## `, `### `
/// - Wiki links: `[[` and `]]`
fn scan_markdown_markers(text: &str) -> Vec<(Range<usize>, Option<Range<usize>>)> {
    let mut markers = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();

    // 1. Headings at start of lines: e.g. `# `, `## `, etc.
    let mut line_start = 0;
    while line_start < len {
        let mut line_end = line_start;
        while line_end < len && bytes[line_end] != b'\n' {
            line_end += 1;
        }

        let line = &text[line_start..line_end];
        let trimmed = line.trim_start();
        let leading_spaces = line.len() - trimmed.len();

        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            if hash_count <= 6 {
                let rest = &trimmed[hash_count..];
                if rest.starts_with(' ') {
                    let marker_start = line_start + leading_spaces;
                    let marker_end = marker_start + hash_count + 1; // hashes + space
                    markers.push((marker_start..marker_end, Some(line_start..line_end)));
                }
            }
        }

        line_start = if line_end < len {
            line_end + 1
        } else {
            line_end
        };
    }

    // 2. Bold `**...**` and Strikethrough `~~...~~`
    scan_delimiter_pairs(text, "**", &mut markers);
    scan_delimiter_pairs(text, "~~", &mut markers);

    // 3. Wiki links `[[...]]`
    let mut i = 0;
    while i + 1 < len {
        if bytes[i] == b'[' && bytes[i + 1] == b'[' {
            let open_start = i;
            let open_end = i + 2;
            if let Some(close_rel) = text[open_end..].find("]]") {
                let close_start = open_end + close_rel;
                let close_end = close_start + 2;
                let span = open_start..close_end;

                markers.push((open_start..open_end, Some(span.clone())));
                markers.push((close_start..close_end, Some(span)));
                i = close_end;
                continue;
            }
        }
        i += 1;
    }

    // 4. Single `*` italic (outside of `**`)
    scan_single_italic(text, &mut markers);

    markers
}

fn scan_delimiter_pairs(
    text: &str,
    delim: &str,
    markers: &mut Vec<(Range<usize>, Option<Range<usize>>)>,
) {
    let mut search_from = 0;
    while let Some(open_rel) = text[search_from..].find(delim) {
        let open_start = search_from + open_rel;
        let open_end = open_start + delim.len();

        if let Some(close_rel) = text[open_end..].find(delim) {
            let close_start = open_end + close_rel;
            let close_end = close_start + delim.len();
            let span = open_start..close_end;

            markers.push((open_start..open_end, Some(span.clone())));
            markers.push((close_start..close_end, Some(span)));
            search_from = close_end;
        } else {
            break;
        }
    }
}

fn scan_single_italic(text: &str, markers: &mut Vec<(Range<usize>, Option<Range<usize>>)>) {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut open_idx = None;

    while i < bytes.len() {
        if bytes[i] == b'*' {
            let prev_is_star = i > 0 && bytes[i - 1] == b'*';
            let next_is_star = i + 1 < bytes.len() && bytes[i + 1] == b'*';

            if !prev_is_star && !next_is_star {
                if let Some(open_start) = open_idx.take() {
                    let close_start = i;
                    let span = open_start..close_start + 1;
                    markers.push((open_start..open_start + 1, Some(span.clone())));
                    markers.push((close_start..close_start + 1, Some(span)));
                } else {
                    open_idx = Some(i);
                }
            }
        }
        i += 1;
    }
}
