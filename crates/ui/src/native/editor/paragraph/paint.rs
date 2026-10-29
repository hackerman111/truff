use super::super::color;
use super::Paragraph;
use crate::editor::Affinity;
use crate::overlays::FlashTarget;
use crate::theme::Theme;
use egui::{Color32, FontId, Painter, Pos2, Rect, epaint::text::PlacedRow};
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

use egui::epaint::text::Glyph;

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

fn cluster_vertical_bounds(origin_y: f32, row: &PlacedRow, glyphs: &[Glyph]) -> (f32, f32) {
    if let Some((first, rest)) = glyphs.split_first() {
        let (mut min_y, mut max_y) = glyph_vertical_bounds(origin_y, row, first);
        for g in rest {
            let (top, bottom) = glyph_vertical_bounds(origin_y, row, g);
            min_y = min_y.min(top);
            max_y = max_y.max(bottom);
        }
        (min_y, max_y)
    } else if let Some(last) = row.glyphs.last() {
        glyph_vertical_bounds(origin_y, row, last)
    } else {
        let baseline_y = origin_y + row.pos.y + row.size.y * 0.6;
        let cap_height = (row.size.y * 0.6).max(10.0);
        (
            baseline_y - cap_height - HIGHLIGHT_PAD_Y,
            baseline_y + HIGHLIGHT_PAD_Y,
        )
    }
}

fn row_vertical_bounds(origin_y: f32, row: &PlacedRow) -> (f32, f32) {
    if !row.glyphs.is_empty() {
        cluster_vertical_bounds(origin_y, row, &row.glyphs)
    } else {
        let baseline_y = origin_y + row.pos.y + row.size.y * 0.6;
        let cap_height = (row.size.y * 0.6).max(10.0);
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
        let (r_top, r_bottom) = row_vertical_bounds(origin_y, row);
        min_y = min_y.min(r_top);
        max_y = max_y.max(r_bottom);
    }
    if min_y <= max_y {
        (min_y, max_y)
    } else {
        (
            origin_y - HIGHLIGHT_PAD_Y,
            origin_y + super::super::LINE_HEIGHT + HIGHLIGHT_PAD_Y,
        )
    }
}

