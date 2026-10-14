use super::super::color;
use super::Paragraph;
use crate::editor::Affinity;
use crate::overlays::FlashTarget;
use crate::theme::Theme;
use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect,
    epaint::text::{Glyph, PlacedRow},
};
use std::ops::Range;
use truff_core::{text::BytePos, vim::Mode};

pub(in super::super) struct ParagraphView<'a> {
    pub(in super::super) theme: &'a Theme,
    pub(in super::super) selection: Option<&'a Range<usize>>,
    pub(in super::super) cursor: Option<usize>,
    pub(in super::super) mode: Mode,
    pub(in super::super) preedit: Option<&'a str>,
    pub(in super::super) flash: &'a [FlashTarget],
    pub(in super::super) content_width: f32,
}
const HIGHLIGHT_PAD_Y: f32 = 2.0;
const HIGHLIGHT_PAD_X: f32 = 1.0;

fn glyph_vertical_bounds(origin_y: f32, row: &PlacedRow, glyph: &Glyph) -> (f32, f32) {
    let baseline_y = origin_y + row.pos.y + glyph.pos.y;
    let cap_height = (glyph.font_face_ascent * 0.75).max(10.0);
    let (top, bottom) = if !glyph.uv_rect.is_nothing() {
        let ink_top = baseline_y + glyph.uv_rect.offset.y;
        let ink_bottom = ink_top + glyph.uv_rect.size.y;
        (
            ink_top.min(baseline_y - cap_height),
            ink_bottom.max(baseline_y),
        )
    } else {
        (baseline_y - cap_height, baseline_y)
    };
    (top - HIGHLIGHT_PAD_Y, bottom + HIGHLIGHT_PAD_Y)
}

fn row_fallback_vertical_bounds(origin_y: f32, row: &PlacedRow) -> (f32, f32) {
    if let Some(glyph) = row.glyphs.first() {
        glyph_vertical_bounds(origin_y, row, glyph)
    } else {
        let baseline_y = origin_y + row.pos.y + row.size.y * 0.6;
        let cap_height = 12.0;
        (
            baseline_y - cap_height - HIGHLIGHT_PAD_Y,
            baseline_y + HIGHLIGHT_PAD_Y,
        )
    }
}

fn paragraph_vertical_bounds(origin_y: f32, rows: &[PlacedRow]) -> (f32, f32) {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for row in rows {
        if !row.glyphs.is_empty() {
            for glyph in &row.glyphs {
                let (g_top, g_bottom) = glyph_vertical_bounds(origin_y, row, glyph);
                min_y = min_y.min(g_top);
                max_y = max_y.max(g_bottom);
            }
        } else {
            let (r_top, r_bottom) = row_fallback_vertical_bounds(origin_y, row);
            min_y = min_y.min(r_top);
            max_y = max_y.max(r_bottom);
        }
    }
    if min_y <= max_y {
        (min_y, max_y)
    } else if let Some(first_row) = rows.first() {
        row_fallback_vertical_bounds(origin_y, first_row)
    } else {
        let baseline_y = origin_y + super::super::LINE_HEIGHT * 0.6;
        let cap_height = 12.0;
        (
            baseline_y - cap_height - HIGHLIGHT_PAD_Y,
            baseline_y + HIGHLIGHT_PAD_Y,
        )
    }
}

impl Paragraph {
    fn char_rect_at(&self, origin: Pos2, source: usize) -> Rect {
        let visual = self
            .display
            .source_to_visual(
                BytePos::new(source.saturating_sub(self.range.start)),
                Affinity::Right,
            )
            .get();
        let chars = self.display.visual_text()[..visual.min(self.display.visual_text().len())]
            .chars()
            .count();
        let ccursor = egui::text::CCursor::new(chars);
        let layout_cursor = self.galley.layout_from_cursor(ccursor);
        if let Some(row) = self.galley.rows.get(layout_cursor.row) {
            let x = row.x_offset(layout_cursor.column) + row.pos.x + origin.x;
            let (width, (min_y, max_y)) =
                if let Some(glyph) = row.glyphs.get(layout_cursor.column.0) {
                    (
                        glyph.advance_width,
                        glyph_vertical_bounds(origin.y, row, glyph),
                    )
                } else if let Some(glyph) = row.glyphs.last() {
                    (
                        (row.size.y * 0.5).max(8.5),
                        glyph_vertical_bounds(origin.y, row, glyph),
                    )
                } else {
                    (
                        (row.size.y * 0.5).max(8.5),
                        row_fallback_vertical_bounds(origin.y, row),
                    )
                };
            Rect::from_min_max(
                Pos2::new(x - HIGHLIGHT_PAD_X, min_y),
                Pos2::new(x + width.max(2.0) + HIGHLIGHT_PAD_X, max_y),
            )
        } else {
            let caret = self
                .galley
                .pos_from_cursor(ccursor)
                .translate(origin.to_vec2());
            Rect::from_min_size(
                caret.left_top(),
                egui::vec2(8.5, caret.height().max(super::super::LINE_HEIGHT)),
            )
        }
    }

