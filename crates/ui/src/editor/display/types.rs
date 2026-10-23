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
    Tag(String),
    TextColor([u8; 4]),
    Highlight([u8; 4]),
    Math,
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
    pub source_start: u32,
    pub source_end: u32,
    pub visual_start: u32,
    pub visual_len: u32,
    pub enclosing_start: u32,
    pub enclosing_end: u32,
    pub is_marker: bool,
    pub hidden: bool,
}

impl DisplayChunk {
    #[inline(always)]
    pub fn source_range(&self) -> Range<usize> {
        (self.source_start as usize)..(self.source_end as usize)
    }

    #[inline(always)]
    pub fn enclosing_span(&self) -> Option<Range<usize>> {
        if self.enclosing_start == u32::MAX {
            None
        } else {
            Some((self.enclosing_start as usize)..(self.enclosing_end as usize))
        }
    }
}
