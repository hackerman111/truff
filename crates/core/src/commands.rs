use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

use crate::text::{BytePos, EditError, Transaction};
use crate::workspace::StoreJob;
use crate::workspace::buffers::{BufferId, BufferState, SnapshotKey, VaultEpoch};
use crate::workspace::views::ViewId;

/// Strongly-typed identifier for a command.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct CommandId(pub Cow<'static, str>);

impl CommandId {
    /// Constructs a `CommandId` from a static string slice.
    pub const fn from_static(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// Constructs a `CommandId` from an owned string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(Cow::Owned(id.into()))
    }

    /// Returns the command identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Actions dispatched to the `Workspace` from UI, shortcuts, commands, or tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Open a buffer by path.
    OpenPath(PathBuf),
    /// Create a new view for an existing buffer.
    CreateView(BufferId),
    /// Close an existing view.
    CloseView(ViewId),
    /// Set focus to a specific view.
    FocusView(ViewId),
    /// Move cursor and selection within a view.
    MoveCursor {
        view_id: ViewId,
        anchor: BytePos,
        head: BytePos,
    },
    /// Scroll a view to a top line.
    Scroll { view_id: ViewId, top_line: usize },
    /// Apply an editing transaction to a view's buffer.
    ApplyEdit {
        view_id: ViewId,
        transaction: Transaction,
    },
    /// Request saving a buffer to disk.
    SaveBuffer(BufferId),
    /// Force reload a buffer from disk.
    ReloadBuffer(BufferId),
    /// Explicitly discard local edits and reload the current disk content.
    ReloadDiscard(BufferId),
    /// Explicitly replace external disk content with the local snapshot.
    OverwriteBuffer(BufferId),
    /// Save to a new path without replacing any existing destination.
    SaveBufferAs { buffer_id: BufferId, path: PathBuf },
    /// Undo the last transaction in the view's buffer.
    Undo(ViewId),
    /// Redo the last undone transaction in the view's buffer.
    Redo(ViewId),
    /// Switch active vault epoch.
    SwitchVault(VaultEpoch),
    /// Create a new file on disk.
    CreateFile(PathBuf),
    /// Create a new folder on disk.
    CreateFolder(PathBuf),
    /// Rename or move a path.
    RenamePath { from: PathBuf, to: PathBuf },
    /// Move a path to trash / recovery.
    TrashPath(PathBuf),
}

/// Effects produced by `Workspace::dispatch` or `Workspace::accept`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Submit an asynchronous I/O job to the store worker.
    SubmitStoreJob(StoreJob),
    /// Notification that a view was closed.
    ViewClosed(ViewId),
    /// Notification that a view's cursor/scroll or text changed and requires redraw.
    InvalidateView(ViewId),
    /// Notification that buffer state changed (Clean, Dirty, Conflict).
    BufferStateChanged {
        buffer_id: BufferId,
        state: BufferState,
    },
    /// Status message to be displayed to the user.
    StatusMessage(String),
}

/// Specification for a command in the command registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub id: CommandId,
    pub title: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub group: Cow<'static, str>,
    pub contexts: Vec<CommandContext>,
}

