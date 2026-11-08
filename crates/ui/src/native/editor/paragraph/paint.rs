use std::ops::Range;
use std::sync::Arc;

use egui::{Color32, FontId, Galley, Painter, Pos2, Rect};
use truff_core::{text::BytePos, vim::Mode};

use super::super::color;
use super::Paragraph;
use super::spans::{
    HIGHLIGHT_PAD_X, HIGHLIGHT_PAD_Y, char_rect_in_galley, for_each_row_char_span,
    paint_inline_decorations, paragraph_vertical_bounds, row_vertical_bounds, visual_char_range,
};
use crate::editor::Affinity;
use crate::overlays::FlashTarget;
use crate::theme::Theme;

pub(in super::super) struct ParagraphView<'a> {
    pub(in super::super) theme: &'a Theme,
    pub(in super::super) selection: Option<&'a Range<usize>>,
    pub(in super::super) cursor: Option<usize>,
    pub(in super::super) cursor_visible: bool,
    pub(in super::super) pixels_per_point: f32,
    pub(in super::super) mode: Mode,
    pub(in super::super) preedit: Option<&'a str>,
    pub(in super::super) flash: &'a [FlashTarget],
    pub(in super::super) content_width: f32,
}

fn paint_obsidian_cursor(
    painter: &Painter,
    mut caret: Rect,
    boundary_pad_x: f32,
    glyph_overlay: Option<(Pos2, &Arc<Galley>)>,
    view: &ParagraphView<'_>,
) -> Rect {
    let cur = color(view.theme.cursor);
    if view.mode == Mode::Insert {
        const INSERT_CARET_WIDTH: f32 = 2.0;
        let ppp = view.pixels_per_point.max(1.0);
        let boundary_x = caret.min.x + boundary_pad_x;
        let raw_left = boundary_x - INSERT_CARET_WIDTH * 0.5;
        let snapped_left = (raw_left * ppp).round() / ppp;
        let px_width = (INSERT_CARET_WIDTH * ppp).round().max(2.0) / ppp;
        caret.min.x = snapped_left;
        caret.max.x = snapped_left + px_width;
        if view.cursor_visible {
            let caret_fill = Color32::from_rgba_unmultiplied(cur.r(), cur.g(), cur.b(), 235);
            painter.rect_filled(caret, 0.0, caret_fill);
        }
        return caret;
    }

    let block_fill = Color32::from_rgba_unmultiplied(cur.r(), cur.g(), cur.b(), 232);
    painter.rect_filled(caret, 0.0, block_fill);
    if let Some((galley_origin, galley)) = glyph_overlay
        && !galley.is_empty()
    {
        let luma = 0.299 * (cur.r() as f32) + 0.587 * (cur.g() as f32) + 0.114 * (cur.b() as f32);
        let cursor_fg = if luma >= 165.0 {
            Color32::from_rgb(24, 26, 31)
        } else {
            Color32::from_rgb(236, 239, 244)
        };
        let clip = caret.intersect(painter.clip_rect());
        if clip.is_positive() {
            painter.with_clip_rect(clip).add(
                egui::epaint::TextShape::new(galley_origin, galley.clone(), cursor_fg)
                    .with_override_text_color(cursor_fg),
            );
        }
    }
    caret
}

impl Paragraph {
    pub(in crate::native::editor) fn char_rect_at(&self, origin: Pos2, source: usize) -> Rect {
        let rel_source = source.saturating_sub(self.range.start);
        if let Some(table) = &self.table_row {
            return table.char_rect_at(origin, rel_source);
        }
        char_rect_in_galley(&self.display, &self.galley, origin, rel_source, false)
    }

    fn paint_selection_range(
        &self,
        painter: &Painter,
        origin: Pos2,
        start_source: usize,
        end_source: usize,
        color: Color32,
    ) {
        let local_range = start_source.saturating_sub(self.range.start)
            ..end_source.saturating_sub(self.range.start);
        let Some(chars) = visual_char_range(&self.display, local_range) else {
            return;
        };
        for_each_row_char_span(
            &self.galley.rows,
            chars,
            origin.x,
            |row, _, x_start, x_end| {
                let (min_y, max_y) = row_vertical_bounds(origin.y, row);
                let rect = Rect::from_min_max(
                    Pos2::new(x_start.min(x_end) - HIGHLIGHT_PAD_X, min_y),
                    Pos2::new(x_start.max(x_end) + HIGHLIGHT_PAD_X, max_y),
                );
                painter.rect_filled(rect, 0.0, color);
            },
        );
    }

