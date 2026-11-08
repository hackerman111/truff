use super::{HubTask, HubTaskOp, HubTaskStatus};

/// Parses a single markdown line as a checkbox task item (`- [ ] ...`, `- [/] ...`, `- [x] ...`).
/// Returns `(prefix_up_to_open_bracket, status, task_text)`.
pub fn parse_task_line(line: &str) -> Option<(&str, HubTaskStatus, &str)> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let trimmed = line.trim_start_matches([' ', '\t']);
    let indent_len = line.len() - trimmed.len();
    let rest = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))?;
    let bytes = rest.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'[' || bytes[2] != b']' {
        return None;
    }
    let status = match bytes[1] {
        b' ' => HubTaskStatus::Todo,
        b'/' | b'-' => HubTaskStatus::InProgress,
        b'x' | b'X' => HubTaskStatus::Done,
        _ => return None,
    };
    let after_bracket = &rest[3..];
    let text = if after_bracket.is_empty() {
        ""
    } else if let Some(stripped) = after_bracket.strip_prefix([' ', '\t']) {
        stripped.trim()
    } else {
        return None;
    };
    let prefix_end = indent_len + 2; // indent + "- "
    Some((&line[..prefix_end], status, text))
}

/// Parses all human-readable task lines (`- [ ] ...`, `- [/] ...`, `- [x] ...`) from an `index.md` file.
pub fn parse_index_tasks(markdown: &str) -> Vec<HubTask> {
    markdown
        .lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let (_, status, text) = parse_task_line(line)?;
            Some(HubTask {
                text: text.to_string(),
                status,
                line_index,
            })
        })
        .collect()
}

/// Parses tasks from inside ```` ```tasks ... ``` ```` fenced blocks in a note's markdown.
pub fn parse_fenced_file_tasks(markdown: &str) -> Vec<HubTask> {
    let mut tasks = Vec::new();
    let mut in_tasks_fence = false;
    for (line_index, raw_line) in markdown.lines().enumerate() {
        let trimmed = raw_line.trim();
        if !in_tasks_fence {
            if let Some(info) = trimmed
                .strip_prefix("```")
                .or_else(|| trimmed.strip_prefix("~~~"))
            {
                let tag = info.split_whitespace().next().unwrap_or("");
                if tag.eq_ignore_ascii_case("tasks") || tag.eq_ignore_ascii_case("task") {
                    in_tasks_fence = true;
                }
            }
            continue;
        }
        if trimmed == "```" || trimmed == "~~~" {
            in_tasks_fence = false;
            continue;
        }
        if let Some((_, status, text)) = parse_task_line(raw_line) {
            tasks.push(HubTask {
                text: text.to_string(),
                status,
                line_index,
            });
        }
    }
    tasks
}

