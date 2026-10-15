//! Rope-backed viewport rendering. Retained text and glyphs are bounded by the viewport.
mod invalidation;
mod navigation;
mod paragraph;
mod syntax;
mod viewport;

use paragraph::Paragraph;
use std::collections::BTreeMap;
use std::ops::Range;

use eframe::egui::{Color32, Rect};

use crate::editor::HeightIndex;
use crate::theme::Color;

pub(super) const LINE_HEIGHT: f32 = 24.0;

#[derive(Clone, Copy)]
struct ViewScroll {
    buffer_id: truff_core::text::BufferId,
    revision: u64,
    anchor_byte: usize,
    local_y: f32,
    horizontal: f32,
}

#[derive(Default)]
pub struct EditorWidget {
    cache: BTreeMap<usize, Paragraph>,
    scroll: f32,
    horizontal: f32,
    visible: Range<usize>,
    painted_lines: Range<usize>,
    cursor_rect: Option<Rect>,
    last_cursor: Option<(truff_core::workspace::ViewId, usize)>,
    builds: usize,
    heights: HeightIndex,
    buffer_id: Option<truff_core::text::BufferId>,
    revision: Option<u64>,
    syntax: syntax::EditorSyntax,
    view_scrolls: BTreeMap<truff_core::workspace::ViewId, ViewScroll>,
    last_viewport: Option<eframe::egui::Rect>,
    follow_pending: bool,
    pending_scroll_pos: Option<truff_core::vim::ScrollPosition>,
    pub(in crate::native) input_blocked: bool,
}

pub fn color(value: Color) -> Color32 {
    crate::theme::style::color(value)
}

impl EditorWidget {
    pub const TOP_MARGIN: f32 = 48.0;

    /// Calculates the constrained readable column width and centered horizontal margin.
    pub fn calculate_column_metrics(available_width: f32) -> (f32, f32) {
        crate::theme::GeometryTokens::compact().column_metrics(available_width)
    }

    pub fn scroll_cursor(&mut self, position: truff_core::vim::ScrollPosition) {
        self.pending_scroll_pos = Some(position);
    }

    pub fn is_input_blocked(&self) -> bool {
        self.input_blocked
    }

    pub fn visible_source_range(&self) -> Range<usize> {
        self.visible.clone()
    }
    pub fn visible_source_windows(&self) -> Vec<Range<usize>> {
        self.cache
            .range(self.painted_lines.clone())
            .map(|(_, entry)| entry.range.clone())
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
        self.heights
            .block_at_y((self.scroll - Self::TOP_MARGIN).max(0.0))
            .0
    }
    pub(in crate::native) fn retain_views(&mut self, workspace: &truff_core::workspace::Workspace) {
        self.view_scrolls
            .retain(|id, _| workspace.get_view(*id).is_some());
    }
    pub fn cursor_rect(&self) -> Option<Rect> {
        self.cursor_rect
    }
    pub fn char_rect(&self, line: usize, source_byte: usize) -> Option<Rect> {
        self.cache
            .get(&line)
            .map(|entry| entry.char_rect_at(eframe::egui::Pos2::ZERO, source_byte))
    }
}
