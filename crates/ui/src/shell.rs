use truff_core::commands::{Action, CommandContext, CommandId, CommandRegistry, Effect};
use truff_core::config::{BindingLookup, CompiledKeymap, KeymapConfig, KeymapError};
use truff_core::navigation::{Move, RowId, RowItem, RowModel};
use truff_core::text::{Buffer, BytePos, EditError, Transaction};
use truff_core::vim::motion::{SyntaxObjectRange, snap_grapheme};
use truff_core::vim::{InputContext, InputResult, Key, Mode, VimState};
use truff_core::workspace::BufferChange;
use truff_core::workspace::{View, Workspace};

use crate::editor::input::{EditorInput, InputAdapter};

use crate::overlays::{FlashKind, FlashOverlay, WhichKeyOverlay};
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
    Core(Action),
    Command(String),
    Error(String),
    VisualRow { down: bool, count: usize },
    RegisteredCommand { id: CommandId, argument: String },
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
    sidebar_files: Vec<String>,
    sidebar_model: RowModel,
    right_model: RowModel,
    row_vim: VimState,
    workspace: Workspace,
    input: InputAdapter,
    flash: FlashOverlay,
    flash_kind: FlashKind,
    command: Option<String>,
    keymap: CompiledKeymap,
    custom_keymap: bool,
    binding_prefix: Vec<Key>,
    key_prefix: Vec<Key>,
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
            sidebar_files: Vec::new(),
            sidebar_model: RowModel::new(),
            right_model: RowModel::new(),
            row_vim: VimState::new(),
            workspace,
            input: InputAdapter::new(),
            flash: FlashOverlay::new(),
            flash_kind: FlashKind::Word,
            command: None,
            keymap: KeymapConfig::default()
                .compile()
                .expect("default keymap is valid"),
            custom_keymap: false,
            binding_prefix: Vec::new(),
            key_prefix: Vec::new(),
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
        if self.focus != focus {
            self.reset_editor_input();
        }
        self.focus = focus;
        self.pending_prefix = None;
        self.key_prefix.clear();
        self.binding_prefix.clear();
        self.row_vim.cancel_input();
        self.which_key.hide();
        self.flash.cancel();
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

    pub(crate) fn select_sidebar_file(&mut self, path: &str) {
        if let Some(index) = self.sidebar_files.iter().position(|file| file == path) {
            self.sidebar_model.move_selection(Move::Top);
            self.sidebar_model.move_selection(Move::Down(index));
        }
    }

    /// Sets initial text in the editor buffer.
    pub fn set_editor_text(&mut self, text: &str) {
        self.workspace = Workspace::new();
        let id = self
            .workspace
            .buffers_mut()
            .open_from_text(None, text)
            .expect("test text satisfies buffer limits");
        self.workspace
            .create_view(id)
            .expect("new workspace admits one view");
        self.reset_editor_input();
    }

    /// Current text in the editor buffer.
    pub fn editor_text(&self) -> String {
        self.editor_buffer().map(Buffer::as_str).unwrap_or_default()
    }

    /// Theme settings.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Updates the visual theme without touching buffer contents or syntax structures.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
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
        self.editor_vim.mode
    }

    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub fn flash(&self) -> &FlashOverlay {
        &self.flash
    }

    pub fn start_flash(&mut self, range: std::ops::Range<usize>) {
        let _ = self.start_flash_kind(range, FlashKind::Word);
    }
    /// Scans only the requested visible source slice. A chosen sentence/paragraph
    /// is expanded by the core object resolver after its label is selected.
    pub fn start_flash_kind(
        &mut self,
        range: std::ops::Range<usize>,
        kind: FlashKind,
    ) -> Result<(), InputError> {
        self.start_flash_ranges(&[range], kind)
    }
    /// Actual shaped source windows, rather than their possibly giant source envelope.
    /// At most 1024 windows and 1 MiB of source text may be scanned per activation.
    pub fn start_flash_ranges(
        &mut self,
        ranges: &[std::ops::Range<usize>],
        kind: FlashKind,
    ) -> Result<(), InputError> {
        use unicode_segmentation::UnicodeSegmentation;
        let buffer = self.editor_buffer().ok_or(InputError::UnavailableTarget)?;
        if ranges.len() > 1024 {
            return Err(InputError::Vim(
                "Flash viewport window limit exceeded".into(),
            ));
        }
        let mut total = 0usize;
        for range in ranges {
            if range.start > range.end || range.end > buffer.byte_len() {
                return Err(InputError::Vim("Invalid Flash viewport range".into()));
            }
            if !buffer.is_char_boundary(range.start) || !buffer.is_char_boundary(range.end) {
                return Err(InputError::Vim(
                    "Flash viewport is not a UTF-8 boundary".into(),
                ));
            }
            total = total.saturating_add(range.len());
        }
        if total > 1024 * 1024 {
            return Err(InputError::Vim(
                "Flash viewport text limit exceeded; pass shaped source windows".into(),
            ));
        }
        // A remapped operator prefix must reach the Vim parser before Flash consumes it.
        if !self.binding_prefix.is_empty() {
            let prefix = std::mem::take(&mut self.binding_prefix);
            let custom = self.custom_keymap;
            self.custom_keymap = false;
            for key in prefix {
                if let ShellAction::Error(error) = self.feed_key(key) {
                    self.custom_keymap = custom;
                    return Err(InputError::Vim(error));
                }
            }
            self.custom_keymap = custom;
        }
        let buffer = self.editor_buffer().ok_or(InputError::UnavailableTarget)?;
        let mut targets = Vec::new();
        if kind == FlashKind::SyntaxObject {
            let objects = self
                .editor_vim
                .syntax_object_ranges(buffer.revision())
                .ok_or_else(|| {
                    InputError::Vim("Syntax targets unavailable at this revision".into())
                })?;
            targets.extend(
                objects
                    .iter()
                    .filter(|o| {
                        ranges
                            .iter()
                            .any(|range| o.around.start < range.end && o.around.end > range.start)
                    })
                    .take(100)
                    .map(|o| o.around.clone()),
            );
        } else {
            for range in ranges {
                if targets.len() == 100 {
                    break;
                }
                let visible = buffer.slice(range.clone()).to_string();
                let candidates = match kind {
                    FlashKind::Word => FlashOverlay::generate_targets(&visible, range.start)
                        .into_iter()
                        .map(|t| t.source_range)
                        .collect::<Vec<_>>(),
                    FlashKind::Sentence => visible
                        .split_sentence_bound_indices()
                        .filter(|(_, s)| !s.trim().is_empty())
                        .take(100)
                        .map(|(offset, s)| {
                            let whitespace = s.len() - s.trim_start().len();
                            range.start + offset + whitespace..range.start + offset + s.len()
                        })
                        .collect(),
                    FlashKind::Paragraph => {
                        let mut candidates = Vec::new();
                        let mut offset = range.start;
                        let mut start = None;
                        for line in visible.split_inclusive('\n') {
                            if line.trim().is_empty() {
                                if let Some(start) = start.take() {
                                    candidates.push(start..offset);
                                }
                            } else if start.is_none() {
                                start = Some(offset);
                            }
                            offset += line.len();
                            if candidates.len() == 100 {
                                break;
                            }
                        }
                        if candidates.len() < 100
                            && let Some(start) = start
                        {
                            candidates.push(start..offset);
                        }
                        candidates
                    }
                    FlashKind::SyntaxObject => unreachable!(),
                };
                for target in candidates {
                    if !targets.contains(&target) {
                        targets.push(target);
                    }
                    if targets.len() == 100 {
                        break;
                    }
                }
            }
        }
        self.flash.activate_ranges(targets);
        self.flash_kind = kind;
        Ok(())
    }

    pub fn reset_editor_input(&mut self) {
        self.editor_vim.cancel_input();
        self.editor_vim.clear_syntax_objects();
        if let Some(id) = self.workspace.active_view()
            && let Some(view) = self.workspace.get_view_mut(id)
        {
            view.set_cursor(view.head(), view.head());
        }
        self.input = InputAdapter::new();
        self.flash.cancel();
        self.key_prefix.clear();
        self.which_key.hide();
        self.command = None;
    }

    pub fn ime_preedit(&self) -> Option<&str> {
        self.input.ime_preedit()
    }

    pub fn set_keymap(&mut self, config: KeymapConfig) -> Result<(), KeymapError> {
        self.keymap = config.compile()?;
        self.custom_keymap = true;
        self.binding_prefix.clear();
        Ok(())
    }
    /// Same registry resolution used by keyboard bindings, picker rows and buttons.
    pub fn execute_registered(&mut self, id: CommandId, argument: String) -> ShellAction {
        let context = if self.focus == Focus::Editor {
            CommandContext::Editor
        } else {
            CommandContext::Rows
        };
        let registry = CommandRegistry::default();
        let command = match registry.vim_command(&id, context) {
            Ok(c) => c,
            Err(e) => return ShellAction::Error(e.to_string()),
        };
        let Some(command) = command else {
            return ShellAction::RegisteredCommand { id, argument };
        };
        let result = if self.focus == Focus::Editor {
            let Some(view_id) = self.workspace.active_view() else {
                return ShellAction::Error("Editor target unavailable".into());
            };
            let Some((buffer, view)) = self.workspace.editor_parts_mut(view_id) else {
                return ShellAction::Error("Editor target unavailable".into());
            };
            let result = self.editor_vim.run_binding(
                command,
                InputContext::Editor {
                    view_id,
                    buffer,
                    view,
                },
            );
            if let Err(e) = self.publish_vim_changes(view_id) {
                return ShellAction::Error(e.to_string());
            }
            result
        } else {
            let model = if self.focus == Focus::Sidebar {
                &mut self.sidebar_model
            } else {
                &mut self.right_model
            };
            self.row_vim.run_binding(command, InputContext::Rows(model))
        };
        match result {
            InputResult::Error(e) => ShellAction::Error(e),
            InputResult::VisualRow { down, count } => ShellAction::VisualRow { down, count },
            InputResult::Action(action) => ShellAction::Core(action),
            _ => ShellAction::EditorEdit,
        }
    }
    pub fn right_model(&self) -> &RowModel {
        &self.right_model
    }
    pub fn right_model_mut(&mut self) -> &mut RowModel {
        &mut self.right_model
    }
    pub fn search_query(&self) -> Option<&str> {
        if self.focus == Focus::Editor {
            self.editor_vim.search_query()
        } else {
            self.row_vim.search_query()
        }
    }
    pub fn set_syntax_objects(&mut self, revision: u64, objects: Vec<SyntaxObjectRange>) {
        self.editor_vim.set_syntax_objects(revision, objects);
    }
    pub fn text_object_range(
        &self,
        obj: truff_core::vim::TextObject,
    ) -> Result<std::ops::Range<usize>, InputError> {
        let buffer = self.editor_buffer().ok_or(InputError::UnavailableTarget)?;
        let view = self.editor_view().ok_or(InputError::UnavailableTarget)?;
        self.editor_vim
            .text_object_range(buffer, view.head(), obj)
            .map_err(InputError::Vim)
    }
    /// Shared source range entry for Flash words, sentences, paragraphs and syntax targets.
    pub fn apply_target_range(&mut self, range: std::ops::Range<usize>) -> ShellAction {
        let Some(view_id) = self.workspace.active_view() else {
            return ShellAction::Error("Editor target unavailable".into());
        };
        let Some((buffer, view)) = self.workspace.editor_parts_mut(view_id) else {
            return ShellAction::Error("Editor target unavailable".into());
        };
        let result = self.editor_vim.target_range(
            range,
            InputContext::Editor {
                view_id,
                buffer,
                view,
            },
        );
        if let Err(e) = self.publish_vim_changes(view_id) {
            return ShellAction::Error(e.to_string());
        }
        match result {
            InputResult::Error(e) => ShellAction::Error(e),
            _ => ShellAction::EditorEdit,
        }
    }
    pub fn take_buffer_changes(&mut self) -> Vec<BufferChange> {
        self.workspace.take_buffer_changes()
    }
    /// Renderer resolves gj/gk, then returns the source byte position here.
    pub fn apply_visual_row(&mut self, target: usize) -> Result<(), InputError> {
        let id = self
            .workspace
            .active_view()
            .ok_or(InputError::UnavailableTarget)?;
        let (buffer, view) = self
            .workspace
            .editor_parts_mut(id)
            .ok_or(InputError::UnavailableTarget)?;
        let pos = BytePos::new(snap_grapheme(buffer, target));
        self.editor_vim.apply_visual_row(view, pos);
        Ok(())
    }
    fn publish_vim_changes(
        &mut self,
        view_id: truff_core::workspace::ViewId,
    ) -> Result<(), InputError> {
        let changes = self.editor_vim.take_changes();
        self.workspace
            .publish_editor_changes(view_id, &changes)
            .map_err(InputError::Workspace)
    }
    /// Single persistent commit path shared by keyboard Text, IME commit and Paste.
    pub fn commit_text(&mut self, text: &str) -> Result<(), InputError> {
        if self.focus != Focus::Editor {
            return Err(InputError::UnavailableTarget);
        }
        let id = self
            .workspace
            .active_view()
            .ok_or(InputError::UnavailableTarget)?;
        let (buffer, view) = self
            .workspace
            .editor_parts_mut(id)
            .ok_or(InputError::UnavailableTarget)?;
        let result = self.editor_vim.commit_text(
            text,
            InputContext::Editor {
                view_id: id,
                buffer,
                view,
            },
        );
        self.publish_vim_changes(id)?;
        if let InputResult::Error(e) = result {
            return Err(InputError::Vim(e));
        }
        Ok(())
    }
    pub fn apply_input(&mut self, input: EditorInput) -> Result<(), InputError> {
        match input {
            EditorInput::InsertText(text) | EditorInput::Paste(text) => {
                return self.commit_text(&text);
            }
            EditorInput::ImeCommit(text) => {
                self.apply_input(EditorInput::ImeCancel)?;
                return self.commit_text(&text);
            }
            EditorInput::ImeDeleteSurrounding {
                before_chars,
                after_chars,
            } => {
                let id = self
                    .workspace
                    .active_view()
                    .ok_or(InputError::UnavailableTarget)?;
                let (buffer, view) = self
                    .workspace
                    .editor_parts_mut(id)
                    .ok_or(InputError::UnavailableTarget)?;
                let result = self.editor_vim.run_command(
                    truff_core::vim::Command::DeleteSurrounding {
                        before_chars,
                        after_chars,
                    },
                    InputContext::Editor {
                        view_id: id,
                        buffer,
                        view,
                    },
                );
                self.publish_vim_changes(id)?;
                if let InputResult::Error(e) = result {
                    return Err(InputError::Vim(e));
                }
                return Ok(());
            }
            EditorInput::DeleteBackward | EditorInput::DeleteForward
                if self.mode() == Mode::Insert =>
            {
                let key = if input == EditorInput::DeleteBackward {
                    Key::Backspace
                } else {
                    Key::Delete
                };
                if let ShellAction::Error(e) = self.feed_key(key) {
                    return Err(InputError::Vim(e));
                }
                return Ok(());
            }
            _ => {}
        }
        let id = self
            .workspace
            .active_view()
            .ok_or(InputError::UnavailableTarget)?;
        let (buffer, view) = self
            .workspace
            .editor_parts_mut(id)
            .ok_or(InputError::UnavailableTarget)?;
        let mut cursor = view.head().get();
        let mut selection = (view.anchor() != view.head()).then(|| {
            (
                view.anchor().get().min(cursor),
                view.anchor().get().max(cursor),
            )
        });
        let changes = self
            .input
            .handle_input(buffer, &mut cursor, &mut selection, input)
            .map_err(InputError::Edit)?;
        if let Some(changes) = &changes {
            view.apply_change_set(changes);
        }
        let (anchor, head) = selection
            .map(|(start, end)| (if cursor == start { end } else { start }, cursor))
            .unwrap_or((cursor, cursor));
        view.set_cursor(BytePos::new(anchor), BytePos::new(head));
        let changes = changes.into_iter().collect::<Vec<_>>();
        self.workspace
            .publish_editor_changes(id, &changes)
            .map_err(InputError::Workspace)
    }

    pub fn apply_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Result<Vec<Effect>, truff_core::commands::AppError> {
        let Some(view_id) = self.workspace.active_view() else {
            return Ok(Vec::new());
        };
        self.workspace.dispatch(Action::ApplyEdit {
            view_id,
            transaction,
        })
    }

    /// Dispatches a key event according to focus routing rules.
    pub fn feed_key(&mut self, key: Key) -> ShellAction {
        if let Some(command) = &mut self.command {
            match key {
                Key::Esc => {
                    self.command = None;
                }
                Key::Enter => {
                    return ShellAction::Command(self.command.take().unwrap());
                }
                Key::Backspace => {
                    command.pop();
                }
                Key::Char(ch) => {
                    if command.len() + ch.len_utf8() > 65536 {
                        return ShellAction::Error("Command text limit exceeded".into());
                    }
                    command.push(ch);
                }
                _ => {}
            }
            return ShellAction::Incomplete;
        }
        if self.flash.is_active() {
            match key {
                Key::Esc => {
                    self.flash.cancel();
                    self.editor_vim.cancel_input();
                    self.key_prefix.clear();
                    self.which_key.hide();
                }
                Key::Char(ch) => {
                    if let Some(mut range) = self.flash.feed_char(ch) {
                        use truff_core::vim::{TextObject, TextObjectKind, TextObjectTarget};
                        let target = match self.flash_kind {
                            FlashKind::Sentence => Some(TextObjectTarget::Sentence),
                            FlashKind::Paragraph => Some(TextObjectTarget::Paragraph),
                            _ => None,
                        };
                        if let Some(target) = target
                            && let Some(buffer) = self.editor_buffer()
                            && let Some(expanded) = truff_core::vim::motion::eval_text_object_range(
                                buffer,
                                BytePos::new(range.start),
                                TextObject {
                                    kind: TextObjectKind::Around,
                                    target,
                                },
                            )
                        {
                            range = expanded;
                        }
                        return self.apply_target_range(range);
                    }
                }
                _ => {}
            }
            return ShellAction::EditorEdit;
        }
        if key == Key::Char(':') && (self.focus != Focus::Editor || self.mode() != Mode::Insert) {
            self.command = Some(String::new());
            self.which_key.hide();
            return ShellAction::Incomplete;
        }
        // 1. Handle pane navigation prefixes (<C-w>h / <C-w>l)
        if let Some(Key::Ctrl('w')) = self.pending_prefix {
            self.pending_prefix = None;
            self.which_key.hide();
            match key {
                Key::Char('h') | Key::Left => {
                    let next = match self.focus {
                        Focus::RightPanel => Focus::Editor,
                        Focus::Editor => Focus::Sidebar,
                        Focus::Sidebar => Focus::Sidebar,
                    };
                    self.set_focus(next);
                    return ShellAction::FocusChanged(next);
                }
                Key::Char('l') | Key::Right => {
                    let next = match self.focus {
                        Focus::Sidebar => Focus::Editor,
                        Focus::Editor => Focus::RightPanel,
                        Focus::RightPanel => Focus::RightPanel,
                    };
                    self.set_focus(next);
                    return ShellAction::FocusChanged(next);
                }
                Key::Char('j' | 'k') | Key::Up | Key::Down => {
                    return ShellAction::FocusChanged(self.focus);
                }
                _ => return ShellAction::Ignored,
            }
        }

        if key == Key::Ctrl('w') {
            self.pending_prefix = Some(key);
            self.which_key.set_prefix_hints(vec![
                truff_core::config::KeyHint {
                    key: Key::Char('h'),
                    description: "Focus left pane".into(),
                    is_prefix: false,
                },
                truff_core::config::KeyHint {
                    key: Key::Char('l'),
                    description: "Focus right pane".into(),
                    is_prefix: false,
                },
                truff_core::config::KeyHint {
                    key: Key::Char('j'),
                    description: "Focus lower pane".into(),
                    is_prefix: false,
                },
                truff_core::config::KeyHint {
                    key: Key::Char('k'),
                    description: "Focus upper pane".into(),
                    is_prefix: false,
                },
            ]);
            return ShellAction::Incomplete;
        }

        if self.custom_keymap
            && self.mode() != Mode::Insert
            && self.search_query().is_none()
            && key != Key::Esc
        {
            // Counts are parsed by the canonical Vim parser before the binding resolves.
            if !matches!(key, Key::Char('0'..='9')) || !self.binding_prefix.is_empty() {
                self.binding_prefix.push(key);
                let context = if self.focus == Focus::Editor {
                    CommandContext::Editor
                } else {
                    CommandContext::Rows
                };
                let mode = if self.focus == Focus::Editor {
                    self.mode()
                } else {
                    Mode::Normal
                };
                match self.keymap.lookup(mode, &self.binding_prefix, context) {
                    BindingLookup::Complete(id) => {
                        self.binding_prefix.clear();
                        self.which_key.hide();
                        return self.execute_registered(id, String::new());
                    }
                    BindingLookup::Prefix => {
                        self.which_key.set_prefix_hints(self.keymap.hints(
                            mode,
                            &self.binding_prefix,
                            context,
                        ));
                        return ShellAction::Incomplete;
                    }
                    BindingLookup::Missing => {
                        let mut prefix = std::mem::take(&mut self.binding_prefix);
                        prefix.pop();
                        self.custom_keymap = false;
                        for key in prefix {
                            self.feed_key(key);
                        }
                        self.custom_keymap = true;
                    }
                }
            }
        }
        if key == Key::Esc {
            self.binding_prefix.clear();
        }

        // 2. Route strictly according to active panel focus
        match self.focus {
            Focus::Editor => {
                let Some(view_id) = self.workspace.active_view() else {
                    return ShellAction::Ignored;
                };
                let Some((buffer, view)) = self.workspace.editor_parts_mut(view_id) else {
                    return ShellAction::Ignored;
                };
                let buffer_id = buffer.id();
                let res = self.editor_vim.feed_key(
                    key,
                    InputContext::Editor {
                        view_id,
                        buffer,
                        view,
                    },
                );
                let _ = buffer_id;
                if let Err(e) = self.publish_vim_changes(view_id) {
                    return ShellAction::Error(e.to_string());
                }
                if res != InputResult::Incomplete {
                    self.key_prefix.clear();
                    self.which_key.hide();
                }
                match res {
                    InputResult::Incomplete => {
                        self.key_prefix.push(key);
                        self.which_key.set_prefix_hints(self.keymap.hints(
                            self.mode(),
                            &self.key_prefix,
                            CommandContext::Editor,
                        ));
                        ShellAction::Incomplete
                    }
                    InputResult::VisualRow { down, count } => {
                        ShellAction::VisualRow { down, count }
                    }
                    InputResult::Command { id, argument } => {
                        ShellAction::RegisteredCommand { id, argument }
                    }
                    InputResult::Action(action) => ShellAction::Core(action),
                    InputResult::Actions(actions) => {
                        if actions.len() == 2
                            && matches!(actions[0], Action::SaveBuffer(_))
                            && matches!(actions[1], Action::CloseView(_))
                        {
                            ShellAction::Command("wq".into())
                        } else {
                            ShellAction::Error("Application action sequence unavailable".into())
                        }
                    }
                    InputResult::Error(error) => ShellAction::Error(error),
                    InputResult::Ignored => ShellAction::Ignored,
                    _ => {
                        self.key_prefix.clear();
                        self.which_key.hide();
                        ShellAction::EditorEdit
                    }
                }
            }
            Focus::Sidebar | Focus::RightPanel => {
                let model = if self.focus == Focus::Sidebar {
                    &mut self.sidebar_model
                } else {
                    &mut self.right_model
                };
                if key == Key::Char('d')
                    && self.focus == Focus::Sidebar
                    && self.row_vim.search_query().is_none()
                {
                    return model
                        .selected_row()
                        .and_then(|row| row.data.clone())
                        .map(ShellAction::SidebarDeleteFile)
                        .unwrap_or(ShellAction::Ignored);
                }
                if key == Key::Enter && self.row_vim.search_query().is_none() {
                    return model
                        .selected_row()
                        .and_then(|row| row.data.clone())
                        .map(ShellAction::SidebarSelectFile)
                        .unwrap_or(ShellAction::Ignored);
                }
                let result = self.row_vim.feed_key(key, InputContext::Rows(model));
                match result {
                    InputResult::Error(e) => ShellAction::Error(e),
                    InputResult::Ignored => ShellAction::Ignored,
                    _ => ShellAction::Incomplete,
                }
            }
        }
    }
}