    fn paint_selection_range(
        &self,
        painter: &Painter,
        origin: Pos2,
        start_source: usize,
        end_source: usize,
        color: Color32,
    ) {
        let visual_start = self
            .display
            .source_to_visual(
                BytePos::new(start_source.saturating_sub(self.range.start)),
                Affinity::Right,
            )
            .get();
        let visual_end = self
            .display
            .source_to_visual(
                BytePos::new(end_source.saturating_sub(self.range.start)),
                Affinity::Right,
            )
            .get();
        let start_chars = self.display.visual_text()
            [..visual_start.min(self.display.visual_text().len())]
            .chars()
            .count();
        let end_chars = self.display.visual_text()
            [..visual_end.min(self.display.visual_text().len())]
            .chars()
            .count();
        if start_chars >= end_chars && !self.galley.rows.is_empty() {
            return;
        }

        let mut row_char_start = 0;
        for row in &self.galley.rows {
            let row_char_count = row.char_count_including_newline().0;
            let row_char_end = row_char_start + row_char_count;
            let sel_start = start_chars.max(row_char_start);
            let sel_end = end_chars.min(row_char_end);
            if sel_start < sel_end {
                let col_start = sel_start - row_char_start;
                let col_end = sel_end - row_char_start;
                let x_start = row.x_offset(egui::text::CharIndex(col_start)) + row.pos.x + origin.x;
                let x_end = row.x_offset(egui::text::CharIndex(col_end)) + row.pos.x + origin.x;
                let glyph_slice =
                    &row.glyphs[col_start.min(row.glyphs.len())..col_end.min(row.glyphs.len())];
                let (min_y, max_y) = if !glyph_slice.is_empty() {
                    let mut min_y = f32::INFINITY;
                    let mut max_y = f32::NEG_INFINITY;
                    for glyph in glyph_slice {
                        let (g_top, g_bottom) = glyph_vertical_bounds(origin.y, row, glyph);
                        min_y = min_y.min(g_top);
                        max_y = max_y.max(g_bottom);
                    }
                    (min_y, max_y)
                } else {
                    row_fallback_vertical_bounds(origin.y, row)
                };
                let rect = Rect::from_min_max(
                    Pos2::new(x_start.min(x_end), min_y),
                    Pos2::new(x_start.max(x_end), max_y),
                );
                painter.rect_filled(rect, 2.0, color);
            }
            row_char_start = row_char_end;
        }
    }

    pub(in super::super) fn paint(
        &self,
        painter: &Painter,
        origin: Pos2,
        view: &ParagraphView<'_>,
    ) -> Option<Rect> {
        let mut cursor_rect = None;
        let caret_at = |source: usize| {
            let visual = self
                .display
                .source_to_visual(
                    BytePos::new(source.saturating_sub(self.range.start)),
                    Affinity::Right,
                )
                .get();
            let chars = self.display.visual_text()[..visual.min(self.display.visual_text().len())]
                .chars()
                .count();
            self.galley
                .pos_from_cursor(egui::text::CCursor::new(chars))
                .translate(origin.to_vec2())
        };
        if let Some(selection) = view.selection {
            if view.mode == Mode::VisualLine {
                let is_selected = (selection.start <= self.range.start
                    && selection.end > self.range.start)
                    || (self.range.start == self.range.end
                        && selection.contains(&self.range.start))
                    || self.range.contains(&selection.start);
                if is_selected {
                    let width = (self.galley.size().x + self.text_indent).max(view.content_width);
                    let (min_y, max_y) = paragraph_vertical_bounds(origin.y, &self.galley.rows);
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(origin.x - self.text_indent, min_y),
                            Pos2::new(origin.x - self.text_indent + width, max_y),
                        ),
                        2.0,
                        color(view.theme.selection),
                    );
                }
            } else {
                let a = selection.start.max(self.range.start);
                let b = selection.end.min(self.range.end);
                if a < b {
                    self.paint_selection_range(painter, origin, a, b, color(view.theme.selection));
                }
            }
        }
        painter.galley(origin, self.galley.clone(), self.color);
        if let Some(cursor) = view
            .cursor
            .filter(|cursor| self.range.contains(cursor) || *cursor == self.range.end)
        {
            let mut caret = self.char_rect_at(origin, cursor);
            if view.mode == Mode::Insert {
                caret.min.x += HIGHLIGHT_PAD_X;
                caret.max.x = caret.min.x + 1.5;
            }
            painter.rect_filled(
                caret,
                2.0,
                color(view.theme.cursor).gamma_multiply(if view.mode == Mode::Insert {
                    1.0
                } else {
                    0.45
                }),
            );
            cursor_rect = Some(caret);
            if let Some(text) = view.preedit {
                let galley =
                    painter.layout_no_wrap(text.to_owned(), FontId::proportional(14.0), self.color);
                painter.galley(caret.left_top(), galley, self.color);
            }
        }
        for target in view
            .flash
            .iter()
            .filter(|target| self.range.contains(&target.source_range.start))
        {
            let position = caret_at(target.source_range.start).left_top();
            let badge = painter.layout_no_wrap(
                target.label.clone(),
                FontId::monospace(14.0),
                color(view.theme.flash_fg),
            );
            painter.rect_filled(
                Rect::from_min_size(position, badge.size()),
                3.0,
                color(view.theme.flash_bg),
            );
            painter.galley(position, badge, color(view.theme.flash_fg));
        }
        cursor_rect
    }
}
