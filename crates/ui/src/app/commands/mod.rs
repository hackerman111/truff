mod appearance;
mod editing;
mod files;
mod links;

use std::path::Path;

use truff_core::commands::Action;
use truff_store::catalog::CatalogJob;

use super::{AppResult, Application};
use crate::editor::ProjectionMode;

impl Application {
    pub fn execute_command(&mut self, command: &str) -> AppResult<()> {
        let before = self
            .shell
            .editor_buffer()
            .map(|buffer| (buffer.id(), buffer.revision()));
        let result = self.execute_command_inner(command);
        if let Some(buffer) = self.shell.editor_buffer()
            && before
                .is_some_and(|(id, revision)| id == buffer.id() && revision != buffer.revision())
        {
            self.recovery_deadline = Some(
                std::time::Instant::now()
                    + if buffer.is_dirty() {
                        std::time::Duration::from_millis(750)
                    } else {
                        std::time::Duration::ZERO
                    },
            );
        }
        result
    }

    fn execute_command_inner(&mut self, command: &str) -> AppResult<()> {
        let command = command.trim();
        let (name, argument) = command
            .split_once(char::is_whitespace)
            .unwrap_or((command, ""));
        let argument = argument.trim();
        match name {
            "vault" => {
                if argument.is_empty() {
                    let config = super::global_config::GlobalConfig::load();
                    if config.recent_vaults.is_empty() {
                        self.status = "No saved vaults. Usage: :vault <path>".into();
                    } else {
                        let list = config
                            .recent_vaults
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        self.status = format!("Saved vaults: {list}");
                    }
                    Ok(())
                } else {
                    self.open_vault(Path::new(argument))
                }
            }
            "vaults" => {
                let config = super::global_config::GlobalConfig::load();
                if config.recent_vaults.is_empty() {
                    self.status = "No saved vaults".into();
                } else {
                    let list = config
                        .recent_vaults
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.status = format!("Saved vaults: {list}");
                }
                Ok(())
            }
            "e" | "open" => self.open_note(argument),
            "w" | "write" => self.save(false),
            "wq" => self.save(true),
            "w!" | "overwrite" => {
                let id = self
                    .shell
                    .editor_view()
                    .ok_or("Open a note first")?
                    .buffer_id();
                self.dispatch(Action::OverwriteBuffer(id))
            }
            "reload!" => {
                let id = self
                    .shell
                    .editor_view()
                    .ok_or("Open a note first")?
                    .buffer_id();
                self.dispatch(Action::ReloadDiscard(id))
            }
            "saveas" => {
                if argument.is_empty() {
                    return Err("Usage: :saveas <new/path.md>".into());
                }
                let path = self
                    .paths
                    .as_ref()
                    .ok_or("Open a vault first")?
                    .resolve_path(Path::new(argument))?;
                let buffer_id = self
                    .shell
                    .editor_view()
                    .ok_or("Open a note first")?
                    .buffer_id();
                self.dispatch(Action::SaveBufferAs { buffer_id, path })
            }
            "q" | "bd" => {
                if self
                    .shell
                    .editor_buffer()
                    .is_some_and(|buffer| buffer.is_dirty())
                {
                    return Err(
                        "Unsaved changes: use :w or close the window to keep recovery".into(),
                    );
                }
                if let Some(view) = self.shell.editor_view() {
                    self.dispatch(Action::CloseView(view.id()))?;
                }
                Ok(())
            }
            "color" => self.color_selection(argument),
            "link" => self.link_selection(argument),
            "backlinks" => self.query_backlinks(argument),
            "follow" => self.follow_link(argument),
            "rename" => self.rename_note(argument),
            "trash" => {
                let path = if argument.is_empty() {
                    self.active_relative_path().ok_or("Open a note first")?
                } else {
                    argument.to_owned()
                };
                self.trash(&path)
            }
            "new" => {
                if argument.is_empty() || !argument.ends_with(".md") {
                    return Err("Usage: :new <path.md>".into());
                }
                self.start_file_operation(CatalogJob::Create {
                    path: argument.to_owned(),
                })
            }
            "theme" => self.apply_theme_command(argument),
            "settings" | "config" => self.apply_settings_command(argument),
            "source" => {
                self.projection = ProjectionMode::Source;
                Ok(())
            }
            "preview" => {
                self.projection = ProjectionMode::LivePreview;
                Ok(())
            }
            "help" => {
                self.shell
                    .which_key_mut()
                    .show_help(crate::overlays::WhichKeyOverlay::default_help_categories());
                self.status = "Which-key справка открыта (Esc для закрытия)".into();
                Ok(())
            }
            _ => Err(format!("Unknown command: {command}").into()),
        }
    }
}
