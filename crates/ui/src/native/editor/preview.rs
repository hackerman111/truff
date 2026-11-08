//! Shared line presentation normalization, table width caching, and cached Markdown preview rendering.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use truff_core::Buffer;
use truff_core::markdown::IncrementalSyntaxTree;
use truff_core::markdown::presentation::{LinePresentation, ListKind};
use truff_core::vim::Mode;

use super::paragraph::{Paragraph, ParagraphStyle, ParagraphView};
use super::{EditorWidget, LINE_HEIGHT, color};
use crate::app::global_config::TypographyConfig;
use crate::editor::ProjectionMode;

pub(super) fn line_count(buffer: &Buffer) -> usize {
    let trailing = buffer
        .rope()
        .chunks()
        .next_back()
        .is_some_and(|chunk| chunk.ends_with('\n'));
    (buffer.line_len() + usize::from(trailing)).max(1)
}

pub(super) fn line_start(buffer: &Buffer, line: usize) -> usize {
    if line >= buffer.line_len() {
        buffer.byte_len()
    } else {
        buffer.byte_of_line(line)
    }
}

pub(super) fn slice_is_table_row(slice: &truff_core::RopeSlice<'_>) -> bool {
    let mut non_ws = slice
        .chunks()
        .flat_map(str::bytes)
        .filter(|b| !b.is_ascii_whitespace());
    let Some(first) = non_ws.next() else {
        return false;
    };
    first == b'|' && non_ws.next_back() == Some(b'|')
}

pub(super) fn slice_is_table_delimiter(slice: &truff_core::RopeSlice<'_>) -> bool {
    slice_is_table_row(slice)
        && slice.chunks().flat_map(str::bytes).any(|b| b == b'-')
        && slice
            .chunks()
            .flat_map(str::bytes)
            .all(|b| b.is_ascii_whitespace() || matches!(b, b'|' | b'-' | b':'))
}

#[inline]
pub(super) fn str_is_table_row(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('|') && t.ends_with('|') && t.len() > 1
}

#[inline]
pub(super) fn str_is_table_delimiter(s: &str) -> bool {
    let t = s.trim();
    str_is_table_row(t)
        && t.bytes().any(|b| b == b'-')
        && t.bytes().all(|b| matches!(b, b'|' | b'-' | b':' | b' '))
}

/// Normalizes a document-relative `ListMarker` into line-local byte coordinates without allocating.
pub(super) fn normalize_list_presentation(
    presentation: &mut LinePresentation,
    line_start_byte: usize,
    line_byte_len: usize,
    leading_ws_in_marker: impl FnOnce(Range<usize>) -> usize,
) {
    let Some(list) = &mut presentation.list else {
        return;
    };
    list.range.start = list.range.start.saturating_sub(line_start_byte);
    list.range.end = list
        .range
        .end
        .saturating_sub(line_start_byte)
        .min(line_byte_len);
    let ws_len = leading_ws_in_marker(list.range.clone());
    list.range.start += ws_len;
    if let ListKind::Task { checkbox, .. } = &mut list.kind {
        *checkbox = checkbox.saturating_sub(line_start_byte);
    }
}

impl EditorWidget {
    pub(super) fn invalidate_active_table_on_edit(
        &mut self,
        first: usize,
        old_last: usize,
        line_delta: isize,
    ) {
        let Some((range, _)) = &mut self.active_table else {
            return;
        };
        let touches_or_borders = first <= range.end && old_last.saturating_add(1) >= range.start;
        if touches_or_borders {
            self.active_table = None;
        } else if old_last < range.start && line_delta != 0 {
            range.start = range.start.saturating_add_signed(line_delta);
            range.end = range.end.saturating_add_signed(line_delta);
        }
    }

    pub(super) fn get_or_compute_table_col_widths(
        &mut self,
        buffer: &Buffer,
        line: usize,
    ) -> Option<Arc<Vec<usize>>> {
        if let Some((range, widths)) = &self.active_table
            && range.contains(&line)
        {
            return Some(Arc::clone(widths));
        }

        let mut start_line = line;
        while start_line > 0 {
            let prev = start_line - 1;
            let s = line_start(buffer, prev);
            let e = line_start(buffer, start_line);
            if slice_is_table_row(&buffer.slice(s..e)) {
                start_line = prev;
            } else {
                break;
            }
        }

        let mut end_line = line + 1;
        while end_line < buffer.line_len() {
            let s = line_start(buffer, end_line);
            let e = line_start(buffer, end_line + 1);
            if slice_is_table_row(&buffer.slice(s..e)) {
                end_line += 1;
            } else {
                break;
            }
        }

        let mut rows = Vec::with_capacity(end_line - start_line);
        for l in start_line..end_line {
            let s = line_start(buffer, l);
            let e = line_start(buffer, l + 1);
            let mut line_str = buffer.slice(s..e).to_string();
            while line_str.ends_with('\n') || line_str.ends_with('\r') {
                line_str.pop();
            }
            rows.push(line_str);
        }

        let row_refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        let widths = truff_core::markdown::table::compute_table_col_widths(row_refs)?;
        let arc_widths = Arc::new(widths);
        self.active_table = Some((start_line..end_line, Arc::clone(&arc_widths)));
        Some(arc_widths)
    }
}

