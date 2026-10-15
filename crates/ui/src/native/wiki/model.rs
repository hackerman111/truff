use crate::app::Application;
use std::collections::HashSet;
use truff_core::Buffer;

/// An extracted task item from a markdown note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiTask {
    pub note_path: String,
    pub note_title: String,
    pub line: usize,
    pub text: String,
    pub checked: bool,
}

/// A note item within the folder wiki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiNoteItem {
    pub path: String,
    pub name: String,
    pub line_count: usize,
    pub byte_size: usize,
    pub headings: Vec<(u8, String, usize)>, // (level, text, line)
    pub task_count: usize,
    pub pending_tasks: usize,
    pub is_master: bool,
}

/// Incoming backlink reference to a note in this folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiBacklink {
    pub source_note: String,
    pub target_note: String,
    pub snippet: String,
}

/// Active tab within the Folder Wiki dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WikiTab {
    #[default]
    Overview,
    NotesTree,
    Tasks,
    MetadataTable,
    Backlinks,
}

impl WikiTab {
    pub const ALL: [Self; 5] = [
        Self::Overview,
        Self::NotesTree,
        Self::Tasks,
        Self::MetadataTable,
        Self::Backlinks,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Self::Overview => "Обзор и мастер-файл",
            Self::NotesTree => "Заметки и разделы",
            Self::Tasks => "Задачи и идеи",
            Self::MetadataTable => "Таблица базы данных",
            Self::Backlinks => "Связи и ссылки",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Overview => Self::NotesTree,
            Self::NotesTree => Self::Tasks,
            Self::Tasks => Self::MetadataTable,
            Self::MetadataTable => Self::Backlinks,
            Self::Backlinks => Self::Overview,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Overview => Self::Backlinks,
            Self::NotesTree => Self::Overview,
            Self::Tasks => Self::NotesTree,
            Self::MetadataTable => Self::Tasks,
            Self::Backlinks => Self::MetadataTable,
        }
    }
}

/// Complete state and extracted data for the Folder Wiki dashboard.
#[derive(Debug, Clone, Default)]
pub struct FolderWikiData {
    pub folder_path: String,
    pub folder_name: String,
    pub master_file_name: String,
    pub has_master_file: bool,
    pub master_preview: Vec<String>,
    pub notes: Vec<WikiNoteItem>,
    pub tasks: Vec<WikiTask>,
    pub backlinks: Vec<WikiBacklink>,
    pub active_tab: WikiTab,
    pub selected_index: usize,
    pub search_query: String,
    pub collapsed_notes: HashSet<String>,
}

impl FolderWikiData {
    pub fn new(folder_path: &str) -> Self {
        let name = std::path::Path::new(folder_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(if folder_path.is_empty() {
                "Vault"
            } else {
                folder_path
            })
            .to_string();

        Self {
            folder_path: folder_path.to_string(),
            folder_name: name,
            master_file_name: "index.md".to_string(),
            has_master_file: false,
            master_preview: Vec::new(),
            notes: Vec::new(),
            tasks: Vec::new(),
            backlinks: Vec::new(),
            active_tab: WikiTab::Overview,
            selected_index: 0,
            search_query: String::new(),
            collapsed_notes: HashSet::new(),
        }
    }

    /// Loads notes, master file, tasks and statistics from the application's catalog/workspace.
    pub fn load_from_vault(&mut self, folder: &str, app: &Application) {
        self.folder_path = folder.trim_matches('/').to_string();
        self.folder_name = if self.folder_path.is_empty() {
            "Корень хранилища".to_string()
        } else {
            std::path::Path::new(&self.folder_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&self.folder_path)
                .to_string()
        };

        self.notes.clear();
        self.tasks.clear();
        self.backlinks.clear();
        self.master_preview.clear();
        self.has_master_file = false;

        let master_exact = if self.folder_path.is_empty() {
            "index.md".to_string()
        } else {
            format!("{}/index.md", self.folder_path)
        };

        let readme_exact = if self.folder_path.is_empty() {
            "README.md".to_string()
        } else {
            format!("{}/README.md", self.folder_path)
        };

        // Collect matching file paths
        let folder_prefix = if self.folder_path.is_empty() {
            String::new()
        } else {
            format!("{}/", self.folder_path)
        };

        let mut file_paths: Vec<String> = app
            .shell
            .sidebar_file_paths()
            .filter(|p| {
                if folder_prefix.is_empty() {
                    !p.contains('/')
                } else {
                    p.starts_with(&folder_prefix)
                }
            })
            .map(str::to_owned)
            .collect();

        if file_paths.is_empty()
            && let Some(vault_root) = app.vault_path()
        {
            let target_dir = if self.folder_path.is_empty() {
                vault_root.to_path_buf()
            } else {
                vault_root.join(&self.folder_path)
            };
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().is_some_and(|ext| ext == "md")
                        && let Ok(rel) = path.strip_prefix(vault_root)
                        && let Some(s) = rel.to_str()
                    {
                        file_paths.push(s.to_string());
                    }
                }
            }
        }
        file_paths.sort();

