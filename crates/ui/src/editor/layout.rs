//! Text layout, HeightIndex, Parley paragraph virtualization, and hit-testing (§8.1).
//!
//! Guarantees:
//! - Virtualization: Parley layout is built ONLY for visible blocks (viewport ± 1 screen), caching them.
//! - Stable scroll anchor: Height adjustments above viewport rebase scroll offset without content jumping.
//! - Cursor movement does NOT invalidate or rebuild layouts of unchanged paragraphs.
//! - Grapheme cluster preservation on all hit-testing.

use std::collections::HashMap;
use std::ops::Range;

use parley::layout::{Cluster, ClusterSide};
use parley::style::StyleProperty;
use parley::{FontContext, Layout, LayoutContext};
use unicode_segmentation::UnicodeSegmentation;

/// Compact information about a document paragraph/block stored in `HeightIndex`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockInfo {
    /// Source byte range of this paragraph/block.
    pub source_range: Range<usize>,
    /// Estimated or exact measured height in logical pixels.
    pub height: f32,
    /// Whether this block has been measured via Parley layout.
    pub is_measured: bool,
    /// Number of lines in this block.
    pub line_count: usize,
    /// Revision counter of the block content.
    pub revision: u64,
}

/// Compact index of block heights for virtualized layout and smooth scrolling (§8.1).
#[derive(Debug, Clone)]
pub struct HeightIndex {
    blocks: Vec<BlockInfo>,
    default_line_height: f32,
}

impl Default for HeightIndex {
    fn default() -> Self {
        Self::new(24.0)
    }
}

impl HeightIndex {
    /// Creates a new `HeightIndex` with a default estimated line height.
    pub fn new(default_line_height: f32) -> Self {
        Self {
            blocks: Vec::new(),
            default_line_height,
        }
    }

    /// Initializes block entries from text, assigning initial estimated heights.
    pub fn init_from_text(&mut self, text: &str) {
        self.blocks.clear();
        let bytes = text.as_bytes();
        let mut start = 0;

        for line in text.lines() {
            let line_len = line.len();
            let end = start + line_len;
            let line_count = 1;
            let estimated_height = self.default_line_height * (line_count as f32);

            self.blocks.push(BlockInfo {
                source_range: start..end,
                height: estimated_height,
                is_measured: false,
                line_count,
                revision: 0,
            });

            start = end;
            if start < bytes.len() && bytes[start] == b'\r' {
                start += 1;
            }
            if start < bytes.len() && bytes[start] == b'\n' {
                start += 1;
            }
        }

        if self.blocks.is_empty() {
            self.blocks.push(BlockInfo {
                source_range: 0..0,
                height: self.default_line_height,
                is_measured: false,
                line_count: 1,
                revision: 0,
            });
        }
    }

    /// Number of blocks in the index.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Access all blocks.
    pub fn blocks(&self) -> &[BlockInfo] {
        &self.blocks
    }

    /// Mutable access to blocks.
    pub fn blocks_mut(&mut self) -> &mut [BlockInfo] {
        &mut self.blocks
    }

    /// Total vertical height of all blocks.
    pub fn total_height(&self) -> f32 {
        self.blocks.iter().map(|b| b.height).sum()
    }

    /// Returns the top y coordinate of `block_index`.
    pub fn y_for_block(&self, block_index: usize) -> f32 {
        let count = block_index.min(self.blocks.len());
        self.blocks[..count].iter().map(|b| b.height).sum()
    }

    /// Finds which block is located at coordinate `y`, returning `(block_index, local_y_within_block)`.
    pub fn block_at_y(&self, y: f32) -> (usize, f32) {
        if self.blocks.is_empty() || y <= 0.0 {
            return (0, 0.0);
        }

        let mut acc = 0.0f32;
        for (i, block) in self.blocks.iter().enumerate() {
            let next = acc + block.height;
            if y < next || i + 1 == self.blocks.len() {
                return (i, (y - acc).max(0.0));
            }
            acc = next;
        }

        (self.blocks.len().saturating_sub(1), 0.0)
    }

    /// Updates the measured height of a block, re-anchoring the scroll position if needed.
    ///
    /// GUARANTEE: If a height changes for a block ABOVE `anchor`, `anchor.scroll_offset_y`
    /// is adjusted by the delta, completely preventing visual scroll jumping (`stable_scroll_anchor`).
    pub fn update_block_height(
        &mut self,
        block_index: usize,
        new_height: f32,
        anchor: Option<&mut ScrollAnchor>,
    ) {
        if block_index >= self.blocks.len() {
            return;
        }

        let old_height = self.blocks[block_index].height;
        let delta = new_height - old_height;
        self.blocks[block_index].height = new_height;
        self.blocks[block_index].is_measured = true;

        if let Some(anc) = anchor
            && block_index < anc.block_index
        {
            // Height changed above viewport: adjust scroll offset so content doesn't jump
            anc.scroll_offset_y += delta;
        }
    }
}

