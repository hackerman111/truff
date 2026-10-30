use super::types::{NavKey, NavigatorAction};

/// One of the 3 task categories shown in the right column of `<Space> Enter` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubTaskCategory {
    /// Vault-wide tasks from `<vault_root>/index.md`.
    Vault = 0,
    /// Active note tasks from a ```` ```tasks ```` fenced block in the note.
    File = 1,
    /// Project (folder) tasks from `<active_file_dir>/index.md`.
    Project = 2,
}

impl HubTaskCategory {
    pub const ALL: [Self; 3] = [Self::Vault, Self::File, Self::Project];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn from_index(index: usize) -> Self {
        match index % 3 {
            0 => Self::Vault,
            1 => Self::File,
            _ => Self::Project,
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Vault => "Общие задачи",
            Self::File => "Задачи файла",
            Self::Project => "Задачи проекта",
        }
    }

    pub const fn subtitle(self) -> &'static str {
        match self {
            Self::Vault => "index.md (убежище)",
            Self::File => "```tasks (заметка)",
            Self::Project => "index.md (папка)",
        }
    }
}

/// Task completion status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubTaskStatus {
    Todo,
    InProgress,
    Done,
}

impl HubTaskStatus {
    pub const fn cycle(self) -> Self {
        match self {
            Self::Todo => Self::InProgress,
            Self::InProgress => Self::Done,
            Self::Done => Self::Todo,
        }
    }

    pub const fn toggle_done(self) -> Self {
        match self {
            Self::Done => Self::Todo,
            Self::Todo | Self::InProgress => Self::Done,
        }
    }

    pub const fn marker_char(self) -> char {
        match self {
            Self::Todo => ' ',
            Self::InProgress => '/',
            Self::Done => 'x',
        }
    }

    pub const fn badge(self) -> &'static str {
        match self {
            Self::Todo => "☐",
            Self::InProgress => "◐",
            Self::Done => "☑",
        }
    }
}

/// A parsed task item in one of the 3 hub categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubTask {
    pub text: String,
    pub status: HubTaskStatus,
    pub line_index: usize,
}

/// Active inline add/edit state inside the task hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubTaskEdit {
    Add {
        category: HubTaskCategory,
        input: String,
    },
    Edit {
        category: HubTaskCategory,
        task_index: usize,
        line_index: usize,
        input: String,
    },
}

/// Persistence operation emitted when a user modifies a task in the hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubTaskOp {
    Add {
        category: HubTaskCategory,
        text: String,
    },
    Edit {
        category: HubTaskCategory,
        line_index: usize,
        text: String,
    },
    SetStatus {
        category: HubTaskCategory,
        line_index: usize,
        status: HubTaskStatus,
    },
    Delete {
        category: HubTaskCategory,
        line_index: usize,
    },
}

/// State of the 3-category task hub inside `<Space> Enter` (`Outline` / `Links`) mode.
#[derive(Debug, Clone, Default)]
pub struct HubTasksState {
    pub focus_tasks: bool,
    pub active_category: usize,
    pub selected: [usize; 3],
    pub lists: [Vec<HubTask>; 3],
    pub editing: Option<HubTaskEdit>,
    pub pending_g: bool,
}

impl HubTasksState {
    pub fn category(&self) -> HubTaskCategory {
        HubTaskCategory::from_index(self.active_category)
    }

    pub fn set_category(&mut self, category: HubTaskCategory) {
        self.active_category = category.index();
        self.pending_g = false;
        self.clamp_selection();
    }

    pub fn active_tasks(&self) -> &[HubTask] {
        &self.lists[self.active_category % 3]
    }

    pub fn selected_task(&self) -> Option<&HubTask> {
        let cat = self.active_category % 3;
        let idx = self.selected[cat];
        self.lists[cat].get(idx)
    }

