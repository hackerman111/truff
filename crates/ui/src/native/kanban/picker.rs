use truff_core::markdown::kanban::KanbanCardLink;
use truff_core::vim::Key;

fn file_stem(path: &str) -> &str {
    let base = path.rsplit('/').next().unwrap_or(path);
    let base = base.rsplit('\\').next().unwrap_or(base);
    base.strip_suffix(".md").unwrap_or(base)
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut hay_chars = haystack.chars();
    for n in needle.chars() {
        if !hay_chars.any(|h| h == n) {
            return false;
        }
    }
    true
}

/// Matches and ranks files: exact stem (0) > stem prefix (1) > path prefix (2) >
/// stem substring (3) > path substring (4) > stem fuzzy (5) > path fuzzy (6).
fn rank_match<'a>(query_lower: &str, path: &'a str) -> Option<(u8, usize, &'a str)> {
    if query_lower.is_empty() {
        return Some((0, path.len(), path));
    }
    let stem = file_stem(path);
    let stem_lower = stem.to_lowercase();
    let path_lower = path.to_lowercase();

    let tier = if stem_lower == query_lower {
        0
    } else if stem_lower.starts_with(query_lower) {
        1
    } else if path_lower.starts_with(query_lower) {
        2
    } else if stem_lower.contains(query_lower) {
        3
    } else if path_lower.contains(query_lower) {
        4
    } else if is_subsequence(query_lower, &stem_lower) {
        5
    } else if is_subsequence(query_lower, &path_lower) {
        6
    } else {
        return None;
    };

    Some((tier, path.len(), path))
}

/// Action resulting from handling a key event in `FuzzyFilePicker`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerAction {
    Confirm(String),
    Cancel,
    Handled,
}

/// Fuzzy file picker overlay state for attaching links to Kanban cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyFilePicker {
    pub query: String,
    pub selected: usize,
    pub filtered: Vec<String>,
    pub all_files: Vec<String>,
}

impl FuzzyFilePicker {
    pub fn new(vault_files: &[String]) -> Self {
        let mut picker = Self {
            query: String::new(),
            selected: 0,
            filtered: Vec::new(),
            all_files: vault_files.to_vec(),
        };
        picker.filter();
        picker
    }

    /// Recomputes filtered files based on current query using tiered ranking.
    pub fn filter(&mut self) {
        let q = self.query.trim();
        if q.is_empty() {
            self.filtered = self.all_files.clone();
        } else {
            let query_lower = q.to_lowercase();
            let mut matches: Vec<(u8, usize, &str)> = self
                .all_files
                .iter()
                .filter_map(|file| rank_match(&query_lower, file))
                .collect();
            matches.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(b.2))
            });
            self.filtered = matches
                .into_iter()
                .map(|(_, _, path)| path.to_string())
                .collect();
        }
        if self.selected >= self.filtered.len() {
            self.selected = 0;
        }
    }

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.filter();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.filter();
    }

    pub fn next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.filtered.is_empty() {
            if self.selected == 0 {
                self.selected = self.filtered.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn selected_file(&self) -> Option<&str> {
        self.filtered.get(self.selected).map(|s| s.as_str())
    }

    pub fn handle_key(&mut self, key: Key) -> Option<PickerAction> {
        match key {
            Key::Char(c) => {
                self.type_char(c);
                Some(PickerAction::Handled)
            }
            Key::Backspace => {
                self.backspace();
                Some(PickerAction::Handled)
            }
            Key::Down | Key::Ctrl('j') => {
                self.next();
                Some(PickerAction::Handled)
            }
            Key::Up | Key::Ctrl('k') => {
                self.prev();
                Some(PickerAction::Handled)
            }
            Key::Esc => Some(PickerAction::Cancel),
            Key::Enter => self
                .selected_file()
                .map(|f| PickerAction::Confirm(f.to_string())),
            _ => None,
        }
    }
}

/// Inline autocomplete state for `[[` triggers while editing Kanban cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineFileAutocomplete {
    pub trigger_pos: usize,
    pub query: String,
    pub selected: usize,
    pub candidates: Vec<String>,
}

