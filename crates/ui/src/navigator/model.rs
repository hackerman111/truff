use truff_core::commands::CommandId;
use truff_core::text::BufferId;

/// Active mode of the Universal Navigator overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigatorMode {
    #[default]
    Files,
    Sessions,
    Boards,
    Outline,
    Links,
    Commands,
}

impl NavigatorMode {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Files => "Файлы и папки",
            Self::Sessions => "Сессии и вкладки",
            Self::Boards => "Kanban и задачи",
            Self::Outline => "Структура документа (Outline)",
            Self::Links => "Ссылки и связи (Backlinks)",
            Self::Commands => "Все команды",
        }
    }

    pub fn search_placeholder(&self) -> &'static str {
        match self {
            Self::Files => "Поиск файлов, папок и заметок... (/ для ввода)",
            Self::Sessions => "Поиск открытых вкладок и буферов... (/ для ввода)",
            Self::Boards => "Поиск досок и задач... (/ для ввода)",
            Self::Outline => "Поиск заголовков в документе... (/ для ввода)",
            Self::Links => "Поиск ссылок и связей... (/ для ввода)",
            Self::Commands => "Поиск команд по названию или действию... (/ для ввода)",
        }
    }
}

/// Payload associated with a specific item in the Navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigatorItemPayload {
    File {
        path: String,
        is_dir: bool,
    },
    Buffer {
        buffer_id: BufferId,
        path: Option<String>,
    },
    Command {
        command_id: CommandId,
    },
    Heading {
        line: usize,
        level: usize,
        text: String,
    },
    Link {
        target: String,
    },
    Board {
        name: String,
    },
    Custom {
        id: String,
        data: String,
    },
}

/// A single selectable item in the Universal Navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigatorItem {
    pub id: u64,
    pub title: String,
    pub subtitle: Option<String>,
    pub category: Option<String>,
    pub shortcut: Option<String>,
    pub is_folder: bool,
    pub is_collapsed: bool,
    pub depth: usize,
    pub preview: Option<String>,
    pub payload: NavigatorItemPayload,
}

impl NavigatorItem {
    pub fn new_file(
        id: u64,
        name: String,
        relative_path: String,
        is_dir: bool,
        depth: usize,
    ) -> Self {
        let subtitle = if is_dir {
            Some("Папка".to_string())
        } else {
            std::path::Path::new(&relative_path)
                .parent()
                .and_then(|p| p.to_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };

        Self {
            id,
            title: name,
            subtitle,
            category: Some("Файлы".to_string()),
            shortcut: None,
            is_folder: is_dir,
            is_collapsed: false,
            depth,
            preview: None,
            payload: NavigatorItemPayload::File {
                path: relative_path,
                is_dir,
            },
        }
    }

    pub fn new_buffer(
        id: u64,
        buffer_id: BufferId,
        title: String,
        path: Option<String>,
        is_dirty: bool,
        line_count: usize,
    ) -> Self {
        let mut subtitle = format!("{line_count} строк");
        if is_dirty {
            subtitle.push_str(" · Не сохранено");
        }
        if let Some(p) = &path {
            subtitle.push_str(&format!(" · {p}"));
        }

        Self {
            id,
            title,
            subtitle: Some(subtitle),
            category: Some("Буферы".to_string()),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: None,
            payload: NavigatorItemPayload::Buffer { buffer_id, path },
        }
    }

    pub fn new_command(
        id: u64,
        command_id: CommandId,
        title: String,
        description: String,
        category: String,
        shortcut: Option<String>,
    ) -> Self {
        Self {
            id,
            title,
            subtitle: Some(description.clone()),
            category: Some(category),
            shortcut,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(description),
            payload: NavigatorItemPayload::Command { command_id },
        }
    }

    pub fn new_heading(id: u64, line: usize, level: usize, text: String) -> Self {
        let prefix = "#".repeat(level);
        let title = format!("{prefix} {text}");
        Self {
            id,
            title,
            subtitle: Some(format!("Строка {line}")),
            category: Some("Заголовки".to_string()),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: level.saturating_sub(1),
            preview: Some(format!("Заголовок уровня {level}: {text}\nСтрока: {line}")),
            payload: NavigatorItemPayload::Heading { line, level, text },
        }
    }
}

/// Action produced by interacting with the Navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigatorAction {
    None,
    Close,
    OpenFile { path: String },
    CreateFile { parent_dir: Option<String> },
    CreateFolder { parent_dir: Option<String> },
    RenameFile { path: String },
    DeleteFile { path: String },
    SwitchBuffer { buffer_id: BufferId },
    CloseBuffer { buffer_id: BufferId },
    ExecuteCommand { command_id: CommandId },
    JumpHeading { line: usize },
    OpenLink { target: String },
    SelectCustom { id: String, data: String },
}