/// Applies a [`HubTaskOp`] to the contents of an `index.md` file, preserving all other human-written content.
pub fn apply_task_op_to_index_text(markdown: &str, op: &HubTaskOp) -> String {
    let mut lines: Vec<String> = markdown
        .lines()
        .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
        .collect();
    match op {
        HubTaskOp::Add { text, .. } => {
            let new_line = format!("- [ ] {text}");
            if lines.is_empty() || markdown.trim().is_empty() {
                return format!("# Задачи\n\n{new_line}\n");
            }
            if let Some(last_task_idx) = lines
                .iter()
                .rposition(|line| parse_task_line(line).is_some())
            {
                lines.insert(last_task_idx + 1, new_line);
            } else {
                if !lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push(new_line);
            }
        }
        HubTaskOp::Edit {
            line_index, text, ..
        } => {
            if let Some(line) = lines.get_mut(*line_index)
                && let Some((prefix, status, _)) = parse_task_line(line)
            {
                *line = format!("{prefix}[{}] {text}", status.marker_char());
            }
        }
        HubTaskOp::SetStatus {
            line_index, status, ..
        } => {
            if let Some(line) = lines.get_mut(*line_index)
                && let Some((prefix, _, text)) = parse_task_line(line)
            {
                *line = format!("{prefix}[{}] {text}", status.marker_char());
            }
        }
        HubTaskOp::Delete { line_index, .. } => {
            if *line_index < lines.len() && parse_task_line(&lines[*line_index]).is_some() {
                lines.remove(*line_index);
            }
        }
    }
    let mut out = lines.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Applies a [`HubTaskOp`] to a note's markdown inside its ```` ```tasks ... ``` ```` fenced block
/// (creating the block at the end of the file if needed when adding a task).
pub fn apply_task_op_to_file_text(markdown: &str, op: &HubTaskOp) -> String {
    match op {
        HubTaskOp::Add { text, .. } => {
            let mut lines: Vec<String> = markdown
                .lines()
                .map(|l| l.strip_suffix('\r').unwrap_or(l).to_string())
                .collect();
            let new_line = format!("- [ ] {text}");
            let mut in_tasks_fence = false;
            let mut fence_close_idx = None;
            let mut fence_open_idx = None;
            for (idx, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if !in_tasks_fence {
                    if let Some(info) = trimmed
                        .strip_prefix("```")
                        .or_else(|| trimmed.strip_prefix("~~~"))
                    {
                        let tag = info.split_whitespace().next().unwrap_or("");
                        if tag.eq_ignore_ascii_case("tasks") || tag.eq_ignore_ascii_case("task") {
                            in_tasks_fence = true;
                            fence_open_idx = Some(idx);
                        }
                    }
                } else if trimmed == "```" || trimmed == "~~~" {
                    fence_close_idx = Some(idx);
                    break;
                }
            }
            if let Some(close_idx) = fence_close_idx {
                lines.insert(close_idx, new_line);
            } else if fence_open_idx.is_some() {
                lines.push(new_line);
                lines.push("```".to_string());
            } else {
                if !lines.is_empty() && !lines.last().is_some_and(|l| l.trim().is_empty()) {
                    lines.push(String::new());
                }
                lines.push("```tasks".to_string());
                lines.push(new_line);
                lines.push("```".to_string());
            }
            let mut out = lines.join("\n");
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out
        }
        HubTaskOp::Delete { .. } => {
            let after_delete = apply_task_op_to_index_text(markdown, op);
            if parse_fenced_file_tasks(&after_delete).is_empty() {
                let mut lines: Vec<String> = after_delete.lines().map(str::to_string).collect();
                let mut open_idx = None;
                let mut close_idx = None;
                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();
                    if open_idx.is_none() {
                        if let Some(info) = trimmed
                            .strip_prefix("```")
                            .or_else(|| trimmed.strip_prefix("~~~"))
                            && info.split_whitespace().next().is_some_and(|tag| {
                                tag.eq_ignore_ascii_case("tasks")
                                    || tag.eq_ignore_ascii_case("task")
                            })
                        {
                            open_idx = Some(idx);
                        }
                    } else if trimmed == "```" || trimmed == "~~~" {
                        close_idx = Some(idx);
                        break;
                    } else if !trimmed.is_empty() {
                        // Non-empty non-task line remains inside fence; keep block
                        return after_delete;
                    }
                }
                if let (Some(open), Some(close)) = (open_idx, close_idx) {
                    lines.drain(open..=close);
                    while lines.last().is_some_and(|l| l.trim().is_empty()) {
                        lines.pop();
                    }
                    let mut out = lines.join("\n");
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    return out;
                }
            }
            after_delete
        }
        _ => apply_task_op_to_index_text(markdown, op),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigator::{
        HubTaskCategory, NavKey, NavigatorAction, NavigatorMode, NavigatorState,
    };

    #[test]
    fn parse_and_mutate_index_tasks_preserves_surrounding_markdown() {
        let content = "# Индекс проекта\n\nЗаметки по проекту.\n\n- [ ] Первая задача\n- [/] В работе\n- [x] Готово\n\nКонец файла.\n";
        let tasks = parse_index_tasks(content);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0].text, "Первая задача");
        assert_eq!(tasks[0].status, HubTaskStatus::Todo);
        assert_eq!(tasks[1].status, HubTaskStatus::InProgress);
        assert_eq!(tasks[2].status, HubTaskStatus::Done);

        // Toggle status of first task
        let toggled = apply_task_op_to_index_text(
            content,
            &HubTaskOp::SetStatus {
                category: HubTaskCategory::Project,
                line_index: tasks[0].line_index,
                status: HubTaskStatus::InProgress,
            },
        );
        assert!(toggled.contains("- [/] Первая задача\n"));
        assert!(toggled.contains("Заметки по проекту.\n"));
        assert!(toggled.contains("Конец файла.\n"));

        // Add a new task right after existing tasks
        let added = apply_task_op_to_index_text(
            &toggled,
            &HubTaskOp::Add {
                category: HubTaskCategory::Project,
                text: "Новая задача".to_string(),
            },
        );
        assert!(added.contains("- [x] Готово\n- [ ] Новая задача\n\nКонец файла.\n"));

        // Edit task text
        let edited = apply_task_op_to_index_text(
            &added,
            &HubTaskOp::Edit {
                category: HubTaskCategory::Project,
                line_index: tasks[1].line_index,
                text: "Обновлённая задача".to_string(),
            },
        );
        assert!(edited.contains("- [/] Обновлённая задача\n"));

        // Delete task
        let deleted = apply_task_op_to_index_text(
            &edited,
            &HubTaskOp::Delete {
                category: HubTaskCategory::Project,
                line_index: tasks[0].line_index,
            },
        );
        assert!(!deleted.contains("Первая задача"));
    }

    #[test]
    fn parse_and_mutate_fenced_file_tasks() {
        let note = "# Заметка\n\n- [ ] Обычный чекбокс вне окружения\n\n```tasks\n- [ ] Задача внутри окружения\n- [x] Закрытая задача\n```\n";
        let tasks = parse_fenced_file_tasks(note);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].text, "Задача внутри окружения");
        assert_eq!(tasks[1].text, "Закрытая задача");

        // Adding a task inserts before closing ```
        let with_new = apply_task_op_to_file_text(
            note,
            &HubTaskOp::Add {
                category: HubTaskCategory::File,
                text: "Третья задача".to_string(),
            },
        );
        let reparsed = parse_fenced_file_tasks(&with_new);
        assert_eq!(reparsed.len(), 3);
        assert_eq!(reparsed[2].text, "Третья задача");

        // Adding a task to a note without a ```tasks block creates one
        let plain_note = "# Заголовок\n\nТекст заметки.\n";
        let created = apply_task_op_to_file_text(
            plain_note,
            &HubTaskOp::Add {
                category: HubTaskCategory::File,
                text: "Первая задача файла".to_string(),
            },
        );
        assert!(created.contains("```tasks\n- [ ] Первая задача файла\n```\n"));
        let created_tasks = parse_fenced_file_tasks(&created);
        assert_eq!(created_tasks.len(), 1);

        // Deleting the last task removes the empty ```tasks block
        let cleaned = apply_task_op_to_file_text(
            &created,
            &HubTaskOp::Delete {
                category: HubTaskCategory::File,
                line_index: created_tasks[0].line_index,
            },
        );
        assert!(!cleaned.contains("```tasks"));
        assert_eq!(cleaned, plain_note);
    }

    #[test]
    fn outline_hub_deletes_tasks_without_manual_focus_switch() {
        let mut nav = NavigatorState::new();
        nav.open_mode(NavigatorMode::Outline, "Note.md", Vec::new());
        nav.hub_tasks.set_lists(
            Vec::new(),
            vec![
                HubTask {
                    text: "Первая".into(),
                    status: HubTaskStatus::Todo,
                    line_index: 4,
                },
                HubTask {
                    text: "Вторая".into(),
                    status: HubTaskStatus::InProgress,
                    line_index: 5,
                },
            ],
            Vec::new(),
        );
        // Active category auto-selected File because Vault was empty
        assert_eq!(nav.hub_tasks.category(), HubTaskCategory::File);
        assert!(!nav.hub_tasks.focus_tasks);

        // Pressing 'd' directly in Outline mode focuses tasks and deletes the first task
        let action = nav.handle_key_event(NavKey::Char('d'), false);
        assert_eq!(
            action,
            NavigatorAction::HubTask(HubTaskOp::Delete {
                category: HubTaskCategory::File,
                line_index: 4,
            })
        );
        assert_eq!(nav.hub_tasks.active_tasks().len(), 1);

        // Pressing Delete removes the remaining task
        let action2 = nav.handle_key_event(NavKey::Delete, false);
        assert_eq!(
            action2,
            NavigatorAction::HubTask(HubTaskOp::Delete {
                category: HubTaskCategory::File,
                line_index: 5,
            })
        );
        assert!(nav.hub_tasks.active_tasks().is_empty());
    }
}
