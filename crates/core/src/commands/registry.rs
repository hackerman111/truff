use super::{CommandContext, CommandId, CommandSpec, RegistryError};

/// Command lookup shared by keymaps, which-key, pickers and buttons.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    pub(super) specs: Vec<CommandSpec>,
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