/// Errors occurring during workspace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    BufferNotFound(BufferId),
    ViewNotFound(ViewId),
    EditFailed(EditError),
    StaleSnapshotKey {
        expected: SnapshotKey,
        actual: SnapshotKey,
    },
    StaleEpoch {
        current: VaultEpoch,
        received: VaultEpoch,
    },
    BufferConflict {
        buffer_id: BufferId,
    },
    BufferDirty {
        buffer_id: BufferId,
    },
    BufferAlreadyOpen(PathBuf),
    MaxViewsExceeded(usize),
    MaxBuffersExceeded(usize),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id}"),
            Self::ViewNotFound(id) => write!(f, "view not found: {id}"),
            Self::EditFailed(err) => write!(f, "edit failed: {err}"),
            Self::StaleSnapshotKey { expected, actual } => {
                write!(
                    f,
                    "stale snapshot key: received {actual:?}, expected {expected:?}"
                )
            }
            Self::StaleEpoch { current, received } => {
                write!(
                    f,
                    "stale vault epoch: received {received:?}, current {current:?}"
                )
            }
            Self::BufferConflict { buffer_id } => write!(f, "buffer conflict for {buffer_id}"),
            Self::BufferDirty { buffer_id } => write!(f, "buffer {buffer_id} has unsaved changes"),
            Self::BufferAlreadyOpen(path) => write!(f, "buffer already open: {}", path.display()),
            Self::MaxViewsExceeded(max) => write!(f, "maximum views exceeded: {max}"),
            Self::MaxBuffersExceeded(max) => write!(f, "maximum buffers exceeded: {max}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<EditError> for AppError {
    fn from(e: EditError) -> Self {
        Self::EditFailed(e)
    }
}

/// Input target admitted by a registered command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CommandContext {
    Editor,
    Rows,
    Global,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    UnknownCommand(CommandId),
    UnavailableContext {
        id: CommandId,
        context: CommandContext,
    },
}
impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(id) => write!(f, "unknown command: {id}"),
            Self::UnavailableContext { id, context } => {
                write!(f, "command {id} unavailable in {context:?}")
            }
        }
    }
}
impl std::error::Error for RegistryError {}