impl InlineFileAutocomplete {
    /// Detects an unclosed `[[` in `buffer` and constructs autocomplete candidates from `vault_files`.
    pub fn detect(buffer: &str, vault_files: &[String]) -> Option<Self> {
        let pos = buffer.rfind("[[")?;
        let after = &buffer[pos + 2..];
        if after.contains("]]") {
            return None;
        }
        let mut ac = Self {
            trigger_pos: pos,
            query: after.to_string(),
            selected: 0,
            candidates: Vec::new(),
        };
        ac.refresh(vault_files);
        Some(ac)
    }

    pub fn refresh(&mut self, vault_files: &[String]) {
        let q = self.query.trim();
        if q.is_empty() {
            self.candidates = vault_files.iter().take(8).cloned().collect();
        } else {
            let query_lower = q.to_lowercase();
            let mut matches: Vec<(u8, usize, &str)> = vault_files
                .iter()
                .filter_map(|file| rank_match(&query_lower, file))
                .collect();
            matches.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(b.2))
            });
            self.candidates = matches
                .into_iter()
                .take(8)
                .map(|(_, _, path)| path.to_string())
                .collect();
        }
        if self.selected >= self.candidates.len() {
            self.selected = 0;
        }
    }

    pub fn next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = (self.selected + 1) % self.candidates.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.candidates.is_empty() {
            if self.selected == 0 {
                self.selected = self.candidates.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn selected_candidate(&self) -> Option<&str> {
        self.candidates.get(self.selected).map(|s| s.as_str())
    }

    pub fn apply_completion(&self, buffer: &mut String) -> bool {
        let Some(candidate) = self.selected_candidate() else {
            return false;
        };
        let target = candidate.strip_suffix(".md").unwrap_or(candidate);
        if self.trigger_pos <= buffer.len() {
            buffer.truncate(self.trigger_pos);
            buffer.push_str(&format!("[[{target}]]"));
            true
        } else {
            false
        }
    }
}

/// Action resulting from handling a key in `LinkJumpPicker`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkJumpAction {
    Jump(String),
    Cancel,
    Handled,
}

/// Picker overlay for navigating multiple wiki-links on a card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkJumpPicker {
    pub links: Vec<KanbanCardLink>,
    pub selected: usize,
}

impl LinkJumpPicker {
    pub fn new(links: Vec<KanbanCardLink>) -> Self {
        Self { links, selected: 0 }
    }

