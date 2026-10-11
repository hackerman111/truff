use super::{EditorWidget, LINE_HEIGHT};
use crate::editor::{Affinity, VisualPos};
use eframe::egui;
use truff_core::{Buffer, text::BytePos, workspace::View};

impl EditorWidget {
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
