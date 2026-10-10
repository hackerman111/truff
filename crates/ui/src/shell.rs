use truff_core::navigation::{Move, RowId, RowItem, RowModel};
use truff_core::text::Buffer;
use truff_core::vim::{InputContext, InputResult, Key, VimState};
use truff_core::workspace::views::{View, ViewId};

use crate::overlays::WhichKeyOverlay;
use crate::theme::Theme;

/// Active focused panel in the application layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    Sidebar,
    #[default]
    Editor,
    RightPanel,
}

/// Action resulting from feeding an input key to the Shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    Incomplete,
    EditorEdit,
    SidebarDeleteFile(String),
    SidebarSelectFile(String),
    FocusChanged(Focus),
    Ignored,
}

/// Main application shell coordinating sidebar, editor, right panel, overlays, and focus routing.
pub struct Shell {
    focus: Focus,
    sidebar_files: Vec<String>,
    sidebar_model: RowModel,
    editor_buffer: Buffer,
    editor_view: View,
    editor_vim: VimState,
    which_key: WhichKeyOverlay,
    theme: Theme,
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
        let buffer = Buffer::from_str("");
        let view = View::new(ViewId::new(1), buffer.id());

        Self {
            focus: Focus::Editor,
            sidebar_files: Vec::new(),
            sidebar_model: RowModel::new(),
            editor_buffer: buffer,
            editor_view: view,
            editor_vim: VimState::new(),
            which_key: WhichKeyOverlay::new(),
            theme: Theme::default(),
            pending_prefix: None,
        }
    }

    /// Current focus.
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// Sets the focused panel.
    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.pending_prefix = None;
    }

    /// Populates sidebar files and initializes its row model.
    pub fn set_sidebar_files(&mut self, files: Vec<String>) {
        self.sidebar_files = files.clone();
        let items: Vec<RowItem> = files
            .into_iter()
            .enumerate()
            .map(|(i, f)| RowItem {
                id: RowId::new(i as u64),
                title: f.clone(),
                depth: 0,
                is_folder: false,
                is_collapsed: false,
                data: Some(f),
            })
            .collect();
        self.sidebar_model.set_items(items);
    }

    /// Returns list of files currently in the sidebar.
    pub fn sidebar_files(&self) -> Vec<String> {
        self.sidebar_files.clone()
    }

    /// Access the sidebar row model.
    pub fn sidebar_model(&self) -> &RowModel {
        &self.sidebar_model
    }

    /// Mutable access to the sidebar row model.
    pub fn sidebar_model_mut(&mut self) -> &mut RowModel {
        &mut self.sidebar_model
    }

    /// Sets initial text in the editor buffer.
    pub fn set_editor_text(&mut self, text: &str) {
        self.editor_buffer = Buffer::from_str(text);
        self.editor_view = View::new(ViewId::new(1), self.editor_buffer.id());
    }

    /// Current text in the editor buffer.
    pub fn editor_text(&self) -> String {
        self.editor_buffer.as_str()
    }

    /// Theme settings.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Access the WhichKey overlay controller.
    pub fn which_key(&self) -> &WhichKeyOverlay {
        &self.which_key
    }

    /// Mutable access to the WhichKey overlay controller.
    pub fn which_key_mut(&mut self) -> &mut WhichKeyOverlay {
        &mut self.which_key
    }

    /// Dispatches a key event according to focus routing rules.
    pub fn feed_key(&mut self, key: Key) -> ShellAction {
        // 1. Handle pane navigation prefixes (<C-w>h / <C-w>l)
        if let Some(Key::Ctrl('w')) = self.pending_prefix {
            self.pending_prefix = None;
            match key {
                Key::Char('h') | Key::Left => {
                    let next = match self.focus {
                        Focus::RightPanel => Focus::Editor,
                        Focus::Editor => Focus::Sidebar,
                        Focus::Sidebar => Focus::Sidebar,
                    };
                    self.focus = next;
                    return ShellAction::FocusChanged(next);
                }
                Key::Char('l') | Key::Right => {
                    let next = match self.focus {
                        Focus::Sidebar => Focus::Editor,
                        Focus::Editor => Focus::RightPanel,
                        Focus::RightPanel => Focus::RightPanel,
                    };
                    self.focus = next;
                    return ShellAction::FocusChanged(next);
                }
                _ => return ShellAction::Ignored,
            }
        }

        if key == Key::Ctrl('w') {
            self.pending_prefix = Some(key);
            return ShellAction::Incomplete;
        }

        // 2. Route strictly according to active panel focus
        match self.focus {
            Focus::Editor => {
                let ctx = InputContext::Editor {
                    view_id: self.editor_view.id(),
                    buffer: &mut self.editor_buffer,
                    view: &mut self.editor_view,
                };
                let res = self.editor_vim.feed_key(key, ctx);
                match res {
                    InputResult::Incomplete => ShellAction::Incomplete,
                    _ => ShellAction::EditorEdit,
                }
            }
            Focus::Sidebar => match key {
                Key::Char('d') => {
                    let selected_idx = self.sidebar_model.selected_index();
                    if selected_idx < self.sidebar_files.len() {
                        let deleted_file = self.sidebar_files.remove(selected_idx);
                        self.set_sidebar_files(self.sidebar_files.clone());
                        ShellAction::SidebarDeleteFile(deleted_file)
                    } else {
                        ShellAction::Ignored
                    }
                }
                Key::Char('j') | Key::Down => {
                    self.sidebar_model.move_selection(Move::Down(1));
                    ShellAction::Incomplete
                }
                Key::Char('k') | Key::Up => {
                    self.sidebar_model.move_selection(Move::Up(1));
                    ShellAction::Incomplete
                }
                Key::Char('G') => {
                    self.sidebar_model.move_selection(Move::Bottom);
                    ShellAction::Incomplete
                }
                Key::Char('g') => {
                    if self.pending_prefix == Some(Key::Char('g')) {
                        self.pending_prefix = None;
                        self.sidebar_model.move_selection(Move::Top);
                    } else {
                        self.pending_prefix = Some(Key::Char('g'));
                    }
                    ShellAction::Incomplete
                }
                Key::Enter => {
                    let idx = self.sidebar_model.selected_index();
                    if let Some(file) = self.sidebar_files.get(idx) {
                        ShellAction::SidebarSelectFile(file.clone())
                    } else {
                        ShellAction::Ignored
                    }
                }
                _ => ShellAction::Ignored,
            },
            Focus::RightPanel => match key {
                Key::Char('j') | Key::Down => ShellAction::Incomplete,
                Key::Char('k') | Key::Up => ShellAction::Incomplete,
                _ => ShellAction::Ignored,
            },
        }
    }
}