/// Quick action displayed in the bottom footer or context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickAction {
    pub key: String,
    pub label: String,
    pub description: String,
    pub action: NavigatorAction,
}

/// Abstract navigation key for headless testing and unified event handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    Tab,
}

/// State of the Universal Navigator overlay.
#[derive(Debug, Clone)]
pub struct NavigatorState {
    pub open: bool,
    pub mode: NavigatorMode,
    pub context_title: String,
    pub query: String,
    pub search_mode: bool,
    pub items: Vec<NavigatorItem>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub preview_visible: bool,
    pub actions_menu_open: bool,
    pub actions_menu_selected: usize,
}

impl Default for NavigatorState {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigatorState {
    pub fn new() -> Self {
        Self {
            open: false,
            mode: NavigatorMode::Files,
            context_title: "Vault".to_string(),
            query: String::new(),
            search_mode: false,
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            preview_visible: true,
            actions_menu_open: false,
            actions_menu_selected: 0,
        }
    }

    /// Opens Navigator in a specific mode with items.
    pub fn open_mode(
        &mut self,
        mode: NavigatorMode,
        context_title: impl Into<String>,
        items: Vec<NavigatorItem>,
    ) {
        self.open = true;
        self.mode = mode;
        self.context_title = context_title.into();
        self.query.clear();
        self.search_mode = false;
        self.items = items;
        self.actions_menu_open = false;
        self.actions_menu_selected = 0;
        self.recompute_filter();
    }

    /// Closes the Navigator overlay.
    pub fn close(&mut self) {
        self.open = false;
        self.search_mode = false;
        self.actions_menu_open = false;
        self.query.clear();
    }

