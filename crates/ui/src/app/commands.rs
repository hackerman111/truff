use std::path::{Path, PathBuf};

use truff_core::commands::Action;
use truff_core::markdown::facts::MarkdownFacts;
use truff_core::markdown::style::apply_text_color;
use truff_core::vim::Mode;
use truff_store::catalog::CatalogJob;
use truff_store::operations::LinkSelection;

use super::{AppResult, Application};
use crate::editor::ProjectionMode;
use crate::shell::Focus;
use crate::theme::Theme;

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
            "color" => {
                let hex = argument.strip_prefix('#').unwrap_or(argument);
                if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("Usage: :color #RRGGBB".into());
                }
                let range = self.selection_range()?;
                let text = self.shell.editor_text();
                let transaction = apply_text_color(&text, range, argument)?;
                self.shell.apply_transaction(transaction)?;
                self.shell.feed_key(truff_core::vim::Key::Esc);
                self.status = "Color applied; u to undo".into();
                Ok(())
            }
            "link" => {
                if argument.is_empty() || argument.contains(['[', ']', '\n', '\r']) {
                    return Err("Usage: :link <note or note#heading>".into());
                }
                let range = self.selection_range()?;
                let text = self.shell.editor_text();
                let label = &text[range.clone()];
                self.shell.apply_transaction(LinkSelection::wikilink(
                    range,
                    argument,
                    Some(label),
                ))?;
                self.shell.feed_key(truff_core::vim::Key::Esc);
                self.status = "Link created; :follow to open".into();
                Ok(())
            }
            "backlinks" => {
                let path = if argument.is_empty() {
                    self.active_relative_path().ok_or("Open a note first")?
                } else {
                    argument.to_owned()
                };
                self.submit_catalog(CatalogJob::Backlinks {
                    path,
                    dirty: self.dirty_texts(),
                })
            }
            "follow" => {
                let target = if argument.is_empty() {
                    let head = self
                        .shell
                        .editor_view()
                        .ok_or("Open a note first")?
                        .head()
                        .get();
                    let facts = MarkdownFacts::extract(&self.shell.editor_text())?;
                    let link = facts
                        .links
                        .iter()
                        .find(|link| link.range.start <= head && head <= link.range.end)
                        .ok_or("Place the cursor inside a link")?;
                    match &link.heading {
                        Some(heading) => format!("{}#{}", link.target, heading),
                        None => link.target.clone(),
                    }
                } else {
                    argument.to_owned()
                };
                self.submit_catalog(CatalogJob::Follow {
                    source: self.active_relative_path().ok_or("Open a note first")?,
                    target,
                })
            }
            "rename" => {
                let from = if self.shell.focus() == Focus::Sidebar {
                    self.shell
                        .sidebar_model()
                        .selected_row()
                        .and_then(|row| row.data.clone())
                        .ok_or("Select a note first")?
                } else {
                    self.active_relative_path().ok_or("Open a note first")?
                };
                if argument.is_empty() {
                    return Err("Usage: :rename <new/path.md>".into());
                }
                let paths = self.paths.as_ref().ok_or("Open a vault first")?;
                let destination = paths.resolve_path(Path::new(argument))?;
                if self
                    .shell
                    .workspace()
                    .buffers()
                    .get_by_path(&destination)
                    .is_some()
                {
                    return Err("Destination is already tracked by the workspace".into());
                }
                self.start_file_operation(CatalogJob::Rename {
                    from,
                    to: argument.to_owned(),
                    dirty: self.dirty_paths(),
                })
            }
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
            "theme" => {
                self.shell.set_theme(match argument {
                    "light" => Theme::light(),
                    "dark" => Theme::obsidian_dark(),
                    _ => return Err("Usage: :theme light|dark".into()),
                });
                Ok(())
            }
            "source" => {
                self.projection = ProjectionMode::Source;
                Ok(())
            }
            "preview" => {
                self.projection = ProjectionMode::LivePreview;
                Ok(())
            }
            "help" => {
                self.status = "i edit · Esc normal · Ctrl-w h/l panes · s Flash · :w · :color #3388ff · :link note · :backlinks · :rename name.md".into();
                Ok(())
            }
            _ => Err(format!("Unknown command: {command}").into()),
        }
    }

    pub(crate) fn save(&mut self, close: bool) -> AppResult<()> {
        if self.file_operation_pending() {
            return Err("Wait for the file operation to finish".into());
        }
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let id = view.buffer_id();
        let view_id = view.id();
        if self.active_path().is_none() {
            return Err("Scratch has no path; open a vault and use :new name.md".into());
        }
        self.dispatch(Action::SaveBuffer(id))?;
        if close {
            self.close_after_save.insert(id, view_id);
        }
        Ok(())
    }

    pub(crate) fn trash(&mut self, path: &str) -> AppResult<()> {
        self.start_file_operation(CatalogJob::Trash {
            path: path.to_owned(),
            dirty: self.dirty_paths(),
        })
    }

    fn start_file_operation(&mut self, job: CatalogJob) -> AppResult<()> {
        if self.file_operation_pending()
            || !self.pending_saves.is_empty()
            || !self.pending_loads.is_empty()
        {
            return Err("Wait for pending file I/O to finish".into());
        }
        self.submit_catalog(job)?;
        // Freeze edits until apply + affected-buffer reload completes, avoiding dirty overwrite races.
        self.file_operation_pending = true;
        self.status = "Applying file operation…".into();
        Ok(())
    }

    pub(super) fn submit_catalog(&mut self, job: CatalogJob) -> AppResult<()> {
        self.catalog
            .as_ref()
            .ok_or("Open a vault first")?
            .try_submit(job)?;
        self.pending_catalog += 1;
        Ok(())
    }

    pub(crate) fn selection_range(&self) -> AppResult<std::ops::Range<usize>> {
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let buffer = self.shell.editor_buffer().ok_or("Note is still loading")?;
        let start = view.anchor().get().min(view.head().get());
        let mut end = view.anchor().get().max(view.head().get());
        match self.shell.mode() {
            Mode::VisualChar => {
                end = buffer
                    .grapheme_next(truff_core::text::BytePos::new(end))
                    .map(|pos| pos.get())
                    .unwrap_or(end)
            }
            Mode::VisualLine => {
                let first = buffer.byte_of_line(buffer.line_of_byte(start));
                let last = buffer.line_of_byte(end) + 1;
                return Ok(first..if last < buffer.line_len() {
                    buffer.byte_of_line(last)
                } else {
                    buffer.byte_len()
                });
            }
            _ if start == end => return Err("Select text with v or V first".into()),
            _ => {}
        }
        Ok(start..end)
    }

    fn dirty_paths(&self) -> Vec<PathBuf> {
        let Some(paths) = &self.paths else {
            return Vec::new();
        };
        self.shell
            .workspace()
            .buffers()
            .entries()
            .filter(|entry| entry.buffer().is_some_and(|buffer| buffer.is_dirty()))
            .filter_map(|entry| {
                paths
                    .relative_path(entry.path()?)
                    .ok()
                    .map(Path::to_path_buf)
            })
            .collect()
    }

    fn dirty_texts(&self) -> Vec<(String, String)> {
        let Some(paths) = &self.paths else {
            return Vec::new();
        };
        self.shell
            .workspace()
            .buffers()
            .entries()
            .filter_map(|entry| {
                let buffer = entry.buffer()?;
                if !buffer.is_dirty() {
                    return None;
                }
                Some((
                    paths
                        .relative_path(entry.path()?)
                        .ok()?
                        .to_string_lossy()
                        .into_owned(),
                    buffer.as_str(),
                ))
            })
            .collect()
    }
}
