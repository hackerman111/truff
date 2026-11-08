use truff_core::commands::{CommandId, CommandRegistry};
use truff_core::vim::{InputContext, InputResult, Key};

use crate::shell::{Focus, Shell, ShellAction};

impl Shell {
    /// Same registry resolution used by keyboard bindings, picker rows and buttons.
    pub fn execute_registered(&mut self, id: CommandId, argument: String) -> ShellAction {
        let context = self.command_context();
        let registry = CommandRegistry::default();
        if let Err(error) = registry.require(&id, context) {
            return ShellAction::Error(error.to_string());
        }
        match id.as_str() {
            "app.help" => {
                self.show_help();
                return ShellAction::Incomplete;
            }
            "app.command_line" => {
                self.start_command();
                return ShellAction::Incomplete;
            }
            "row.activate" => return self.feed_key(Key::Enter),
            "note.cycle_task" => {
                if let Some(view_id) = self.workspace.active_view() {
                    let cycle_tx =
                        self.workspace
                            .editor_parts_mut(view_id)
                            .and_then(|(buffer, view)| {
                                truff_core::markdown::plan_cycle_task(buffer, view.head())
                            });
                    if let Some(tx) = cycle_tx {
                        return self.apply_planned_tx(view_id, tx);
                    }
                }
                return ShellAction::Ignored;
            }
            "focus.left" | "focus.right" | "focus.up" | "focus.down" => {
                let next = match (id.as_str(), self.focus) {
                    ("focus.left", Focus::RightPanel) | ("focus.right", Focus::Sidebar) => {
                        Focus::Editor
                    }
                    ("focus.left", Focus::Editor) if self.has_sidebar() => Focus::Sidebar,
                    ("focus.right", Focus::Editor) if self.has_right_panel() => Focus::RightPanel,
                    _ => self.focus,
                };
                self.set_focus(next);
                return ShellAction::FocusChanged(next);
            }
            "style.text_action_menu" => return ShellAction::OpenTextActionMenu,
            "panel.boards" => return ShellAction::OpenKanban,
            _ => {}
        }

        let panel = match id.as_str() {
            "panel.files" => Some(crate::navigator::NavigatorMode::Files),
            "panel.sessions" => Some(crate::navigator::NavigatorMode::Sessions),
            "panel.outline" => Some(crate::navigator::NavigatorMode::Outline),
            "panel.links" => Some(crate::navigator::NavigatorMode::Links),
            "panel.commands" => Some(crate::navigator::NavigatorMode::Commands),
            "style.text_color" if argument.is_empty() => {
                Some(crate::navigator::NavigatorMode::Colors)
            }
            _ => None,
        };
        if let Some(mode) = panel {
            return ShellAction::OpenNavigator(mode);
        }

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
}
