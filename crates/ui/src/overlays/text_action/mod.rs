use std::ops::Range;
use truff_core::vim::Key;

/// Visual palette option for color and highlight choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorOption {
    pub name: &'static str,
    pub hex: &'static str,
    pub key: char,
}

pub const PALETTE: &[ColorOption] = &[
    ColorOption {
        name: "Красный",
        hex: "#BF616A",
        key: 'r',
    },
    ColorOption {
        name: "Оранжевый",
        hex: "#D08770",
        key: 'o',
    },
    ColorOption {
        name: "Жёлтый",
        hex: "#EBCB8B",
        key: 'y',
    },
    ColorOption {
        name: "Зелёный",
        hex: "#A3BE8C",
        key: 'g',
    },
    ColorOption {
        name: "Голубой",
        hex: "#88C0D0",
        key: 'b',
    },
    ColorOption {
        name: "Синий",
        hex: "#81A1C1",
        key: 's',
    },
    ColorOption {
        name: "Фиолетовый",
        hex: "#B48EAD",
        key: 'p',
    },
];

/// Task completion status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Todo,
    InProgress,
    Done,
    Remove,
}

impl TaskStatus {
    pub const fn checkbox_str(self) -> Option<&'static str> {
        match self {
            Self::Todo => Some("[ ]"),
            Self::InProgress => Some("[/]"),
            Self::Done => Some("[x]"),
            Self::Remove => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Todo => "К выполнению (Todo)",
            Self::InProgress => "В процессе (In Progress)",
            Self::Done => "Выполнено (Done)",
            Self::Remove => "Убрать задачу (Remove task)",
        }
    }

    pub const fn icon(self) -> &'static str {
        match self {
            Self::Todo => "☐",
            Self::InProgress => "◐",
            Self::Done => "☑",
            Self::Remove => "—",
        }
    }
}

pub struct TaskStatusOption {
    pub status: TaskStatus,
    pub title: &'static str,
    pub desc: &'static str,
    pub marker: &'static str,
    pub key: char,
}

pub const TASK_STATUS_OPTIONS: &[TaskStatusOption] = &[
    TaskStatusOption {
        status: TaskStatus::Todo,
        title: "К выполнению",
        desc: "Маркер [ ]",
        marker: "☐",
        key: '1',
    },
    TaskStatusOption {
        status: TaskStatus::InProgress,
        title: "В процессе",
        desc: "Маркер [/]",
        marker: "◐",
        key: '2',
    },
    TaskStatusOption {
        status: TaskStatus::Done,
        title: "Выполнено",
        desc: "Маркер [x]",
        marker: "☑",
        key: '3',
    },
    TaskStatusOption {
        status: TaskStatus::Remove,
        title: "Убрать задачу",
        desc: "Обычный текст",
        marker: "—",
        key: '4',
    },
];

/// Mode / step of the TextAction overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextActionMode {
    Root {
        has_selection: bool,
    },
    ColorSelect {
        is_highlight: bool,
    },
    TaskStatusSelect {
        selected_index: usize,
    },
    LinkInput {
        is_url: bool,
        has_selection: bool,
        target_input: String,
        title_input: String,
        active_field: usize,
        suggestions: Vec<String>,
        selected_suggestion: Option<usize>,
    },
}

impl Default for TextActionMode {
    fn default() -> Self {
        Self::Root {
            has_selection: false,
        }
    }
}

/// Action produced by the TextAction overlay for the editor to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextActionResponse {
    None,
    Close,
    ApplyColor {
        hex: &'static str,
    },
    ApplyHighlight {
        hex: &'static str,
    },
    ApplyLink {
        is_url: bool,
        target: String,
        title: Option<String>,
    },
    ApplyTask {
        status: TaskStatus,
    },
    RemoveStyle,
}

/// Controller for the inline text action menu triggered by `<Space> c`.
#[derive(Debug, Clone, Default)]
pub struct TextActionOverlay {
    is_open: bool,
    mode: TextActionMode,
    cursor_pos: Option<[f32; 2]>,
    color_index: usize,
    initial_text: Option<String>,
}

