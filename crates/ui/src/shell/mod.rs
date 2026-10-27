use truff_core::commands::{Action, CommandId};
use truff_core::config::CompiledKeymap;
use truff_core::navigation::RowModel;
use truff_core::text::{Buffer, EditError};
use truff_core::vim::{Key, Mode, VimState};
use truff_core::workspace::{View, Workspace};

use crate::editor::input::InputAdapter;
use crate::overlays::{FlashKind, FlashOverlay, WhichKeyOverlay};
use crate::theme::Theme;

mod command_line;
mod editor_input;
pub(crate) use command_line::Completion;
mod flash;
mod help;
mod panels;
mod routing;
mod tree;

pub use crate::focus::{Focus, FocusManager};

/// Action resulting from feeding an input key to the Shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    Incomplete,
    EditorEdit,
    SidebarDeleteFile(String),
    SidebarSelectFile(String),
    FocusChanged(Focus),
    Ignored,
    Core(Action),
    Command(String),
    Error(String),
    VisualRow { down: bool, count: usize },
    RegisteredCommand { id: CommandId, argument: String },
    OpenNavigator(crate::navigator::NavigatorMode),
    ScrollCursor(truff_core::vim::ScrollPosition),
}

#[derive(Debug)]
pub enum InputError {
    UnavailableTarget,
    Edit(EditError),
    Vim(String),
    Workspace(truff_core::commands::AppError),
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnavailableTarget => f.write_str("Editor target unavailable"),
            Self::Edit(e) => e.fmt(f),
            Self::Vim(e) => f.write_str(e),
            Self::Workspace(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for InputError {}

/// Main application shell coordinating sidebar, editor, right panel, overlays, and focus routing.
pub struct Shell {
    focus: Focus,
    focus_manager: FocusManager,
    sidebar_model: RowModel,
    right_model: RowModel,
    right_available: bool,
    sidebar_available: bool,
    row_vim: VimState,
    workspace: Workspace,
    input: InputAdapter,
    flash: FlashOverlay,
    flash_kind: FlashKind,
    command_line: command_line::CommandLine,
    keymap: CompiledKeymap,
    custom_keymap: bool,
    binding_prefix: Vec<Key>,
    key_prefix: Vec<Key>,
    editor_vim: VimState,
    which_key: WhichKeyOverlay,
    theme: Theme,
    typography: crate::app::global_config::TypographyConfig,
    pending_prefix: Option<Key>,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    /// Creates a new Shell instance with default editor buffer and layout.
    pub fn new() -> Self {
        let mut workspace = Workspace::new();
        let buffer_id = workspace
            .buffers_mut()
            .open_from_text(None, "")
            .expect("empty scratch buffer is valid");
        workspace
            .create_view(buffer_id)
            .expect("new workspace admits one view");

        Self {
            focus: Focus::Editor,
            focus_manager: FocusManager::new(),
            sidebar_model: RowModel::new(),
            right_model: RowModel::new(),
            right_available: false,
            sidebar_available: false,
            row_vim: VimState::new(),
            workspace,
            input: InputAdapter::new(),
            flash: FlashOverlay::new(),
            flash_kind: FlashKind::Word,
            command_line: command_line::CommandLine::default(),
            keymap: routing::default_keymap()
                .compile()
                .expect("default keymap is valid"),
            custom_keymap: false,
            binding_prefix: Vec::new(),
            key_prefix: Vec::new(),
            editor_vim: VimState::new(),
            which_key: WhichKeyOverlay::new(),
            theme: Theme::default(),
            typography: crate::app::global_config::TypographyConfig::default(),
            pending_prefix: None,
        }
    }

    /// Theme settings.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Updates the visual theme without touching buffer contents or syntax structures.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn typography(&self) -> &crate::app::global_config::TypographyConfig {
        &self.typography
    }

    pub fn set_typography(&mut self, typography: crate::app::global_config::TypographyConfig) {
        self.theme.geometry.markdown_max_width = typography.max_content_width;
        self.typography = typography;
    }

    /// Access the WhichKey overlay controller.
    pub fn which_key(&self) -> &WhichKeyOverlay {
        &self.which_key
    }

    /// Mutable access to the WhichKey overlay controller.
    pub fn which_key_mut(&mut self) -> &mut WhichKeyOverlay {
        &mut self.which_key
    }

    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Takes the most recently yanked text from vim motion, if any.
    pub fn take_last_yank(&mut self) -> Option<String> {
        self.editor_vim.take_last_yank()
    }

    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspace
    }

    pub fn editor_view(&self) -> Option<&View> {
        self.workspace
            .active_view()
            .and_then(|id| self.workspace.get_view(id))
    }

    pub fn editor_buffer(&self) -> Option<&Buffer> {
        self.editor_view()
            .and_then(|view| self.workspace.buffers().get_buffer(view.buffer_id()))
    }

    pub fn mode(&self) -> Mode {
        if matches!(self.focus, Focus::Sidebar | Focus::RightPanel) {
            self.row_vim.mode
        } else {
            self.editor_vim.mode
        }
    }

    pub fn command(&self) -> Option<&str> {
        self.command_line
            .input
            .as_ref()
            .map(|input| input.text.as_str())
    }

    pub fn search_query(&self) -> Option<&str> {
        if self.focus == Focus::Editor {
            self.editor_vim.search_query()
        } else {
            self.row_vim.search_query()
        }
    }
}