/// Stable scroll anchor holding the top visible block and local offset within it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollAnchor {
    /// Index of the anchored block at top of viewport.
    pub block_index: usize,
    /// Local y offset inside the anchored block.
    pub local_y: f32,
    /// Absolute scroll offset y.
    pub scroll_offset_y: f32,
}

impl ScrollAnchor {
    /// Constructs a `ScrollAnchor`.
    pub fn new(block_index: usize, local_y: f32, scroll_offset_y: f32) -> Self {
        Self {
            block_index,
            local_y,
            scroll_offset_y,
        }
    }
}

/// Virtualized paragraph layout manager that caches Parley paragraphs
/// ONLY for the visible window (viewport ± 1 screen) (§8.1).
pub struct VirtualizedLayout {
    font_cx: FontContext,
    layout_cx: LayoutContext<[u8; 4]>,
    font_size: f32,
    cache: HashMap<usize, (u64, u32, Layout<[u8; 4]>)>,
    rebuild_count: usize,
}

impl Default for VirtualizedLayout {
    fn default() -> Self {
        Self::new(16.0)
    }
}

impl VirtualizedLayout {
    /// Creates a new virtualized layout engine.
    pub fn new(font_size: f32) -> Self {
        Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            font_size,
            cache: HashMap::new(),
            rebuild_count: 0,
        }
    }

    /// Number of paragraph layouts built from scratch since creation.
    pub fn rebuild_count(&self) -> usize {
        self.rebuild_count
    }

    /// Clears the layout cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Lays out blocks visible in the window `[viewport_top - viewport_height, viewport_top + 2 * viewport_height]`.
    ///
    /// GUARANTEES:
    /// - Only blocks in the visible window are formatted.
    /// - Cached blocks with matching revision and wrap width are reused without rebuild.
    /// - Height updates above the scroll anchor automatically adjust `anchor.scroll_offset_y`.
    pub fn layout_visible_window(
        &mut self,
        text: &str,
        height_index: &mut HeightIndex,
        viewport_top: f32,
        viewport_height: f32,
        wrap_width: Option<f32>,
        mut anchor: Option<&mut ScrollAnchor>,
    ) -> Vec<usize> {
        if height_index.block_count() == 0 {
            return Vec::new();
        }

        let window_top = (viewport_top - viewport_height).max(0.0);
        let window_bottom = viewport_top + viewport_height * 2.0;

        let (start_block, _) = height_index.block_at_y(window_top);
        let (end_block, _) = height_index.block_at_y(window_bottom);

        let wrap_bits = wrap_width.map(|w| w.to_bits()).unwrap_or(0);
        let mut visible_blocks = Vec::with_capacity(end_block - start_block + 1);

        for block_idx in start_block..=end_block {
            if block_idx >= height_index.block_count() {
                break;
            }

            visible_blocks.push(block_idx);
            let block = &height_index.blocks()[block_idx];
            let rev = block.revision;

            // Check cache hit
            if let Some((cached_rev, cached_wrap, _)) = self.cache.get(&block_idx)
                && *cached_rev == rev
                && *cached_wrap == wrap_bits
            {
                continue; // CACHE HIT: reuse existing layout without rebuilding!
            }

            // CACHE MISS: Build Parley paragraph for this block
            let block_text =
                if block.source_range.start <= text.len() && block.source_range.end <= text.len() {
                    &text[block.source_range.clone()]
                } else {
                    ""
                };

            let mut builder =
                self.layout_cx
                    .ranged_builder(&mut self.font_cx, block_text, 1.0, true);
            builder.push_default(StyleProperty::FontSize(self.font_size));
            builder.push_default(StyleProperty::Brush([255, 255, 255, 255]));

            let mut layout = builder.build(block_text);
            if let Some(w) = wrap_width
                && w > 0.0
            {
                layout.break_all_lines(Some(w));
            } else {
                layout.break_all_lines(None);
            }

            let measured_height = layout.height().max(self.font_size);
            self.rebuild_count += 1;

            // Update height in HeightIndex with anchor rebasing
            height_index.update_block_height(block_idx, measured_height, anchor.as_deref_mut());

            self.cache.insert(block_idx, (rev, wrap_bits, layout));
        }

        visible_blocks
    }

    /// Access cached layout for a block if present.
    pub fn get_layout(&self, block_idx: usize) -> Option<&Layout<[u8; 4]>> {
        self.cache.get(&block_idx).map(|(_, _, l)| l)
    }
}

/// Text layout wrapper ensuring UTF-8 and grapheme cluster boundary safety.
pub struct EditorLayout {
    font_cx: FontContext,
    layout_cx: LayoutContext<[u8; 4]>,
    layout: Layout<[u8; 4]>,
    font_size: f32,
    last_width: f32,
}

impl Default for EditorLayout {
    fn default() -> Self {
        Self::new(16.0)
    }
}

