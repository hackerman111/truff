use crate::app::Application;

/// Filter mode for the Note Stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamFilter {
    #[default]
    Today,
    Unfinished,
    Related,
    Revisit,
    All,
}

impl StreamFilter {
    pub const ALL: [Self; 5] = [
        Self::Today,
        Self::Unfinished,
        Self::Related,
        Self::Revisit,
        Self::All,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Today => "Сегодня",
            Self::Unfinished => "Незавершенные задачи",
            Self::Related => "Связанные заметки",
            Self::Revisit => "Перечитать (Revisit)",
            Self::All => "Все заметки",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Today => Self::Unfinished,
            Self::Unfinished => Self::Related,
            Self::Related => Self::Revisit,
            Self::Revisit => Self::All,
            Self::All => Self::Today,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Today => Self::All,
            Self::Unfinished => Self::Today,
            Self::Related => Self::Unfinished,
            Self::Revisit => Self::Related,
            Self::All => Self::Revisit,
        }
    }
}

/// A single rendered note card in the timeline stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCard {
    pub id: u64,
    pub path: String,
    pub title: String,
    pub date_str: String,
    pub reason_badge: String,
    pub preview_snippet: Vec<String>,
    pub task_count: usize,
    pub pending_tasks: usize,
    pub line_count: usize,
}

/// Model managing the collection and virtual filtering of stream cards.
#[derive(Debug, Clone, Default)]
pub struct NoteStreamModel {
    pub all_cards: Vec<StreamCard>,
    pub filter: StreamFilter,
    pub selected_index: usize,
    pub search_query: String,
}

