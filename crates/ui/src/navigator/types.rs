use std::borrow::Cow;

use truff_core::commands::CommandId;
use truff_core::text::BufferId;

/// Active mode of the Universal Navigator overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigatorMode {
    #[default]
    Files,
    Folders,
    Vaults,
    Content,
    Mixed,
    Recent,
    Colors,
    Tasks,
    Sessions,
    Boards,
    Outline,
    Links,
    Commands,
}

impl NavigatorMode {
    pub const ALL: [Self; 13] = [
        Self::Files,
        Self::Folders,
        Self::Content,
        Self::Mixed,
        Self::Sessions,
        Self::Recent,
        Self::Vaults,
        Self::Outline,
        Self::Links,
        Self::Commands,
        Self::Colors,
        Self::Boards,
        Self::Tasks,
    ];

    pub fn next(self, backwards: bool) -> Self {
        let index = Self::ALL.iter().position(|&mode| mode == self).unwrap_or(0);
        Self::ALL[(index + if backwards { Self::ALL.len() - 1 } else { 1 }) % Self::ALL.len()]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Files => "Файлы и папки",
            Self::Folders => "Папки",
            Self::Vaults => "Хранилища",
            Self::Content => "Поиск по содержимому",
            Self::Mixed => "Файлы и содержимое",
            Self::Recent => "Недавно открытые заметки",
            Self::Colors => "Цвет текста",
            Self::Tasks => "Задачи текущей заметки",
            Self::Sessions => "Сессии и вкладки",
            Self::Boards => "Kanban и задачи",
            Self::Outline => "Структура документа (Outline)",
            Self::Links => "Ссылки и связи (Backlinks)",
            Self::Commands => "Все команды",
        }
    }

    pub fn search_placeholder(&self) -> &'static str {
        match self {
            Self::Files | Self::Folders | Self::Recent => {
                "Поиск файлов, папок и заметок... (/ для ввода)"
            }
            Self::Vaults => "Поиск хранилищ...",
            Self::Content | Self::Mixed => "Поиск по содержимому хранилища...",
            Self::Colors => "Поиск цвета...",
            Self::Tasks => "Поиск задач...",
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
    View {
        view_id: truff_core::ViewId,
        path: Option<String>,
    },
    Vault {
        path: std::path::PathBuf,
    },
    Location {
        path: String,
        byte: usize,
    },
    Color {
        hex: &'static str,
    },
    Highlight {
        hex: &'static str,
    },
    Callout {
        kind: truff_core::markdown::CalloutKind,
    },
    RemoveStyle,
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
    Task {
        line: usize,
        checked: bool,
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
    pub title: Box<str>,
    pub subtitle: Option<Box<str>>,
    pub category: Option<Cow<'static, str>>,
    pub shortcut: Option<Box<str>>,
    pub is_folder: bool,
    pub is_collapsed: bool,
    pub depth: usize,
    pub preview: Option<Box<str>>,
    pub disabled_reason: Option<Box<str>>,
    pub payload: NavigatorItemPayload,
    pub char_mask: u64,
}

#[inline]
pub fn compute_char_mask(s: &str) -> u64 {
    if s.is_ascii() {
        let mut mask = 0u64;
        for &b in s.as_bytes() {
            let lower = b.to_ascii_lowercase();
            let bit = match lower {
                b'a'..=b'z' => (lower - b'a') as u32,
                b'0'..=b'9' => 26 + (lower - b'0') as u32,
                b'_' => 36,
                b'-' => 37,
                b'/' | b'\\' => 38,
                b'.' => 39,
                b' ' => 40,
                _ => continue,
            };
            mask |= 1u64 << bit;
        }
        return mask;
    }

    let mut mask = 0u64;
    for c in s.chars() {
        if c.is_ascii() {
            let lower = (c as u8).to_ascii_lowercase();
            let bit = match lower {
                b'a'..=b'z' => (lower - b'a') as u32,
                b'0'..=b'9' => 26 + (lower - b'0') as u32,
                b'_' => 36,
                b'-' => 37,
                b'/' | b'\\' => 38,
                b'.' => 39,
                b' ' => 40,
                _ => continue,
            };
            mask |= 1u64 << bit;
            continue;
        }
        for lower_c in c.to_lowercase() {
            let bit = match lower_c {
                'а'..='я' => 41 + (((lower_c as u32) - ('а' as u32)) % 22),
                'ё' => 63,
                _ => continue,
            };
            mask |= 1u64 << bit;
        }
    }
    mask
}

#[inline]
fn category_char_mask(cat: &str) -> u64 {
    const FILES_MASK: u64 = (1u64 << (41 + ((('ф' as u32) - ('а' as u32)) % 22)))
        | (1u64 << 41)
        | (1u64 << (41 + ((('й' as u32) - ('а' as u32)) % 22)))
        | (1u64 << (41 + ((('л' as u32) - ('а' as u32)) % 22)))
        | (1u64 << (41 + ((('ы' as u32) - ('а' as u32)) % 22)));
    const FOLDER_MASK: u64 = (1u64 << (41 + ((('п' as u32) - ('а' as u32)) % 22)))
        | (1u64 << 41)
        | (1u64 << (41 + ((('к' as u32) - ('а' as u32)) % 22)));
    match cat {
        "Файлы" => FILES_MASK,
        "Папка" => FOLDER_MASK,
        _ => compute_char_mask(cat),
    }
}

fn file_name_start(path: &str) -> usize {
    // Separators are ASCII bytes, so the result is always a UTF-8 boundary.
    path.as_bytes()
        .iter()
        .rposition(|&byte| byte == b'/' || byte == b'\\')
        .map_or(0, |index| index + 1)
}

impl NavigatorItem {
    pub fn update_char_mask(&mut self) {
        let mut mask = if self.title.is_empty()
            && self.subtitle.is_none()
            && let NavigatorItemPayload::File { path, .. } = &self.payload
        {
            compute_char_mask(path)
        } else {
            let mut m = compute_char_mask(self.display_title());
            if let Some(sub) = self.display_subtitle() {
                m |= category_char_mask(sub);
            }
            m
        };
        if let Some(cat) = self.category.as_deref() {
            mask |= category_char_mask(cat);
        }
        self.char_mask = mask;
    }

    pub fn new_file(
        id: u64,
        name: &str,
        relative_path: String,
        is_dir: bool,
        depth: usize,
    ) -> Self {
        let basename = &relative_path[file_name_start(&relative_path)..];
        let title = if name == basename {
            Box::default()
        } else {
            name.into()
        };

        let mut item = Self {
            id,
            title,
            subtitle: None,
            category: Some(Cow::Borrowed("Файлы")),
            shortcut: None,
            is_folder: is_dir,
            is_collapsed: false,
            depth,
            preview: None,
            disabled_reason: None,
            payload: NavigatorItemPayload::File {
                path: relative_path,
                is_dir,
            },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn display_title(&self) -> &str {
        if !self.title.is_empty() {
            return &self.title;
        }
        match &self.payload {
            NavigatorItemPayload::File { path, .. }
            | NavigatorItemPayload::Location { path, .. } => &path[file_name_start(path)..],
            NavigatorItemPayload::Vault { path } => path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Vault"),
            NavigatorItemPayload::Link { target } => target,
            _ => &self.title,
        }
    }

    pub fn display_subtitle(&self) -> Option<&str> {
        if self.subtitle.is_some() {
            return self.subtitle.as_deref();
        }
        match &self.payload {
            NavigatorItemPayload::File { is_dir: true, .. } => Some("Папка"),
            NavigatorItemPayload::File { path, .. } => file_name_start(path)
                .checked_sub(1)
                .map(|index| &path[..index])
                .filter(|parent| !parent.is_empty()),
            NavigatorItemPayload::Command { .. } => self.preview.as_deref(),
            _ => None,
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

        let mut item = Self {
            id,
            title: title.into_boxed_str(),
            subtitle: Some(subtitle.into_boxed_str()),
            category: Some(Cow::Borrowed("Буферы")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: None,
            disabled_reason: None,
            payload: NavigatorItemPayload::Buffer { buffer_id, path },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_link(id: u64, title: &str, target: String) -> Self {
        let mut item = Self {
            id,
            title: title.into(),
            subtitle: None,
            category: Some(Cow::Borrowed("Исходящие")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(target.clone().into_boxed_str()),
            disabled_reason: None,
            payload: NavigatorItemPayload::Link { target },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_command(
        id: u64,
        command_id: CommandId,
        title: String,
        description: String,
        category: Cow<'static, str>,
        shortcut: Option<String>,
    ) -> Self {
        let mut item = Self {
            id,
            title: title.into_boxed_str(),
            subtitle: None,
            category: Some(category),
            shortcut: shortcut.map(String::into_boxed_str),
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(description.into_boxed_str()),
            disabled_reason: None,
            payload: NavigatorItemPayload::Command { command_id },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_heading(id: u64, line: usize, level: usize, text: String) -> Self {
        let prefix = "#".repeat(level);
        let title = format!("{prefix} {text}");
        let mut item = Self {
            id,
            title: title.into_boxed_str(),
            subtitle: Some(format!("Строка {line}").into_boxed_str()),
            category: Some(Cow::Borrowed("Заголовки")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: level.saturating_sub(1),
            preview: Some(
                format!("Заголовок уровня {level}: {text}\nСтрока: {line}").into_boxed_str(),
            ),
            disabled_reason: None,
            payload: NavigatorItemPayload::Heading { line, level, text },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }

    pub fn new_task(id: u64, line: usize, checked: bool, text: String) -> Self {
        let state = if checked {
            "Выполнено"
        } else {
            "Не выполнено"
        };
        let mut item = Self {
            id,
            title: text.into_boxed_str(),
            subtitle: Some(state.into()),
            category: Some(Cow::Borrowed("Задачи")),
            shortcut: None,
            is_folder: false,
            is_collapsed: false,
            depth: 0,
            preview: Some(
                format!("{state}\nСтрока: {line}\nEnter — перейти к задаче").into_boxed_str(),
            ),
            disabled_reason: None,
            payload: NavigatorItemPayload::Task { line, checked },
            char_mask: 0,
        };
        item.update_char_mask();
        item
    }
}

/// Action produced by interacting with the Navigator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigatorAction {
    None,
    Close,
    OpenFile {
        path: String,
    },
    RevealFolder {
        path: String,
    },
    SearchPage {
        forward: bool,
    },
    CreateFile {
        parent_dir: Option<String>,
    },
    CreateFolder {
        parent_dir: Option<String>,
    },
    RenameFile {
        path: String,
    },
    DeleteFile {
        path: String,
    },
    SwitchProvider {
        mode: NavigatorMode,
    },
    SwitchView {
        view_id: truff_core::ViewId,
    },
    CloseView {
        view_id: truff_core::ViewId,
    },
    PinView {
        view_id: truff_core::ViewId,
    },
    CloseOtherViews {
        view_id: truff_core::ViewId,
    },
    ReopenView,
    OpenVault {
        path: std::path::PathBuf,
    },
    OpenLocation {
        path: String,
        byte: usize,
    },
    CopyPath {
        path: String,
    },
    ApplyColor {
        hex: &'static str,
    },
    ApplyHighlight {
        hex: &'static str,
    },
    ApplyCallout {
        kind: truff_core::markdown::CalloutKind,
    },
    RemoveStyle,
    SwitchBuffer {
        buffer_id: BufferId,
    },
    CloseBuffer {
        buffer_id: BufferId,
    },
    ExecuteCommand {
        command_id: CommandId,
    },
    JumpHeading {
        line: usize,
    },
    OpenLink {
        target: String,
    },
    SelectCustom {
        id: String,
        data: String,
    },
}

impl NavigatorAction {
    pub(crate) fn closes_navigator(&self) -> bool {
        matches!(
            self,
            Self::Close
                | Self::OpenFile { .. }
                | Self::SwitchBuffer { .. }
                | Self::SwitchView { .. }
                | Self::ExecuteCommand { .. }
                | Self::JumpHeading { .. }
                | Self::OpenLink { .. }
                | Self::OpenVault { .. }
                | Self::OpenLocation { .. }
                | Self::CopyPath { .. }
                | Self::ApplyColor { .. }
                | Self::ApplyHighlight { .. }
                | Self::ApplyCallout { .. }
                | Self::RemoveStyle
                | Self::SelectCustom { .. }
        )
    }
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
    BackTab,
    PageUp,
    PageDown,
    Home,
    End,
}

impl NavKey {
    pub(crate) fn from_core(key: truff_core::vim::Key) -> Option<Self> {
        use truff_core::vim::Key;
        Some(match key {
            Key::Char(ch) => Self::Char(ch),
            Key::Esc => Self::Escape,
            Key::Enter => Self::Enter,
            Key::Backspace => Self::Backspace,
            Key::Up => Self::Up,
            Key::Down => Self::Down,
            Key::Left => Self::Left,
            Key::Right => Self::Right,
            Key::Tab => Self::Tab,
            Key::Home => Self::Home,
            Key::End => Self::End,
            Key::PageUp => Self::PageUp,
            Key::PageDown => Self::PageDown,
            _ => return None,
        })
    }
}