type CachedPreviewLines = Arc<[(Range<usize>, LinePresentation)]>;

struct PreviewCacheEntry {
    hash: u64,
    len: usize,
    lines: CachedPreviewLines,
}

const MAX_PREVIEW_CACHE_ENTRIES: usize = 16;

thread_local! {
    static PREVIEW_CACHE: RefCell<VecDeque<PreviewCacheEntry>> = const { RefCell::new(VecDeque::new()) };
}

fn cached_preview_presentations(text: &str) -> CachedPreviewLines {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    let hash = hasher.finish();
    let len = text.len();

    if let Some(hit) = PREVIEW_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|e| e.hash == hash && e.len == len)
            .map(|e| Arc::clone(&e.lines))
    }) {
        return hit;
    }

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

    let mut parsed = Vec::with_capacity(line_ranges.len());
    let mut active_table: Option<(Range<usize>, Arc<Vec<usize>>)> = None;

    for (idx, range) in line_ranges.iter().enumerate() {
        let line_str = &text[range.clone()];
        let mut presentation = snapshot.line_presentation_in_str(text, range.clone());
        normalize_list_presentation(&mut presentation, range.start, line_str.len(), |r| {
            line_str[r]
                .bytes()
                .take_while(|&b| b == b' ' || b == b'\t')
                .count()
        });

        if str_is_table_row(line_str) {
            if str_is_table_delimiter(line_str) {
                presentation.table_delimiter = true;
            } else if line_ranges
                .get(idx + 1)
                .is_some_and(|next| str_is_table_delimiter(&text[next.clone()]))
            {
                presentation.table_header = true;
            }
            let widths = if let Some((ref tr, ref w)) = active_table
                && tr.contains(&idx)
            {
                Some(Arc::clone(w))
            } else {
                let mut start_idx = idx;
                while start_idx > 0 && str_is_table_row(&text[line_ranges[start_idx - 1].clone()]) {
                    start_idx -= 1;
                }
                let mut end_idx = idx + 1;
                while end_idx < line_ranges.len()
                    && str_is_table_row(&text[line_ranges[end_idx].clone()])
                {
                    end_idx += 1;
                }
                let row_slices = (start_idx..end_idx)
                    .map(|i| text[line_ranges[i].clone()].trim_end_matches(['\r', '\n']));
                truff_core::markdown::table::compute_table_col_widths(row_slices).map(|v| {
                    let arc = Arc::new(v);
                    active_table = Some((start_idx..end_idx, Arc::clone(&arc)));
                    arc
                })
            };
            presentation.table_col_widths = widths;
        }
        parsed.push((range.clone(), presentation));
    }

    let lines: CachedPreviewLines = parsed.into();
    PREVIEW_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_PREVIEW_CACHE_ENTRIES {
            cache.pop_front();
        }
        cache.push_back(PreviewCacheEntry {
            hash,
            len,
            lines: Arc::clone(&lines),
        });
    });
    lines
}

/// Renders a read-only Markdown fragment or document using the canonical editor `Paragraph` pipeline.
pub fn render_markdown_preview(ui: &mut egui::Ui, text: &str, theme: &crate::theme::Theme) {
    let available_width = ui.available_width().max(1.0);
    let foreground = color(theme.text_primary);
    let pixels_per_point = ui.ctx().pixels_per_point();
    let typography = TypographyConfig {
        base_font_size: theme.geometry.ui_font_size,
        ..TypographyConfig::default()
    };
    let parsed_lines = cached_preview_presentations(text);

    let style = ParagraphStyle {
        foreground,
        pixels_per_point,
        width: available_width,
        projection: ProjectionMode::Reading,
        typography: &typography,
        in_insert_mode: false,
    };
    let view = ParagraphView {
        cursor: None,
        cursor_visible: false,
        pixels_per_point,
        selection: None,
        preedit: None,
        flash: &[],
        mode: Mode::Normal,
        theme,
        content_width: available_width,
    };

    for (range, presentation) in parsed_lines.iter() {
        if presentation.task_fence || range.end > text.len() {
            continue;
        }
        let line_str = &text[range.clone()];
        let para = Paragraph::new(
            range.clone(),
            line_str.to_owned(),
            presentation.clone(),
            ui.painter(),
            style,
        );
        let size = para.size();
        let is_hidden_delimiter =
            para.presentation.table_delimiter && para.display.has_hidden_content();
        let row_h = if is_hidden_delimiter {
            0.001
        } else {
            size.y.max(LINE_HEIGHT * 0.85)
        };
        let row_w = size.x.min(available_width);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(row_w, row_h), egui::Sense::hover());
        let painter = ui.painter_at(egui::Rect::from_min_size(
            rect.min,
            egui::vec2(available_width, row_h + 4.0),
        ));
        para.paint(&painter, rect.min, &view);
    }
}
