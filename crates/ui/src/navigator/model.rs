pub use super::tasks::*;
pub use super::types::*;
use std::borrow::Cow;
use truff_core::text::BufferId;

/// State of the Universal Navigator overlay.
#[derive(Debug, Clone)]
pub struct NavigatorState {
    pub open: bool,
    pub mode: NavigatorMode,
    pub context_title: String,
    pub query: String,
    pub search_mode: bool,
    pub items: Vec<NavigatorItem>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub preview_visible: bool,
    pub jump_labels_active: bool,
    pub actions_menu_open: bool,
    pub actions_menu_selected: usize,
    pub last_filtered_query: String,
    pub hub_tasks: HubTasksState,
    pub(crate) search_page: usize,
    pub(crate) search_total: usize,
    pub(crate) results_query: Option<String>,
    pub(crate) scroll: crate::rows::RowScroll,
    pub(crate) motion_parser: truff_core::vim::parser::VimParser,
    pub(crate) vault_root: Option<std::path::PathBuf>,
    pub(crate) cached_preview: Option<(u64, Box<str>)>,
}

impl Default for NavigatorState {
    fn default() -> Self {
        Self::new()
    }
}

impl NavigatorState {
    pub fn new() -> Self {
        Self {
            open: false,
            mode: NavigatorMode::Files,
            context_title: "Vault".to_string(),
            query: String::new(),
            search_mode: false,
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            preview_visible: true,
            jump_labels_active: false,
            actions_menu_open: false,
            actions_menu_selected: 0,
            last_filtered_query: String::new(),
            hub_tasks: HubTasksState::default(),
            search_page: 0,
            search_total: 0,
            results_query: None,
            scroll: crate::rows::RowScroll::default(),
            motion_parser: truff_core::vim::parser::VimParser::new(),
            vault_root: None,
            cached_preview: None,
        }
    }

    /// Opens Navigator in a specific mode with items.
    pub fn open_mode(
        &mut self,
        mode: NavigatorMode,
        context_title: impl Into<String>,
        mut items: Vec<NavigatorItem>,
    ) {
        let was_hub =
            self.open && matches!(self.mode, NavigatorMode::Outline | NavigatorMode::Links);
        let is_hub = matches!(mode, NavigatorMode::Outline | NavigatorMode::Links);
        self.open = true;
        self.mode = mode;
        self.context_title = context_title.into();
        self.query.clear();
        self.last_filtered_query.clear();
        self.results_query = None;
        self.search_page = 0;
        self.search_total = 0;
        self.search_mode = false;
        self.jump_labels_active = false;
        self.cached_preview = None;
        self.preview_visible = true;
        if !(was_hub && is_hub) {
            self.hub_tasks.focus_tasks = false;
            self.hub_tasks.editing = None;
            self.hub_tasks.pending_g = false;
        }
        for item in &mut items {
            if item.char_mask == 0 {
                item.update_char_mask();
            }
        }
        self.items = items;
        self.actions_menu_open = false;
        self.actions_menu_selected = 0;
        self.scroll = crate::rows::RowScroll::default();
        self.motion_parser.reset();
        self.recompute_filter();
    }

    /// Closes the Navigator overlay.
    pub fn close(&mut self) {
        self.open = false;
        self.results_query = None;
        self.search_page = 0;
        self.search_total = 0;
        self.motion_parser.reset();
        self.search_mode = false;
        self.jump_labels_active = false;
        self.actions_menu_open = false;
        self.query = String::new();
        self.last_filtered_query.clear();
        self.context_title = String::new();
        self.items = Vec::new();
        self.filtered_indices = Vec::new();
        self.cached_preview = None;
        self.selected_index = 0;
        self.actions_menu_selected = 0;
        self.hub_tasks = HubTasksState::default();
    }