    pub fn set_lists(&mut self, vault: Vec<HubTask>, file: Vec<HubTask>, project: Vec<HubTask>) {
        self.lists = [vault, file, project];
        if self.editing.is_none() && self.lists[self.active_category % 3].is_empty() {
            for candidate in [
                HubTaskCategory::File,
                HubTaskCategory::Project,
                HubTaskCategory::Vault,
            ] {
                if !self.lists[candidate.index()].is_empty() {
                    self.active_category = candidate.index();
                    break;
                }
            }
        }
        self.clamp_selection();
    }

    pub fn clamp_selection(&mut self) {
        for (cat, list) in self.lists.iter().enumerate() {
            if list.is_empty() {
                self.selected[cat] = 0;
            } else if self.selected[cat] >= list.len() {
                self.selected[cat] = list.len() - 1;
            }
        }
    }

    pub fn move_down(&mut self) {
        let cat = self.active_category % 3;
        let len = self.lists[cat].len();
        if len > 0 {
            self.selected[cat] = (self.selected[cat] + 1).min(len - 1);
        }
    }

    pub fn move_up(&mut self) {
        let cat = self.active_category % 3;
        self.selected[cat] = self.selected[cat].saturating_sub(1);
    }

    pub fn move_top(&mut self) {
        let cat = self.active_category % 3;
        self.selected[cat] = 0;
    }

    pub fn move_bottom(&mut self) {
        let cat = self.active_category % 3;
        let len = self.lists[cat].len();
        self.selected[cat] = len.saturating_sub(1);
    }

    pub fn apply_op_in_memory(&mut self, op: &HubTaskOp) {
        match op {
            HubTaskOp::Add { category, text } => {
                let cat = category.index();
                let next_line = self.lists[cat].last().map_or(0, |task| task.line_index + 1);
                self.lists[cat].push(HubTask {
                    text: text.clone(),
                    status: HubTaskStatus::Todo,
                    line_index: next_line,
                });
                self.selected[cat] = self.lists[cat].len().saturating_sub(1);
            }
            HubTaskOp::Edit {
                category,
                line_index,
                text,
            } => {
                let cat = category.index();
                if let Some(task) = self.lists[cat]
                    .iter_mut()
                    .find(|t| t.line_index == *line_index)
                {
                    task.text = text.clone();
                }
            }
            HubTaskOp::SetStatus {
                category,
                line_index,
                status,
            } => {
                let cat = category.index();
                if let Some(task) = self.lists[cat]
                    .iter_mut()
                    .find(|t| t.line_index == *line_index)
                {
                    task.status = *status;
                }
            }
            HubTaskOp::Delete {
                category,
                line_index,
            } => {
                let cat = category.index();
                if let Some(pos) = self.lists[cat]
                    .iter()
                    .position(|t| t.line_index == *line_index)
                {
                    self.lists[cat].remove(pos);
                }
                self.clamp_selection();
            }
        }
    }