        for file_path in &file_paths {
            let is_master = *file_path == master_exact || *file_path == readme_exact;
            if is_master {
                self.has_master_file = true;
                self.master_file_name = std::path::Path::new(file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("index.md")
                    .to_string();
            }

            let name = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file_path)
                .to_string();

            // Try reading buffer if currently open in workspace
            let mut line_count = 0;
            let mut byte_size = 0;
            let mut headings = Vec::new();
            let mut note_tasks = Vec::new();

            // Check if buffer is resident in workspace
            let buffers = app.shell.workspace().buffers();
            if let Some(buf) = buffers
                .get_by_path(std::path::Path::new(file_path))
                .and_then(|id| buffers.get_buffer(id))
            {
                line_count = buf.line_len();
                byte_size = buf.byte_len();
                extract_note_facts(buf, &mut headings, &mut note_tasks, file_path, &name);
            } else if let Some(root) = app.vault_path() {
                // Read from disk safely if not resident
                let full = root.join(file_path);
                if let Ok(content) = std::fs::read_to_string(&full) {
                    byte_size = content.len();
                    line_count = content.lines().count();
                    extract_text_facts(&content, &mut headings, &mut note_tasks, file_path, &name);
                }
            }

            if is_master && self.master_preview.is_empty() {
                // Populate master preview lines
                let buffers = app.shell.workspace().buffers();
                if let Some(buf) = buffers
                    .get_by_path(std::path::Path::new(file_path))
                    .and_then(|id| buffers.get_buffer(id))
                {
                    let text = buf.as_str();
                    self.master_preview = text.lines().take(6).map(str::to_owned).collect();
                } else if let Some(root) = app.vault_path() {
                    let full = root.join(file_path);
                    if let Ok(content) = std::fs::read_to_string(&full) {
                        self.master_preview = content.lines().take(6).map(str::to_owned).collect();
                    }
                }
            }

            let task_count = note_tasks.len();
            let pending_tasks = note_tasks.iter().filter(|t| !t.checked).count();
            self.tasks.extend(note_tasks);

            self.notes.push(WikiNoteItem {
                path: file_path.clone(),
                name,
                line_count,
                byte_size,
                headings,
                task_count,
                pending_tasks,
                is_master,
            });
        }

        // Sort notes so master file comes first, then alphabetically
        self.notes.sort_by(|a, b| {
            if a.is_master != b.is_master {
                b.is_master.cmp(&a.is_master)
            } else {
                a.name.cmp(&b.name)
            }
        });

        // Collect backlinks from app backlinks
        for bl in &app.backlinks {
            let target_in_folder = file_paths.iter().any(|p| p.ends_with(&bl.raw_target));
            if target_in_folder {
                self.backlinks.push(WikiBacklink {
                    source_note: bl.source_path.clone(),
                    target_note: bl.raw_target.clone(),
                    snippet: bl.label.clone().unwrap_or_default(),
                });
            }
        }
    }

    pub fn total_tasks(&self) -> usize {
        self.tasks.len()
    }

    pub fn pending_tasks(&self) -> usize {
        self.tasks.iter().filter(|t| !t.checked).count()
    }

    pub fn completed_tasks(&self) -> usize {
        self.tasks.iter().filter(|t| t.checked).count()
    }

    pub fn total_lines(&self) -> usize {
        self.notes.iter().map(|n| n.line_count).sum()
    }

    pub fn move_up(&mut self, count: usize) {
        if self.selected_index >= count {
            self.selected_index -= count;
        } else {
            self.selected_index = 0;
        }
    }

    pub fn move_down(&mut self, count: usize) {
        let max_len = self.current_tab_item_count();
        if max_len > 0 {
            self.selected_index = (self.selected_index + count).min(max_len - 1);
        }
    }

    pub fn current_tab_item_count(&self) -> usize {
        match self.active_tab {
            WikiTab::Overview | WikiTab::MetadataTable => self.notes.len(),
            WikiTab::NotesTree => {
                let mut count = self.notes.len();
                for note in &self.notes {
                    if !self.collapsed_notes.contains(&note.path) {
                        count += note.headings.len();
                    }
                }
                count
            }
            WikiTab::Tasks => self.tasks.len(),
            WikiTab::Backlinks => self.backlinks.len(),
        }
    }

    pub fn toggle_note_collapse(&mut self, path: &str) {
        if self.collapsed_notes.contains(path) {
            self.collapsed_notes.remove(path);
        } else {
            self.collapsed_notes.insert(path.to_string());
        }
    }
}

fn extract_note_facts(
    buf: &Buffer,
    headings: &mut Vec<(u8, String, usize)>,
    tasks: &mut Vec<WikiTask>,
    path: &str,
    note_name: &str,
) {
    let text = buf.as_str();
    extract_text_facts(&text, headings, tasks, path, note_name);
}

fn extract_text_facts(
    text: &str,
    headings: &mut Vec<(u8, String, usize)>,
    tasks: &mut Vec<WikiTask>,
    path: &str,
    note_name: &str,
) {
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        // Headings
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            if level > 0 && level <= 6 {
                let heading_text = trimmed[level..].trim().to_string();
                if !heading_text.is_empty() {
                    headings.push((level as u8, heading_text, line_idx + 1));
                }
            }
        }
        // Tasks: - [ ] or - [x] or * [ ]
        if let Some(rest) = trimmed
            .strip_prefix("- [ ] ")
            .or_else(|| trimmed.strip_prefix("* [ ] "))
        {
            tasks.push(WikiTask {
                note_path: path.to_string(),
                note_title: note_name.to_string(),
                line: line_idx + 1,
                text: rest.to_string(),
                checked: false,
            });
        } else if let Some(rest) = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "))
            .or_else(|| trimmed.strip_prefix("* [x] "))
        {
            tasks.push(WikiTask {
                note_path: path.to_string(),
                note_title: note_name.to_string(),
                line: line_idx + 1,
                text: rest.to_string(),
                checked: true,
            });
        }
    }
}