impl Paragraph {
    pub(in crate::native::editor) fn char_rect_at(&self, origin: Pos2, source: usize) -> Rect {
        use unicode_segmentation::UnicodeSegmentation;

        let rel_source = source.saturating_sub(self.range.start);
        let visual = self
            .display
            .source_to_visual(BytePos::new(rel_source), Affinity::Right)
            .get();
        let visual_text = self.display.visual_text();

        let cluster_char_len = if visual < visual_text.len() {
            visual_text[visual..]
                .graphemes(true)
                .next()
                .map_or(1, |g| g.chars().count())
        } else {
            1
        };

        let chars_before = visual_text[..visual.min(visual_text.len())].chars().count();
        let ccursor = egui::text::CCursor::new(chars_before);
        let layout_cursor = self.galley.layout_from_cursor(ccursor);

        if let Some(row) = self.galley.rows.get(layout_cursor.row) {
            let col_start = layout_cursor.column;
            let col_end = egui::text::CharIndex(col_start.0 + cluster_char_len);

            let x_start = row.x_offset(col_start) + row.pos.x + origin.x;
            let (min_y, max_y) = if col_start.0 < row.glyphs.len() {
                let slice = &row.glyphs[col_start.0..col_end.0.min(row.glyphs.len())];
                cluster_vertical_bounds(origin.y, row, slice)
            } else if let Some(last) = row.glyphs.last() {
                glyph_vertical_bounds(origin.y, row, last)
            } else {
                row_vertical_bounds(origin.y, row)
            };

            let width = if col_start.0 < row.glyphs.len() {
                let x_end = row.x_offset(col_end) + row.pos.x + origin.x;
                (x_end - x_start).max(row.glyphs[col_start.0].advance_width)
            } else if let Some(glyph) = row.glyphs.last() {
                glyph.advance_width
            } else {
                (row.size.y * 0.55).round().max(4.0)
            };

            Rect::from_min_max(
                Pos2::new(x_start - HIGHLIGHT_PAD_X, min_y),
                Pos2::new(x_start + width.max(2.0) + HIGHLIGHT_PAD_X, max_y),
            )
        } else {
            let caret = self
                .galley
                .pos_from_cursor(ccursor)
                .translate(origin.to_vec2());
            let line_h = caret.height().max(super::super::LINE_HEIGHT);
            let width = (line_h * 0.55).round().max(4.0);
            Rect::from_min_size(caret.left_top(), egui::vec2(width, line_h))
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
                Affinity::Left,
            )
            .get();
        let visual_text = self.display.visual_text();
        let start_chars = visual_text[..visual_start.min(visual_text.len())]
            .chars()
            .count();
        let end_chars = visual_text[..visual_end.min(visual_text.len())]
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
                let (min_y, max_y) = row_vertical_bounds(origin.y, row);
                let rect = Rect::from_min_max(
                    Pos2::new(x_start.min(x_end) - HIGHLIGHT_PAD_X, min_y),
                    Pos2::new(x_start.max(x_end) + HIGHLIGHT_PAD_X, max_y),
                );
                painter.rect_filled(rect, 0.0, color);
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
                    let width = self.galley.size().x.max(view.content_width);
                    let (min_y, max_y) = paragraph_vertical_bounds(origin.y, &self.galley.rows);
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(origin.x, min_y),
                            Pos2::new(origin.x + width, max_y),
                        ),
                        0.0,
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
        } else if let Some(cursor) = view
            .cursor
            .filter(|cursor| self.range.contains(cursor) || *cursor == self.range.end)
        {
            let caret = self.char_rect_at(origin, cursor);
            let width = self.galley.size().x.max(view.content_width);
            let line_rect = Rect::from_min_max(
                Pos2::new(origin.x, caret.min.y - HIGHLIGHT_PAD_Y),
                Pos2::new(origin.x + width, caret.max.y + HIGHLIGHT_PAD_Y),
            );
            let line_bg = if view.theme.is_dark() {
                Color32::from_rgba_unmultiplied(180, 142, 173, 26) // Soft purple tint
            } else {
                Color32::from_rgba_unmultiplied(180, 142, 173, 35)
            };
            painter.rect_filled(line_rect, 0.0, line_bg);
        }
        if self.presentation.code {
            let (min_y, max_y) = paragraph_vertical_bounds(origin.y, &self.galley.rows);
            let width = self.galley.size().x.max(view.content_width);
            let code_rect = Rect::from_min_max(
                Pos2::new(origin.x, min_y - 1.0),
                Pos2::new(origin.x + width, max_y + 1.0),
            );
            painter.rect_filled(
                code_rect,
                4.0,
                color(view.theme.bg_panel).gamma_multiply(0.45),
            );
        }
        if self.display.source_text().trim_start().starts_with('>') {
            let (min_y, max_y) = paragraph_vertical_bounds(origin.y, &self.galley.rows);
            let width = (self.galley.size().x + 24.0).max(view.content_width);
            let quote_rect = Rect::from_min_max(
                Pos2::new(origin.x, min_y - 2.5),
                Pos2::new(origin.x + width, max_y + 2.5),
            );
            painter.rect_filled(
                quote_rect,
                4.0,
                color(view.theme.bg_panel).gamma_multiply(0.35),
            );
            let bar_x = origin.x + 4.0;
            painter.line_segment(
                [Pos2::new(bar_x, min_y - 0.5), Pos2::new(bar_x, max_y + 0.5)],
                egui::Stroke::new(3.0, color(view.theme.accent).gamma_multiply(0.85)),
            );
        }
        if self.presentation.table_header {
            let (min_y, max_y) = paragraph_vertical_bounds(origin.y, &self.galley.rows);
            let width = self.galley.size().x.max(view.content_width);
            let header_rect = Rect::from_min_max(
                Pos2::new(origin.x, min_y - 1.0),
                Pos2::new(origin.x + width, max_y + 1.0),
            );
            painter.rect_filled(
                header_rect,
                2.0,
                color(view.theme.bg_panel).gamma_multiply(0.4),
            );
            let line_y = max_y + 1.0;
            let border_stroke =
                egui::Stroke::new(1.0, color(view.theme.border_subtle).gamma_multiply(1.5));
            painter.hline(origin.x..=(origin.x + width), line_y, border_stroke);
        }
        if self.presentation.table_delimiter {
            let y = (origin.y + self.galley.size().y * 0.5).round();
            let width = self.galley.size().x.max(view.content_width);
            painter.hline(
                origin.x..=(origin.x + width),
                y,
                egui::Stroke::new(1.0, color(view.theme.border_subtle).gamma_multiply(1.2)),
            );
        }
        let code_bg = if view.theme.is_dark() {
            Color32::from_rgba_unmultiplied(128, 128, 128, 50)
        } else {
            Color32::from_rgba_unmultiplied(100, 100, 100, 40)
        };
        for span in self.display.styled_spans() {
            if span.style == crate::editor::InlineStyle::Code {
                let visual_start = self
                    .display
                    .source_to_visual(BytePos::new(span.content.start), Affinity::Right)
                    .get();
                let visual_end = self
                    .display
                    .source_to_visual(BytePos::new(span.content.end), Affinity::Left)
                    .get();
                let visual_text = self.display.visual_text();
                let start_chars = visual_text[..visual_start.min(visual_text.len())]
                    .chars()
                    .count();
                let end_chars = visual_text[..visual_end.min(visual_text.len())]
                    .chars()
                    .count();
                if start_chars >= end_chars {
                    continue;
                }
                let mut row_char_start = 0;
                for row in &self.galley.rows {
                    let row_char_count = row.char_count_including_newline().0;
                    let row_char_end = row_char_start + row_char_count;
                    let s_start = start_chars.max(row_char_start);
                    let s_end = end_chars.min(row_char_end);
                    if s_start < s_end {
                        let col_start = s_start - row_char_start;
                        let col_end = s_end - row_char_start;
                        let x_start =
                            row.x_offset(egui::text::CharIndex(col_start)) + row.pos.x + origin.x;
                        let x_end =
                            row.x_offset(egui::text::CharIndex(col_end)) + row.pos.x + origin.x;
                        let (min_y, max_y) = row_vertical_bounds(origin.y, row);
                        let pad_x = 3.5;
                        let pad_y = 1.0;
                        let rect = Rect::from_min_max(
                            Pos2::new(x_start.min(x_end) - pad_x, min_y - pad_y),
                            Pos2::new(x_start.max(x_end) + pad_x, max_y + pad_y),
                        );
                        painter.rect_filled(rect, 4.0, code_bg);
                    }
                    row_char_start = row_char_end;
                }
            } else if span.style == crate::editor::InlineStyle::Link {
                let visual_start = self
                    .display
                    .source_to_visual(BytePos::new(span.content.start), Affinity::Right)
                    .get();
                let visual_end = self
                    .display
                    .source_to_visual(BytePos::new(span.content.end), Affinity::Left)
                    .get();
                let visual_text = self.display.visual_text();
                let start_chars = visual_text[..visual_start.min(visual_text.len())]
                    .chars()
                    .count();
                let end_chars = visual_text[..visual_end.min(visual_text.len())]
                    .chars()
                    .count();
                if start_chars >= end_chars {
                    continue;
                }
                let link_color = Color32::from_rgba_unmultiplied(136, 192, 208, 220);
                let mut row_char_start = 0;
                for row in &self.galley.rows {
                    let row_char_count = row.char_count_including_newline().0;
                    let row_char_end = row_char_start + row_char_count;
                    let s_start = start_chars.max(row_char_start);
                    let s_end = end_chars.min(row_char_end);
                    if s_start < s_end {
                        let col_start = s_start - row_char_start;
                        let col_end = s_end - row_char_start;
                        let x_start =
                            row.x_offset(egui::text::CharIndex(col_start)) + row.pos.x + origin.x;
                        let x_end =
                            row.x_offset(egui::text::CharIndex(col_end)) + row.pos.x + origin.x;
                        let line_y = if let Some(g) = row.glyphs.get(col_start) {
                            (origin.y + row.pos.y + g.pos.y + 1.5).round()
                        } else {
                            let (_min_y, max_y) = row_vertical_bounds(origin.y, row);
                            (max_y - HIGHLIGHT_PAD_Y + 1.0).round()
                        };
                        painter.hline(
                            x_start.min(x_end)..=x_start.max(x_end),
                            line_y,
                            egui::Stroke::new(1.0, link_color),
                        );
                    }
                    row_char_start = row_char_end;
                }
            }
        }
        if let Some(list) = &self.presentation.list
            && list.depth > 0
        {
            let guide_color = if view.theme.is_dark() {
                color(view.theme.text_muted).gamma_multiply(0.65)
            } else {
                color(view.theme.text_muted).gamma_multiply(0.55)
            };
            let bullet_glyph = self
                .galley
                .rows
                .first()
                .and_then(|row| row.glyphs.iter().find(|g| !g.chr.is_whitespace()));
            let (dot_half, circle_half, default_step) =
                if let Some(section) = self.galley.job.sections.first() {
                    let font = section.format.font_id.clone();
                    let g_dot = painter.layout_no_wrap("•".to_owned(), font.clone(), self.color);
                    let g_circle = painter.layout_no_wrap("◦".to_owned(), font.clone(), self.color);
                    let g_space = painter.layout_no_wrap("    ".to_owned(), font, self.color);
                    (
                        g_dot.size().x * 0.5,
                        g_circle.size().x * 0.5,
                        g_space.size().x,
                    )
                } else {
                    (3.1, 5.1, 16.0)
                };
            let step_width = if list.depth > 0 {
                bullet_glyph.map_or(default_step, |g| g.pos.x / (list.depth as f32))
            } else {
                default_step
            };
            for level in 0..list.depth {
                let half = if level % 2 == 0 {
                    dot_half
                } else {
                    circle_half
                };
                let guide_x = (origin.x + (level as f32) * step_width + half).round();
                for (row_idx, row) in self.galley.rows.iter().enumerate() {
                    let has_text_collision = if row_idx == 0 {
                        row.glyphs.iter().any(|g| {
                            if g.chr.is_whitespace() {
                                return false;
                            }
                            let g_left = origin.x + row.pos.x + g.pos.x;
                            let g_right = g_left + g.size().x.max(g.advance_width);
                            guide_x >= g_left - 1.0 && guide_x <= g_right + 1.0
                        })
                    } else {
                        origin.x + row.pos.x <= guide_x + 4.0
                    };
                    if !has_text_collision {
                        let row_top = origin.y + row.pos.y;
                        let row_bottom = row_top + row.size.y.max(super::super::LINE_HEIGHT);
                        painter.line_segment(
                            [Pos2::new(guide_x, row_top), Pos2::new(guide_x, row_bottom)],
                            egui::Stroke::new(1.0, guide_color),
                        );
                    }
                }
            }
        }
        painter.galley(origin, self.galley.clone(), self.color);
        if self.display.is_rule_hidden() {
            let y = (origin.y + self.galley.size().y * 0.5).round();
            let width = self.galley.size().x.max(view.content_width);
            let stroke =
                egui::Stroke::new(2.0, color(view.theme.border_subtle).gamma_multiply(1.5));
            painter.hline(origin.x..=(origin.x + width), y, stroke);
        }
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
                0.0,
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
