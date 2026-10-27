use super::color;
use super::paragraph::{Paragraph, ParagraphStyle, ParagraphView};
use super::{EditorWidget, LINE_HEIGHT, ViewScroll};
use crate::app::global_config::TypographyConfig;
use crate::editor::{EditorInput, ProjectionMode, ScrollAnchor, VisualPos};
use crate::overlays::FlashTarget;
use crate::theme::Theme;
use egui::{Sense, Vec2};
use truff_core::{Buffer, text::BytePos, vim::Mode, workspace::View};
const MAX_LINE_BYTES: usize = 4096;

impl EditorWidget {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        buffer: Option<&Buffer>,
        view: Option<&View>,
        mode: Mode,
        projection: ProjectionMode,
        theme: &Theme,
        typography: &TypographyConfig,
        preedit: Option<&str>,
        flash: &[FlashTarget],
        note_title: Option<&str>,
    ) -> Option<EditorInput> {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, color(theme.bg_primary));
        let caret_was_visible = self
            .cursor_rect
            .zip(self.last_viewport)
            .is_some_and(|(caret, viewport)| viewport.contains_rect(caret));
        self.cursor_rect = None;
        self.painted_lines = 0..0;
        self.visible = 0..0;
        let (Some(buffer), Some(view)) = (buffer, view) else {
            return None;
        };
        let (content_width, margin_x) = theme.geometry.column_metrics(rect.width());
        let cursor = view.head().get().min(buffer.byte_len());
        let cursor_line = buffer.line_of_byte(cursor);
        let lines = line_count(buffer);
        let view_changed = self.last_cursor.is_none_or(|(id, _)| id != view.id());
        if self.buffer_id != Some(buffer.id())
            || self.heights.block_count() != lines
            || self.revision != Some(buffer.revision())
        {
            self.heights.init_uniform(lines);
            self.buffer_id = Some(buffer.id());
            self.cache.clear();
        }
        self.revision = Some(buffer.revision());
        let identity = (view.id(), cursor);
        let cursor_changed = self.last_cursor != Some(identity);
        let resized = self.last_viewport.map(|viewport| viewport.size()) != Some(rect.size());
        self.last_viewport = Some(rect);
        let saved =
            self.view_scrolls.get(&view.id()).copied().filter(|saved| {
                saved.buffer_id == buffer.id() && saved.revision == buffer.revision()
            });
        let follow = if view_changed && saved.is_some() {
            false
        } else {
            cursor_changed || (resized && caret_was_visible) || self.follow_pending
        };
        if view_changed {
            let (line, local, horizontal) = saved
                .map(|saved| {
                    (
                        buffer.line_of_byte(saved.anchor_byte.min(buffer.byte_len())),
                        saved.local_y,
                        saved.horizontal,
                    )
                })
                .unwrap_or((
                    view.scroll_top_line(),
                    if view.scroll_top_line() == 0 {
                        -Self::TOP_MARGIN
                    } else {
                        0.0
                    },
                    0.0,
                ));
            self.scroll = (Self::TOP_MARGIN + self.heights.y_for_block(line) + local).max(0.0);
            self.horizontal = horizontal;
            self.cache.clear();
        }
        let scroll_pos = self.pending_scroll_pos.take();
        if let Some(pos) = scroll_pos {
            let caret_y = Self::TOP_MARGIN + self.heights.y_for_block(cursor_line);
            match pos {
                truff_core::vim::ScrollPosition::Center => {
                    self.scroll = caret_y - (rect.height() - LINE_HEIGHT) / 2.0;
                }
                truff_core::vim::ScrollPosition::Top => {
                    self.scroll = caret_y;
                }
                truff_core::vim::ScrollPosition::Bottom => {
                    self.scroll = caret_y - rect.height() + LINE_HEIGHT;
                }
            }
        } else if follow {
            let y = Self::TOP_MARGIN + self.heights.y_for_block(cursor_line);
            if y < self.scroll {
                self.scroll = y;
            }
            if y + LINE_HEIGHT > self.scroll + rect.height() {
                self.scroll = y + LINE_HEIGHT - rect.height();
            }
        }
        if !self.input_blocked && response.hovered() {
            self.scroll_by(-ui.input(|input| input.smooth_scroll_delta.y));
            self.horizontal =
                (self.horizontal - ui.input(|input| input.smooth_scroll_delta.x)).max(0.0);
        }
        if !self.input_blocked
            && response.dragged()
            && let Some(position) = response.interact_pointer_pos()
        {
            let edge = LINE_HEIGHT.min(rect.height() / 4.0);
            let distance = if position.y < rect.top() + edge {
                position.y - rect.top() - edge
            } else if position.y > rect.bottom() - edge {
                position.y - rect.bottom() + edge
            } else {
                0.0
            };
            if distance != 0.0 {
                self.scroll_by(
                    distance.clamp(-edge, edge) * ui.input(|input| input.stable_dt.min(0.1)) * 12.0,
                );
                ui.ctx().request_repaint();
            }
        }
        self.scroll = self.scroll.clamp(
            0.0,
            (Self::TOP_MARGIN + self.heights.total_height() - rect.height()).max(0.0),
        );
        self.last_cursor = Some(identity);
        let (top, local_y) = self
            .heights
            .block_at_y((self.scroll - Self::TOP_MARGIN).max(0.0));
        let mut anchor = ScrollAnchor::new(top, local_y, self.scroll);
        let overscan = LINE_HEIGHT * 4.0;
        let first = self
            .heights
            .block_at_y((self.scroll - Self::TOP_MARGIN - overscan).max(0.0))
            .0;
        let last = (self
            .heights
            .block_at_y((self.scroll + rect.height() - Self::TOP_MARGIN + overscan).max(0.0))
            .0
            + 1)
        .min(lines);
        self.cache
            .retain(|line, _| (*line >= first && *line < last) || (follow && *line == cursor_line));
        let foreground = color(theme.text_primary);
        let selection = if mode == Mode::VisualChar {
            let start = view.anchor().get().min(cursor);
            let end = view.anchor().get().max(cursor);
            let end = buffer
                .grapheme_next(BytePos::new(end))
                .map_or(buffer.byte_len(), BytePos::get);
            Some(start..end)
        } else if mode == Mode::VisualLine {
            let first = buffer.line_of_byte(view.anchor().get().min(cursor));
            let last = buffer.line_of_byte(view.anchor().get().max(cursor));
            let start = buffer.byte_of_line(first);
            let end = if last + 1 < buffer.line_len() {
                buffer.byte_of_line(last + 1)
            } else {
                buffer.byte_len()
            };
            Some(start..end)
        } else if view.anchor().get() != cursor {
            Some(view.anchor().get().min(cursor)..view.anchor().get().max(cursor))
        } else {
            None
        };
        let mut clicked = None;
        let syntax = self.syntax.snapshot(buffer);
        // Measure a distant target directly, even when it is outside the virtual window.
        let target = (follow && !(first..last).contains(&cursor_line)).then_some(cursor_line);
        for line in (first..last).chain(target) {
            let start = line_start(buffer, line);
            let end = line_start(buffer, line + 1);
            let mut range = start..end;
            // A pathological logical line uses a bounded source window around its caret.
            // Other lines remain visible; the Rope and source offsets stay authoritative.
            let long = end - start > MAX_LINE_BYTES;
            let width = if long { f32::INFINITY } else { content_width };
            if long {
                let center = if line == cursor_line { cursor } else { start };
                range.start = center.saturating_sub(MAX_LINE_BYTES / 2).max(start);
                while !buffer.rope().is_char_boundary(range.start) {
                    range.start += 1;
                }
                range.end = (range.start + MAX_LINE_BYTES).min(end);
                while !buffer.rope().is_char_boundary(range.end) {
                    range.end -= 1;
                }
            }
            while range.end > range.start
                && matches!(buffer.rope().byte(range.end - 1), b'\n' | b'\r')
            {
                range.end -= 1;
            }
            let style = ParagraphStyle {
                foreground,
                pixels_per_point: ui.ctx().pixels_per_point(),
                width,
                projection: if long {
                    ProjectionMode::Source
                } else {
                    projection
                },
                typography,
            };
            let up_to_date = self.cache.get(&line).is_some_and(|entry| {
                entry.revision == buffer.revision()
                    && entry.presentation_revision == buffer.revision()
                    && entry.range == range
            });
            if !up_to_date {
                let slice = buffer.slice(range.clone());
                let presentation = if let Some(entry) = self.cache.get(&line).filter(|entry| {
                    entry.presentation_revision == buffer.revision() && entry.range == range
                }) {
                    entry.presentation.clone()
                } else {
                    let mut presentation = syntax
                        .as_ref()
                        .map(|syntax| syntax.line_presentation(range.clone()))
                        .unwrap_or_default();
                    if let Some(list) = &mut presentation.list {
                        list.range.start = list.range.start.saturating_sub(range.start);
                        list.range.end = list
                            .range
                            .end
                            .saturating_sub(range.start)
                            .min(slice.byte_len());
                        let line_str = slice.to_string();
                        let ws_len = line_str[list.range.start..list.range.end]
                            .bytes()
                            .take_while(|b| *b == b' ' || *b == b'\t')
                            .count();
                        list.range.start += ws_len;
                        if let truff_core::markdown::presentation::ListKind::Task {
                            checkbox, ..
                        } = &mut list.kind
                        {
                            *checkbox = checkbox.saturating_sub(range.start);
                        }
                    }
                    if slice_is_table_row(&slice) {
                        let next_start = line_start(buffer, line + 1);
                        let next_end = line_start(buffer, line + 2);
                        if next_start < buffer.byte_len()
                            && slice_is_table_delimiter(&buffer.slice(next_start..next_end))
                        {
                            presentation.table_header = true;
                        }
                    }
                    presentation
                };
                let changed = self.cache.get(&line).is_none_or(|entry| {
                    entry.presentation != presentation
                        || (entry.range != range || entry.revision != buffer.revision())
                            && !slice
                                .chunks()
                                .flat_map(str::bytes)
                                .eq(entry.display.source_text().bytes())
                });
                if changed {
                    self.cache.insert(
                        line,
                        Paragraph::new(
                            range.clone(),
                            slice.to_string(),
                            presentation,
                            &painter,
                            style,
                        ),
                    );
                    self.builds += 1;
                }
                let entry = self
                    .cache
                    .get_mut(&line)
                    .expect("visible paragraph was inserted");
                entry.range = range.clone();
                entry.revision = buffer.revision();
                entry.presentation_revision = buffer.revision();
            }
            let entry = self
                .cache
                .get_mut(&line)
                .expect("visible paragraph was inserted");
            let paragraph_cursor = BytePos::new(if line == cursor_line {
                cursor.saturating_sub(range.start)
            } else {
                usize::MAX
            });
            let paragraph_selection = selection.as_ref().and_then(|selection| {
                if mode == Mode::VisualLine {
                    let is_selected = (selection.start <= range.start
                        && selection.end > range.start)
                        || (range.start == range.end && selection.contains(&range.start))
                        || range.contains(&selection.start);
                    is_selected.then(|| {
                        (
                            BytePos::new(0),
                            BytePos::new(range.end.saturating_sub(range.start)),
                        )
                    })
                } else {
                    let a = selection.start.max(range.start);
                    let b = selection.end.min(range.end);
                    (a < b).then(|| (BytePos::new(a - range.start), BytePos::new(b - range.start)))
                }
            });
            if entry.refresh(&painter, style, paragraph_cursor, paragraph_selection) {
                self.builds += 1;
            }
            if long && line == cursor_line && follow {
                let caret = entry.char_rect_at(egui::Pos2::ZERO, cursor);
                if caret.left() < self.horizontal {
                    self.horizontal = caret.left().max(0.0);
                }
                if caret.right() > self.horizontal + rect.width() - margin_x {
                    self.horizontal = (caret.right() + margin_x - rect.width()).max(0.0);
                }
            }
            self.heights.update_block_height(
                line,
                entry.galley.size().y.max(LINE_HEIGHT),
                Some(&mut anchor),
            );
        }
        self.scroll = anchor.scroll_offset_y;
        if scroll_pos.is_none()
            && follow
            && let Some(entry) = self.cache.get(&cursor_line)
        {
            let caret = entry.char_rect_at(
                egui::pos2(
                    0.0,
                    Self::TOP_MARGIN + self.heights.y_for_block(cursor_line),
                ),
                cursor,
            );
            if caret.bottom() > self.scroll + rect.height() {
                self.scroll = caret.bottom() - rect.height();
            } else if caret.top() < self.scroll {
                self.scroll = caret.top();
            }
        }
        self.scroll = self.scroll.clamp(
            0.0,
            (Self::TOP_MARGIN + self.heights.total_height() - rect.height()).max(0.0),
        );
        let paint_first = self
            .heights
            .block_at_y((self.scroll - Self::TOP_MARGIN).max(0.0))
            .0;
        let paint_last = (self
            .heights
            .block_at_y((self.scroll + rect.height() - Self::TOP_MARGIN).max(0.0))
            .0
            + 1)
        .min(lines);
        self.follow_pending = follow && (paint_first < first || paint_last > last);
        if paint_first < first || paint_last > last {
            ui.ctx().request_repaint();
        }
        if typography.show_inline_title
            && let Some(title) = note_title
        {
            let start0 = line_start(buffer, 0);
            let end0 = line_start(buffer, 1).min(buffer.byte_len());
            let first_line = buffer.slice(start0..end0);
            let first_is_h1 = first_line
                .chunks()
                .any(|c| c.trim_start().starts_with("# "));
            if !first_is_h1 {
                let title_y = rect.top() + Self::TOP_MARGIN - 34.0 - self.scroll;
                if title_y + 32.0 > rect.top() && title_y < rect.bottom() {
                    let title_pos = egui::pos2(rect.left() + margin_x, title_y);
                    let title_font = egui::FontId::new(
                        (typography.base_font_size * 1.6).round().max(24.0),
                        egui::FontFamily::Proportional,
                    );
                    let title_color = if theme.is_dark() {
                        egui::Color32::from_rgb(235, 203, 139)
                    } else {
                        egui::Color32::from_rgb(180, 120, 20)
                    };
                    painter.text(
                        title_pos,
                        egui::Align2::LEFT_TOP,
                        title,
                        title_font,
                        title_color,
                    );
                }
            }
        }
        for line in paint_first..paint_last {
            let Some(entry) = self.cache.get(&line) else {
                continue;
            };
            let origin = rect.left_top()
                + Vec2::new(
                    margin_x
                        - if entry.width.is_infinite() {
                            self.horizontal
                        } else {
                            0.0
                        },
                    Self::TOP_MARGIN + self.heights.y_for_block(line) - self.scroll,
                );
            let intersects_viewport = origin.y < rect.bottom()
                && origin.y + entry.galley.size().y.max(LINE_HEIGHT) > rect.top();
            if intersects_viewport {
                if self.painted_lines.is_empty() {
                    self.painted_lines.start = line;
                    self.visible.start = entry.range.start;
                }
                self.painted_lines.end = line + 1;
                self.visible.end = entry.range.end;
            }
            if intersects_viewport {
                let line_flash = if flash.is_empty() {
                    &[][..]
                } else {
                    let start_idx =
                        flash.partition_point(|t| t.source_range.start < entry.range.start);
                    let end_idx = flash[start_idx..]
                        .partition_point(|t| t.source_range.start < entry.range.end)
                        + start_idx;
                    &flash[start_idx..end_idx]
                };
                if let Some(caret) = entry.paint(
                    &painter,
                    origin,
                    &ParagraphView {
                        theme,
                        selection: selection.as_ref(),
                        cursor: (line == cursor_line).then_some(cursor),
                        mode,
                        preedit,
                        flash: line_flash,
                        content_width,
                    },
                ) {
                    self.cursor_rect = Some(caret);
                }
            }
            if intersects_viewport
                && (!self.input_blocked && (response.clicked() || response.dragged()))
                && let Some(position) = response.interact_pointer_pos()
                && let position = if response.dragged() {
                    rect.shrink(1.0).clamp(position)
                } else {
                    position
                }
                && position.y >= origin.y
                && position.y < origin.y + entry.galley.size().y.max(LINE_HEIGHT)
            {
                let index = entry.galley.cursor_from_pos(position - origin).index;
                let visual = entry
                    .display
                    .visual_text()
                    .char_indices()
                    .nth(index.0)
                    .map_or(entry.display.visual_text().len(), |(offset, _)| offset);
                if response.clicked()
                    && let Some(checkbox) = entry.display.task_at_visual(VisualPos::new(visual))
                {
                    clicked = Some(EditorInput::ToggleTask {
                        checkbox: entry.range.start + checkbox,
                    });
                    continue;
                }
                let source = entry.range.start
                    + entry
                        .display
                        .visual_to_source(VisualPos::new(visual))
                        .pos
                        .get();
                clicked = Some(EditorInput::SetSelection {
                    anchor: if response.dragged() {
                        view.anchor().get()
                    } else {
                        source
                    },
                    head: source,
                });
            }
        }
        if !self.input_blocked && clicked.is_none() && (response.clicked() || response.dragged()) {
            let end = buffer.byte_len();
            clicked = Some(EditorInput::SetSelection {
                anchor: if response.dragged() {
                    view.anchor().get()
                } else {
                    end
                },
                head: end,
            });
        }
        if mode == Mode::Insert
            && let Some(cursor_rect) = self.cursor_rect
        {
            ui.ctx().output_mut(|output| {
                output.ime = Some(egui::output::IMEOutput {
                    purpose: egui::IMEPurpose::Normal,
                    rect,
                    cursor_rect,
                    should_interrupt_composition: false,
                })
            });
        }
        let (top, _) = self
            .heights
            .block_at_y((self.scroll - Self::TOP_MARGIN).max(0.0));
        self.view_scrolls.insert(
            view.id(),
            ViewScroll {
                buffer_id: buffer.id(),
                revision: buffer.revision(),
                anchor_byte: line_start(buffer, top),
                local_y: self.scroll - Self::TOP_MARGIN - self.heights.y_for_block(top),
                horizontal: self.horizontal,
            },
        );
        clicked
    }
}

pub(super) fn line_count(buffer: &Buffer) -> usize {
    let trailing = buffer
        .rope()
        .chunks()
        .next_back()
        .is_some_and(|chunk| chunk.ends_with('\n'));
    (buffer.line_len() + usize::from(trailing)).max(1)
}

fn line_start(buffer: &Buffer, line: usize) -> usize {
    if line >= buffer.line_len() {
        buffer.byte_len()
    } else {
        buffer.byte_of_line(line)
    }
}

fn slice_is_table_row(slice: &truff_core::RopeSlice<'_>) -> bool {
    let mut non_ws = slice
        .chunks()
        .flat_map(str::bytes)
        .filter(|b| !b.is_ascii_whitespace());
    let Some(first) = non_ws.next() else {
        return false;
    };
    first == b'|' && non_ws.next_back() == Some(b'|')
}

fn slice_is_table_delimiter(slice: &truff_core::RopeSlice<'_>) -> bool {
    slice_is_table_row(slice)
        && slice
            .chunks()
            .flat_map(str::bytes)
            .all(|b| b.is_ascii_whitespace() || matches!(b, b'|' | b'-' | b':'))
}
