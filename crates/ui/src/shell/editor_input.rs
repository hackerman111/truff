use truff_core::commands::{Action, Effect};
use truff_core::text::{Buffer, BytePos, Transaction};
use truff_core::vim::motion::{SyntaxObjectRange, snap_grapheme};
use truff_core::vim::{InputContext, InputResult, Key, Mode};
use truff_core::workspace::{BufferChange, Workspace};

use crate::editor::input::{EditorInput, InputAdapter};

use super::{Focus, InputError, Shell, ShellAction};

impl Shell {
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
        self.command_line.input = None;
    }

    pub fn ime_preedit(&self) -> Option<&str> {
        self.input.ime_preedit()
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

    pub(super) fn publish_vim_changes(
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
}
