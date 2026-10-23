//! Measured paragraph heights with logarithmic source-row lookup.
use std::ops::Range;

/// Compact information about a document paragraph/block stored in `HeightIndex`.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockInfo {
    /// Source byte range of this paragraph/block.
    pub source_range: Range<usize>,
    /// Estimated or exact measured height in logical pixels.
    pub height: f32,
    /// Whether this block has been measured via egui layout.
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
    sums: Vec<f64>,
    default_line_height: f32,
    measured_limit: usize,
    total_blocks: usize,
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
            sums: Vec::new(),
            default_line_height,
            measured_limit: 0,
            total_blocks: 0,
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
        if text.ends_with('\n') {
            self.blocks.push(BlockInfo {
                source_range: text.len()..text.len(),
                height: self.default_line_height,
                is_measured: false,
                line_count: 1,
                revision: 0,
            });
        }
        self.total_blocks = self.blocks.len();
        self.measured_limit = 0;
        self.rebuild_sums();
    }

    /// Source offsets remain in the Rope; the native viewport needs only row heights.
    pub fn init_uniform(&mut self, count: usize) {
        self.total_blocks = count.max(1);
        self.blocks.clear();
        self.sums.clear();
        self.measured_limit = 0;
    }

    fn ensure_blocks_len(&mut self, len: usize) {
        if self.blocks.len() < len {
            let default_height = self.default_line_height;
            self.blocks.resize_with(len, || BlockInfo {
                source_range: 0..0,
                height: default_height,
                is_measured: false,
                line_count: 1,
                revision: 0,
            });
        }
    }

    /// Replaces edited source rows, retaining measured heights of unchanged rows.
    pub(crate) fn replace_blocks(&mut self, old: Range<usize>, count: usize) {
        let clamped_end = old.end.min(self.total_blocks);
        let clamped_start = old.start.min(clamped_end);
        let removed = clamped_end - clamped_start;
        self.total_blocks = (self.total_blocks - removed + count).max(1);

        if clamped_start >= self.blocks.len() {
            return;
        }

        if removed == count
            && clamped_end <= self.blocks.len()
            && count <= (self.blocks.len() / 16).max(1)
        {
            for index in clamped_start..clamped_end {
                self.update_block_height(index, self.default_line_height, None);
                self.blocks[index].is_measured = false;
            }
            if self.measured_limit > 0 && !self.blocks[self.measured_limit - 1].is_measured {
                self.measured_limit = self.blocks[..self.measured_limit]
                    .iter()
                    .rposition(|b| b.is_measured)
                    .map_or(0, |p| p + 1);
                self.rebuild_sums();
            }
            return;
        }

        if clamped_end > self.blocks.len() {
            self.blocks.truncate(clamped_start);
            self.measured_limit = self.blocks[..self.measured_limit.min(self.blocks.len())]
                .iter()
                .rposition(|b| b.is_measured)
                .map_or(0, |p| p + 1);
            self.rebuild_sums();
            return;
        }

        let default_height = self.default_line_height;
        let replacement = std::iter::repeat_with(|| BlockInfo {
            source_range: 0..0,
            height: default_height,
            is_measured: false,
            line_count: 1,
            revision: 0,
        })
        .take(count);
        let old_limit = self.measured_limit;
        self.blocks.splice(clamped_start..clamped_end, replacement);
        if old_limit == 0 || clamped_start >= old_limit {
            return;
        }
        let search_end = (old_limit + count).min(self.blocks.len());
        self.measured_limit = self.blocks[..search_end]
            .iter()
            .rposition(|b| b.is_measured)
            .map_or(0, |p| p + 1);
        self.rebuild_sums();
    }

    fn rebuild_sums(&mut self) {
        let count = self.measured_limit.min(self.blocks.len());
        if count == 0 {
            self.sums.clear();
            return;
        }
        self.sums.clear();
        self.sums.resize(count + 1, 0.0);
        for index in 1..=count {
            self.sums[index] += f64::from(self.blocks[index - 1].height);
            let parent = index + (index & index.wrapping_neg());
            if parent <= count {
                self.sums[parent] += self.sums[index];
            }
        }
    }

    #[inline]
    fn fenwick_prefix(&self, count: usize) -> f32 {
        let mut index = count.min(self.sums.len().saturating_sub(1));
        let mut sum = 0.0;
        while index > 0 {
            sum += self.sums[index];
            index &= index - 1;
        }
        sum as f32
    }

    /// Number of blocks in the index.
    pub fn block_count(&self) -> usize {
        self.total_blocks
    }

    /// Access materialized blocks.
    pub fn blocks(&self) -> &[BlockInfo] {
        &self.blocks
    }

    /// Mutable access to blocks.
    pub fn set_block_revision(&mut self, index: usize, revision: u64) {
        if let Some(block) = self.blocks.get_mut(index) {
            block.revision = revision;
        }
    }

    /// Total vertical height of all blocks.
    pub fn total_height(&self) -> f32 {
        self.y_for_block(self.total_blocks)
    }

    /// Returns the top y coordinate of `block_index`.
    pub fn y_for_block(&self, block_index: usize) -> f32 {
        let limit = self.measured_limit.min(self.blocks.len());
        let clamped = block_index.min(self.total_blocks);
        if clamped <= limit {
            self.fenwick_prefix(clamped)
        } else {
            let base = self.fenwick_prefix(limit);
            base + ((clamped - limit) as f32) * self.default_line_height
        }
    }

    /// Finds which block is located at coordinate `y`, returning `(block_index, local_y_within_block)`.
    pub fn block_at_y(&self, y: f32) -> (usize, f32) {
        if self.total_blocks == 0 || y <= 0.0 {
            return (0, 0.0);
        }

        let limit = self.measured_limit.min(self.blocks.len());
        let measured_y = self.fenwick_prefix(limit);
        if limit == 0 || y >= measured_y {
            let extra = if self.default_line_height > 0.0 {
                ((y - measured_y).max(0.0) / self.default_line_height).floor() as usize
            } else {
                0
            };
            let index = (limit + extra).min(self.total_blocks - 1);
            let block_y = self.y_for_block(index);
            return (index, (y - block_y).max(0.0));
        }

        let mut index = 0;
        let mut sum = 0.0;
        let mut step = limit.next_power_of_two();
        while step > 0 {
            let next = index + step;
            if next < self.sums.len() && sum + self.sums[next] <= f64::from(y) {
                sum += self.sums[next];
                index = next;
            }
            step >>= 1;
        }
        let index = index.min(self.total_blocks - 1);
        (index, (y - self.y_for_block(index)).max(0.0))
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
        if block_index >= self.total_blocks {
            return;
        }

        if !new_height.is_finite() || new_height <= 0.0 {
            return;
        }

        if block_index >= self.blocks.len()
            && (new_height - self.default_line_height).abs() <= f32::EPSILON
        {
            return;
        }

        self.ensure_blocks_len(block_index + 1);
        let old_height = self.blocks[block_index].height;
        let delta = new_height - old_height;
        self.blocks[block_index].height = new_height;
        self.blocks[block_index].is_measured = true;

        if block_index >= self.measured_limit {
            self.measured_limit = block_index + 1;
            self.rebuild_sums();
        } else {
            let mut index = block_index + 1;
            while index < self.sums.len() {
                self.sums[index] += f64::from(delta);
                index += index & index.wrapping_neg();
            }
        }

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
