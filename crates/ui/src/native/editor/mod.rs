//! Rope-backed viewport rendering. Retained text and glyphs are bounded by the viewport.
mod headings;
mod hit_test;
mod invalidation;
mod navigation;
mod paragraph;
mod preview;
mod syntax;
mod viewport;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use egui::{Color32, Rect};

use crate::editor::HeightIndex;
use crate::theme::Color;
use paragraph::Paragraph;
pub use paragraph::TASK_PADDING_X;
pub use preview::render_markdown_preview;

pub(super) const LINE_HEIGHT: f32 = 24.0;

#[derive(Clone, Copy)]
struct ViewScroll {
    buffer_id: truff_core::text::BufferId,
    revision: u64,
    anchor_byte: usize,
    local_y: f32,
    horizontal: f32,
}

#[derive(Clone, Copy, Default)]
pub(super) struct CursorBlink {
    last_activity_time: f64,
    last_mode: Option<truff_core::vim::Mode>,
    last_revision: Option<u64>,
}

impl CursorBlink {
    const HALF_PERIOD_SECS: f64 = 0.53;

    pub(super) fn resolve(
        &mut self,
        ui: &egui::Ui,
        mode: truff_core::vim::Mode,
        cursor_changed: bool,
        revision: u64,
        input_blocked: bool,
    ) -> bool {
        let (now, focused, input_activity) = ui.input(|i| {
            let active = i.events.iter().any(|ev| {
                matches!(
                    ev,
                    egui::Event::Key { pressed: true, .. }
                        | egui::Event::Text(_)
                        | egui::Event::Ime(_)
                        | egui::Event::Paste(_)
                        | egui::Event::PointerButton { pressed: true, .. }
                )
            });
            (i.time, i.focused, active)
        });

        if cursor_changed
            || input_activity
            || self.last_mode != Some(mode)
            || self.last_revision != Some(revision)
            || now < self.last_activity_time
        {
            self.last_activity_time = now;
            self.last_mode = Some(mode);
            self.last_revision = Some(revision);
        }

        if mode != truff_core::vim::Mode::Insert || !focused || input_blocked {
            return true;
        }

        let elapsed = (now - self.last_activity_time).max(0.0);
        let phase = (elapsed / Self::HALF_PERIOD_SECS).floor() as u64;
        let rem = Self::HALF_PERIOD_SECS - (elapsed % Self::HALF_PERIOD_SECS);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(rem.max(0.016)));
        phase.is_multiple_of(2)
    }
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
    blink: CursorBlink,
    builds: usize,
    heights: HeightIndex,
    buffer_id: Option<truff_core::text::BufferId>,
    revision: Option<u64>,
    syntax: syntax::EditorSyntax,
    view_scrolls: BTreeMap<truff_core::workspace::ViewId, ViewScroll>,
    last_viewport: Option<egui::Rect>,
    follow_pending: bool,
    pending_scroll_pos: Option<truff_core::vim::ScrollPosition>,
    active_table: Option<(Range<usize>, Arc<Vec<usize>>)>,
    buffer_folds: BTreeMap<truff_core::text::BufferId, BTreeSet<usize>>,
    cached_headings: Option<headings::HeadingCache>,
    pub(in crate::native) active_hidden_ranges: Vec<Range<usize>>,
    pub(in crate::native) input_blocked: bool,
}

pub fn color(value: Color) -> Color32 {
    crate::theme::style::color(value)
}

impl EditorWidget {
    pub const TOP_MARGIN: f32 = 84.0;

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
            .map(|entry| entry.char_rect_at(egui::Pos2::ZERO, source_byte))
    }
}
