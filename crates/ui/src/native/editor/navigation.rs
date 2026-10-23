use super::{EditorWidget, LINE_HEIGHT};
use crate::editor::{Affinity, VisualPos};
use truff_core::{Buffer, text::BytePos, workspace::View};

impl EditorWidget {
    pub(in crate::native) fn page_target(
        &self,
        buffer: &Buffer,
        view: &View,
        down: bool,
        half: bool,
    ) -> usize {
        let line = buffer.line_of_byte(view.head().get());
        let local = self.cache.get(&line).map_or(0.0, |entry| {
            entry
                .char_rect_at(egui::Pos2::ZERO, view.head().get())
                .center()
                .y
        });
        let height = self.last_viewport.map_or(LINE_HEIGHT, |rect| rect.height());
        let distance = if half { height / 2.0 } else { height };
        let y = self.heights.y_for_block(line) + local + if down { distance } else { -distance };
        let (target_line, local) = self.heights.block_at_y(y.max(0.0));
        let Some(entry) = self.cache.get(&target_line) else {
            return if target_line >= buffer.line_len() {
                buffer.byte_len()
            } else {
                buffer.byte_of_line(target_line)
            };
        };
        let index = entry.galley.cursor_from_pos(egui::vec2(0.0, local)).index.0;
        let visual = entry
            .display
            .visual_text()
            .char_indices()
            .nth(index)
            .map_or(entry.display.visual_text().len(), |(byte, _)| byte);
        entry.range.start
            + entry
                .display
                .visual_to_source(VisualPos::new(visual))
                .pos
                .get()
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
}
