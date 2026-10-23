mod appearance;
mod editing;
mod files;
mod links;

use std::path::Path;

use truff_core::commands::Action;

use super::{AppResult, Application};
use crate::editor::ProjectionMode;

pub(crate) fn command_parts(command: &str) -> (&str, &str) {
    let command = command.trim();
    let (name, argument) = command
        .split_once(char::is_whitespace)
        .unwrap_or((command, ""));
    (name, argument.trim())
}

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
        let (name, argument) = command_parts(command);
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
            "new" => self.create_note(argument),
            "mkdir" => self.create_folder(argument),
            "theme" => self.apply_theme_command(argument),
            "settings" | "config" => self.apply_settings_command(argument),
            "source" => {
                self.set_projection(ProjectionMode::Source);
                Ok(())
            }
            "preview" => {
                self.set_projection(ProjectionMode::LivePreview);
                Ok(())
            }
            "reading" => {
                self.set_projection(ProjectionMode::Reading);
                Ok(())
            }
            "focus" => {
                self.focus_mode = !self.focus_mode;
                self.shell.set_focus(crate::shell::Focus::Editor);
                self.status = if self.focus_mode {
                    "Focus mode · :focus для возврата панелей"
                } else {
                    "Панели восстановлены"
                }
                .into();
                Ok(())
            }
            "help" => {
                self.shell.show_help();
                self.status = "Which-key справка открыта (Esc для закрытия)".into();
                Ok(())
            }
            "toc" => {
                self.toc_requested = Some(argument.to_owned());
                self.status = "Оглавление (Floating TOC)".into();
                Ok(())
            }
            "wiki" | "folder" => {
                self.wiki_requested = Some(argument.to_owned());
                self.status = "Вики папки (Folder Wiki)".into();
                Ok(())
            }
            "stream" => {
                self.stream_requested = Some(argument.to_owned());
                self.status = "Лента заметок (Note Stream)".into();
                Ok(())
            }
            "graph" => {
                if argument.eq_ignore_ascii_case("dock") || argument.eq_ignore_ascii_case("panel") {
                    self.right_dock_tab_requested = Some(3);
                    self.right_dock_visible = true;
                    self.status = "Граф открыт в правой панели".into();
                } else {
                    self.graph_view_open = !self.graph_view_open;
                    if self.graph_view_open {
                        self.database_view_open = false;
                    }
                    self.status = if self.graph_view_open {
                        "Интерактивный граф открыт (:graph для возврата)"
                    } else {
                        "Редактор"
                    }
                    .into();
                }
                Ok(())
            }
            "kanban" | "board" => {
                let next_state = !self.is_active_note_kanban();
                self.kanban_mode_override = Some(next_state);
                self.graph_view_open = false;
                self.database_view_open = false;
                self.status = if next_state {
                    "Режим Канбан-доски включен (:kanban для возврата в текст)"
                } else {
                    "Текстовый режим редактора"
                }
                .into();
                Ok(())
            }
            "database" | "sql" | "db" => {
                self.database_view_open = !self.database_view_open;
                if self.database_view_open {
                    self.graph_view_open = false;
                }
                self.status = if self.database_view_open {
                    "SQL база данных открыта (:database или Esc для возврата)"
                } else {
                    "Редактор"
                }
                .into();
                Ok(())
            }
            "tags" => {
                self.right_dock_tab_requested = Some(2);
                self.right_dock_visible = true;
                self.status = "Панель тегов открыта справа".into();
                Ok(())
            }
            "template" => {
                let kind = if argument.is_empty() {
                    truff_core::markdown::NoteTemplateKind::ZettelkastenPermanent
                } else {
                    truff_core::markdown::NoteTemplateKind::parse_alias(argument)
                        .ok_or("Usage: :template <zk | lit | fleeting | wiki | kanban | sql>")?
                };
                let title = self
                    .active_relative_path()
                    .and_then(|p| {
                        std::path::Path::new(&p)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "Новая заметка".to_string());
                let rendered = truff_core::markdown::render_template(kind, &title, "2026-10-23");
                let buf = self.shell.editor_buffer().ok_or("Open a note first")?;
                let len = buf.byte_len();
                let tx = if buf.as_str().trim().is_empty() {
                    truff_core::Transaction::replace(0..len, rendered)
                } else {
                    truff_core::Transaction::insert(len, format!("\n{rendered}"))
                };
                self.apply_active_transaction(tx)?;
                self.status = format!("Шаблон «{}» применен (u для отмены)", kind.label_ru());
                Ok(())
            }
            "wiki-sync" => {
                let target_folder = if argument.is_empty() {
                    self.active_relative_path()
                        .and_then(|p| {
                            std::path::Path::new(&p)
                                .parent()
                                .and_then(|d| d.to_str())
                                .map(str::to_owned)
                        })
                        .unwrap_or_default()
                } else {
                    argument.to_owned()
                };
                let master_rel = self.sync_folder_wiki_master(&target_folder)?;
                let _ = self.open_note(&master_rel);
                self.status = format!("Мастер-файл {master_rel} синхронизирован");
                Ok(())
            }
            _ => Err(format!("Unknown command: {command}").into()),
        }
    }
}
