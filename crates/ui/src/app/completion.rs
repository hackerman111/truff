//! Suggestions adapt existing Core command metadata and the cached catalog. No filesystem reads.
use super::Application;
use crate::shell::Completion;
use truff_core::commands::CommandRegistry;

impl Application {
    pub(crate) fn refresh_command_completions(&mut self) {
        let Some(input) = self.shell.command_input() else {
            return;
        };
        if input.completed_text.as_deref() == Some(&input.text) {
            return;
        }
        let text = input.text.clone();
        let mut suggestions = Vec::new();
        if text.chars().any(char::is_whitespace) {
            let (name, argument) = super::command_parts(&text);
            let argument = argument.trim_start();
            let query = argument.to_lowercase();
            if matches!(
                name,
                "link" | "follow" | "note.link_selection" | "note.open_target_heading"
            ) && let Some((path, heading_query)) = argument.split_once('#')
                && (path.is_empty() || self.active_relative_path().as_deref() == Some(path))
                && let Some(facts) = self.document_facts()
            {
                let heading_query = heading_query.to_lowercase();
                for heading in facts
                    .headings
                    .iter()
                    .filter(|heading| {
                        heading.text.to_lowercase().contains(&heading_query)
                            || heading.anchor.contains(&heading_query)
                    })
                    .take(8)
                {
                    suggestions.push(Completion {
                        text: format!("{name} {path}#{}", heading.anchor),
                        description: format!("Заголовок: {}", heading.text),
                    });
                }
            }
            match name {
                "open"
                | "e"
                | "new"
                | "rename"
                | "trash"
                | "link"
                | "follow"
                | "buffer.open"
                | "file.create"
                | "file.rename"
                | "file.move"
                | "file.trash"
                | "note.link_selection" => {
                    for path in self
                        .shell
                        .sidebar_file_paths()
                        .filter(|path| path.to_lowercase().contains(&query))
                        .take(8)
                    {
                        suggestions.push(Completion {
                            text: format!("{name} {path}"),
                            description: "Путь в текущем хранилище".into(),
                        });
                    }
                }
                "vault" => {
                    for path in self
                        .recent_vault_paths
                        .iter()
                        .filter(|path| path.to_string_lossy().contains(argument))
                        .take(8)
                    {
                        suggestions.push(Completion {
                            text: format!("{name} {}", path.display()),
                            description: "Недавнее хранилище".into(),
                        });
                    }
                }
                "template" => {
                    for (alias, label) in [
                        ("zk", "Zettelkasten: Вечная заметка"),
                        ("lit", "Zettelkasten: Литературная заметка"),
                        ("fleeting", "Zettelkasten: Мимолетная мысль"),
                        ("wiki", "Вики-Мастер папки (index.md)"),
                        ("kanban", "Интерактивная Канбан-доска"),
                        ("sql", "База данных с SQL-таблицей"),
                    ] {
                        if alias.starts_with(&query) {
                            suggestions.push(Completion {
                                text: format!("template {alias}"),
                                description: label.into(),
                            });
                        }
                    }
                }
                _ => {}
            }
        } else if !text.trim().is_empty() {
            let ui_cmds = [
                ("toc", "Оглавление документа / хранилища (Floating TOC)"),
                ("wiki", "Вики-панель и дашборд папки (Folder Wiki)"),
                ("wiki-sync", "Синхронизировать мастер-файл папки (index.md)"),
                ("stream", "Лента заметок с фильтрами (Note Stream)"),
                (
                    "graph",
                    "Интерактивный граф связей (Силовой / Дерево / Радиальный)",
                ),
                ("kanban", "Интерактивная Канбан-доска (Vim + мышь)"),
                ("database", "SQL / псевдо-SQL база данных заметок"),
                ("sql", "SQL / псевдо-SQL таблица заметок"),
                ("tags", "Панель цветных тегов хранилища"),
                (
                    "template",
                    "Применить шаблон заметки (Zettelkasten / Вики / Канбан / SQL)",
                ),
                ("reading", "Режим чтения без разметки"),
                ("preview", "Интерактивный Live Preview"),
                ("source", "Исходный Markdown код"),
                ("focus", "Переключить режим фокуса"),
            ];
            for (cmd, desc) in ui_cmds {
                if cmd.starts_with(text.trim()) {
                    suggestions.push(Completion {
                        text: cmd.to_owned(),
                        description: desc.to_owned(),
                    });
                }
            }
            for spec in CommandRegistry::default().search(&text).into_iter().take(8) {
                suggestions.push(Completion {
                    text: spec.id.as_str().to_owned(),
                    description: format!("{} · {}", spec.title, spec.description),
                });
            }
        }
        suggestions.truncate(8);
        if let Some(input) = self.shell.command_input_mut() {
            input.suggestions = suggestions;
            input.selected = None;
            input.completed_text = Some(text);
        }
    }

    pub fn refresh_link_completions(&mut self, trigger_byte: usize, query: &str) {
        self.link_completion.trigger_byte = trigger_byte;
        self.link_completion.query = query.to_string();
        self.link_completion.active = true;
        let mut candidates = Vec::new();

        if let Some((note_part, heading_part)) = query.split_once('#') {
            let heading_query = heading_part.to_lowercase();
            if (note_part.is_empty() || self.active_relative_path().as_deref() == Some(note_part))
                && let Some(facts) = self.document_facts()
            {
                for heading in facts
                    .headings
                    .iter()
                    .filter(|h| {
                        h.text.to_lowercase().contains(&heading_query)
                            || h.anchor.contains(&heading_query)
                    })
                    .take(8)
                {
                    candidates.push(crate::overlays::LinkCandidate {
                        display: format!("#{text}", text = heading.text),
                        target: if note_part.is_empty() {
                            format!("#{}", heading.anchor)
                        } else {
                            format!("{note_part}#{}", heading.anchor)
                        },
                        description: format!("H{} heading", heading.level),
                    });
                }
            }
        } else {
            let file_query = query.to_lowercase();
            for path in self
                .shell
                .sidebar_file_paths()
                .filter(|p| p.to_lowercase().contains(&file_query))
                .take(8)
            {
                let target = path.strip_suffix(".md").unwrap_or(path).to_string();
                candidates.push(crate::overlays::LinkCandidate {
                    display: path.to_string(),
                    target,
                    description: "Vault note".to_string(),
                });
            }
        }

        candidates.truncate(8);
        self.link_completion.candidates = candidates;
        if self.link_completion.selected >= self.link_completion.candidates.len() {
            self.link_completion.selected = 0;
        }
    }

    pub fn apply_link_completion(&mut self) -> super::AppResult<()> {
        let Some(candidate) = self.link_completion.selected_candidate().cloned() else {
            self.link_completion.dismiss();
            return Ok(());
        };
        let view = self.shell.editor_view().ok_or("Open a note first")?;
        let cursor = view.head().get();
        let trigger = self.link_completion.trigger_byte;
        if trigger > cursor {
            self.link_completion.dismiss();
            return Ok(());
        }

        let replacement = format!("[[{}]]", candidate.target);
        let end_cursor = truff_core::BytePos::new(trigger + replacement.len());
        let transaction = truff_core::Transaction::replace(trigger..cursor, replacement)
            .with_cursor(end_cursor, end_cursor);

        self.shell.apply_transaction(transaction)?;
        self.link_completion.dismiss();
        self.handle_shell_action(crate::shell::ShellAction::EditorEdit);
        Ok(())
    }
}
