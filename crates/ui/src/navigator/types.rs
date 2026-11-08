use truff_core::commands::CommandId;
use truff_core::text::BufferId;

pub use super::item::{NavigatorItem, compute_char_mask};

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
    HubTask(super::tasks::HubTaskOp),
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
    Delete,
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
            Key::Delete => Self::Delete,
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
