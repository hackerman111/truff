use super::color;
use super::paragraph::{Paragraph, ParagraphStyle, ParagraphView};
use super::{EditorWidget, LINE_HEIGHT};
use crate::app::global_config::TypographyConfig;
use crate::editor::{Affinity, EditorInput, ProjectionMode, ScrollAnchor, VisualPos};
use crate::overlays::FlashTarget;
use crate::theme::Theme;
use eframe::egui::{self, Sense, Vec2};
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
    ) -> Option<EditorInput> {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, color(theme.bg_primary));
        self.cursor_rect = None;
        self.painted_lines = 0..0;
        self.visible = 0..0;
        let (Some(buffer), Some(view)) = (buffer, view) else {
            return None;
        };
        let (content_width, margin_x) = Self::calculate_column_metrics(rect.width());
        let cursor = view.head().get().min(buffer.byte_len());
        let cursor_line = buffer.line_of_byte(cursor);
        let lines = line_count(buffer);
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
        if cursor_changed {
            if self.last_cursor.is_none_or(|(id, _)| id != view.id()) {
                self.scroll = self.heights.y_for_block(view.scroll_top_line());
                self.cache.clear();
                self.horizontal = 0.0;
            }
            let y = Self::TOP_MARGIN + self.heights.y_for_block(cursor_line);
            if y < self.scroll {
                self.scroll = y;
            }
            if y + LINE_HEIGHT > self.scroll + rect.height() {
                self.scroll = y + LINE_HEIGHT - rect.height();
            }
            self.last_cursor = Some(identity);
        }
        if response.hovered() {
            self.scroll_by(-ui.input(|input| input.smooth_scroll_delta.y));
            self.horizontal =
                (self.horizontal - ui.input(|input| input.smooth_scroll_delta.x)).max(0.0);
        }
        self.scroll = self
            .scroll
            .clamp(0.0, (self.heights.total_height() - rect.height()).max(0.0));
        let (top, local_y) = self.heights.block_at_y(self.scroll);
        let mut anchor = ScrollAnchor::new(top, local_y, self.scroll);
        let overscan = LINE_HEIGHT * 4.0;
        let first = self.heights.block_at_y((self.scroll - overscan).max(0.0)).0;
        let last = (self
            .heights
            .block_at_y(self.scroll + rect.height() + overscan)
            .0
            + 1)
        .min(lines);
        self.cache.retain(|line, _| *line >= first && *line < last);
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
        for line in first..last {
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
                && matches!(
                    buffer.slice(range.clone()).chars().next_back(),
                    Some('\n' | '\r')
                )
            {
                range.end -= 1;
            }
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
                    list.range.start -= range.start;
                    list.range.end -= range.start;
                    if let truff_core::markdown::presentation::ListKind::Task { checkbox, .. } =
                        &mut list.kind
                    {
                        *checkbox -= range.start;
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
            if line == cursor_line && cursor_changed {
                let visual = entry
                    .display
                    .source_to_visual(
                        BytePos::new(cursor.saturating_sub(range.start)),
                        Affinity::Right,
                    )
                    .get();
                let chars = entry.display.visual_text()
                    [..visual.min(entry.display.visual_text().len())]
                    .chars()
                    .count();
                let x = entry
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(chars))
                    .min
                    .x
                    + entry.text_indent;
                if x < self.horizontal {
                    self.horizontal = x;
                }
                if x + 24.0 > self.horizontal + rect.width() {
                    self.horizontal = (x + 24.0 - rect.width()).max(0.0);
                }
            }
            self.heights.update_block_height(
                line,
                entry.galley.size().y.max(LINE_HEIGHT),
                Some(&mut anchor),
            );
            self.scroll = anchor.scroll_offset_y;
            let origin = rect.left_top()
                + Vec2::new(
                    margin_x + entry.text_indent - if long { self.horizontal } else { 0.0 },
                    Self::TOP_MARGIN + self.heights.y_for_block(line) - self.scroll,
                );
            let intersects_viewport = origin.y < rect.bottom()
                && origin.y + entry.galley.size().y.max(LINE_HEIGHT) > rect.top();
            if intersects_viewport {
                if self.painted_lines.is_empty() {
                    self.painted_lines.start = line;
                    self.visible.start = range.start;
                }
                self.painted_lines.end = line + 1;
                self.visible.end = range.end;
            }
            if intersects_viewport
                && let Some(caret) = entry.paint(
                    &painter,
                    origin,
                    &ParagraphView {
                        theme,
                        selection: selection.as_ref(),
                        cursor: (line == cursor_line).then_some(cursor),
                        mode,
                        preedit,
                        flash,
                        content_width,
                    },
                )
            {
                self.cursor_rect = Some(caret);
                if cursor_changed {
                    let adjustment = if caret.bottom() > rect.bottom() {
                        caret.bottom() - rect.bottom()
                    } else if caret.top() < rect.top() {
                        caret.top() - rect.top()
                    } else {
                        0.0
                    };
                    if adjustment != 0.0 {
                        self.scroll = (self.scroll + adjustment).max(0.0);
                        ui.ctx().request_repaint();
                    }
                }
            }
            if intersects_viewport
                && (response.clicked() || response.dragged())
                && let Some(position) = response.interact_pointer_pos()
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
                        checkbox: range.start + checkbox,
                    });
                    continue;
                }
                let source = range.start
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
        if clicked.is_none() && (response.clicked() || response.dragged()) {
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