impl NoteStreamModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Collects notes from vault, evaluates filter conditions, extracts lightweight snippets.
    /// Does NOT concatenate text into a massive string; bounded memory per card.
    pub fn load_from_vault(&mut self, app: &Application) {
        self.all_cards.clear();

        let active_path = app.active_relative_path();
        let today_str = chrono_today_str();

        let mut file_paths: Vec<String> = app
            .shell
            .sidebar_file_paths()
            .filter(|p| p.ends_with(".md"))
            .map(str::to_owned)
            .collect();

        if file_paths.is_empty()
            && let Some(vault_root) = app.vault_path()
        {
            fn collect_md(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            collect_md(&p, root, out);
                        } else if p.is_file()
                            && p.extension().is_some_and(|ext| ext == "md")
                            && let Ok(rel) = p.strip_prefix(root)
                            && let Some(s) = rel.to_str()
                        {
                            out.push(s.to_string());
                        }
                    }
                }
            }
            collect_md(vault_root, vault_root, &mut file_paths);
        }
        file_paths.sort();

        for (id_gen, file_path) in (1u64..).zip(&file_paths) {
            let title = std::path::Path::new(file_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(file_path)
                .to_string();

            let mut preview_lines = Vec::new();
            let mut line_count = 0;
            let mut task_count = 0;
            let mut pending_tasks = 0;
            let mut date_str = today_str.clone();

            // Read content from resident buffer or disk
            let buffers = app.shell.workspace().buffers();
            if let Some(buf) = buffers
                .get_by_path(std::path::Path::new(file_path))
                .and_then(|id| buffers.get_buffer(id))
            {
                line_count = buf.line_len();
                let text = buf.as_str();
                extract_stream_metrics(
                    &text,
                    &mut preview_lines,
                    &mut task_count,
                    &mut pending_tasks,
                );
            } else if let Some(root) = app.vault_path() {
                let full = root.join(file_path);
                if let Ok(metadata) = std::fs::metadata(&full)
                    && let Ok(mod_time) = metadata.modified()
                {
                    let dur = std::time::SystemTime::now()
                        .duration_since(mod_time)
                        .unwrap_or_default();
                    let days_ago = dur.as_secs() / 86400;
                    if days_ago == 0 {
                        date_str = "Сегодня".to_string();
                    } else if days_ago == 1 {
                        date_str = "Вчера".to_string();
                    } else {
                        date_str = format!("{days_ago} дн. назад");
                    }
                }
                if let Ok(content) = std::fs::read_to_string(&full) {
                    line_count = content.lines().count();
                    extract_stream_metrics(
                        &content,
                        &mut preview_lines,
                        &mut task_count,
                        &mut pending_tasks,
                    );
                }
            }

            // Determine why shown / reason badge
            let is_today = file_path.contains(&today_str) || date_str == "Сегодня";
            let has_pending = pending_tasks > 0;
            let is_related = active_path.as_deref().is_some_and(|act| {
                app.backlinks.iter().any(|bl| {
                    (bl.source_path == *file_path && act.contains(&bl.raw_target))
                        || (bl.source_path == act && file_path.contains(&bl.raw_target))
                })
            });

            let reason_badge = if is_today {
                "Сегодня".to_string()
            } else if has_pending {
                format!("{pending_tasks} незаверш. задач")
            } else if is_related {
                "Связана с текущей".to_string()
            } else {
                "Не открывалась более недели".to_string()
            };

            self.all_cards.push(StreamCard {
                id: id_gen,
                path: file_path.to_string(),
                title,
                date_str,
                reason_badge,
                preview_snippet: preview_lines,
                task_count,
                pending_tasks,
                line_count,
            });
        }

        self.clamp_selection();
    }

    /// Returns list of cards matching the current filter and search query.
    pub fn filtered_cards(&self) -> Vec<&StreamCard> {
        let query = self.search_query.trim().to_lowercase();
        let is_searching = !query.is_empty();

        self.all_cards
            .iter()
            .filter(|card| {
                // Filter matching
                let filter_ok = match self.filter {
                    StreamFilter::Today => {
                        card.reason_badge.contains("Сегодня") || card.date_str == "Сегодня"
                    }
                    StreamFilter::Unfinished => card.pending_tasks > 0,
                    StreamFilter::Related => card.reason_badge.contains("Связана"),
                    StreamFilter::Revisit => {
                        card.reason_badge.contains("Не открывалась")
                            || card.date_str.contains("назад")
                    }
                    StreamFilter::All => true,
                };
                if !filter_ok {
                    return false;
                }

                if is_searching {
                    card.title.to_lowercase().contains(&query)
                        || card.path.to_lowercase().contains(&query)
                        || card
                            .preview_snippet
                            .iter()
                            .any(|l| l.to_lowercase().contains(&query))
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn move_up(&mut self, count: usize) {
        if self.selected_index >= count {
            self.selected_index -= count;
        } else {
            self.selected_index = 0;
        }
    }

    pub fn move_down(&mut self, count: usize) {
        let total = self.filtered_cards().len();
        if total > 0 {
            self.selected_index = (self.selected_index + count).min(total - 1);
        }
    }

    pub fn clamp_selection(&mut self) {
        let total = self.filtered_cards().len();
        if total == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= total {
            self.selected_index = total - 1;
        }
    }

    pub fn selected_card(&self) -> Option<&StreamCard> {
        let cards = self.filtered_cards();
        cards.get(self.selected_index).copied()
    }
}

fn extract_stream_metrics(
    text: &str,
    preview: &mut Vec<String>,
    task_count: &mut usize,
    pending_tasks: &mut usize,
) {
    let mut collected = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && collected < 3 {
            preview.push(trimmed.to_string());
            collected += 1;
        }
        if trimmed.starts_with("- [ ] ") || trimmed.starts_with("* [ ] ") {
            *task_count += 1;
            *pending_tasks += 1;
        } else if trimmed.starts_with("- [x] ")
            || trimmed.starts_with("- [X] ")
            || trimmed.starts_with("* [x] ")
        {
            *task_count += 1;
        }
    }
}

fn chrono_today_str() -> String {
    // Return standard ISO date for today or fallback
    let now = std::time::SystemTime::now();
    let since_epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Rough days since 1970
    let days = since_epoch / 86400;
    // Year approximate (days * 400 / 146097)
    let year = 1970 + days / 365;
    let day_of_year = days % 365;
    let month = (day_of_year / 30).min(11) + 1;
    let day = (day_of_year % 30) + 1;
    format!("{year:04}-{month:02}-{day:02}")
}
