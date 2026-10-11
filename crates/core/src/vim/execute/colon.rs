use super::{InputContext, InputResult, VimState};
use crate::commands::{Action, CommandId};

impl VimState {
    pub(super) fn colon_command(&mut self, cmd: &str, ctx: InputContext<'_>) -> InputResult {
        let trimmed = cmd.trim();
        match trimmed {
            "w" => {
                if let InputContext::Editor { buffer, .. } = ctx {
                    return InputResult::Action(Action::SaveBuffer(buffer.id()));
                }
                InputResult::Consumed
            }
            "q" => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    if buffer.is_dirty() {
                        let err = "No write since last change (use ! to override)";
                        self.last_error = Some(err.to_string());
                        return InputResult::Error(err.to_string());
                    }
                    return InputResult::Action(Action::CloseView(view.id()));
                }
                InputResult::Consumed
            }
            "q!" => {
                if let InputContext::Editor { view, .. } = ctx {
                    return InputResult::Action(Action::CloseView(view.id()));
                }
                InputResult::Consumed
            }
            "wq" => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    return InputResult::Actions(vec![
                        Action::SaveBuffer(buffer.id()),
                        Action::CloseView(view.id()),
                    ]);
                }
                InputResult::Consumed
            }
            "bd" => {
                if let InputContext::Editor { buffer, view, .. } = ctx {
                    if buffer.is_dirty() {
                        let err = "No write since last change (use ! to override)";
                        self.last_error = Some(err.to_string());
                        return InputResult::Error(err.to_string());
                    }
                    return InputResult::Action(Action::CloseView(view.id()));
                }
                InputResult::Consumed
            }
            _ => {
                let (name, argument) = trimmed.split_once(' ').unwrap_or((trimmed, ""));
                let id = match name {
                    "e" => "buffer.open",
                    "b" => "buffer.select",
                    "bn" => "buffer.next",
                    "bp" => "buffer.previous",
                    _ => return self.error(format!("Unknown command: {name}")),
                };
                InputResult::Command {
                    id: CommandId::new(id),
                    argument: argument.into(),
                }
            }
        }
    }
}
