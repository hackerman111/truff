//! Rope-backed viewport rendering. Retained text and glyphs are bounded by the viewport.
mod invalidation;
mod navigation;
mod paragraph;
mod syntax;
mod viewport;

use paragraph::Paragraph;
pub use paragraph::TASK_PADDING_X;
use std::collections::BTreeMap;
use std::ops::Range;

use egui::{Color32, Rect};

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
    last_viewport: Option<egui::Rect>,
    follow_pending: bool,
    pending_scroll_pos: Option<truff_core::vim::ScrollPosition>,
    active_table: Option<(Range<usize>, std::sync::Arc<Vec<usize>>)>,
    buffer_folds: BTreeMap<truff_core::text::BufferId, std::collections::BTreeSet<usize>>,
    cached_headings: Option<(
        truff_core::text::BufferId,
        u64,
        Vec<truff_core::markdown::HeadingItem>,
    )>,
    pub(in crate::native) active_hidden_ranges: Vec<Range<usize>>,
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
            .map(|entry| entry.char_rect_at(egui::Pos2::ZERO, source_byte))
    }

    pub fn headings_for_buffer(
        &mut self,
        buffer: &truff_core::Buffer,
    ) -> Vec<truff_core::markdown::HeadingItem> {
        if let Some((id, rev, ref items)) = self.cached_headings
            && id == buffer.id()
            && rev == buffer.revision()
        {
            return items.clone();
        }
        let facts =
            truff_core::markdown::facts::MarkdownFacts::extract_headings_only(&buffer.as_str())
                .unwrap_or_default();
        let items: Vec<truff_core::markdown::HeadingItem> = facts
            .iter()
            .map(|h| {
                truff_core::markdown::HeadingItem::new(buffer.line_of_byte(h.range.start), h.level)
            })
            .collect();
        self.cached_headings = Some((buffer.id(), buffer.revision(), items.clone()));
        items
    }

    pub fn toggle_fold(&mut self, heading_line: usize, buffer: &truff_core::Buffer) {
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        if !folds.remove(&heading_line) {
            folds.insert(heading_line);
        }
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_close(&mut self, heading_line: usize, buffer: &truff_core::Buffer) {
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        folds.insert(heading_line);
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_open(&mut self, heading_line: usize, buffer: &truff_core::Buffer) {
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        folds.remove(&heading_line);
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_close_all(&mut self, buffer: &truff_core::Buffer) {
        let headings = self.headings_for_buffer(buffer);
        let folds = self.buffer_folds.entry(buffer.id()).or_default();
        for h in headings {
            folds.insert(h.line);
        }
        self.update_hidden_ranges(buffer);
    }

    pub fn fold_open_all(&mut self, buffer: &truff_core::Buffer) {
        self.buffer_folds.remove(&buffer.id());
        self.update_hidden_ranges(buffer);
    }

    pub fn is_line_folded(&self, line: usize) -> bool {
        truff_core::markdown::is_line_in_hidden_ranges(line, &self.active_hidden_ranges)
    }

    pub fn is_heading_folded(&self, buffer_id: truff_core::text::BufferId, line: usize) -> bool {
        self.buffer_folds
            .get(&buffer_id)
            .is_some_and(|set| set.contains(&line))
    }

    pub fn update_hidden_ranges(&mut self, buffer: &truff_core::Buffer) {
        let headings = self.headings_for_buffer(buffer);
        let empty_set = std::collections::BTreeSet::new();
        let folds = self.buffer_folds.get(&buffer.id()).unwrap_or(&empty_set);
        let lines = buffer.line_len();
        let hidden = truff_core::markdown::compute_hidden_line_ranges(&headings, folds, lines);

        // Update heights in HeightIndex for newly folded or unfolded lines
        for r in &hidden {
            for l in r.clone() {
                self.heights.update_block_height(l, 0.0, None);
            }
        }
        for r in &self.active_hidden_ranges {
            for l in r.clone() {
                if !truff_core::markdown::is_line_in_hidden_ranges(l, &hidden) {
                    self.heights
                        .update_block_height(l, self.heights.default_line_height(), None);
                }
            }
        }

        self.active_hidden_ranges = hidden;
        self.cache.clear();
    }
}

/// Renders a read-only Markdown fragment or document using the canonical editor `Paragraph` pipeline.
pub fn render_markdown_preview(ui: &mut egui::Ui, text: &str, theme: &crate::theme::Theme) {
    use crate::app::global_config::TypographyConfig;
    use crate::editor::ProjectionMode;
    use paragraph::{ParagraphStyle, ParagraphView};
    use truff_core::markdown::IncrementalSyntaxTree;

    let available_width = ui.available_width().max(1.0);
    let foreground = color(theme.text_primary);
    let pixels_per_point = ui.ctx().pixels_per_point();
    let typography = TypographyConfig {
        base_font_size: theme.geometry.ui_font_size,
        ..TypographyConfig::default()
    };
    let snapshot = IncrementalSyntaxTree::parse_fresh(text).snapshot();

    let mut line_ranges = Vec::new();
    let mut start = 0usize;
    for raw_line in text.split_inclusive('\n') {
        let trimmed = raw_line.trim_end_matches(['\n', '\r']);
        line_ranges.push(start..(start + trimmed.len()));
        start += raw_line.len();
    }
    if line_ranges.is_empty() {
        line_ranges.push(0..0);
    }

    let is_table_row = |s: &str| {
        let t = s.trim();
        t.starts_with('|') && t.ends_with('|') && t.len() > 1
    };
    let is_table_delimiter = |s: &str| {
        let t = s.trim();
        is_table_row(t) && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
    };

    for (idx, range) in line_ranges.iter().enumerate() {
        let line_str = &text[range.clone()];
        let mut presentation = snapshot.line_presentation_in_str(text, range.clone());
        if presentation.task_fence {
            continue;
        }
        if let Some(list) = &mut presentation.list {
            list.range.start = list.range.start.saturating_sub(range.start);
            list.range.end = list
                .range
                .end
                .saturating_sub(range.start)
                .min(line_str.len());
            let ws_len = line_str[list.range.start..list.range.end]
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
            list.range.start += ws_len;
            if let truff_core::markdown::presentation::ListKind::Task { checkbox, .. } =
                &mut list.kind
            {
                *checkbox = checkbox.saturating_sub(range.start);
            }
        }
        if is_table_row(line_str) {
            if is_table_delimiter(line_str) {
                presentation.table_delimiter = true;
            } else if line_ranges
                .get(idx + 1)
                .is_some_and(|next| is_table_delimiter(&text[next.clone()]))
            {
                presentation.table_header = true;
            }
            let mut start_idx = idx;
            while start_idx > 0 && is_table_row(&text[line_ranges[start_idx - 1].clone()]) {
                start_idx -= 1;
            }
            let mut end_idx = idx + 1;
            while end_idx < line_ranges.len() && is_table_row(&text[line_ranges[end_idx].clone()]) {
                end_idx += 1;
            }
            let row_slices: Vec<&str> = (start_idx..end_idx)
                .map(|i| text[line_ranges[i].clone()].trim_end_matches(['\r', '\n']))
                .collect();
            if let Some(widths) = truff_core::markdown::table::compute_table_col_widths(row_slices)
            {
                presentation.table_col_widths = Some(std::sync::Arc::new(widths));
            }
        }

        let style = ParagraphStyle {
            foreground,
            pixels_per_point,
            width: available_width,
            projection: ProjectionMode::Reading,
            typography: &typography,
            in_insert_mode: false,
        };
        let para = Paragraph::new(
            range.clone(),
            line_str.to_owned(),
            presentation,
            ui.painter(),
            style,
        );
        let is_hidden_delimiter =
            para.presentation.table_delimiter && para.display.has_hidden_content();
        let row_h = if is_hidden_delimiter {
            0.001
        } else {
            para.galley.size().y.max(LINE_HEIGHT * 0.85)
        };
        let row_w = para.galley.size().x.min(available_width);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(row_w, row_h), egui::Sense::hover());
        let painter = ui.painter_at(egui::Rect::from_min_size(
            rect.min,
            egui::vec2(available_width, row_h + 4.0),
        ));
        para.paint(
            &painter,
            rect.min,
            &ParagraphView {
                cursor: None,
                selection: None,
                preedit: None,
                flash: &[],
                mode: truff_core::vim::Mode::Normal,
                theme,
                content_width: available_width,
            },
        );
    }
}