impl TextActionOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    pub fn mode(&self) -> &TextActionMode {
        &self.mode
    }

    pub fn mode_mut(&mut self) -> &mut TextActionMode {
        &mut self.mode
    }

    pub fn cursor_pos(&self) -> Option<[f32; 2]> {
        self.cursor_pos
    }

    pub fn selected_color_index(&self) -> usize {
        self.color_index
    }

    pub fn open(&mut self, initial_text: Option<String>, cursor_pos: Option<[f32; 2]>) {
        self.is_open = true;
        let has_selection = initial_text.as_ref().is_some_and(|s| !s.trim().is_empty());
        self.mode = TextActionMode::Root { has_selection };
        self.cursor_pos = cursor_pos;
        self.color_index = 0;
        self.initial_text = initial_text;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.mode = TextActionMode::Root {
            has_selection: false,
        };
        self.cursor_pos = None;
        self.initial_text = None;
    }

    pub fn initial_text(&self) -> Option<&str> {
        self.initial_text.as_deref()
    }

    pub fn handle_key(&mut self, key: Key) -> TextActionResponse {
        if !self.is_open {
            return TextActionResponse::None;
        }

        match &mut self.mode {
            TextActionMode::Root { has_selection } => {
                let has_sel = *has_selection;
                match key {
                    Key::Char('c' | 'C') if has_sel => {
                        self.mode = TextActionMode::ColorSelect {
                            is_highlight: false,
                        };
                        self.color_index = 0;
                        TextActionResponse::None
                    }
                    Key::Char('h' | 'H') if has_sel => {
                        self.mode = TextActionMode::ColorSelect { is_highlight: true };
                        self.color_index = 0;
                        TextActionResponse::None
                    }
                    Key::Char('n' | 'N' | 'u' | 'U') => {
                        let is_url = matches!(key, Key::Char('u' | 'U'));
                        let text = if has_sel {
                            self.initial_text.clone().unwrap_or_default()
                        } else {
                            String::new()
                        };
                        self.mode = TextActionMode::LinkInput {
                            is_url,
                            has_selection: has_sel,
                            target_input: String::new(),
                            title_input: text,
                            active_field: usize::from(has_sel),
                            suggestions: Vec::new(),
                            selected_suggestion: None,
                        };
                        TextActionResponse::None
                    }
                    Key::Char('t' | 'T') => {
                        self.mode = TextActionMode::TaskStatusSelect { selected_index: 0 };
                        TextActionResponse::None
                    }
                    Key::Char('x' | 'X' | '0') if has_sel => {
                        self.close();
                        TextActionResponse::RemoveStyle
                    }
                    Key::Esc => {
                        self.close();
                        TextActionResponse::Close
                    }
                    _ => TextActionResponse::None,
                }
            }
            TextActionMode::ColorSelect { is_highlight } => {
                let is_highlight = *is_highlight;
                match key {
                    Key::Left | Key::Up => {
                        self.color_index = if self.color_index == 0 {
                            PALETTE.len().saturating_sub(1)
                        } else {
                            self.color_index - 1
                        };
                        TextActionResponse::None
                    }
                    Key::Right | Key::Down => {
                        self.color_index = (self.color_index + 1) % PALETTE.len();
                        TextActionResponse::None
                    }
                    Key::Enter => {
                        let hex = PALETTE[self.color_index].hex;
                        self.close();
                        if is_highlight {
                            TextActionResponse::ApplyHighlight { hex }
                        } else {
                            TextActionResponse::ApplyColor { hex }
                        }
                    }
                    Key::Char(ch) => {
                        let lower = ch.to_ascii_lowercase();
                        if let Some(opt) = PALETTE.iter().find(|o| o.key == lower) {
                            let hex = opt.hex;
                            self.close();
                            if is_highlight {
                                TextActionResponse::ApplyHighlight { hex }
                            } else {
                                TextActionResponse::ApplyColor { hex }
                            }
                        } else {
                            TextActionResponse::None
                        }
                    }
                    Key::Esc | Key::Backspace => {
                        self.mode = TextActionMode::Root {
                            has_selection: true,
                        };
                        TextActionResponse::None
                    }
                    _ => TextActionResponse::None,
                }
            }
            TextActionMode::TaskStatusSelect { selected_index } => match key {
                Key::Up | Key::Char('k') => {
                    *selected_index = if *selected_index == 0 {
                        TASK_STATUS_OPTIONS.len().saturating_sub(1)
                    } else {
                        *selected_index - 1
                    };
                    TextActionResponse::None
                }
                Key::Down | Key::Char('j') => {
                    *selected_index = (*selected_index + 1) % TASK_STATUS_OPTIONS.len();
                    TextActionResponse::None
                }
                Key::Enter => {
                    let status = TASK_STATUS_OPTIONS[*selected_index].status;
                    self.close();
                    TextActionResponse::ApplyTask { status }
                }
                Key::Char('1' | 't' | 'T' | 's') => {
                    self.close();
                    TextActionResponse::ApplyTask {
                        status: TaskStatus::Todo,
                    }
                }
                Key::Char('2' | 'p' | 'P' | '/') => {
                    self.close();
                    TextActionResponse::ApplyTask {
                        status: TaskStatus::InProgress,
                    }
                }
                Key::Char('3' | 'd' | 'D' | 'x' | 'X') => {
                    self.close();
                    TextActionResponse::ApplyTask {
                        status: TaskStatus::Done,
                    }
                }
                Key::Char('4' | 'r' | 'R' | '0') => {
                    self.close();
                    TextActionResponse::ApplyTask {
                        status: TaskStatus::Remove,
                    }
                }
                Key::Backspace => {
                    let has_selection = self
                        .initial_text
                        .as_ref()
                        .is_some_and(|s| !s.trim().is_empty());
                    self.mode = TextActionMode::Root { has_selection };
                    TextActionResponse::None
                }
                Key::Esc => {
                    self.close();
                    TextActionResponse::Close
                }
                _ => TextActionResponse::None,
            },
            TextActionMode::LinkInput {
                is_url,
                has_selection: _,
                target_input,
                title_input,
                active_field,
                suggestions,
                selected_suggestion,
            } => {
                let is_url = *is_url;
                match key {
                    Key::Esc => {
                        self.close();
                        TextActionResponse::Close
                    }
                    Key::Down => {
                        if *active_field == 1 && !suggestions.is_empty() {
                            *selected_suggestion = Some(match *selected_suggestion {
                                None => 0,
                                Some(i) => (i + 1) % suggestions.len(),
                            });
                        }
                        TextActionResponse::None
                    }
                    Key::Up => {
                        if *active_field == 1 && !suggestions.is_empty() {
                            *selected_suggestion = Some(match *selected_suggestion {
                                None | Some(0) => suggestions.len().saturating_sub(1),
                                Some(i) => i.saturating_sub(1),
                            });
                        }
                        TextActionResponse::None
                    }
                    Key::Tab | Key::ShiftTab => {
                        if *active_field == 1
                            && key == Key::Tab
                            && let Some(i) = *selected_suggestion
                            && let Some(sugg) = suggestions.get(i)
                        {
                            *target_input = sugg.clone();
                            *selected_suggestion = None;
                        } else {
                            *active_field = (*active_field + 1) % 2;
                        }
                        TextActionResponse::None
                    }
                    Key::Enter => {
                        if *active_field == 1
                            && let Some(i) = *selected_suggestion
                            && let Some(sugg) = suggestions.get(i)
                        {
                            *target_input = sugg.clone();
                        }
                        let target = target_input.trim().to_string();
                        if target.is_empty() {
                            return TextActionResponse::None;
                        }
                        let title = {
                            let t = title_input.trim();
                            (!t.is_empty()).then(|| t.to_string())
                        };
                        self.close();
                        TextActionResponse::ApplyLink {
                            is_url,
                            target,
                            title,
                        }
                    }
                    Key::Char(ch) => {
                        *selected_suggestion = None;
                        if *active_field == 0 {
                            title_input.push(ch);
                        } else {
                            target_input.push(ch);
                        }
                        TextActionResponse::None
                    }
                    Key::Backspace => {
                        *selected_suggestion = None;
                        if *active_field == 0 {
                            title_input.pop();
                        } else {
                            target_input.pop();
                        }
                        TextActionResponse::None
                    }
                    _ => TextActionResponse::None,
                }
            }
        }
    }
}

/// Identifies word boundary `[start..end]` around `byte_pos` in `buffer`.
pub fn word_boundary_at(buffer: &truff_core::Buffer, byte_pos: usize) -> Option<Range<usize>> {
    buffer.word_boundary_at(byte_pos)
}

#[cfg(test)]
mod tests;
