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
    /// Reading mode: hide syntax markers unconditionally regardless of cursor.
    Reading,
}

/// Supported inline formatting styles in Markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineStyle {
    Bold,
    Italic,
    Strikethrough,
    Code,
    Link,
    TextColor([u8; 4]),
    Highlight([u8; 4]),
}

/// A formatted inline span with source byte range and style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub content: Range<usize>,
    pub style: InlineStyle,
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