    /// Checks if Navigator is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns the currently active items count.
    pub fn visible_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Returns a reference to the selected item, if any.
    pub fn selected_item(&self) -> Option<&NavigatorItem> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&idx| self.items.get(idx))
    }

    /// Recomputes filtered indices based on search query.
    pub fn recompute_filter(&mut self) {
        if matches!(self.mode, NavigatorMode::Content | NavigatorMode::Mixed)
            && self.results_query.as_deref() != Some(self.query.as_str())
        {
            self.items.clear();
            self.search_page = 0;
            self.search_total = 0;
        }
        let q = lowercase(self.query.trim());
        if q.is_empty() || matches!(self.mode, NavigatorMode::Content | NavigatorMode::Mixed) {
            self.last_filtered_query.clear();
            let mut hidden_below = None;
            self.filtered_indices.clear();
            if self.filtered_indices.capacity() < self.items.len() {
                self.filtered_indices
                    .reserve_exact(self.items.len() - self.filtered_indices.len());
            }
            for (index, item) in self.items.iter().enumerate() {
                if hidden_below.is_some_and(|depth| item.depth > depth) {
                    continue;
                }
                hidden_below = if item.is_folder && item.is_collapsed {
                    Some(item.depth)
                } else {
                    None
                };
                self.filtered_indices.push(index);
            }
        } else {
            let q_mask = compute_char_mask(&q);
            let incremental = !self.last_filtered_query.is_empty()
                && q.starts_with(&self.last_filtered_query)
                && !self.filtered_indices.is_empty();

            if !incremental {
                for item in &mut self.items {
                    if item.char_mask == 0
                        || !item.title.is_empty()
                        || item.subtitle.is_some()
                        || item.category.as_deref() != Some("Файлы")
                    {
                        item.update_char_mask();
                    }
                }
            }

            let score_item = |idx: usize, item: &NavigatorItem| -> Option<(usize, i32)> {
                if item.char_mask != 0 && (item.char_mask & q_mask) != q_mask {
                    return None;
                }

                let title_lower = lowercase(item.display_title());
                let score = if title_lower == q {
                    200
                } else if title_lower.starts_with(q.as_ref()) {
                    100
                } else if title_lower.contains(q.as_ref()) {
                    50
                } else if item
                    .display_subtitle()
                    .is_some_and(|sub| lowercase(sub).contains(q.as_ref()))
                {
                    25
                } else if item
                    .category
                    .as_deref()
                    .is_some_and(|cat| lowercase(cat).contains(q.as_ref()))
                {
                    15
                } else if is_subsequence(q.as_ref(), title_lower.as_ref()) {
                    10
                } else {
                    return None;
                };

                Some((idx, score))
            };

            let mut scored: Vec<(usize, i32)> = if incremental {
                self.filtered_indices
                    .iter()
                    .filter_map(|&idx| score_item(idx, &self.items[idx]))
                    .collect()
            } else {
                self.items
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, item)| score_item(idx, item))
                    .collect()
            };

            scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            self.filtered_indices.clear();
            self.filtered_indices
                .extend(scored.into_iter().map(|(idx, _)| idx));
            self.last_filtered_query.clear();
            self.last_filtered_query.push_str(&q);
        }

        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
        self.scroll
            .request(self.selected_index, crate::rows::Alignment::Nearest);
    }

    /// Sets query string and updates filtering.
    pub fn set_query(&mut self, query: &str) {
        self.query.clear();
        self.query.push_str(query);
        self.recompute_filter();
    }

    /// Moves selection down.
    pub fn move_down(&mut self) {
        if self.actions_menu_open {
            let count = self.context_actions().len();
            if count > 0 {
                self.actions_menu_selected = (self.actions_menu_selected + 1).min(count - 1);
            }
        } else if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.filtered_indices.len() - 1);
        }
    }

    /// Moves selection up.
    pub fn move_up(&mut self) {
        if self.actions_menu_open {
            self.actions_menu_selected = self.actions_menu_selected.saturating_sub(1);
        } else {
            self.selected_index = self.selected_index.saturating_sub(1);
        }
    }

    /// Moves selection to top.
    pub fn move_top(&mut self) {
        if self.actions_menu_open {
            self.actions_menu_selected = 0;
        } else {
            self.selected_index = 0;
            self.scroll.request(0, crate::rows::Alignment::Start);
        }
    }

    /// Moves selection to bottom.
    pub fn move_bottom(&mut self) {
        if self.actions_menu_open {
            let count = self.context_actions().len();
            self.actions_menu_selected = count.saturating_sub(1);
        } else {
            self.selected_index = self.filtered_indices.len().saturating_sub(1);
            self.scroll
                .request(self.selected_index, crate::rows::Alignment::End);
        }
    }

    /// Collapses the selected folder or selects its nearest visible ancestor.
    pub fn collapse_or_parent(&mut self) {
        let Some(&index) = self.filtered_indices.get(self.selected_index) else {
            return;
        };
        let depth = self.items[index].depth;
        if self.items[index].is_folder && !self.items[index].is_collapsed {
            self.items[index].is_collapsed = true;
            self.recompute_filter();
            return;
        }
        if let Some(parent) = self.filtered_indices[..self.selected_index]
            .iter()
            .rposition(|&index| self.items[index].depth < depth)
        {
            self.selected_index = parent;
        }
    }

    pub fn expand_or_child(&mut self) {
        let Some(&index) = self.filtered_indices.get(self.selected_index) else {
            return;
        };
        if !self.items[index].is_folder {
            return;
        }
        if self.items[index].is_collapsed {
            self.items[index].is_collapsed = false;
            self.recompute_filter();
        } else if self
            .filtered_indices
            .get(self.selected_index + 1)
            .is_some_and(|&child| self.items[child].depth > self.items[index].depth)
        {
            self.move_down();
        }
    }

    pub(crate) fn reveal_path(&mut self, path: &str) {
        let Some(index) = self.items.iter().position(|item| matches!(&item.payload, NavigatorItemPayload::File { path: value, .. } if value == path)) else { return; };
        let mut depth = self.items[index].depth;
        for item in self.items[..index].iter_mut().rev() {
            if item.depth < depth {
                item.is_collapsed = false;
                depth = item.depth;
            }
        }
        self.set_query("");
        self.search_mode = false;
        if let Some(visible) = self
            .filtered_indices
            .iter()
            .position(|&value| value == index)
        {
            self.selected_index = visible;
        }
    }

    pub fn current_selected_path(&self) -> Option<String> {
        self.selected_item().and_then(|it| match &it.payload {
            NavigatorItemPayload::File { path, .. } => Some(path.clone()),
            _ => None,
        })
    }

    pub fn current_selected_buffer(&self) -> Option<BufferId> {
        self.selected_item().and_then(|it| match &it.payload {
            NavigatorItemPayload::Buffer { buffer_id, .. } => Some(*buffer_id),
            _ => None,
        })
    }

    pub fn current_parent_dir(&self) -> Option<String> {
        self.selected_item().and_then(|it| match &it.payload {
            NavigatorItemPayload::File { path, is_dir } => {
                if *is_dir {
                    Some(path.clone())
                } else {
                    std::path::Path::new(path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                }
            }
            _ => None,
        })
    }
}

#[inline]
fn is_subsequence(sub: &str, target: &str) -> bool {
    if sub.len() > target.len() {
        return false;
    }
    if sub.is_ascii() && target.is_ascii() {
        let mut target_bytes = target.bytes();
        for sb in sub.bytes() {
            if !target_bytes.any(|tb| tb == sb) {
                return false;
            }
        }
        return true;
    }
    let mut target_chars = target.chars();
    for sc in sub.chars() {
        if !target_chars.any(|tc| tc == sc) {
            return false;
        }
    }
    true
}

fn lowercase(value: &str) -> Cow<'_, str> {
    // Built-in categories repeat across thousands of rows in every filter pass.
    match value {
        "Файлы" => return Cow::Borrowed("файлы"),
        "Буферы" => return Cow::Borrowed("буферы"),
        "Заголовки" => return Cow::Borrowed("заголовки"),
        _ => {}
    }
    if value.is_ascii() && !value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(value.to_lowercase())
    }
}
