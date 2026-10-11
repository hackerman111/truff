//! Rope-backed viewport rendering. Retained text and glyphs are bounded by the viewport.
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use eframe::egui::{self, Color32, FontId, Galley, Rect, Sense, Vec2};
use truff_core::{Buffer, text::BytePos, vim::Mode, workspace::View};

use crate::editor::{
    Affinity, DisplayMap, EditorInput, HeightIndex, ProjectionMode, ScrollAnchor, VisualPos,
};
use crate::overlays::FlashTarget;
use crate::theme::{Color, Theme};

const LINE_HEIGHT: f32 = 24.0;
const MAX_LINE_BYTES: usize = 4096;

struct Paragraph {
    range: Range<usize>,
    display: DisplayMap,
    galley: Arc<Galley>,
    color: Color32,
    pixels_per_point: f32,
    revision: u64,
    width: f32,
}

#[derive(Default)]
pub struct EditorWidget {
    cache: BTreeMap<usize, Paragraph>,
    scroll: f32,
    horizontal: f32,
    visible: Range<usize>,
    cursor_rect: Option<Rect>,
    last_cursor: Option<(truff_core::workspace::ViewId, usize)>,
    builds: usize,
    heights: HeightIndex,
    buffer_id: Option<truff_core::text::BufferId>,
}