    pub(in super::super) fn paint(
        &self,
        painter: &Painter,
        origin: Pos2,
        view: &ParagraphView<'_>,
    ) -> Option<Rect> {
        let mut cursor_rect = None;
        if let Some(table) = &self.table_row {
            table.paint(
                painter,
                origin,
                &self.range,
                self.presentation.table_header,
                view,
            );
            if let Some(cursor) = view
                .cursor
                .filter(|cursor| self.range.contains(cursor) || *cursor == self.range.end)
            {
                let rel_source = cursor.saturating_sub(self.range.start);
                let raw_caret = self.char_rect_at(origin, cursor);
                let overlay = table.cell_galley_for_cursor(origin, rel_source);
                cursor_rect = Some(paint_obsidian_cursor(
                    painter, raw_caret, 0.0, overlay, view,
                ));
            }
            return cursor_rect;
        }

        let is_task = self.is_task();
        let text_pad_x = if is_task { super::TASK_PADDING_X } else { 0.0 };
        let text_origin = Pos2::new(origin.x + text_pad_x, origin.y);
        let caret_at = |source: usize| {
            let visual = self
                .display
                .source_to_visual(
                    BytePos::new(source.saturating_sub(self.range.start)),
                    Affinity::Right,
                )
                .get();
            let vis_text = self.display.visual_text();
            let chars = vis_text[..visual.min(vis_text.len())].chars().count();
            self.galley
                .pos_from_cursor(egui::text::CCursor::new(chars))
                .translate(text_origin.to_vec2())
        };

        if let Some(selection) = view.selection {
            if view.mode == Mode::VisualLine {
                let is_selected = (selection.start <= self.range.start
                    && selection.end > self.range.start)
                    || (self.range.start == self.range.end
                        && selection.contains(&self.range.start))
                    || self.range.contains(&selection.start);
                if is_selected {
                    let width = (self.galley.size().x + text_pad_x).max(view.content_width);
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
                    self.paint_selection_range(
                        painter,
                        text_origin,
                        a,
                        b,
                        color(view.theme.selection),
                    );
                }
            }
        } else if (!self.presentation.task_fence
            || self.display.mode() == crate::editor::ProjectionMode::Source)
            && let Some(cursor) = view
                .cursor
                .filter(|cursor| self.range.contains(cursor) || *cursor == self.range.end)
        {
            let caret = self.char_rect_at(text_origin, cursor);
            let width = (self.galley.size().x + text_pad_x).max(view.content_width);
            let scale = (caret.height() / super::spans::BASE_CURSOR_HEIGHT).max(1.0);
            let pad_y = if scale > 1.2 {
                ((50.0 - caret.height()) * 0.5).max(HIGHLIGHT_PAD_Y)
            } else {
                HIGHLIGHT_PAD_Y
            };
            let line_rect = Rect::from_min_max(
                Pos2::new(origin.x, caret.min.y - pad_y),
                Pos2::new(origin.x + width, caret.max.y + pad_y),
            );
            let cur = color(view.theme.cursor);
            let line_alpha = if view.theme.is_dark() { 46 } else { 20 };
            let line_bg = Color32::from_rgba_unmultiplied(cur.r(), cur.g(), cur.b(), line_alpha);
            painter.rect_filled(line_rect, 0.0, line_bg);
        }

        if let Some(list) = &self.presentation.list
            && let truff_core::markdown::presentation::ListKind::Task { checked, checkbox } =
                list.kind
        {
            let status_byte = self
                .display
                .source_text()
                .as_bytes()
                .get(checkbox)
                .copied()
                .unwrap_or(b' ');
            let is_in_progress = !checked && matches!(status_byte, b'/' | b'-');
            let (status_color, bg_alpha) = if checked {
                (color(view.theme.callout_tip), 0.14)
            } else if is_in_progress {
                (color(view.theme.callout_warning), 0.16)
            } else {
                (color(view.theme.callout_important), 0.12)
            };
            let (min_y, max_y) = paragraph_vertical_bounds(origin.y, &self.galley.rows);
            let width = (self.galley.size().x + text_pad_x).max(view.content_width);
            let task_rect = Rect::from_min_max(
                Pos2::new(origin.x, min_y - 1.5),
                Pos2::new(origin.x + width, max_y + 1.5),
            );
            painter.rect_filled(task_rect, 4.0, status_color.gamma_multiply(bg_alpha));
            let bar_x = origin.x + 1.5;
            painter.line_segment(
                [Pos2::new(bar_x, min_y - 0.5), Pos2::new(bar_x, max_y + 0.5)],
                egui::Stroke::new(3.0, status_color.gamma_multiply(0.85)),
            );
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

        if self.presentation.table_delimiter && !self.display.has_hidden_content() {
            let y = (origin.y + self.galley.size().y * 0.5).round();
            let width = self.galley.size().x;
            painter.hline(
                origin.x..=(origin.x + width),
                y,
                egui::Stroke::new(1.0, color(view.theme.border_subtle).gamma_multiply(1.2)),
            );
        }

        paint_inline_decorations(
            painter,
            &self.display,
            &self.galley.rows,
            text_origin,
            origin.y,
            view.theme.is_dark(),
            false,
        );

        if !self.guide_xs.is_empty() {
            let guide_color = if view.theme.is_dark() {
                color(view.theme.text_muted).gamma_multiply(0.65)
            } else {
                color(view.theme.text_muted).gamma_multiply(0.55)
            };
            for &center_x in &self.guide_xs {
                let guide_x = text_origin.x + center_x;
                for (row_idx, row) in self.galley.rows.iter().enumerate() {
                    let has_text_collision = if row_idx == 0 {
                        row.glyphs.iter().any(|g| {
                            if g.chr.is_whitespace() {
                                return false;
                            }
                            let g_left = text_origin.x + row.pos.x + g.pos.x;
                            let g_right = g_left + g.size().x.max(g.advance_width);
                            guide_x >= g_left - 1.0 && guide_x <= g_right + 1.0
                        })
                    } else {
                        text_origin.x + row.pos.x <= guide_x + 4.0
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

        painter.galley(text_origin, self.galley.clone(), self.color);

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
            let raw_caret = self.char_rect_at(text_origin, cursor);
            let rel_source = cursor.saturating_sub(self.range.start);
            let visual = self
                .display
                .source_to_visual(BytePos::new(rel_source), Affinity::Right)
                .get();
            let overlay =
                (visual < self.display.visual_text().len()).then_some((text_origin, &self.galley));
            let caret = paint_obsidian_cursor(painter, raw_caret, HIGHLIGHT_PAD_X, overlay, view);
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
