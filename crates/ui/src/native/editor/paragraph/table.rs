use std::ops::Range;
use std::sync::Arc;

use egui::{Galley, Painter, Pos2, Rect, Vec2};
use truff_core::text::BytePos;
use truff_core::vim::Mode;

use super::ParagraphStyle;
use super::paint::ParagraphView;
use super::spans::{
    char_rect_in_galley, for_each_row_char_span, paint_inline_decorations, visual_char_range,
};
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
        row_cursor: BytePos,
        row_selection: Option<(BytePos, BytePos)>,
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

            let cell_cursor =
                if row_cursor.get() >= content_start && row_cursor.get() <= content_end {
                    BytePos::new(row_cursor.get() - content_start)
                } else {
                    BytePos::new(usize::MAX)
                };

            let cell_selection = row_selection.and_then(|(sel_a, sel_b)| {
                let sel_start = sel_a.get().min(sel_b.get());
                let sel_end = sel_a.get().max(sel_b.get());
                if sel_start < content_end && sel_end > content_start {
                    let a =
                        BytePos::new(sel_start.saturating_sub(content_start).min(trimmed.len()));
                    let b = BytePos::new(sel_end.saturating_sub(content_start).min(trimmed.len()));
                    Some((a, b))
                } else {
                    None
                }
            });

            let mut cell_display = DisplayMap::new(trimmed, style.projection);
            cell_display.set_insert_mode(style.in_insert_mode);
            cell_display.set_cursor(cell_cursor);
            cell_display.set_selection(cell_selection);

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
        let block_w = (line_h * 0.45).round().max(8.0);

        for cell in &self.cells {
            if source_rel >= cell.source_range.start && source_rel <= cell.source_range.end {
                let cell_origin =
                    Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);
                if source_rel < cell.content_start {
                    return char_rect_in_galley(&cell.display, &cell.galley, cell_origin, 0, true);
                }
                let local_byte = (source_rel - cell.content_start)
                    .min(cell.content_end.saturating_sub(cell.content_start));
                return char_rect_in_galley(
                    &cell.display,
                    &cell.galley,
                    cell_origin,
                    local_byte,
                    true,
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
        if let Some(cell) = self.cells.get(best_col) {
            let sample = char_rect_in_galley(
                &cell.display,
                &cell.galley,
                Pos2::new(origin.x + px, origin.y + CELL_PAD_Y),
                0,
                true,
            );
            return Rect::from_min_size(sample.min, Vec2::new(block_w, sample.height()));
        }
        Rect::from_min_size(
            Pos2::new(origin.x + px, origin.y + CELL_PAD_Y),
            Vec2::new(block_w, super::spans::BASE_CURSOR_HEIGHT),
        )
    }

    pub(super) fn cell_galley_for_cursor(
        &self,
        origin: Pos2,
        source_rel: usize,
    ) -> Option<(Pos2, &Arc<Galley>)> {
        let cell = self
            .cells
            .iter()
            .find(|c| source_rel >= c.content_start && source_rel < c.content_end)?;
        let cell_origin = Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);
        Some((cell_origin, &cell.galley))
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
        row_range: &Range<usize>,
        is_header: bool,
        view: &ParagraphView<'_>,
    ) {
        let y_top = origin.y;
        let y_bottom = origin.y + self.height;
        let border_color = color(view.theme.border_subtle).gamma_multiply(1.3);
        let border_stroke = egui::Stroke::new(1.0, border_color);

        if let Some(selection) = view.selection
            && view.mode == Mode::VisualLine
        {
            let is_selected = (selection.start <= row_range.start
                && selection.end > row_range.start)
                || (row_range.start == row_range.end && selection.contains(&row_range.start))
                || (row_range.start >= selection.start && row_range.end <= selection.end);
            if is_selected {
                let fx = origin.x + self.col_xs.first().copied().unwrap_or(0.0);
                let lx = origin.x + self.col_xs.last().copied().unwrap_or(self.total_width);
                let width = (lx - fx).max(view.content_width);
                let row_rect =
                    Rect::from_min_max(Pos2::new(fx, y_top), Pos2::new(fx + width, y_bottom));
                painter.rect_filled(row_rect, 0.0, color(view.theme.selection));
            }
        }

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

        if let Some(selection) = view.selection
            && view.mode != Mode::VisualLine
            && selection.start < row_range.end
            && selection.end > row_range.start
        {
            let sel_color = color(view.theme.selection);
            let rel_start = selection.start.max(row_range.start) - row_range.start;
            let rel_end = selection.end.min(row_range.end) - row_range.start;

            for cell in &self.cells {
                let cell_origin =
                    Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);
                let p_open = cell.source_range.start.saturating_sub(1);
                let p_close = cell.source_range.end;

                let y_top = origin.y;
                let y_bottom = origin.y + self.height;
                let min_y = y_top;
                let max_y = y_bottom;

                if cell.content_start < cell.content_end {
                    if rel_start < cell.content_start && rel_end > p_open {
                        let rect = Rect::from_min_max(
                            Pos2::new(origin.x + cell.col_x, min_y),
                            Pos2::new(cell_origin.x, max_y),
                        );
                        painter.rect_filled(rect, 0.0, sel_color);
                    }

                    let a = rel_start.max(cell.content_start);
                    let b = rel_end.min(cell.content_end);
                    if a < b
                        && let Some(chars) = visual_char_range(
                            &cell.display,
                            (a - cell.content_start)..(b - cell.content_start),
                        )
                    {
                        for_each_row_char_span(
                            &cell.galley.rows,
                            chars,
                            cell_origin.x,
                            |row, _, x_start, x_end| {
                                let is_first = cell
                                    .galley
                                    .rows
                                    .first()
                                    .is_some_and(|r| std::ptr::eq(r, row));
                                let is_last = cell
                                    .galley
                                    .rows
                                    .last()
                                    .is_some_and(|r| std::ptr::eq(r, row));
                                let r_min_y = if is_first {
                                    y_top
                                } else {
                                    cell_origin.y + row.pos.y
                                };
                                let r_max_y = if is_last {
                                    y_bottom
                                } else {
                                    cell_origin.y + row.pos.y + row.size.y
                                };
                                let rect = Rect::from_min_max(
                                    Pos2::new(x_start.min(x_end) - 1.0, r_min_y),
                                    Pos2::new(x_start.max(x_end) + 1.0, r_max_y),
                                );
                                painter.rect_filled(rect, 0.0, sel_color);
                            },
                        );
                    }

                    if rel_start < p_close && rel_end > cell.content_end {
                        let text_end_x = cell_origin.x + cell.galley.size().x;
                        let cell_end_x = origin.x + cell.col_x + cell.width;
                        if text_end_x < cell_end_x {
                            let rect = Rect::from_min_max(
                                Pos2::new(text_end_x, min_y),
                                Pos2::new(cell_end_x, max_y),
                            );
                            painter.rect_filled(rect, 0.0, sel_color);
                        }
                    }
                } else if rel_start < p_close && rel_end > p_open {
                    let rect = Rect::from_min_max(
                        Pos2::new(origin.x + cell.col_x, min_y),
                        Pos2::new(origin.x + cell.col_x + cell.width, max_y),
                    );
                    painter.rect_filled(rect, 0.0, sel_color);
                }
            }
        }

        let is_dark = view.theme.is_dark();
        let text_fg = color(view.theme.text_primary);
        for cell in &self.cells {
            let cell_origin = Pos2::new(origin.x + cell.col_x + CELL_PAD_X, origin.y + CELL_PAD_Y);
            paint_inline_decorations(
                painter,
                &cell.display,
                &cell.galley.rows,
                cell_origin,
                cell_origin.y,
                is_dark,
                true,
            );
            painter.galley(cell_origin, cell.galley.clone(), text_fg);
        }
    }
}