/// Command lookup shared by keymaps, which-key, pickers and buttons.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    specs: Vec<CommandSpec>,
}
impl Default for CommandRegistry {
    fn default() -> Self {
        let mut specs = Vec::new();
        for id in [
            "motion.left",
            "motion.right",
            "motion.up",
            "motion.down",
            "motion.first_line",
            "motion.last_line",
        ] {
            specs.push(spec(
                id,
                "motion",
                vec![CommandContext::Editor, CommandContext::Rows],
            ));
        }
        for id in [
            "motion.word_forward",
            "motion.word_backward",
            "motion.word_end",
            "motion.line_start",
            "motion.first_non_blank",
            "motion.line_end",
            "motion.screen_down",
            "motion.screen_up",
            "edit.delete_line",
            "edit.change_line",
            "edit.yank_line",
            "edit.delete_char",
            "edit.paste_after",
            "edit.paste_before",
            "edit.undo",
            "edit.redo",
            "edit.repeat",
            "edit.insert",
            "edit.append",
            "edit.insert_start",
            "edit.append_end",
            "edit.open_below",
            "edit.open_above",
            "edit.visual",
            "edit.visual_line",
            "edit.indent",
            "edit.dedent",
            "note.follow_link",
            "note.link_selection",
            "note.open_backlinks",
            "note.create",
            "note.open_target_heading",
            "style.text_color",
            "style.highlight_color",
            "style.wrap_callout",
            "style.pane_accent",
            "flash.jump_word",
            "flash.select_sentence",
            "flash.select_paragraph",
            "flash.select_syntax_object",
        ] {
            specs.push(spec(
                id,
                id.split('.').next().unwrap_or("command"),
                vec![CommandContext::Editor],
            ));
        }
        for id in [
            "buffer.open",
            "buffer.select",
            "buffer.next",
            "buffer.previous",
            "buffer.save",
            "buffer.close",
            "file.create",
            "folder.create",
            "file.rename",
            "file.move",
            "file.trash",
            "focus.left",
            "focus.right",
            "focus.up",
            "focus.down",
        ] {
            specs.push(spec(
                id,
                id.split('.').next().unwrap_or("command"),
                vec![CommandContext::Global],
            ));
        }
        Self { specs }
    }
}
fn spec(id: &'static str, group: &'static str, contexts: Vec<CommandContext>) -> CommandSpec {
    CommandSpec {
        id: CommandId::from_static(id),
        title: Cow::Borrowed(id),
        description: Cow::Borrowed(id),
        group: Cow::Borrowed(group),
        contexts,
    }
}
impl CommandRegistry {
    pub fn specs(&self) -> &[CommandSpec] {
        &self.specs
    }
    pub fn get(&self, id: &CommandId) -> Result<&CommandSpec, RegistryError> {
        self.specs
            .iter()
            .find(|s| &s.id == id)
            .ok_or_else(|| RegistryError::UnknownCommand(id.clone()))
    }
    pub fn require(
        &self,
        id: &CommandId,
        context: CommandContext,
    ) -> Result<&CommandSpec, RegistryError> {
        let spec = self.get(id)?;
        if spec.contexts.contains(&context) || spec.contexts.contains(&CommandContext::Global) {
            Ok(spec)
        } else {
            Err(RegistryError::UnavailableContext {
                id: id.clone(),
                context,
            })
        }
    }
    /// Core editor commands resolve to the same semantic commands used by the parser.
    /// Application commands retain their stable ID for the frontend dispatcher.
    pub fn vim_command(
        &self,
        id: &CommandId,
        context: CommandContext,
    ) -> Result<Option<crate::vim::Command>, RegistryError> {
        self.require(id, context)?;
        use crate::vim::parser::InsertEntry;
        use crate::vim::{Command, Mode, Motion, Operator};
        let motion = match id.as_str() {
            "motion.left" => Some(Motion::Left),
            "motion.right" => Some(Motion::Right),
            "motion.up" => Some(Motion::Up),
            "motion.down" => Some(Motion::Down),
            "motion.first_line" => Some(Motion::FirstLine),
            "motion.last_line" => Some(Motion::LastLine),
            "motion.screen_down" => Some(Motion::ScreenDown),
            "motion.screen_up" => Some(Motion::ScreenUp),
            "motion.word_forward" => Some(Motion::WordForward),
            "motion.word_backward" => Some(Motion::WordBackward),
            "motion.word_end" => Some(Motion::WordEnd),
            "motion.line_start" => Some(Motion::LineStart),
            "motion.first_non_blank" => Some(Motion::LineFirstNonBlank),
            "motion.line_end" => Some(Motion::LineEnd),
            _ => None,
        };
        if let Some(motion) = motion {
            return Ok(Some(Command::Motion { motion, count: 1 }));
        }
        let cmd = match id.as_str() {
            "edit.delete_line" => Command::OperatorMotion {
                op: Operator::Delete,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.change_line" => Command::OperatorMotion {
                op: Operator::Change,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.yank_line" => Command::OperatorMotion {
                op: Operator::Yank,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.indent" => Command::OperatorMotion {
                op: Operator::Indent,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.dedent" => Command::OperatorMotion {
                op: Operator::Dedent,
                motion: Motion::CurrentLine,
                count: 1,
            },
            "edit.delete_char" => Command::DeleteChar(1),
            "edit.paste_after" => Command::Paste {
                after: true,
                count: 1,
            },
            "edit.paste_before" => Command::Paste {
                after: false,
                count: 1,
            },
            "edit.undo" => Command::Undo,
            "edit.redo" => Command::Redo,
            "edit.repeat" => Command::RepeatLastEdit,
            "edit.insert" => Command::Insert(InsertEntry::AtCursor),
            "edit.append" => Command::Insert(InsertEntry::AfterCursor),
            "edit.insert_start" => Command::Insert(InsertEntry::LineStart),
            "edit.append_end" => Command::Insert(InsertEntry::LineEnd),
            "edit.open_below" => Command::Insert(InsertEntry::NewLineBelow),
            "edit.open_above" => Command::Insert(InsertEntry::NewLineAbove),
            "edit.visual" => Command::Visual(Mode::VisualChar),
            "edit.visual_line" => Command::Visual(Mode::VisualLine),
            _ => return Ok(None),
        };
        Ok(Some(cmd))
    }
}
