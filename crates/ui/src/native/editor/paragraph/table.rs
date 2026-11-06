use std::ops::Range;
use std::sync::Arc;

use egui::{Color32, Galley, Painter, Pos2, Rect, Vec2};
use truff_core::text::BytePos;
use unicode_segmentation::UnicodeSegmentation;

use super::ParagraphStyle;
use super::paint::ParagraphView;
use crate::editor::{Affinity, DisplayMap, VisualPos};
use crate::native::editor::color;

pub(super) const CELL_PAD_X: f32 = 8.0;
pub(super) const CELL_PAD_Y: f32 = 6.0;

#[derive(Clone)]
pub(super) struct TableCell {
    pub(super) source_range: Range<usize>,
    pub(super) content_start: usize,
    pub(super) content_end: usize,
    pub(super) col_x: f32,
    pub(super) width: f32,
    pub(super) display: DisplayMap,
    pub(super) galley: Arc<Galley>,
}

#[derive(Clone)]
pub(crate) struct TableRowLayout {
    pub(super) cells: Vec<TableCell>,
    pub(super) col_xs: Vec<f32>,
    pub(super) height: f32,
    pub(super) total_width: f32,
}

impl TableRowLayout {
    pub(super) fn build(
        source: &str,
        presentation: &truff_core::markdown::presentation::LinePresentation,
        painter: &Painter,
        style: ParagraphStyle<'_>,
    ) -> Option<Self> {
        let is_del =
            presentation.table_delimiter || truff_core::markdown::table::is_table_delimiter(source);
        if is_del || style.projection == crate::editor::ProjectionMode::Source {
            return None;
        }

        let pipe_indices = truff_core::markdown::table::find_table_pipe_indices(source);
        if pipe_indices.len() < 2 {
            return None;
        }
        let num_cols = pipe_indices.len() - 1;

        let mut char_widths_storage = Vec::new();
        let col_widths_slice: &[usize] =
            if let Some(widths) = presentation.table_col_widths.as_deref() {
                if widths.len() < num_cols {
                    char_widths_storage = widths.to_vec();
                    char_widths_storage.resize(num_cols, 10);
                    &char_widths_storage
                } else {
                    &widths[..num_cols]
                }
            } else if let Some(widths) =
                truff_core::markdown::table::compute_table_col_widths(std::iter::once(source))
            {
                char_widths_storage = widths;
                if char_widths_storage.len() < num_cols {
                    char_widths_storage.resize(num_cols, 10);
                    &char_widths_storage
                } else {
                    &char_widths_storage[..num_cols]
                }
            } else {
                char_widths_storage.resize(num_cols, 10);
                &char_widths_storage
            };

        let (font, _) = crate::native::paint::resolve_font_and_line_height("a", style.typography);
        let char_w = painter
            .layout_no_wrap("0".to_owned(), font, style.foreground)
            .size()
            .x
            .max(7.0);

        let col_widths_px = truff_core::markdown::table::compute_column_pixel_widths(
            col_widths_slice,
            style.width,
            char_w,
        );
        if col_widths_px.len() < num_cols {
            return None;
        }

        let mut col_xs = Vec::with_capacity(num_cols + 1);
        let mut cur_x = 0.0;
        col_xs.push(cur_x);
        for &w in &col_widths_px {
            cur_x += w;
            col_xs.push(cur_x);
        }
        let total_width = cur_x;

        let mut cells = Vec::with_capacity(num_cols);
        let mut max_cell_height: f32 = crate::native::editor::LINE_HEIGHT;

        for col in 0..num_cols {
            let p_open = pipe_indices[col];
            let p_close = pipe_indices[col + 1];
            let raw_cell = &source[p_open + 1..p_close];
            let trimmed = raw_cell.trim();

            let leading_ws = raw_cell.len() - raw_cell.trim_start().len();
            let content_start = (p_open + 1) + leading_ws;
            let content_end = content_start + trimmed.len();

            let col_x = col_xs[col];
            let col_w = col_widths_px[col];
            let text_wrap_w = (col_w - 2.0 * CELL_PAD_X).max(10.0);

            let cell_display = DisplayMap::new(trimmed, style.projection);
            let (cell_galley, _) = crate::native::paint::shape(
                painter,
                &cell_display,
                style.foreground,
                text_wrap_w,
                style.typography,
            );

            max_cell_height = max_cell_height.max(cell_galley.size().y);

            cells.push(TableCell {
                source_range: (p_open + 1)..p_close,
                content_start,
                content_end,
                col_x,
                width: col_w,
                display: cell_display,
                galley: cell_galley,
            });
        }

        let height = (max_cell_height + 2.0 * CELL_PAD_Y).max(crate::native::editor::LINE_HEIGHT);

        Some(Self {
            cells,
            col_xs,
            height,
            total_width,
        })
    }

