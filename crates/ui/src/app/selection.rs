use super::{AppResult, Application};
use crate::overlays::text_action::TaskStatus;
use std::ops::Range;
use truff_core::{
    BufferId, ViewId,
    markdown::{
        CalloutKind, LinkSelection,
        style::{apply_highlight, apply_text_color, remove_style, wrap_callout},
    },
};

#[derive(Clone, Debug)]
pub(crate) struct SelectionTarget {
    pub(crate) view: ViewId,
    pub(crate) buffer: BufferId,
    pub(crate) revision: u64,
    pub(crate) range: Range<usize>,
}

impl Application {
    pub(crate) fn capture_selection(&self) -> AppResult<SelectionTarget> {
        let range = self.selection_range()?;
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self
            .shell
            .editor_buffer()
            .ok_or("Selection buffer unavailable")?;
        Ok(SelectionTarget {
            view: view.id(),
            buffer: buffer.id(),
            revision: buffer.revision(),
            range,
        })
    }

    pub(crate) fn capture_selection_or_cursor(&self) -> AppResult<(SelectionTarget, bool)> {
        if let Ok(target) = self.capture_selection() {
            return Ok((target, true));
        }
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self
            .shell
            .editor_buffer()
            .ok_or("Selection buffer unavailable")?;
        let cursor = view.head().get();
        if let Some(range) = crate::overlays::word_boundary_at(buffer, cursor) {
            return Ok((
                SelectionTarget {
                    view: view.id(),
                    buffer: buffer.id(),
                    revision: buffer.revision(),
                    range,
                },
                true,
            ));
        }
        Ok((
            SelectionTarget {
                view: view.id(),
                buffer: buffer.id(),
                revision: buffer.revision(),
                range: cursor..cursor,
            },
            false,
        ))
    }

    pub(crate) fn target_text(&self, target: &SelectionTarget) -> AppResult<String> {
        let buffer = self.selection_buffer(target)?;
        Ok(buffer.slice(target.range.clone()).to_string())
    }

    fn selection_buffer(&self, target: &SelectionTarget) -> AppResult<&truff_core::Buffer> {
        let view = self
            .shell
            .workspace()
            .get_view(target.view)
            .ok_or("Selection View was closed")?;
        if view.buffer_id() != target.buffer {
            return Err("Selection target changed".into());
        }
        let buffer = self
            .shell
            .workspace()
            .buffers()
            .get_buffer(target.buffer)
            .ok_or("Selection buffer unavailable")?;
        if buffer.revision() != target.revision {
            return Err("Selection changed; open the picker again".into());
        }
        Ok(buffer)
    }