    /// Checks if Navigator is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the currently active items count.
    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns a reference to the selected item, if any.
    pub fn selected_item(&self) -> Option<&NavigatorItem> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.items.get(idx))
    }

    /// Recomputes filtered indices based on search query.
    pub fn recompute_filter(&mut self) {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            let mut scored: Vec<(usize, i32)> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    let title_lower = item.title.to_lowercase();
                    let subtitle_lower = item.subtitle.as_deref().unwrap_or("").to_lowercase();
                    let category_lower = item.category.as_deref().unwrap_or("").to_lowercase();

                    let mut score = 0;
                    if title_lower == q {
                        score += 200;
                    } else if title_lower.starts_with(&q) {
                        score += 100;
                    } else if title_lower.contains(&q) {
                        score += 50;
                    } else if subtitle_lower.contains(&q) {
                        score += 25;
                    } else if category_lower.contains(&q) {
                        score += 15;
                    } else if is_subsequence(&q, &title_lower) {
                        score += 10;
                    } else {
                        return None;
                    }

                    Some((idx, score))
                })
                .collect();

            scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            self.filtered_indices = scored.into_iter().map(|(idx, _)| idx).collect();
        }

        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
    }

    /// Sets query string and updates filtering.
    pub fn set_query(&mut self, query: &str) {
        self.query.clear();
        self.query.push_str(query);
        self.recompute_filter();
    }

    /// Moves selection down.
    pub fn move_down(&mut self) {
        if self.actions_menu_open {
            let count = self.context_actions().len();
            if count > 0 {
                self.actions_menu_selected = (self.actions_menu_selected + 1).min(count - 1);
            }
        } else if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.filtered_indices.len() - 1);
        }
    }

    /// Moves selection up.
    pub fn move_up(&mut self) {
        if self.actions_menu_open {
            self.actions_menu_selected = self.actions_menu_selected.saturating_sub(1);
        } else {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    /// Moves selection to top.
    pub fn move_top(&mut self) {
        if self.actions_menu_open {
            self.actions_menu_selected = 0;
        } else {
            self.selected_index = 0;
        }
    }

    /// Moves selection to bottom.
    pub fn move_bottom(&mut self) {
        if self.actions_menu_open {
            let count = self.context_actions().len();
            self.actions_menu_selected = count.saturating_sub(1);
        } else {
            self.selected_index = self.filtered_indices.len().saturating_sub(1);
        }
    }

    /// Collapses folder or moves to parent row.
    pub fn collapse_or_parent(&mut self) {
        if let Some(&real_idx) = self.filtered_indices.get(self.selected_index)
            && let Some(item) = self.items.get_mut(real_idx)
            && item.is_folder
            && !item.is_collapsed
        {
            item.is_collapsed = true;
            return;
        }
        self.move_up();
    }

    /// Expands folder or moves to first child.
    pub fn expand_or_child(&mut self) {
        if let Some(&real_idx) = self.filtered_indices.get(self.selected_index)
            && let Some(item) = self.items.get_mut(real_idx)
            && item.is_folder
            && item.is_collapsed
        {
            item.is_collapsed = false;
            return;
        }
        self.move_down();
    }

    /// Returns the primary default action for the selected item.
    pub fn primary_action(&self) -> NavigatorAction {
        let Some(item) = self.selected_item() else {
            return NavigatorAction::None;
        };

        match &item.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                if *is_dir {
                    NavigatorAction::None
                } else {
                    NavigatorAction::OpenFile { path: path.clone() }
                }
            }
            NavigatorItemPayload::Buffer { buffer_id, .. } => NavigatorAction::SwitchBuffer {
                buffer_id: *buffer_id,
            },
            NavigatorItemPayload::Command { command_id } => NavigatorAction::ExecuteCommand {
                command_id: command_id.clone(),
            },
            NavigatorItemPayload::Heading { line, .. } => {
                NavigatorAction::JumpHeading { line: *line }
            }
            NavigatorItemPayload::Link { target } => NavigatorAction::OpenLink {
                target: target.clone(),
            },
            NavigatorItemPayload::Board { name } => NavigatorAction::SelectCustom {
                id: "board.open".into(),
                data: name.clone(),
            },
            NavigatorItemPayload::Custom { id, data } => NavigatorAction::SelectCustom {
                id: id.clone(),
                data: data.clone(),
            },
        }
    }

    /// Returns quick actions displayed in the bottom footer.
    pub fn quick_actions(&self) -> Vec<QuickAction> {
        match self.mode {
            NavigatorMode::Files => vec![
                QuickAction {
                    key: "↵".to_string(),
                    label: "Открыть".to_string(),
                    description: "Открыть выбранную заметку".to_string(),
                    action: self.primary_action(),
                },
                QuickAction {
                    key: "a".to_string(),
                    label: "Создать".to_string(),
                    description: "Создать новую заметку".to_string(),
                    action: NavigatorAction::CreateFile {
                        parent_dir: self.current_parent_dir(),
                    },
                },
                QuickAction {
                    key: "r".to_string(),
                    label: "Переименовать".to_string(),
                    description: "Переименовать заметку или папку".to_string(),
                    action: self
                        .current_selected_path()
                        .map_or(NavigatorAction::None, |p| NavigatorAction::RenameFile {
                            path: p,
                        }),
                },
                QuickAction {
                    key: "d".to_string(),
                    label: "Удалить".to_string(),
                    description: "Удалить в корзину".to_string(),
                    action: self
                        .current_selected_path()
                        .map_or(NavigatorAction::None, |p| NavigatorAction::DeleteFile {
                            path: p,
                        }),
                },
                QuickAction {
                    key: "/".to_string(),
                    label: "Поиск".to_string(),
                    description: "Режим текстового поиска".to_string(),
                    action: NavigatorAction::None,
                },
                QuickAction {
                    key: "?".to_string(),
                    label: "Все действия".to_string(),
                    description: "Показать полное меню действий".to_string(),
                    action: NavigatorAction::None,
                },
            ],
            NavigatorMode::Sessions => vec![
                QuickAction {
                    key: "↵".to_string(),
                    label: "Перейти".to_string(),
                    description: "Активировать вкладку".to_string(),
                    action: self.primary_action(),
                },
                QuickAction {
                    key: "d".to_string(),
                    label: "Закрыть".to_string(),
                    description: "Закрыть буфер".to_string(),
                    action: self
                        .current_selected_buffer()
                        .map_or(NavigatorAction::None, |b| NavigatorAction::CloseBuffer {
                            buffer_id: b,
                        }),
                },
                QuickAction {
                    key: "/".to_string(),
                    label: "Поиск".to_string(),
                    description: "Фильтр буферов".to_string(),
                    action: NavigatorAction::None,
                },
                QuickAction {
                    key: "?".to_string(),
                    label: "Действия".to_string(),
                    description: "Все операции со вкладками".to_string(),
                    action: NavigatorAction::None,
                },
            ],
            NavigatorMode::Commands => vec![
                QuickAction {
                    key: "↵".to_string(),
                    label: "Выполнить".to_string(),
                    description: "Запустить команду".to_string(),
                    action: self.primary_action(),
                },
                QuickAction {
                    key: "/".to_string(),
                    label: "Поиск".to_string(),
                    description: "Поиск команд".to_string(),
                    action: NavigatorAction::None,
                },
                QuickAction {
                    key: "?".to_string(),
                    label: "Справка".to_string(),
                    description: "Подробности команды".to_string(),
                    action: NavigatorAction::None,
                },
            ],
            _ => vec![
                QuickAction {
                    key: "↵".to_string(),
                    label: "Выбрать".to_string(),
                    description: "Выполнить переход".to_string(),
                    action: self.primary_action(),
                },
                QuickAction {
                    key: "/".to_string(),
                    label: "Поиск".to_string(),
                    description: "Фильтр".to_string(),
                    action: NavigatorAction::None,
                },
            ],
        }
    }

    /// Returns contextual actions list when `?` actions menu is open.
    pub fn context_actions(&self) -> Vec<QuickAction> {
        let Some(item) = self.selected_item() else {
            return Vec::new();
        };

        match &item.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                let mut acts = Vec::new();
                if !*is_dir {
                    acts.push(QuickAction {
                        key: "↵".to_string(),
                        label: "Открыть заметку".to_string(),
                        description: format!("Открыть файл {}", item.title),
                        action: NavigatorAction::OpenFile { path: path.clone() },
                    });
                }
                acts.push(QuickAction {
                    key: "a".to_string(),
                    label: "Создать заметку".to_string(),
                    description: "Создать новую заметку в текущей директории".to_string(),
                    action: NavigatorAction::CreateFile {
                        parent_dir: self.current_parent_dir(),
                    },
                });
                acts.push(QuickAction {
                    key: "A".to_string(),
                    label: "Создать папку".to_string(),
                    description: "Создать новую директорию".to_string(),
                    action: NavigatorAction::CreateFolder {
                        parent_dir: self.current_parent_dir(),
                    },
                });
                acts.push(QuickAction {
                    key: "r".to_string(),
                    label: "Переименовать".to_string(),
                    description: format!("Изменить имя {}", item.title),
                    action: NavigatorAction::RenameFile { path: path.clone() },
                });
                acts.push(QuickAction {
                    key: "d".to_string(),
                    label: "Удалить".to_string(),
                    description: format!("Переместить {} в корзину", item.title),
                    action: NavigatorAction::DeleteFile { path: path.clone() },
                });
                acts
            }
            NavigatorItemPayload::Buffer { buffer_id, path: _ } => vec![
                QuickAction {
                    key: "↵".to_string(),
                    label: "Перейти к вкладке".to_string(),
                    description: "Сделать вкладку активной".to_string(),
                    action: NavigatorAction::SwitchBuffer {
                        buffer_id: *buffer_id,
                    },
                },
                QuickAction {
                    key: "d".to_string(),
                    label: "Закрыть вкладку".to_string(),
                    description: "Закрыть этот буфер".to_string(),
                    action: NavigatorAction::CloseBuffer {
                        buffer_id: *buffer_id,
                    },
                },
            ],
            NavigatorItemPayload::Command { command_id } => vec![QuickAction {
                key: "↵".to_string(),
                label: "Выполнить команду".to_string(),
                description: item.subtitle.clone().unwrap_or_default(),
                action: NavigatorAction::ExecuteCommand {
                    command_id: command_id.clone(),
                },
            }],
            _ => vec![QuickAction {
                key: "↵".to_string(),
                label: "Выбрать".to_string(),
                description: "Перейти".to_string(),
                action: self.primary_action(),
            }],
        }
    }

    pub fn current_selected_path(&self) -> Option<String> {
        self.selected_item().and_then(|it| match &it.payload {
            NavigatorItemPayload::File { path, .. } => Some(path.clone()),
            _ => None,
        })
    }

    pub fn current_selected_buffer(&self) -> Option<BufferId> {
        self.selected_item().and_then(|it| match &it.payload {
            NavigatorItemPayload::Buffer { buffer_id, .. } => Some(*buffer_id),
            _ => None,
        })
    }

    pub fn current_parent_dir(&self) -> Option<String> {
        self.selected_item().and_then(|it| match &it.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                if *is_dir {
                    Some(path.clone())
                } else {
                    std::path::Path::new(path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                }
            }
            _ => None,
        })
    }

    /// Handles keyboard input in a unified, deterministic manner.
    pub fn handle_key_event(&mut self, key: NavKey, ctrl: bool) -> NavigatorAction {
        if self.actions_menu_open {
            return self.handle_actions_menu_key(key);
        }

        if self.search_mode {
            self.handle_search_mode_key(key, ctrl)
        } else {
            self.handle_normal_mode_key(key)
        }
    }

    fn handle_actions_menu_key(&mut self, key: NavKey) -> NavigatorAction {
        match key {
            NavKey::Escape => {
                self.actions_menu_open = false;
                NavigatorAction::None
            }
            NavKey::Down | NavKey::Char('j') => {
                let count = self.context_actions().len();
                if count > 0 {
                    self.actions_menu_selected = (self.actions_menu_selected + 1).min(count - 1);
                }
                NavigatorAction::None
            }
            NavKey::Up | NavKey::Char('k') => {
                self.actions_menu_selected = self.actions_menu_selected.saturating_sub(1);
                NavigatorAction::None
            }
            NavKey::Enter => {
                let actions = self.context_actions();
                if let Some(act) = actions.get(self.actions_menu_selected) {
                    let action = act.action.clone();
                    self.actions_menu_open = false;
                    action
                } else {
                    NavigatorAction::None
                }
            }
            NavKey::Char(c) => {
                let c_str = c.to_string();
                let actions = self.context_actions();
                if let Some(act) = actions.iter().find(|a| a.key.eq_ignore_ascii_case(&c_str)) {
                    let action = act.action.clone();
                    self.actions_menu_open = false;
                    action
                } else {
                    NavigatorAction::None
                }
            }
            _ => NavigatorAction::None,
        }
    }

    fn handle_search_mode_key(&mut self, key: NavKey, ctrl: bool) -> NavigatorAction {
        match key {
            NavKey::Escape => {
                self.search_mode = false;
                NavigatorAction::None
            }
            NavKey::Enter => {
                let action = self.primary_action();
                if action != NavigatorAction::None {
                    self.close();
                }
                action
            }
            NavKey::Down | NavKey::Char('j') if ctrl => {
                self.move_down();
                NavigatorAction::None
            }
            NavKey::Up | NavKey::Char('k') if ctrl => {
                self.move_up();
                NavigatorAction::None
            }
            NavKey::Down => {
                self.move_down();
                NavigatorAction::None
            }
            NavKey::Up => {
                self.move_up();
                NavigatorAction::None
            }
            NavKey::Backspace => {
                self.query.pop();
                self.recompute_filter();
                NavigatorAction::None
            }
            NavKey::Char(c) => {
                self.query.push(c);
                self.recompute_filter();
                NavigatorAction::None
            }
            _ => NavigatorAction::None,
        }
    }

    fn handle_normal_mode_key(&mut self, key: NavKey) -> NavigatorAction {
        match key {
            NavKey::Char('/') => {
                self.search_mode = true;
                NavigatorAction::None
            }
            NavKey::Char('j') | NavKey::Down => {
                self.move_down();
                NavigatorAction::None
            }
            NavKey::Char('k') | NavKey::Up => {
                self.move_up();
                NavigatorAction::None
            }
            NavKey::Char('h') | NavKey::Left => {
                self.collapse_or_parent();
                NavigatorAction::None
            }
            NavKey::Char('l') | NavKey::Right => {
                self.expand_or_child();
                NavigatorAction::None
            }
            NavKey::Char('G') => {
                self.move_bottom();
                NavigatorAction::None
            }
            NavKey::Char(' ') => {
                self.preview_visible = !self.preview_visible;
                NavigatorAction::None
            }
            NavKey::Char('?') => {
                self.actions_menu_open = !self.actions_menu_open;
                self.actions_menu_selected = 0;
                NavigatorAction::None
            }
            NavKey::Enter => {
                let action = self.primary_action();
                if action != NavigatorAction::None {
                    self.close();
                }
                action
            }
            NavKey::Char('a') => {
                let parent_dir = self.current_parent_dir();
                self.close();
                NavigatorAction::CreateFile { parent_dir }
            }
            NavKey::Char('A') => {
                let parent_dir = self.current_parent_dir();
                self.close();
                NavigatorAction::CreateFolder { parent_dir }
            }
            NavKey::Char('r') => {
                if let Some(path) = self.current_selected_path() {
                    self.close();
                    NavigatorAction::RenameFile { path }
                } else {
                    NavigatorAction::None
                }
            }
            NavKey::Char('d') => {
                if self.mode == NavigatorMode::Sessions {
                    if let Some(buf) = self.current_selected_buffer() {
                        NavigatorAction::CloseBuffer { buffer_id: buf }
                    } else {
                        NavigatorAction::None
                    }
                } else if let Some(path) = self.current_selected_path() {
                    self.close();
                    NavigatorAction::DeleteFile { path }
                } else {
                    NavigatorAction::None
                }
            }
            NavKey::Escape => {
                self.close();
                NavigatorAction::Close
            }
            _ => NavigatorAction::None,
        }
    }
}

fn is_subsequence(sub: &str, target: &str) -> bool {
    let mut target_chars = target.chars();
    for sc in sub.chars() {
        if !target_chars.any(|tc| tc == sc) {
            return false;
        }
    }
    true
}
