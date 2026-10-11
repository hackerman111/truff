//! Stable command IDs shared by the keymap, command line and native controls.
use super::{AppResult, Application};
use truff_core::commands::{Action, CommandId};

impl Application {
    pub(crate) fn registered_command(&mut self, id: &CommandId, argument: &str) -> AppResult<()> {
        match id.as_str() {
            "buffer.next" | "buffer.previous" => {
                let mut views = self.shell.workspace().views().values().collect::<Vec<_>>();
                views.sort_by_key(|view| view.id());
                let current = self.shell.workspace().active_view();
                let index = views
                    .iter()
                    .position(|view| Some(view.id()) == current)
                    .unwrap_or(0);
                if views.is_empty() {
                    return Err("No open buffers".into());
                }
                let next = if id.as_str() == "buffer.next" {
                    (index + 1) % views.len()
                } else {
                    (index + views.len() - 1) % views.len()
                };
                self.dispatch(Action::FocusView(views[next].id()))
            }
            "buffer.select" => {
                if argument.is_empty() {
                    return Err("Usage: :b <view id or path>".into());
                }
                let matches = self
                    .shell
                    .workspace()
                    .views()
                    .values()
                    .filter(|view| {
                        view.id().to_string() == argument
                            || self
                                .shell
                                .workspace()
                                .buffers()
                                .get(view.buffer_id())
                                .and_then(|entry| entry.path())
                                .is_some_and(|path| path.to_string_lossy().ends_with(argument))
                    })
                    .map(|view| view.id())
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err("Buffer selector must match exactly one view".into());
                }
                self.dispatch(Action::FocusView(matches[0]))
            }
            _ => {
                let name = match id.as_str() {
                    "buffer.open" => "open",
                    "buffer.save" => "w",
                    "buffer.close" => "bd",
                    "note.follow_link" | "note.open_target_heading" => "follow",
                    "note.link_selection" => "link",
                    "note.open_backlinks" => "backlinks",
                    "note.create" | "file.create" => "new",
                    "folder.create" => "mkdir",
                    "file.rename" | "file.move" => "rename",
                    "file.trash" => "trash",
                    "style.text_color" => "color",
                    "style.highlight_color" => "highlight",
                    "style.wrap_callout" => "callout",
                    "style.pane_accent" => "accent",
                    _ => return Err(format!("Command requires a native target: {id}").into()),
                };
                self.execute_command(&format!("{name} {argument}"))
            }
        }
    }
}
