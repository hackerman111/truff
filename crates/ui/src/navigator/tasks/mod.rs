mod markdown;

use super::types::{NavKey, NavigatorAction};

pub use markdown::{
    apply_task_op_to_file_text, apply_task_op_to_index_text, parse_fenced_file_tasks,
    parse_index_tasks, parse_task_line,
};

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
            Self::File => "```task / ```tasks (заметка)",
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
