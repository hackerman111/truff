use std::borrow::Cow;

use super::{CommandContext, CommandId, CommandRegistry, CommandSpec};

impl Default for CommandRegistry {
    fn default() -> Self {
        let mut specs = Vec::new();
        for id in [
            "motion.left",
            "motion.right",
            "motion.up",
            "motion.down",
            "motion.first_line",
            "motion.last_line",
        ] {
            specs.push(spec(
                id,
                "motion",
                vec![CommandContext::Editor, CommandContext::Rows],
            ));
        }
        for id in [
            "motion.word_forward",
            "motion.word_backward",
            "motion.word_end",
            "motion.line_start",
            "motion.first_non_blank",
            "motion.line_end",
            "motion.screen_down",
            "motion.screen_up",
            "edit.delete_line",
            "edit.change_line",
            "edit.yank_line",
            "edit.delete_char",
            "edit.paste_after",
            "edit.paste_before",
            "edit.undo",
            "edit.redo",
            "edit.repeat",
            "edit.insert",
            "edit.append",
            "edit.insert_start",
            "edit.append_end",
            "edit.open_below",
            "edit.open_above",
            "edit.visual",
            "edit.visual_line",
            "edit.indent",
            "edit.dedent",
            "note.follow_link",
            "note.link_selection",
            "note.open_backlinks",
            "note.create",
            "note.open_target_heading",
            "style.text_color",
            "style.highlight_color",
            "style.wrap_callout",
            "style.pane_accent",
            "flash.jump_word",
            "flash.select_sentence",
            "flash.select_paragraph",
            "flash.select_syntax_object",
        ] {
            specs.push(spec(
                id,
                id.split('.').next().unwrap_or("command"),
                vec![CommandContext::Editor],
            ));
        }
        for id in [
            "buffer.open",
            "buffer.select",
            "buffer.next",
            "buffer.previous",
            "buffer.save",
            "buffer.close",
            "file.create",
            "folder.create",
            "file.rename",
            "file.move",
            "file.trash",
            "focus.left",
            "focus.right",
            "focus.up",
            "focus.down",
        ] {
            specs.push(spec(
                id,
                id.split('.').next().unwrap_or("command"),
                vec![CommandContext::Global],
            ));
        }
        Self { specs }
    }
}
fn spec(id: &'static str, group: &'static str, contexts: Vec<CommandContext>) -> CommandSpec {
    CommandSpec {
        id: CommandId::from_static(id),
        title: Cow::Borrowed(id),
        description: Cow::Borrowed(id),
        group: Cow::Borrowed(group),
        contexts,
    }
}
