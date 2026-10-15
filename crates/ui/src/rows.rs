use std::ops::Range;

mod scroll;
pub(crate) use scroll::{Alignment, RowScroll, selected_rows};

/// Virtualized list layout for high-performance row rendering.
///
/// Ensures memory and widget allocation is bounded strictly by the viewport,
/// regardless of whether there are 10 or 100,000 items in the underlying model.
#[derive(Debug, Clone)]
pub struct VirtualRows {
    row_height: f32,
    viewport_width: f32,
    viewport_height: f32,
    scroll_offset: f32,
    total_count: usize,
}

impl VirtualRows {
    /// Creates a virtual row controller with a uniform row height.
    pub fn new(row_height: f32) -> Self {
        Self {
            row_height: row_height.max(1.0),
            viewport_width: 800.0,
            viewport_height: 600.0,
            scroll_offset: 0.0,
            total_count: 0,
        }
    }

    /// Sets the total count of items in the underlying row model.
    pub fn set_total_count(&mut self, count: usize) {
        self.total_count = count;
    }

    /// Total count of items.
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Sets viewport dimensions (width, height).
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.viewport_width = width.max(1.0);
        self.viewport_height = height.max(1.0);
    }

    /// Sets the vertical scroll offset in pixels.
    pub fn set_scroll_offset(&mut self, offset: f32) {
        let max_scroll = (self.total_height() - self.viewport_height).max(0.0);
        self.scroll_offset = offset.clamp(0.0, max_scroll);
    }

    /// Current vertical scroll offset in pixels.
    pub fn scroll_offset(&self) -> f32 {
        self.scroll_offset
    }

    /// Height of an individual row in pixels.
    pub fn row_height(&self) -> f32 {
        self.row_height
    }

    /// Total height of all rows in pixels.
    pub fn total_height(&self) -> f32 {
        self.total_count as f32 * self.row_height
    }

    /// Number of rows visible simultaneously within the viewport.
    pub fn visible_rows_in_viewport(&self) -> usize {
        (self.viewport_height / self.row_height).ceil() as usize
    }

    /// Range of row indices currently rendered (visible + 1 viewport buffer before and after).
    pub fn rendered_range(&self) -> Range<usize> {
        if self.total_count == 0 {
            return 0..0;
        }

        let visible_count = self.visible_rows_in_viewport();
        let visible_start = (self.scroll_offset / self.row_height).floor() as usize;

        // Overscan buffer: 1 viewport height above and below
        let overscan = visible_count;
        let start = visible_start.saturating_sub(overscan);
        let end = (visible_start + visible_count + overscan).min(self.total_count);

        start..end
    }

    /// Number of currently active widgets / rows.
    ///
    /// Always bounded proportionally to viewport height, never total count.
    pub fn active_widget_count(&self) -> usize {
        let range = self.rendered_range();
        range.end - range.start
    }
}
