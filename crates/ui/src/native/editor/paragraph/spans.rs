//! Shared `egui::Galley` span geometry, vertical glyph bounds, and inline decoration painters.

use std::ops::Range;

use egui::epaint::text::{Glyph, PlacedRow};
use egui::{Color32, FontId, Galley, Painter, Pos2, Rect, Stroke};
use truff_core::text::BytePos;
use unicode_segmentation::UnicodeSegmentation;

use crate::editor::{Affinity, DisplayMap, InlineStyle};

pub(super) const HIGHLIGHT_PAD_Y: f32 = 2.0;
pub(super) const HIGHLIGHT_PAD_X: f32 = 1.0;
pub(super) const BASE_CURSOR_HEIGHT: f32 = 22.0;

#[inline]
pub(super) fn cursor_row_vertical_bounds(
    origin_y: f32,
    row: &PlacedRow,
    base_font_size: f32,
) -> (f32, f32) {
    let font_size = base_font_size.max(10.0);
    let scale = (font_size / 15.0).max(1.0);
    let cursor_height = (BASE_CURSOR_HEIGHT * scale).round();

    let baseline_y = if let Some(best) = row.glyphs.iter().max_by(|a, b| {
        a.font_ascent
            .partial_cmp(&b.font_ascent)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        let primary_pos_y = best.pos.y - best.font_face_ascent + best.font_ascent
            - 0.5 * (best.font_height - best.font_face_height);
        origin_y + row.pos.y + primary_pos_y
    } else {
        origin_y + row.pos.y + font_size * 0.93
    };

    let center_y = baseline_y - 3.5 * scale;
    let half_h = cursor_height * 0.5;
    (center_y - half_h, center_y + half_h)
}

#[inline]
pub(super) fn glyph_vertical_bounds(origin_y: f32, row: &PlacedRow, glyph: &Glyph) -> (f32, f32) {
    let baseline_y = origin_y + row.pos.y + glyph.pos.y;
    let cap_height = (glyph.font_face_ascent * 0.75).max(10.0);
    (
        baseline_y - cap_height - HIGHLIGHT_PAD_Y,
        baseline_y + HIGHLIGHT_PAD_Y,
    )
}

pub(super) fn cluster_vertical_bounds(
    origin_y: f32,
    row: &PlacedRow,
    glyphs: &[Glyph],
) -> (f32, f32) {
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

#[inline]
pub(super) fn row_vertical_bounds(origin_y: f32, row: &PlacedRow) -> (f32, f32) {
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

pub(super) fn paragraph_vertical_bounds(origin_y: f32, rows: &[PlacedRow]) -> (f32, f32) {
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
            origin_y + crate::native::editor::LINE_HEIGHT + HIGHLIGHT_PAD_Y,
        )
    }
}

/// Converts a local source byte range into a character-index range on `display.visual_text()`.
#[inline]
pub(super) fn visual_char_range(
    display: &DisplayMap,
    source_range: Range<usize>,
) -> Option<Range<usize>> {
    let v_start = display
        .source_to_visual(BytePos::new(source_range.start), Affinity::Right)
        .get();
    let v_end = display
        .source_to_visual(BytePos::new(source_range.end), Affinity::Left)
        .get();
    let vis_text = display.visual_text();
    let clamped_start = v_start.min(vis_text.len());
    let clamped_end = v_end.min(vis_text.len());
    if clamped_start >= clamped_end {
        return None;
    }
    let start_chars = vis_text[..clamped_start].chars().count();
    let span_chars = vis_text[clamped_start..clamped_end].chars().count();
    (span_chars > 0).then_some(start_chars..(start_chars + span_chars))
}

/// Visits every row slice overlapping `char_range`, yielding `(row, col_start, x_start, x_end)`.
#[inline]
pub(super) fn for_each_row_char_span(
    rows: &[PlacedRow],
    char_range: Range<usize>,
    text_origin_x: f32,
    mut f: impl FnMut(&PlacedRow, usize, f32, f32),
) {
    let mut row_char_start = 0;
    for row in rows {
        let row_count = row.char_count_including_newline().0;
        let row_char_end = row_char_start + row_count;
        let s_start = char_range.start.max(row_char_start);
        let s_end = char_range.end.min(row_char_end);
        if s_start < s_end {
            let col_start = s_start - row_char_start;
            let col_end = s_end - row_char_start;
            let x_start =
                row.x_offset(egui::text::CharIndex(col_start)) + row.pos.x + text_origin_x;
            let x_end = row.x_offset(egui::text::CharIndex(col_end)) + row.pos.x + text_origin_x;
            f(row, col_start, x_start, x_end);
        }
        row_char_start = row_char_end;
        if row_char_start >= char_range.end {
            break;
        }
    }
}

/// Paints `InlineStyle::Code` pill backgrounds and `InlineStyle::Link` underlines.
pub(super) fn paint_inline_decorations(
    painter: &Painter,
    display: &DisplayMap,
    rows: &[PlacedRow],
    text_origin: Pos2,
    row_y_origin: f32,
    is_dark: bool,
    tight_rows: bool,
) {
    if display.styled_spans().is_empty() {
        return;
    }
    let code_bg = if is_dark {
        Color32::from_rgba_unmultiplied(128, 128, 128, 50)
    } else {
        Color32::from_rgba_unmultiplied(100, 100, 100, 40)
    };
    let link_color = Color32::from_rgba_unmultiplied(136, 192, 208, 220);

    for span in display.styled_spans() {
        match span.style {
            InlineStyle::Code => {
                let Some(chars) = visual_char_range(display, span.content.clone()) else {
                    continue;
                };
                for_each_row_char_span(rows, chars, text_origin.x, |row, _, x_start, x_end| {
                    let (min_y, max_y) = if tight_rows {
                        let top = row_y_origin + row.pos.y;
                        (top, top + row.size.y)
                    } else {
                        row_vertical_bounds(row_y_origin, row)
                    };
                    let rect = Rect::from_min_max(
                        Pos2::new(x_start.min(x_end) - 3.5, min_y - 1.0),
                        Pos2::new(x_start.max(x_end) + 3.5, max_y + 1.0),
                    );
                    painter.rect_filled(rect, 4.0, code_bg);
                });
            }
            InlineStyle::Link => {
                let Some(chars) = visual_char_range(display, span.content.clone()) else {
                    continue;
                };
                for_each_row_char_span(
                    rows,
                    chars,
                    text_origin.x,
                    |row, col_start, x_start, x_end| {
                        let line_y = if tight_rows {
                            (row_y_origin + row.pos.y + row.size.y - 1.0).round()
                        } else if let Some(g) = row.glyphs.get(col_start) {
                            (row_y_origin + row.pos.y + g.pos.y + 1.5).round()
                        } else {
                            let (_min_y, max_y) = row_vertical_bounds(row_y_origin, row);
                            (max_y - HIGHLIGHT_PAD_Y + 1.0).round()
                        };
                        painter.hline(
                            x_start.min(x_end)..=x_start.max(x_end),
                            line_y,
                            Stroke::new(1.0, link_color),
                        );
                    },
                );
            }
            _ => {}
        }
    }
}

/// Computes the bounding rect of the grapheme cluster at `rel_source` in `galley`.
pub(super) fn char_rect_in_galley(
    display: &DisplayMap,
    galley: &Galley,
    origin: Pos2,
    rel_source: usize,
    tight_rows: bool,
) -> Rect {
    let visual = display
        .source_to_visual(BytePos::new(rel_source), Affinity::Right)
        .get();
    let visual_text = display.visual_text();

    let cluster_char_len = if visual < visual_text.len() {
        visual_text[visual..]
            .graphemes(true)
            .next()
            .map_or(1, |g| g.chars().count())
    } else {
        1
    };

    let base_font_size = galley
        .job
        .sections
        .iter()
        .map(|s| s.format.font_id.size)
        .fold(0.0_f32, f32::max);
    let base_font_size = if base_font_size > 0.0 {
        base_font_size
    } else {
        15.0
    };

    let chars_before = visual_text[..visual.min(visual_text.len())].chars().count();
    let ccursor = egui::text::CCursor::new(chars_before);
    let layout_cursor = galley.layout_from_cursor(ccursor);

    if let Some(row) = galley.rows.get(layout_cursor.row) {
        let col_start = layout_cursor.column;
        let col_end = egui::text::CharIndex(col_start.0 + cluster_char_len);
        let x_start = row.x_offset(col_start) + row.pos.x + origin.x;
        let (min_y, max_y) = cursor_row_vertical_bounds(origin.y, row, base_font_size);

        if tight_rows {
            let x_end = row.x_offset(col_end) + row.pos.x + origin.x;
            let width = (x_end - x_start).max(base_font_size * 0.55).max(2.0);
            return Rect::from_min_max(
                Pos2::new(x_start, min_y),
                Pos2::new(x_start + width, max_y),
            );
        }

        let width = if col_start.0 < row.glyphs.len() {
            let x_end = row.x_offset(col_end) + row.pos.x + origin.x;
            (x_end - x_start).max(row.glyphs[col_start.0].advance_width)
        } else if let Some(glyph) = row.glyphs.last() {
            glyph.advance_width
        } else {
            (base_font_size * 0.55).round().max(4.0)
        };

        Rect::from_min_max(
            Pos2::new(x_start - HIGHLIGHT_PAD_X, min_y),
            Pos2::new(x_start + width.max(2.0) + HIGHLIGHT_PAD_X, max_y),
        )
    } else {
        let caret = galley.pos_from_cursor(ccursor).translate(origin.to_vec2());
        let font_size = base_font_size.max(10.0);
        let scale = (font_size / 15.0).max(1.0);
        let cursor_height = (BASE_CURSOR_HEIGHT * scale).round();
        let baseline_y = caret.top() + font_size * 0.93;
        let center_y = baseline_y - 3.5 * scale;
        let half_h = cursor_height * 0.5;
        let min_y = center_y - half_h;
        let max_y = center_y + half_h;
        let width = if tight_rows {
            (base_font_size * 0.55).round().max(2.0)
        } else {
            (base_font_size * 0.55).round().max(4.0)
        };
        Rect::from_min_max(
            Pos2::new(caret.left(), min_y),
            Pos2::new(caret.left() + width, max_y),
        )
    }
}

/// Precomputes nested list indent-guide horizontal offsets once on paragraph build/refresh.
pub(super) fn compute_list_guide_offsets(
    painter: &Painter,
    font: &FontId,
    color: Color32,
    depth: usize,
) -> Box<[f32]> {
    if depth == 0 {
        return Box::default();
    }
    const STANDARD_PREFIXES: [&str; 6] = [
        "•",
        "    ◦",
        "        •",
        "            ◦",
        "                •",
        "                    ◦",
    ];
    let mut xs = Vec::with_capacity(depth);
    for level in 0..depth {
        let prefix = STANDARD_PREFIXES
            .get(level)
            .map(|&s| s.to_owned())
            .unwrap_or_else(|| {
                let bullet_chr = if level % 2 == 0 { "•" } else { "◦" };
                format!("{}{bullet_chr}", "    ".repeat(level))
            });
        let prefix_galley = painter.layout_no_wrap(prefix, font.clone(), color);
        let center_x = if let Some(row) = prefix_galley.rows.first()
            && let Some(g) = row.glyphs.iter().find(|g| !g.chr.is_whitespace())
        {
            if !g.uv_rect.is_nothing() {
                g.pos.x + g.uv_rect.offset.x + g.uv_rect.size.x * 0.5
            } else {
                g.pos.x + g.size().x * 0.5
            }
        } else {
            (level as f32) * 16.0 + 2.5
        };
        xs.push(center_x);
    }
    xs.into_boxed_slice()
}