    pub fn next(&mut self) {
        if !self.links.is_empty() {
            self.selected = (self.selected + 1) % self.links.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.links.is_empty() {
            if self.selected == 0 {
                self.selected = self.links.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn select_index(&mut self, idx: usize) -> Option<String> {
        if idx < self.links.len() {
            self.selected = idx;
            Some(self.links[idx].navigation_target())
        } else {
            None
        }
    }

    pub fn selected_target(&self) -> Option<String> {
        self.links.get(self.selected).map(|l| l.navigation_target())
    }

    pub fn handle_key(&mut self, key: Key) -> Option<LinkJumpAction> {
        match key {
            Key::Char(c) if ('1'..='9').contains(&c) => {
                let idx = (c as usize) - ('1' as usize);
                if idx < self.links.len() {
                    Some(LinkJumpAction::Jump(self.links[idx].navigation_target()))
                } else {
                    None
                }
            }
            Key::Down | Key::Char('j') | Key::Ctrl('j') => {
                self.next();
                Some(LinkJumpAction::Handled)
            }
            Key::Up | Key::Char('k') | Key::Ctrl('k') => {
                self.prev();
                Some(LinkJumpAction::Handled)
            }
            Key::Enter => self.selected_target().map(LinkJumpAction::Jump),
            Key::Esc | Key::Char('q') => Some(LinkJumpAction::Cancel),
            _ => None,
        }
    }
}

/// Deletes the last word or path segment backward in `buffer` (vim `<C-w>` behavior).
pub fn delete_word_backward(buffer: &mut String) {
    if buffer.is_empty() {
        return;
    }
    if buffer.ends_with('/') || buffer.ends_with('\\') {
        buffer.pop();
    }
    while let Some(c) = buffer.pop() {
        if c == '/' || c == '\\' || c == ' ' {
            buffer.push(c);
            break;
        }
    }
}

/// A path autocomplete candidate representing a directory or file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathCandidate {
    pub path: String,
    pub is_dir: bool,
}

/// Autocomplete state for board path inputs with vim motions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathAutocomplete {
    pub query: String,
    pub selected: usize,
    pub candidates: Vec<PathCandidate>,
}

impl PathAutocomplete {
    pub fn new(query: &str, vault_files: &[String]) -> Self {
        let mut ac = Self {
            query: query.to_string(),
            selected: 0,
            candidates: Vec::new(),
        };
        ac.refresh(vault_files);
        ac
    }

    /// Recomputes autocomplete candidates based on `self.query` and `vault_files`.
    pub fn refresh(&mut self, vault_files: &[String]) {
        let mut all_candidates: Vec<PathCandidate> = Vec::new();
        let mut dir_set = std::collections::BTreeSet::new();

        for file in vault_files {
            let normalized = file.replace('\\', "/");
            let mut parts = normalized.split('/').collect::<Vec<_>>();
            if parts.len() > 1 {
                parts.pop();
                let mut cur = String::new();
                for part in parts {
                    cur.push_str(part);
                    cur.push('/');
                    if dir_set.insert(cur.clone()) {
                        all_candidates.push(PathCandidate {
                            path: cur.clone(),
                            is_dir: true,
                        });
                    }
                }
            }
            all_candidates.push(PathCandidate {
                path: normalized,
                is_dir: false,
            });
        }

        let q = self.query.trim();
        if q.is_empty() {
            let mut top = Vec::new();
            for c in &all_candidates {
                let slash_count = c.path.matches('/').count();
                if (c.is_dir && slash_count == 1) || (!c.is_dir && slash_count == 0) {
                    top.push(c.clone());
                }
            }
            if top.is_empty() {
                top = all_candidates.into_iter().take(10).collect();
            } else {
                top.truncate(10);
            }
            self.candidates = top;
        } else {
            let q_lower = q.to_lowercase();
            let mut scored: Vec<(u8, usize, PathCandidate)> = Vec::new();
            for c in all_candidates {
                let path_lower = c.path.to_lowercase();
                let score = if path_lower.starts_with(&q_lower) {
                    let extra = path_lower.len() - q_lower.len();
                    Some((0, extra))
                } else if let Some((tier, len, _)) = rank_match(&q_lower, &c.path) {
                    Some((tier + 1, len))
                } else {
                    None
                };
                if let Some((tier, metric)) = score {
                    scored.push((tier, metric, c));
                }
            }
            scored.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.is_dir.cmp(&b.2.is_dir).reverse())
                    .then_with(|| a.2.path.cmp(&b.2.path))
            });
            self.candidates = scored.into_iter().take(10).map(|(_, _, c)| c).collect();
        }