pub fn color(value: Color) -> Color32 {
    let [r, g, b, a] = value.to_rgba_u8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

impl EditorWidget {
    pub const TOP_MARGIN: f32 = 48.0;

    /// Calculates the constrained readable column width and centered horizontal margin.
    pub fn calculate_column_metrics(available_width: f32) -> (f32, f32) {
        let content_width = (available_width - 48.0).clamp(320.0, 720.0);
        let margin_x = ((available_width - content_width) / 2.0).max(24.0);
        (content_width, margin_x)
    }

    pub fn visible_source_range(&self) -> Range<usize> {
        self.visible.clone()
    }
    pub fn visible_source_windows(&self) -> Vec<Range<usize>> {
        self.cache
            .values()
            .map(|entry| entry.range.clone())
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
        self.heights.block_at_y(self.scroll).0
    }
    pub fn cursor_rect(&self) -> Option<Rect> {
        self.cursor_rect
    }

    pub fn visual_target(
        &self,
        buffer: &Buffer,
        view: &View,
        down: bool,
        count: usize,
    ) -> Option<usize> {
        let cursor = view.head().get();
        let line = buffer.line_of_byte(cursor);
        let entry = self.cache.get(&line)?;
        let visual = entry
            .display
            .source_to_visual(
                BytePos::new(cursor.saturating_sub(entry.range.start)),
                Affinity::Right,
            )
            .get();
        let chars = entry.display.visual_text()[..visual].chars().count();
        let caret = entry
            .galley
            .pos_from_cursor(egui::text::CCursor::new(chars));
        let y = self.heights.y_for_block(line)
            + caret.center().y
            + if down {
                count as f32 * LINE_HEIGHT
            } else {
                -(count as f32) * LINE_HEIGHT
            };
        let (target_line, local_y) = self.heights.block_at_y(y.max(0.0));
        let target = self.cache.get(&target_line)?;
        let index = target
            .galley
            .cursor_from_pos(egui::vec2(caret.left(), local_y))
            .index
            .0;
        let visual = target
            .display
            .visual_text()
            .char_indices()
            .nth(index)
            .map_or(target.display.visual_text().len(), |(byte, _)| byte);
        Some(
            target.range.start
                + target
                    .display
                    .visual_to_source(VisualPos::new(visual))
                    .pos
                    .get(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        buffer: Option<&Buffer>,
        view: Option<&View>,
        mode: Mode,
        projection: ProjectionMode,
        theme: &Theme,
        preedit: Option<&str>,
        flash: &[FlashTarget],
    ) -> Option<EditorInput> {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, color(theme.bg_primary));
        self.cursor_rect = None;
        let (Some(buffer), Some(view)) = (buffer, view) else {
            return None;
        };
        let (content_width, margin_x) = Self::calculate_column_metrics(rect.width());
        let cursor = view.head().get().min(buffer.byte_len());
        let cursor_line = buffer.line_of_byte(cursor);
        let trailing_newline = buffer
            .rope()
            .chunks()
            .next_back()
            .is_some_and(|chunk| chunk.ends_with('\n'));
        let lines = (buffer.line_len() + usize::from(trailing_newline)).max(1);
        if self.buffer_id != Some(buffer.id()) || self.heights.block_count() != lines {
            self.heights.init_uniform(lines);
            self.buffer_id = Some(buffer.id());
            self.cache.clear();
        }
        let identity = (view.id(), cursor);
        let cursor_changed = self.last_cursor != Some(identity);
        if cursor_changed {
            if self.last_cursor.is_none_or(|(id, _)| id != view.id()) {
                self.scroll = self.heights.y_for_block(view.scroll_top_line());
                self.cache.clear();
                self.horizontal = 0.0;
            }
            let y = self.heights.y_for_block(cursor_line);
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
        let first = self
            .heights
            .block_at_y((self.scroll - rect.height()).max(0.0))
            .0;
        let last = (self.heights.block_at_y(self.scroll + rect.height() * 2.0).0 + 1).min(lines);
        self.cache.retain(|line, _| *line >= first && *line < last);
        self.visible =
            line_start(buffer, first.min(lines - 1))..line_start(buffer, last.min(lines));
        let foreground = color(theme.text_primary);
        let selection = (view.anchor().get() != cursor)
            .then(|| view.anchor().get().min(cursor)..view.anchor().get().max(cursor));
        let mut clicked = None;
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
            let changed = self.cache.get(&line).is_none_or(|entry| {
                (entry.range != range || entry.revision != buffer.revision())
                    && !slice
                        .chunks()
                        .flat_map(str::bytes)
                        .eq(entry.display.source_text().bytes())
            });
            if changed {
                let source = slice.to_string();
                let display = DisplayMap::new(
                    &source,
                    if long {
                        ProjectionMode::Source
                    } else {
                        projection
                    },
                );
                let galley = super::paint::shape(&painter, &display, foreground, width);
                self.cache.insert(
                    line,
                    Paragraph {
                        range: range.clone(),
                        display,
                        galley,
                        color: foreground,
                        pixels_per_point: ui.ctx().pixels_per_point(),
                        revision: buffer.revision(),
                        width,
                    },
                );
                self.builds += 1;
                eprintln!("REBUILD at line {}", line);
                eprintln!("BUILD at line {}", line);
            }
            let entry = self
                .cache
                .get_mut(&line)
                .expect("visible paragraph was inserted");
            entry.range = range.clone();
            entry.revision = buffer.revision();
            let paragraph_cursor = BytePos::new(if line == cursor_line {
                cursor.saturating_sub(range.start)
            } else {
                usize::MAX
            });
            let paragraph_selection = selection.as_ref().and_then(|selection| {
                let a = selection.start.max(range.start);
                let b = selection.end.min(range.end);
                (a < b).then(|| (BytePos::new(a - range.start), BytePos::new(b - range.start)))
            });
            let view_changed = !entry
                .display
                .matches_view(paragraph_cursor, paragraph_selection)
                || entry.display.mode()
                    != if long {
                        ProjectionMode::Source
                    } else {
                        projection
                    };
            let old_cursor_projection =
                view_changed.then(|| entry.display.visual_text().to_owned());
            entry.display.set_mode(if long {
                ProjectionMode::Source
            } else {
                projection
            });
            entry.display.set_cursor(paragraph_cursor);
            entry.display.set_selection(paragraph_selection);

            let c1 = old_cursor_projection
                .as_deref()
                .is_some_and(|old| old != entry.display.visual_text());
            let c2 = entry.color != foreground;
            let c3 = entry.pixels_per_point != ui.ctx().pixels_per_point();
            let c4 = entry.width != width;
            if c1 || c2 || c3 || c4 {
                eprintln!(
                    "line {}: c1(proj)={} c2(col)={} c3(ppp)={} c4(w)={} (entry.width={}, width={})",
                    line, c1, c2, c3, c4, entry.width, width
                );
            }
            if c1 || c2 || c3 || c4 {
                entry.galley = super::paint::shape(&painter, &entry.display, foreground, width);
                entry.color = foreground;
                entry.pixels_per_point = ui.ctx().pixels_per_point();
                entry.width = width;
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
                    .x;
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
                    margin_x - if long { self.horizontal } else { 0.0 },
                    Self::TOP_MARGIN + self.heights.y_for_block(line) - self.scroll,
                );
            let caret_at = |source: usize| {
                let visual = entry
                    .display
                    .source_to_visual(
                        BytePos::new(source.saturating_sub(range.start)),
                        Affinity::Right,
                    )
                    .get();
                let chars = entry.display.visual_text()
                    [..visual.min(entry.display.visual_text().len())]
                    .chars()
                    .count();
                entry
                    .galley
                    .pos_from_cursor(egui::text::CCursor::new(chars))
                    .translate(origin.to_vec2())
            };
            if let Some(selection) = &selection {
                let a = selection.start.max(range.start);
                let b = selection.end.min(range.end);
                if a < b {
                    painter.rect_filled(
                        Rect::from_min_max(caret_at(a).left_top(), caret_at(b).right_bottom()),
                        2.0,
                        color(theme.selection),
                    );
                }
            }
            painter.galley(origin, entry.galley.clone(), foreground);
            if line == cursor_line && (range.contains(&cursor) || cursor == range.end) {
                let mut caret = caret_at(cursor);
                caret.max.x = caret.min.x + if mode == Mode::Insert { 1.5 } else { 8.5 };
                painter.rect_filled(
                    caret,
                    1.5,
                    color(theme.cursor).gamma_multiply(if mode == Mode::Insert {
                        1.0
                    } else {
                        0.45
                    }),
                );
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
                if let Some(text) = preedit {
                    let galley = painter.layout_no_wrap(
                        text.to_owned(),
                        FontId::proportional(15.0),
                        foreground,
                    );
                    painter.galley(caret.left_top(), galley, foreground);
                }
            }
            for target in flash
                .iter()
                .filter(|target| range.contains(&target.source_range.start))
            {
                let position = caret_at(target.source_range.start).left_top();
                let badge = painter.layout_no_wrap(
                    target.label.clone(),
                    FontId::monospace(14.0),
                    color(theme.flash_fg),
                );
                painter.rect_filled(
                    Rect::from_min_size(position, badge.size()),
                    3.0,
                    color(theme.flash_bg),
                );
                painter.galley(position, badge, color(theme.flash_fg));
            }
            if (response.clicked() || response.dragged())
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

fn line_start(buffer: &Buffer, line: usize) -> usize {
    if line >= buffer.line_len() {
        buffer.byte_len()
    } else {
        buffer.byte_of_line(line)
    }
}