    pub(super) fn char_rect_at(&self, origin: Pos2, source_rel: usize) -> Rect {
        let line_h = crate::native::editor::LINE_HEIGHT;
        let caret_w = 2.0;

        for cell in &self.cells {
            if source_rel >= cell.source_range.start && source_rel <= cell.source_range.end {
                let cell_origin =
                    Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);
                if source_rel <= cell.content_start {
                    return Rect::from_min_size(cell_origin, Vec2::new(caret_w, line_h));
                }
                if source_rel >= cell.content_end {
                    let chars_count = cell.display.visual_text().chars().count();
                    let pos = cell
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(chars_count));
                    return Rect::from_min_size(
                        cell_origin + pos.min.to_vec2(),
                        Vec2::new(caret_w, line_h),
                    );
                }
                let local_byte = source_rel - cell.content_start;
                let vis_byte = cell
                    .display
                    .source_to_visual(BytePos::new(local_byte), Affinity::Right)
                    .get();
                let vis_text = cell.display.visual_text();
                let cluster_char_len = if vis_byte < vis_text.len() {
                    vis_text[vis_byte..]
                        .graphemes(true)
                        .next()
                        .map_or(1, |g| g.chars().count())
                } else {
                    1
                };
                let chars_before = vis_text[..vis_byte.min(vis_text.len())].chars().count();
                let layout_cursor = cell
                    .galley
                    .layout_from_cursor(egui::text::CCursor::new(chars_before));
                if let Some(row) = cell.galley.rows.get(layout_cursor.row) {
                    let col_start = layout_cursor.column;
                    let col_end = egui::text::CharIndex(col_start.0 + cluster_char_len);
                    let x_start = row.x_offset(col_start) + row.pos.x + cell_origin.x;
                    let x_end = row.x_offset(col_end) + row.pos.x + cell_origin.x;
                    let width = (x_end - x_start).max(row.size.y * 0.55).max(caret_w);
                    let y = cell_origin.y + row.pos.y;
                    return Rect::from_min_size(
                        Pos2::new(x_start, y),
                        Vec2::new(width, row.size.y),
                    );
                }
                let pos = cell
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(chars_before));
                return Rect::from_min_size(
                    cell_origin + pos.min.to_vec2(),
                    Vec2::new(caret_w, line_h),
                );
            }
        }

        let best_col = self
            .cells
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| {
                c.source_range
                    .start
                    .abs_diff(source_rel)
                    .min(c.source_range.end.abs_diff(source_rel))
            })
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        let px = self.col_xs.get(best_col).copied().unwrap_or(0.0);
        Rect::from_min_size(
            Pos2::new(origin.x + px, origin.y + CELL_PAD_Y),
            Vec2::new(caret_w, line_h),
        )
    }

    pub(super) fn cursor_from_pos(
        &self,
        local_pos: egui::Vec2,
        row_display: &DisplayMap,
    ) -> egui::text::CCursor {
        let col_idx = self
            .cells
            .iter()
            .position(|c| local_pos.x >= c.col_x && local_pos.x < c.col_x + c.width)
            .unwrap_or_else(|| {
                if local_pos.x < 0.0 {
                    0
                } else {
                    self.cells.len().saturating_sub(1)
                }
            });

        if let Some(cell) = self.cells.get(col_idx) {
            let cell_x = (local_pos.x - cell.col_x - CELL_PAD_X).max(0.0);
            let cell_y = (local_pos.y - CELL_PAD_Y).max(0.0);
            let cell_cursor = cell.galley.cursor_from_pos(egui::vec2(cell_x, cell_y));
            let cell_vis_text = cell.display.visual_text();
            let cell_vis_byte = cell_vis_text
                .char_indices()
                .nth(cell_cursor.index.0)
                .map_or(cell_vis_text.len(), |(b, _)| b);
            let cell_source_byte = cell
                .display
                .visual_to_source(VisualPos::new(cell_vis_byte))
                .pos
                .get();
            let row_source_byte = cell.content_start + cell_source_byte;
            let row_vis_byte = row_display
                .source_to_visual(BytePos::new(row_source_byte), Affinity::Right)
                .get();
            let row_vis_text = row_display.visual_text();
            let row_char_idx = row_vis_text[..row_vis_byte.min(row_vis_text.len())]
                .chars()
                .count();
            return egui::text::CCursor::new(row_char_idx);
        }

        egui::text::CCursor::default()
    }

    pub(super) fn paint(
        &self,
        painter: &Painter,
        origin: Pos2,
        is_header: bool,
        view: &ParagraphView<'_>,
    ) {
        let y_top = origin.y;
        let y_bottom = origin.y + self.height;
        let border_color = color(view.theme.border_subtle).gamma_multiply(1.3);
        let border_stroke = egui::Stroke::new(1.0, border_color);

        if let (Some(&first_x), Some(&last_x)) = (self.col_xs.first(), self.col_xs.last()) {
            let fx = origin.x + first_x;
            let lx = origin.x + last_x;

            if is_header {
                let header_rect = Rect::from_min_max(Pos2::new(fx, y_top), Pos2::new(lx, y_bottom));
                painter.rect_filled(
                    header_rect,
                    0.0,
                    color(view.theme.bg_panel).gamma_multiply(0.5),
                );
                painter.hline(fx..=lx, y_top.round(), border_stroke);
                painter.hline(
                    fx..=lx,
                    y_bottom.round(),
                    egui::Stroke::new(1.5, border_color),
                );
            } else {
                painter.hline(fx..=lx, y_bottom.round(), border_stroke);
            }

            for &px in &self.col_xs {
                let x = (origin.x + px).round();
                painter.vline(x, y_top..=y_bottom, border_stroke);
            }
        }

        let code_bg = if view.theme.is_dark() {
            Color32::from_rgba_unmultiplied(128, 128, 128, 50)
        } else {
            Color32::from_rgba_unmultiplied(100, 100, 100, 40)
        };

        if let Some(selection) = view.selection {
            let sel_color = color(view.theme.selection);
            for cell in &self.cells {
                let cell_origin =
                    Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);
                let a = selection.start.max(cell.content_start);
                let b = selection.end.min(cell.content_end);
                if a < b {
                    let local_a = a - cell.content_start;
                    let local_b = b - cell.content_start;
                    let va = cell
                        .display
                        .source_to_visual(BytePos::new(local_a), Affinity::Right)
                        .get();
                    let vb = cell
                        .display
                        .source_to_visual(BytePos::new(local_b), Affinity::Left)
                        .get();
                    let vis_text = cell.display.visual_text();
                    let start_chars = vis_text[..va.min(vis_text.len())].chars().count();
                    let end_chars = vis_text[..vb.min(vis_text.len())].chars().count();
                    if start_chars < end_chars {
                        let mut row_char_start = 0;
                        for row in &cell.galley.rows {
                            let row_count = row.char_count_including_newline().0;
                            let row_char_end = row_char_start + row_count;
                            let s_start = start_chars.max(row_char_start);
                            let s_end = end_chars.min(row_char_end);
                            if s_start < s_end {
                                let col_start = s_start - row_char_start;
                                let col_end = s_end - row_char_start;
                                let x_start = row.x_offset(egui::text::CharIndex(col_start))
                                    + row.pos.x
                                    + cell_origin.x;
                                let x_end = row.x_offset(egui::text::CharIndex(col_end))
                                    + row.pos.x
                                    + cell_origin.x;
                                let min_y = cell_origin.y + row.pos.y;
                                let max_y = min_y + row.size.y;
                                let rect = Rect::from_min_max(
                                    Pos2::new(x_start.min(x_end), min_y),
                                    Pos2::new(x_start.max(x_end), max_y),
                                );
                                painter.rect_filled(rect, 0.0, sel_color);
                            }
                            row_char_start = row_char_end;
                        }
                    }
                }
            }
        }

        for cell in &self.cells {
            let cell_origin = Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);

            for span in cell.display.styled_spans() {
                if span.style == crate::editor::InlineStyle::Code {
                    let v_start = cell
                        .display
                        .source_to_visual(BytePos::new(span.content.start), Affinity::Right)
                        .get();
                    let v_end = cell
                        .display
                        .source_to_visual(BytePos::new(span.content.end), Affinity::Left)
                        .get();
                    let vis_text = cell.display.visual_text();
                    let start_chars = vis_text[..v_start.min(vis_text.len())].chars().count();
                    let end_chars = vis_text[..v_end.min(vis_text.len())].chars().count();
                    if start_chars < end_chars {
                        let mut row_char_start = 0;
                        for row in &cell.galley.rows {
                            let row_count = row.char_count_including_newline().0;
                            let row_char_end = row_char_start + row_count;
                            let s_start = start_chars.max(row_char_start);
                            let s_end = end_chars.min(row_char_end);
                            if s_start < s_end {
                                let col_start = s_start - row_char_start;
                                let col_end = s_end - row_char_start;
                                let x_start = row.x_offset(egui::text::CharIndex(col_start))
                                    + row.pos.x
                                    + cell_origin.x;
                                let x_end = row.x_offset(egui::text::CharIndex(col_end))
                                    + row.pos.x
                                    + cell_origin.x;
                                let min_y = cell_origin.y + row.pos.y;
                                let max_y = min_y + row.size.y;
                                let rect = Rect::from_min_max(
                                    Pos2::new(x_start.min(x_end) - 3.5, min_y - 1.0),
                                    Pos2::new(x_start.max(x_end) + 3.5, max_y + 1.0),
                                );
                                painter.rect_filled(rect, 4.0, code_bg);
                            }
                            row_char_start = row_char_end;
                        }
                    }
                }
            }

            painter.galley(
                cell_origin,
                cell.galley.clone(),
                color(view.theme.text_primary),
            );
        }
    }
}