impl EditorLayout {
    /// Creates a new editor layout manager with the given font size in logical pixels.
    pub fn new(font_size: f32) -> Self {
        Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            layout: Layout::new(),
            font_size,
            last_width: 0.0,
        }
    }

    /// Recomputes text layout for the given string with an optional wrap width.
    pub fn compute(&mut self, text: &str, wrap_width: Option<f32>) {
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(self.font_size));
        builder.push_default(StyleProperty::Brush([255, 255, 255, 255]));

        self.layout = builder.build(text);
        if let Some(width) = wrap_width
            && width > 0.0
        {
            self.layout.break_all_lines(Some(width));
            self.last_width = width;
            return;
        }
        self.layout.break_all_lines(None);
        self.last_width = self.layout.width();
    }

    /// Returns the total height of the laid out text.
    pub fn height(&self) -> f32 {
        self.layout.height()
    }

    /// Returns the width of the laid out text.
    pub fn width(&self) -> f32 {
        self.layout.width()
    }

    /// Access the underlying Parley layout for rendering.
    pub fn parley_layout(&self) -> &Layout<[u8; 4]> {
        &self.layout
    }

    /// Hit-tests a coordinate (x, y) in layout space against the given text.
    ///
    /// GUARANTEE: The returned offset is ALWAYS a valid UTF-8 byte index
    /// and snaps cleanly to a grapheme cluster boundary. It never slices
    /// a multi-byte code point or an extended grapheme cluster.
    pub fn hit_test(&self, text: &str, x: f32, y: f32) -> usize {
        if text.is_empty() {
            return 0;
        }

        let raw_offset = self.raw_hit_test(text, x, y);
        snap_to_grapheme_boundary(text, raw_offset)
    }

    fn raw_hit_test(&self, text: &str, x: f32, y: f32) -> usize {
        if y < 0.0 {
            return 0;
        }
        if y > self.layout.height() {
            return text.len();
        }

        if let Some((cluster, side)) = Cluster::from_point(&self.layout, x, y) {
            let range = cluster.text_range();
            return match side {
                ClusterSide::Left => range.start,
                ClusterSide::Right => range.end,
            };
        }

        let mut best_line = None;
        let mut prev_bottom = 0.0f32;

        for line in self.layout.lines() {
            let metrics = line.metrics();
            let top = prev_bottom;
            let bottom = top + metrics.line_height;
            if y >= top && y <= bottom {
                best_line = Some(line);
                break;
            }
            prev_bottom = bottom;
        }

        if let Some(line) = best_line {
            let range = line.text_range();
            if x <= 0.0 { range.start } else { range.end }
        } else if x <= 0.0 {
            0
        } else {
            text.len()
        }
    }
}

/// Snaps an arbitrary byte offset to the nearest valid UTF-8 grapheme cluster boundary.
///
/// Ensures:
/// 1. Returned index <= text.len()
/// 2. `text.is_char_boundary(index)` is true
/// 3. Never splits a combining character, surrogate, or emoji sequence.
pub fn snap_to_grapheme_boundary(text: &str, byte_offset: usize) -> usize {
    if byte_offset == 0 || text.is_empty() {
        return 0;
    }
    if byte_offset >= text.len() {
        return text.len();
    }

    let mut closest = 0;
    let mut min_distance = usize::MAX;

    for (start, cluster) in text.grapheme_indices(true) {
        let start_dist = start.abs_diff(byte_offset);
        if start_dist < min_distance {
            min_distance = start_dist;
            closest = start;
        }

        let end = start + cluster.len();
        let end_dist = end.abs_diff(byte_offset);
        if end_dist < min_distance {
            min_distance = end_dist;
            closest = end;
        }

        if start > byte_offset && start_dist > min_distance {
            break;
        }
    }

    debug_assert!(
        text.is_char_boundary(closest),
        "Result must be on a valid UTF-8 character boundary"
    );
    closest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_to_grapheme_boundary_cyrillic() {
        let text = "Привет, мир!";
        assert_eq!(snap_to_grapheme_boundary(text, 0), 0);
        assert_eq!(snap_to_grapheme_boundary(text, 1), 0);
        assert_eq!(snap_to_grapheme_boundary(text, 2), 2);
        assert_eq!(snap_to_grapheme_boundary(text, 3), 2);
    }

    #[test]
    fn test_snap_to_grapheme_boundary_emoji() {
        let text = "👨‍👩‍👧‍👦🦀";
        let family_len = "👨‍👩‍👧‍👦".len();
        assert_eq!(snap_to_grapheme_boundary(text, 5), 0);
        assert_eq!(snap_to_grapheme_boundary(text, family_len), family_len);
        assert_eq!(snap_to_grapheme_boundary(text, family_len + 1), family_len);
        assert_eq!(snap_to_grapheme_boundary(text, text.len()), text.len());
    }
}