    pub(crate) fn color_target(&mut self, target: SelectionTarget, color: &str) -> AppResult<()> {
        let hex = color.strip_prefix('#').unwrap_or(color);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Usage: :color #RRGGBB".into());
        }
        let buffer = self.selection_buffer(&target)?;
        let transaction = apply_text_color(&buffer.as_str(), target.range.clone(), color)?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn highlight_target(
        &mut self,
        target: SelectionTarget,
        color: &str,
    ) -> AppResult<()> {
        let hex = color.strip_prefix('#').unwrap_or(color);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Usage: :highlight #RRGGBB".into());
        }
        let buffer = self.selection_buffer(&target)?;
        let transaction = apply_highlight(&buffer.as_str(), target.range.clone(), color)?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn callout_target(
        &mut self,
        target: SelectionTarget,
        kind: CalloutKind,
    ) -> AppResult<()> {
        let buffer = self.selection_buffer(&target)?;
        let transaction = wrap_callout(&buffer.as_str(), target.range.clone(), kind)?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn remove_style_target(&mut self, target: SelectionTarget) -> AppResult<()> {
        let buffer = self.selection_buffer(&target)?;
        let transaction = remove_style(&buffer.as_str(), target.range.clone())?;
        self.apply_selection_edit(target, transaction)
    }

    pub(crate) fn link_target(&mut self, target: &SelectionTarget, note: &str) -> AppResult<()> {
        self.apply_note_link_target(target, note, None)
    }

    pub(crate) fn apply_note_link_target(
        &mut self,
        target: &SelectionTarget,
        note: &str,
        custom_title: Option<&str>,
    ) -> AppResult<()> {
        if note.is_empty() || note.contains(['[', ']', '\n', '\r']) {
            return Err("Usage: :link <note or note#heading>".into());
        }
        let buffer = self.selection_buffer(target)?;
        let label = match custom_title {
            Some(t) => t.to_string(),
            None => {
                if target.range.is_empty() {
                    String::new()
                } else {
                    buffer.slice(target.range.clone()).to_string()
                }
            }
        };
        let label_opt = (!label.is_empty() && label != note).then_some(label.as_str());
        let transaction = LinkSelection::wikilink(target.range.clone(), note, label_opt);
        self.apply_selection_edit(target.clone(), transaction)
    }

    pub(crate) fn apply_url_link_target(
        &mut self,
        target: &SelectionTarget,
        url: &str,
        custom_title: Option<&str>,
    ) -> AppResult<()> {
        if url.is_empty() {
            return Err("URL cannot be empty".into());
        }
        let buffer = self.selection_buffer(target)?;
        let label = match custom_title {
            Some(t) => t.to_string(),
            None => {
                if target.range.is_empty() {
                    url.to_string()
                } else {
                    buffer.slice(target.range.clone()).to_string()
                }
            }
        };
        let transaction = LinkSelection::markdown_link(target.range.clone(), url, &label);
        self.apply_selection_edit(target.clone(), transaction)
    }

    pub(crate) fn apply_task_target(
        &mut self,
        target: &SelectionTarget,
        status: TaskStatus,
    ) -> AppResult<()> {
        let buffer = self.selection_buffer(target)?;
        let first_line = buffer.line_of_byte(target.range.start);
        let end_pos =
            if target.range.end > target.range.start && target.range.end <= buffer.byte_len() {
                let mut boundary = target.range.end;
                if boundary > 0 && buffer.rope().byte(boundary - 1) == b'\n' {
                    boundary -= 1;
                    if boundary > 0 && buffer.rope().byte(boundary - 1) == b'\r' {
                        boundary -= 1;
                    }
                }
                boundary
            } else {
                target.range.end
            };
        let last_line = buffer.line_of_byte(end_pos);

        let range_start = buffer.byte_of_line(first_line);
        let range_end = if last_line + 1 < buffer.line_len() {
            buffer.byte_of_line(last_line + 1)
        } else {
            buffer.byte_len()
        };

        let mut new_text = String::new();
        for line_idx in first_line..=last_line {
            let start = buffer.byte_of_line(line_idx);
            let end = if line_idx + 1 < buffer.line_len() {
                buffer.byte_of_line(line_idx + 1)
            } else {
                buffer.byte_len()
            };
            let line_slice = buffer.slice(start..end).to_string();
            new_text.push_str(&apply_task_to_line(&line_slice, status));
        }

        let cursor_offset = match status {
            TaskStatus::Remove => range_start,
            _ => (range_start + 6).min(range_start + new_text.len()),
        };
        let cursor_pos = truff_core::text::BytePos::new(cursor_offset);
        let transaction = truff_core::Transaction::replace(range_start..range_end, new_text)
            .with_cursor(cursor_pos, cursor_pos);
        self.apply_selection_edit(target.clone(), transaction)
    }

    fn apply_selection_edit(
        &mut self,
        target: SelectionTarget,
        transaction: truff_core::Transaction,
    ) -> AppResult<()> {
        self.shell
            .workspace_mut()
            .set_active_view(Some(target.view));
        self.shell.set_focus(crate::shell::Focus::Editor);
        self.shell.apply_transaction(transaction)?;
        self.shell.set_mode(truff_core::vim::Mode::Normal);
        self.handle_shell_action(crate::shell::ShellAction::EditorEdit);
        Ok(())
    }
}

pub(crate) fn apply_task_to_line(line: &str, status: TaskStatus) -> String {
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let content = &line[..line.len() - newline.len()];
    let ws_len = content
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count();
    let ws = &content[..ws_len];
    let rest = &content[ws_len..];

    // Check if rest starts with a list bullet or task
    let bytes = rest.as_bytes();
    if bytes.len() >= 2
        && matches!(bytes[0], b'-' | b'*' | b'+')
        && matches!(bytes[1], b' ' | b'\t')
    {
        let after_bullet = &rest[2..];
        let bytes_after = after_bullet.as_bytes();
        // Check if it has a checkbox `[ ]`, `[x]`, `[/]`, `[-]`
        if bytes_after.len() >= 3
            && bytes_after[0] == b'['
            && matches!(bytes_after[1], b' ' | b'x' | b'X' | b'/' | b'-')
            && bytes_after[2] == b']'
        {
            let post_cb = if bytes_after.get(3).is_some_and(|&b| b == b' ' || b == b'\t') {
                4
            } else {
                3
            };
            let task_body = &after_bullet[post_cb..];
            if status == TaskStatus::Remove {
                return format!("{}{}{}", ws, task_body, newline);
            }
            let cb = status.checkbox_str().unwrap_or("[ ]");
            return format!("{}- {} {}{}", ws, cb, task_body, newline);
        }

        // Plain bullet `- body`
        if status == TaskStatus::Remove {
            return format!("{}{}{}", ws, after_bullet, newline);
        }
        let cb = status.checkbox_str().unwrap_or("[ ]");
        return format!("{}- {} {}{}", ws, cb, after_bullet, newline);
    }

    // Check if rest starts with ordered list e.g. `1. ` or `1) `
    let digit_count = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if digit_count > 0
        && bytes
            .get(digit_count)
            .is_some_and(|&b| b == b'.' || b == b')')
        && bytes
            .get(digit_count + 1)
            .is_some_and(|&b| b == b' ' || b == b'\t')
    {
        let task_body = &rest[digit_count + 2..];
        if status == TaskStatus::Remove {
            return format!("{}{}{}", ws, task_body, newline);
        }
        let cb = status.checkbox_str().unwrap_or("[ ]");
        return format!("{}- {} {}{}", ws, cb, task_body, newline);
    }

    // Empty / whitespace line
    if rest.is_empty() {
        if status == TaskStatus::Remove {
            return format!("{}{}", ws, newline);
        }
        let cb = status.checkbox_str().unwrap_or("[ ]");
        return format!("{}- {} {}", ws, cb, newline);
    }

    // Regular text line
    if status == TaskStatus::Remove {
        return line.to_string();
    }
    let cb = status.checkbox_str().unwrap_or("[ ]");
    format!("{}- {} {}{}", ws, cb, rest, newline)
}