        if self.selected >= self.candidates.len() {
            self.selected = 0;
        }
    }

    pub fn next(&mut self) {
        if !self.candidates.is_empty() {
            self.selected = (self.selected + 1) % self.candidates.len();
        }
    }

    pub fn prev(&mut self) {
        if !self.candidates.is_empty() {
            if self.selected == 0 {
                self.selected = self.candidates.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn selected_candidate(&self) -> Option<&PathCandidate> {
        self.candidates.get(self.selected)
    }

    pub fn apply_completion(&self, buffer: &mut String) -> bool {
        let Some(cand) = self.selected_candidate() else {
            return false;
        };
        buffer.clear();
        buffer.push_str(&cand.path);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_word_backward() {
        let mut path = "projects/backend/api.md".to_string();
        delete_word_backward(&mut path);
        assert_eq!(path, "projects/backend/");

        delete_word_backward(&mut path);
        assert_eq!(path, "projects/");

        delete_word_backward(&mut path);
        assert_eq!(path, "");
    }

    #[test]
    fn test_path_autocomplete_and_vim_motions() {
        let vault = vec![
            "projects/backend/api.md".to_string(),
            "projects/sprint.kanban.md".to_string(),
            "notes/daily.md".to_string(),
            "kanban.md".to_string(),
        ];
        let mut ac = PathAutocomplete::new("proj", &vault);
        assert!(!ac.candidates.is_empty());
        assert_eq!(ac.selected, 0);
        let first = ac.selected_candidate().unwrap();
        assert_eq!(first.path, "projects/");
        assert!(first.is_dir);

        // Next candidate
        ac.next();
        assert_eq!(ac.selected, 1);
        let second = ac.selected_candidate().unwrap();
        assert!(second.path.starts_with("projects/"));

        // Prev candidate (vim motion k / Ctrl-k)
        ac.prev();
        assert_eq!(ac.selected, 0);

        // Apply completion
        let mut buffer = "proj".to_string();
        assert!(ac.apply_completion(&mut buffer));
        assert_eq!(buffer, "projects/");

        // Refresh with completed buffer
        ac.query = buffer.clone();
        ac.refresh(&vault);
        let paths: Vec<&str> = ac.candidates.iter().map(|c| c.path.as_str()).collect();
        assert!(paths.contains(&"projects/backend/"));
        assert!(paths.contains(&"projects/sprint.kanban.md"));
    }

    #[test]
    fn test_fuzzy_file_picker_ranking() {
        let files = vec![
            "src/doc_parser.md".to_string(),
            "notes/doc.md".to_string(),
            "docs/architecture.md".to_string(),
            "notes/random_d_o_c.md".to_string(),
        ];
        let mut picker = FuzzyFilePicker::new(&files);
        picker.type_char('d');
        picker.type_char('o');
        picker.type_char('c');

        // Exact stem "doc" should rank first
        assert_eq!(picker.selected_file(), Some("notes/doc.md"));

        picker.next();
        // Stem prefix "doc_parser" should rank second
        assert_eq!(picker.selected_file(), Some("src/doc_parser.md"));
    }

    #[test]
    fn test_inline_file_autocomplete() {
        let files = vec!["notes/target.md".to_string(), "docs/spec.md".to_string()];
        let mut buffer = "Task [[tar".to_string();
        let ac = InlineFileAutocomplete::detect(&buffer, &files).expect("detects unclosed [[");
        assert_eq!(ac.query, "tar");
        assert_eq!(ac.selected_candidate(), Some("notes/target.md"));

        assert!(ac.apply_completion(&mut buffer));
        assert_eq!(buffer, "Task [[notes/target]]");
    }

    #[test]
    fn test_link_jump_picker_navigation() {
        let links = vec![
            KanbanCardLink {
                target: "docs/one".to_string(),
                heading: None,
                alias: None,
                raw_range_in_line: 0..0,
            },
            KanbanCardLink {
                target: "docs/two".to_string(),
                heading: None,
                alias: None,
                raw_range_in_line: 0..0,
            },
        ];
        let mut picker = LinkJumpPicker::new(links);
        assert_eq!(picker.selected_target(), Some("docs/one".to_string()));

        picker.next();
        assert_eq!(picker.selected_target(), Some("docs/two".to_string()));

        // Quick selection via '1'
        let action = picker.handle_key(Key::Char('1'));
        assert_eq!(action, Some(LinkJumpAction::Jump("docs/one".to_string())));
    }
}
