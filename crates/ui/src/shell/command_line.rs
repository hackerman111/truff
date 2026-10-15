use super::{Shell, ShellAction};
use std::collections::VecDeque;
use truff_core::{
    commands::{CommandId, CommandRegistry},
    vim::Key,
};

#[derive(Clone)]
pub(crate) struct Completion {
    pub text: String,
    pub description: String,
}

pub(crate) struct CommandInput {
    pub text: String,
    pub inserting: bool,
    pub suggestions: Vec<Completion>,
    pub selected: Option<usize>,
    pub completed_text: Option<String>,
}

#[derive(Default)]
pub(super) struct CommandLine {
    pub input: Option<CommandInput>,
    history: VecDeque<String>,
    history_bytes: usize,
    history_index: Option<usize>,
    draft: String,
}

impl Shell {
    pub(crate) fn command_input(&self) -> Option<&CommandInput> {
        self.command_line.input.as_ref()
    }
    pub(crate) fn command_input_mut(&mut self) -> Option<&mut CommandInput> {
        self.command_line.input.as_mut()
    }

    pub(super) fn start_command(&mut self) {
        self.command_line.input = Some(CommandInput {
            text: String::new(),
            inserting: true,
            suggestions: Vec::new(),
            selected: None,
            completed_text: None,
        });
        self.command_line.history_index = None;
        self.command_line.draft.clear();
        self.which_key.hide();
    }

    pub(super) fn command_key(&mut self, key: Key) -> ShellAction {
        let Some(input) = self.command_line.input.as_mut() else {
            return ShellAction::Ignored;
        };
        match key {
            Key::Esc => {
                if input.inserting {
                    input.inserting = false;
                } else {
                    self.command_line.input = None;
                }
            }
            Key::Enter => {
                let input = self
                    .command_line
                    .input
                    .take()
                    .expect("active command input");
                let text = input
                    .selected
                    .and_then(|index| input.suggestions.get(index))
                    .map_or(input.text, |item| item.text.clone());
                if !text.trim().is_empty() && self.command_line.history.back() != Some(&text) {
                    self.command_line.history_bytes += text.len();
                    self.command_line.history.push_back(text.clone());
                    while self.command_line.history.len() > 128
                        || self.command_line.history_bytes > 256 * 1024
                    {
                        if let Some(old) = self.command_line.history.pop_front() {
                            self.command_line.history_bytes -= old.len();
                        }
                    }
                }
                let trimmed = text.trim();
                let (name, argument) = trimmed
                    .split_once(char::is_whitespace)
                    .unwrap_or((trimmed, ""));
                let id = CommandId::new(name);
                if CommandRegistry::default().get(&id).is_ok() {
                    return self.execute_registered(id, argument.trim().to_owned());
                }
                return ShellAction::Command(text);
            }
            Key::Char('i') if !input.inserting => {
                input.inserting = true;
                input.selected = None;
            }
            Key::Tab => {
                if let Some(suggestion) = input.suggestions.get(input.selected.unwrap_or(0)) {
                    input.text.clone_from(&suggestion.text);
                    input.selected = None;
                }
            }
            Key::Char('j' | 'k') if !input.inserting && !input.suggestions.is_empty() => {
                let index = input.selected.unwrap_or(0);
                input.selected = Some(if key == Key::Char('j') {
                    (index + 1).min(input.suggestions.len() - 1)
                } else {
                    index.saturating_sub(1)
                });
            }
            Key::Up | Key::Down | Key::Char('j' | 'k')
                if !input.inserting || matches!(key, Key::Up | Key::Down) =>
            {
                if self.command_line.history.is_empty() {
                    return ShellAction::Incomplete;
                }
                if self.command_line.history_index.is_none() {
                    self.command_line.draft.clone_from(&input.text);
                }
                let index = self
                    .command_line
                    .history_index
                    .unwrap_or(self.command_line.history.len());
                let previous = matches!(key, Key::Up | Key::Char('k'));
                let index = if previous {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(self.command_line.history.len())
                };
                self.command_line.history_index = Some(index);
                input.selected = None;
                input.text.clone_from(
                    self.command_line
                        .history
                        .get(index)
                        .unwrap_or(&self.command_line.draft),
                );
            }
            Key::Backspace if input.inserting => {
                input.text.pop();
                input.selected = None;
            }
            Key::Char(ch) if input.inserting => {
                if input.text.len() + ch.len_utf8() > 65536 {
                    return ShellAction::Error("Command text limit exceeded".into());
                }
                input.text.push(ch);
                input.selected = None;
            }
            _ => {}
        }
        ShellAction::Incomplete
    }
}