    pub fn handle_key(&mut self, key: NavKey) -> NavigatorAction {
        if let Some(edit) = &mut self.editing {
            match key {
                NavKey::Escape => {
                    self.editing = None;
                    return NavigatorAction::None;
                }
                NavKey::Backspace => {
                    match edit {
                        HubTaskEdit::Add { input, .. } | HubTaskEdit::Edit { input, .. } => {
                            input.pop();
                        }
                    }
                    return NavigatorAction::None;
                }
                NavKey::Enter => {
                    let completed = self.editing.take().unwrap();
                    match completed {
                        HubTaskEdit::Add { category, input } => {
                            let text = input.trim().to_string();
                            if text.is_empty() {
                                return NavigatorAction::None;
                            }
                            let op = HubTaskOp::Add { category, text };
                            self.apply_op_in_memory(&op);
                            return NavigatorAction::HubTask(op);
                        }
                        HubTaskEdit::Edit {
                            category,
                            line_index,
                            input,
                            ..
                        } => {
                            let text = input.trim().to_string();
                            if text.is_empty() {
                                return NavigatorAction::None;
                            }
                            let op = HubTaskOp::Edit {
                                category,
                                line_index,
                                text,
                            };
                            self.apply_op_in_memory(&op);
                            return NavigatorAction::HubTask(op);
                        }
                    }
                }
                NavKey::Char(ch) if !ch.is_control() => {
                    match edit {
                        HubTaskEdit::Add { input, .. } | HubTaskEdit::Edit { input, .. } => {
                            input.push(ch);
                        }
                    }
                    return NavigatorAction::None;
                }
                _ => return NavigatorAction::None,
            }
        }

        if self.pending_g {
            self.pending_g = false;
            if key == NavKey::Char('g') {
                self.move_top();
                return NavigatorAction::None;
            }
        }

        match key {
            NavKey::Char('h' | 'р') | NavKey::Left => {
                self.focus_tasks = false;
                NavigatorAction::None
            }
            NavKey::Char('1') => {
                self.set_category(HubTaskCategory::Vault);
                NavigatorAction::None
            }
            NavKey::Char('2') => {
                self.set_category(HubTaskCategory::File);
                NavigatorAction::None
            }
            NavKey::Char('3') => {
                self.set_category(HubTaskCategory::Project);
                NavigatorAction::None
            }
            NavKey::Char('J' | 'L' | 'О' | 'Д') => {
                let next = (self.active_category + 1) % 3;
                self.set_category(HubTaskCategory::from_index(next));
                NavigatorAction::None
            }
            NavKey::Char('K' | 'H' | 'Л' | 'Р') => {
                let prev = (self.active_category + 2) % 3;
                self.set_category(HubTaskCategory::from_index(prev));
                NavigatorAction::None
            }
            NavKey::Char('j' | 'о') | NavKey::Down => {
                self.move_down();
                NavigatorAction::None
            }
            NavKey::Char('k' | 'л') | NavKey::Up => {
                self.move_up();
                NavigatorAction::None
            }
            NavKey::Char('g' | 'п') => {
                self.pending_g = true;
                NavigatorAction::None
            }
            NavKey::Home => {
                self.move_top();
                NavigatorAction::None
            }
            NavKey::Char('G' | 'П') | NavKey::End => {
                self.move_bottom();
                NavigatorAction::None
            }
            NavKey::Char(' ' | 'x' | 'ч') | NavKey::Enter => {
                let category = self.category();
                let Some(task) = self.selected_task() else {
                    return NavigatorAction::None;
                };
                let new_status = if key == NavKey::Enter {
                    task.status.toggle_done()
                } else {
                    task.status.cycle()
                };
                let op = HubTaskOp::SetStatus {
                    category,
                    line_index: task.line_index,
                    status: new_status,
                };
                self.apply_op_in_memory(&op);
                NavigatorAction::HubTask(op)
            }
            NavKey::Char('a' | 'o' | 'ф' | 'щ') => {
                self.editing = Some(HubTaskEdit::Add {
                    category: self.category(),
                    input: String::new(),
                });
                NavigatorAction::None
            }
            NavKey::Char('e' | 'i' | 'c' | 'у' | 'ш' | 'с') => {
                let category = self.category();
                let cat_idx = category.index();
                let task_index = self.selected[cat_idx];
                if let Some(task) = self.lists[cat_idx].get(task_index) {
                    self.editing = Some(HubTaskEdit::Edit {
                        category,
                        task_index,
                        line_index: task.line_index,
                        input: task.text.clone(),
                    });
                }
                NavigatorAction::None
            }
            NavKey::Char('d' | 'D' | 'в' | 'В') | NavKey::Delete | NavKey::Backspace => {
                let category = self.category();
                let Some(task) = self.selected_task() else {
                    return NavigatorAction::None;
                };
                let op = HubTaskOp::Delete {
                    category,
                    line_index: task.line_index,
                };
                self.apply_op_in_memory(&op);
                NavigatorAction::HubTask(op)
            }
            _ => NavigatorAction::None,
        }
    }
}

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
                if tag.eq_ignore_ascii_case("tasks") {
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
                        if tag.eq_ignore_ascii_case("tasks") {
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
                            && info
                                .split_whitespace()
                                .next()
                                .is_some_and(|tag| tag.eq_ignore_ascii_case("tasks"))
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
        use crate::navigator::{NavigatorMode, NavigatorState};
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
