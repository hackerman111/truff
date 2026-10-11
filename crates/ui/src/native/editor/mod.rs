//! Rope-backed viewport rendering. Retained text and glyphs are bounded by the viewport.
mod navigation;
mod paragraph;
mod viewport;

use paragraph::Paragraph;
use std::collections::BTreeMap;
use std::ops::Range;

use eframe::egui::{Color32, Rect};

use crate::editor::HeightIndex;
use crate::theme::Color;

pub(super) const LINE_HEIGHT: f32 = 24.0;

#[derive(Default)]
pub struct EditorWidget {
    cache: BTreeMap<usize, Paragraph>,
    scroll: f32,
    horizontal: f32,
    visible: Range<usize>,
    cursor_rect: Option<Rect>,
    last_cursor: Option<(truff_core::workspace::ViewId, usize)>,
    builds: usize,
    heights: HeightIndex,
    buffer_id: Option<truff_core::text::BufferId>,
}

pub fn color(value: Color) -> Color32 {
    let [r, g, b, a] = value.to_rgba_u8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

impl EditorWidget {
    pub const TOP_MARGIN: f32 = 48.0;

    /// Calculates the constrained readable column width and centered horizontal margin.
    pub fn calculate_column_metrics(available_width: f32) -> (f32, f32) {
        let content_width = (available_width - 48.0).clamp(320.0, 720.0);
        let margin_x = ((available_width - content_width) / 2.0).max(24.0);
        (content_width, margin_x)
    }

    pub fn visible_source_range(&self) -> Range<usize> {
        self.visible.clone()
    }
    pub fn visible_source_windows(&self) -> Vec<Range<usize>> {
        self.cache
            .values()
            .map(|entry| entry.range.clone())
            .collect()
    }
    pub fn cached_paragraph_count(&self) -> usize {
        self.cache.len()
    }
    pub fn layout_rebuild_count(&self) -> usize {
        self.builds
    }
    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }
    pub fn scroll_by(&mut self, amount: f32) {
        self.scroll = (self.scroll + amount).max(0.0);
    }
    pub fn top_line(&self) -> usize {
        self.heights.block_at_y(self.scroll).0
    }
    pub fn cursor_rect(&self) -> Option<Rect> {
        self.cursor_rect
    }
}
