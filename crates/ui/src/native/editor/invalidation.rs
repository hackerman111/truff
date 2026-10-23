use super::EditorWidget;
use truff_core::{Buffer, workspace::BufferChange};

impl EditorWidget {
    /// Font data changes invalidate galleys independently of the compact typography key.
    pub(in crate::native) fn invalidate_fonts(&mut self) {
        self.cache.clear();
    }

    /// Rebase cached rows using the unchanged prefix/suffix of the edit summary.
    pub(in crate::native) fn apply_change(&mut self, buffer: &Buffer, change: &BufferChange) {
        if change.buffer_id != buffer.id() || change.revision != buffer.revision() {
            return;
        }
        let byte_delta = change.new_range.len() as isize - change.old_range.len() as isize;
        self.view_scrolls.retain(|_, saved| {
            if saved.buffer_id != buffer.id() {
                return true;
            }
            if saved.revision.checked_add(change.edit_batches) != Some(change.revision)
                || change.invalidate_suffix
            {
                return false;
            }
            // Only the unchanged suffix has an exact mapping in a merged change summary.
            saved.anchor_byte = if saved.anchor_byte >= change.old_range.end {
                saved.anchor_byte.saturating_add_signed(byte_delta)
            } else if saved.anchor_byte >= change.old_range.start {
                saved.local_y = 0.0;
                change.new_range.start
            } else {
                saved.anchor_byte
            }
            .min(buffer.byte_len());
            saved.revision = buffer.revision();
            true
        });
        self.syntax.apply_change(buffer, change);
        if self.buffer_id != Some(buffer.id())
            || change.buffer_id != buffer.id()
            || change.revision != buffer.revision()
            || self
                .revision
                .and_then(|revision| revision.checked_add(change.edit_batches))
                != Some(change.revision)
            || change.invalidate_suffix
        {
            return;
        }
        let old_count = self.heights.block_count();
        let new_count = super::viewport::line_count(buffer);
        let line_delta = new_count as isize - old_count as isize;
        if change.new_range.end > buffer.byte_len() {
            return;
        }
        let first = buffer.line_of_byte(change.new_range.start);
        let new_last = buffer.line_of_byte(change.new_range.end);
        let Some(old_last) = new_last.checked_add_signed(-line_delta) else {
            return;
        };
        if first > old_last || old_last >= old_count || new_last >= new_count {
            return;
        }
        let old_top = self
            .heights
            .block_at_y((self.scroll - Self::TOP_MARGIN).max(0.0));
        let local_y = self.scroll - Self::TOP_MARGIN - self.heights.y_for_block(old_top.0);
        self.heights
            .replace_blocks(first..old_last + 1, new_last - first + 1);
        let new_top = if old_top.0 > old_last {
            old_top.0.checked_add_signed(line_delta).unwrap_or(first)
        } else if old_top.0 >= first {
            first
        } else {
            old_top.0
        };
        self.scroll = (Self::TOP_MARGIN + self.heights.y_for_block(new_top) + local_y).max(0.0);
        if line_delta == 0 {
            self.cache.retain(|&line, paragraph| {
                if line >= first && line <= old_last {
                    return false;
                }
                if line > old_last {
                    paragraph.range = paragraph.range.start.saturating_add_signed(byte_delta)
                        ..paragraph.range.end.saturating_add_signed(byte_delta);
                }
                paragraph.revision = buffer.revision();
                true
            });
            self.revision = Some(buffer.revision());
            return;
        }
        let mut rebased = std::collections::BTreeMap::new();
        for (line, mut paragraph) in std::mem::take(&mut self.cache) {
            if line >= first && line <= old_last {
                continue;
            }
            let line = if line > old_last {
                paragraph.range = paragraph.range.start.saturating_add_signed(byte_delta)
                    ..paragraph.range.end.saturating_add_signed(byte_delta);
                line.saturating_add_signed(line_delta)
            } else {
                line
            };
            paragraph.revision = buffer.revision();
            rebased.insert(line, paragraph);
        }
        self.cache = rebased;
        self.revision = Some(buffer.revision());
    }
}

#[cfg(test)]
mod tests {
    use crate::{app::Application, native::NativeApp};
    use truff_core::{Transaction, commands::Action};

    fn frame(app: &mut NativeApp, context: &egui::Context) {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1100.0, 700.0),
                )),
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
        let buffer = app.application.shell.editor_buffer().unwrap();
        for (&line, paragraph) in &app.editor.cache {
            assert_eq!(
                paragraph.display.source_text(),
                buffer.slice(paragraph.range.clone()).to_string()
            );
            let start = if line < buffer.line_len() {
                buffer.byte_of_line(line)
            } else {
                buffer.byte_len()
            };
            assert_eq!(paragraph.range.start, start, "line {line}");
            assert_eq!(paragraph.revision, buffer.revision());
        }
    }

    #[test]
    fn sequential_edits_newlines_undo_and_unannounced_replacement_keep_cached_sources_exact() {
        let context = egui::Context::default();
        let text: String = (0..1_000).map(|i| format!("line {i} 字🦀\n")).collect();
        let mut application = Application::new();
        application.shell.set_editor_text(&text);
        let mut app = NativeApp::new(application, &context);
        frame(&mut app, &context);
        let view = app.application.shell.workspace().active_view().unwrap();
        let edit = |app: &mut NativeApp, transaction| {
            app.application
                .shell
                .workspace_mut()
                .dispatch(Action::ApplyEdit {
                    view_id: view,
                    transaction,
                })
                .unwrap();
        };
        let builds = app.editor.layout_rebuild_count();
        edit(&mut app, Transaction::insert(0, "新しい\n"));
        edit(&mut app, Transaction::insert(20, "several\nrows\n"));
        frame(&mut app, &context);
        assert!(app.editor.layout_rebuild_count() - builds < 12);
        let end = app
            .application
            .shell
            .editor_buffer()
            .unwrap()
            .byte_of_line(3);
        edit(&mut app, Transaction::delete(0..end));
        frame(&mut app, &context);
        for action in [Action::Undo(view), Action::Redo(view)] {
            app.application
                .shell
                .workspace_mut()
                .dispatch(action)
                .unwrap();
            frame(&mut app, &context);
        }
        let id = app.application.shell.editor_view().unwrap().buffer_id();
        let len = app.application.shell.editor_buffer().unwrap().byte_len();
        app.application
            .shell
            .workspace_mut()
            .buffers_mut()
            .get_buffer_mut(id)
            .unwrap()
            .apply(Transaction::replace(0..len, "replacement 字\nsecond 🦀\n"))
            .unwrap();
        frame(&mut app, &context);
    }

    #[test]
    fn an_unannounced_edit_followed_by_a_published_edit_rebuilds_stale_rows() {
        let context = egui::Context::default();
        let mut application = Application::new();
        application
            .shell
            .set_editor_text(&"line text\n".repeat(1_000));
        let mut app = NativeApp::new(application, &context);
        frame(&mut app, &context);
        let view = app.application.shell.editor_view().unwrap().id();
        let id = app.application.shell.editor_buffer().unwrap().id();
        app.application
            .shell
            .workspace_mut()
            .buffers_mut()
            .get_buffer_mut(id)
            .unwrap()
            .apply(Transaction::replace(0..4, "EDIT"))
            .unwrap();
        let len = app.application.shell.editor_buffer().unwrap().byte_len();
        app.application
            .shell
            .workspace_mut()
            .dispatch(Action::ApplyEdit {
                view_id: view,
                transaction: Transaction::insert(len, "last line"),
            })
            .unwrap();
        frame(&mut app, &context);
    }
}
