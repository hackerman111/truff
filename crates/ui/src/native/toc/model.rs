use truff_core::Buffer;
use truff_core::markdown::facts::DocFacts;

/// Scope of the Table of Contents hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TocScope {
    #[default]
    ActiveFile,
    Folder,
    Vault,
}

impl TocScope {
    pub const ALL: [Self; 3] = [Self::ActiveFile, Self::Folder, Self::Vault];

    pub fn label(&self) -> &'static str {
        match self {
            Self::ActiveFile => "Активная заметка",
            Self::Folder => "Текущая папка",
            Self::Vault => "Всё хранилище",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::ActiveFile => Self::Folder,
            Self::Folder => Self::Vault,
            Self::Vault => Self::ActiveFile,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::ActiveFile => Self::Vault,
            Self::Folder => Self::ActiveFile,
            Self::Vault => Self::Folder,
        }
    }
}

/// The specific payload of a TOC tree item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TocItemKind {
    Folder {
        path: String,
    },
    Note {
        path: String,
    },
    Heading {
        level: u8,
        text: String,
        line: usize,
        byte_offset: usize,
    },
    Block {
        id: String,
        line: usize,
        byte_offset: usize,
    },
    Related {
        path: String,
    },
}

/// A single row / item in the hierarchical TOC tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocItem {
    pub id: u64,
    pub title: String,
    pub level: usize,
    pub kind: TocItemKind,
    pub line: Option<usize>,
    pub path: Option<String>,
    pub is_collapsed: bool,
    pub has_children: bool,
    pub parent_idx: Option<usize>,
    pub is_active: bool,
}

impl TocItem {
    pub fn new_heading(
        id: u64,
        level: u8,
        text: String,
        line: usize,
        byte_offset: usize,
        path: Option<String>,
    ) -> Self {
        let display_level = level.saturating_sub(1) as usize;
        Self {
            id,
            title: text.clone(),
            level: display_level,
            kind: TocItemKind::Heading {
                level,
                text,
                line,
                byte_offset,
            },
            line: Some(line),
            path,
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_block(
        id: u64,
        block_id: String,
        line: usize,
        byte_offset: usize,
        path: Option<String>,
        parent_level: usize,
    ) -> Self {
        Self {
            id,
            title: format!("^{block_id}"),
            level: parent_level + 1,
            kind: TocItemKind::Block {
                id: block_id,
                line,
                byte_offset,
            },
            line: Some(line),
            path,
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_note(id: u64, name: String, path: String, level: usize) -> Self {
        Self {
            id,
            title: name,
            level,
            kind: TocItemKind::Note { path: path.clone() },
            line: None,
            path: Some(path),
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_folder(id: u64, name: String, path: String, level: usize) -> Self {
        Self {
            id,
            title: name,
            level,
            kind: TocItemKind::Folder { path: path.clone() },
            line: None,
            path: Some(path),
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }

    pub fn new_related(id: u64, target: String, level: usize) -> Self {
        Self {
            id,
            title: format!("[[{target}]]"),
            level,
            kind: TocItemKind::Related {
                path: target.clone(),
            },
            line: None,
            path: Some(target),
            is_collapsed: false,
            has_children: false,
            parent_idx: None,
            is_active: false,
        }
    }
}

/// Computes the sticky parent chain (breadcrumbs) for the active item.
pub fn compute_sticky_chain(all_items: &[TocItem], active_idx: usize) -> Vec<String> {
    if active_idx >= all_items.len() {
        return Vec::new();
    }
    let mut chain = Vec::new();
    let mut curr_idx = Some(active_idx);

    while let Some(idx) = curr_idx {
        let item = &all_items[idx];
        chain.push(item.title.clone());
        curr_idx = item.parent_idx;
    }
    chain.reverse();
    chain
}

/// Identifies the active heading index in `facts` based on current cursor byte offset.
pub fn find_active_heading_index(facts: &DocFacts, caret_byte: usize) -> Option<usize> {
    if facts.headings.is_empty() {
        return None;
    }
    // Find the last heading whose starting byte range is <= caret_byte
    let mut active = None;
    for (idx, heading) in facts.headings.iter().enumerate() {
        if heading.range.start <= caret_byte {
            active = Some(idx);
        } else {
            break;
        }
    }
    active
}

/// State model managing the hierarchical Table of Contents.
#[derive(Debug, Clone, Default)]
pub struct TocModel {
    pub scope: TocScope,
    pub all_items: Vec<TocItem>,
    pub visible_indices: Vec<usize>,
    pub selected_index: usize,
    pub active_item_index: Option<usize>,
    pub sticky_breadcrumbs: Vec<String>,
    pub search_query: String,
    pub search_active: bool,
    pub last_active_facts: Option<usize>,
    pub last_active_path: Option<String>,
}

impl TocModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds the TOC item tree for the active note from its buffer and DocFacts.
    pub fn rebuild_active_file(
        &mut self,
        buffer: Option<&Buffer>,
        facts: Option<&DocFacts>,
        active_path: Option<&str>,
        caret_byte: usize,
    ) {
        let facts_ptr = facts.map(|f| f as *const DocFacts as usize);
        let path_str = active_path.map(str::to_owned);

        if self.last_active_facts == facts_ptr
            && self.last_active_path == path_str
            && !self.all_items.is_empty()
        {
            self.sync_caret(caret_byte, facts);
            return;
        }

        self.last_active_facts = facts_ptr;
        self.last_active_path = path_str;
        self.all_items.clear();
        let Some(buffer) = buffer else {
            self.refresh_visible();
            return;
        };
        let Some(facts) = facts else {
            self.refresh_visible();
            return;
        };

        let active_heading_idx = find_active_heading_index(facts, caret_byte);
        let mut id_gen = 1u64;

        // Track parent indices for H1..H6 hierarchy
        let mut parent_stack: Vec<(u8, usize)> = Vec::new();

        for (h_idx, heading) in facts.headings.iter().enumerate() {
            let line = buffer.line_of_byte(heading.range.start) + 1;
            let mut item = TocItem::new_heading(
                id_gen,
                heading.level,
                heading.text.clone(),
                line,
                heading.range.start,
                active_path.map(str::to_owned),
            );
            id_gen += 1;

            if Some(h_idx) == active_heading_idx {
                item.is_active = true;
            }

            // Determine parent index
            while let Some(&(parent_level, parent_item_idx)) = parent_stack.last() {
                if parent_level < heading.level {
                    item.parent_idx = Some(parent_item_idx);
                    if let Some(parent) = self.all_items.get_mut(parent_item_idx) {
                        parent.has_children = true;
                    }
                    break;
                } else {
                    parent_stack.pop();
                }
            }

            let item_idx = self.all_items.len();
            parent_stack.push((heading.level, item_idx));
            self.all_items.push(item);
        }

        // Attach block references under nearest preceding heading
        for block in &facts.blocks {
            let line = buffer.line_of_byte(block.range.start) + 1;
            let parent_idx = self.all_items.iter().rposition(|item| {
                matches!(item.kind, TocItemKind::Heading { .. }) && item.line.unwrap_or(0) <= line
            });

            let parent_level = parent_idx.map_or(0, |idx| self.all_items[idx].level);
            let mut block_item = TocItem::new_block(
                id_gen,
                block.id.clone(),
                line,
                block.range.start,
                active_path.map(str::to_owned),
                parent_level,
            );
            id_gen += 1;
            block_item.parent_idx = parent_idx;
            if let Some(pidx) = parent_idx
                && let Some(parent) = self.all_items.get_mut(pidx)
            {
                parent.has_children = true;
            }
            self.all_items.push(block_item);
        }

        // Add related outgoing links if any
        if !facts.links.is_empty() {
            let max_level = self.all_items.iter().map(|it| it.level).max().unwrap_or(0);
            for link in &facts.links {
                let related_item = TocItem::new_related(id_gen, link.target.clone(), max_level);
                id_gen += 1;
                self.all_items.push(related_item);
            }
        }

        self.update_active_and_sticky();
        self.refresh_visible();
    }

    /// Rebuilds folder or vault hierarchy.
    pub fn rebuild_hierarchy(
        &mut self,
        scope: TocScope,
        files: &[String],
        active_path: Option<&str>,
        active_buffer: Option<&Buffer>,
        active_facts: Option<&DocFacts>,
    ) {
        self.scope = scope;
        self.last_active_facts = None;
        self.last_active_path = None;
        if scope == TocScope::ActiveFile {
            let caret = 0;
            self.rebuild_active_file(active_buffer, active_facts, active_path, caret);
            return;
        }

        self.all_items.clear();
        let mut id_gen = 1u64;

        let active_dir = active_path
            .and_then(|p| std::path::Path::new(p).parent())
            .and_then(|p| p.to_str())
            .unwrap_or("");

        let filtered_files: Vec<&String> = files
            .iter()
            .filter(|p| {
                if scope == TocScope::Folder {
                    let parent = std::path::Path::new(p.as_str())
                        .parent()
                        .and_then(|p| p.to_str())
                        .unwrap_or("");
                    parent == active_dir
                } else {
                    true
                }
            })
            .collect();

        let mut current_folder = String::new();
        let mut folder_idx = None;

        for file in filtered_files {
            let path_obj = std::path::Path::new(file.as_str());
            let folder_part = path_obj.parent().and_then(|p| p.to_str()).unwrap_or("");
            let file_name = path_obj
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(file);

            if folder_part != current_folder && !folder_part.is_empty() {
                current_folder = folder_part.to_string();
                let f_item = TocItem::new_folder(
                    id_gen,
                    folder_part.to_string(),
                    folder_part.to_string(),
                    0,
                );
                id_gen += 1;
                folder_idx = Some(self.all_items.len());
                self.all_items.push(f_item);
            }

            let note_level = if folder_idx.is_some() { 1 } else { 0 };
            let mut note_item =
                TocItem::new_note(id_gen, file_name.to_string(), file.clone(), note_level);
            id_gen += 1;
            note_item.parent_idx = folder_idx;
            if let Some(fidx) = folder_idx
                && let Some(f) = self.all_items.get_mut(fidx)
            {
                f.has_children = true;
            }

            let is_curr_note = active_path == Some(file.as_str());
            if is_curr_note {
                note_item.is_active = true;
            }
            let note_idx = self.all_items.len();
            self.all_items.push(note_item);

            // If this is the active note and we have active facts, embed headings!
            if is_curr_note
                && let Some(facts) = active_facts
                && let Some(buf) = active_buffer
            {
                for heading in &facts.headings {
                    let line = buf.line_of_byte(heading.range.start) + 1;
                    let mut h_item = TocItem::new_heading(
                        id_gen,
                        heading.level,
                        heading.text.clone(),
                        line,
                        heading.range.start,
                        Some(file.clone()),
                    );
                    id_gen += 1;
                    h_item.level = note_level + heading.level as usize;
                    h_item.parent_idx = Some(note_idx);
                    if let Some(parent_note) = self.all_items.get_mut(note_idx) {
                        parent_note.has_children = true;
                    }
                    self.all_items.push(h_item);
                }
            }
        }

        self.update_active_and_sticky();
        self.refresh_visible();
    }

    /// Synchronizes active heading when editor caret moves.
    pub fn sync_caret(&mut self, caret_byte: usize, facts: Option<&DocFacts>) {
        let Some(facts) = facts else { return };
        let active_heading_idx = find_active_heading_index(facts, caret_byte);

        if let Some(active_idx) = self.active_item_index {
            let current_heading_idx = self.all_items[..=active_idx]
                .iter()
                .filter(|it| matches!(it.kind, TocItemKind::Heading { .. }))
                .count()
                .checked_sub(1);
            if current_heading_idx == active_heading_idx {
                return;
            }
        } else if active_heading_idx.is_none() {
            return;
        }

        let mut matched_idx = None;
        let mut count = 0;
        for (idx, item) in self.all_items.iter_mut().enumerate() {
            if matches!(item.kind, TocItemKind::Heading { .. }) {
                let is_it = Some(count) == active_heading_idx;
                item.is_active = is_it;
                if is_it {
                    matched_idx = Some(idx);
                }
                count += 1;
            } else {
                item.is_active = false;
            }
        }

        self.active_item_index = matched_idx;
        self.update_active_and_sticky();

        // If active item is in visible list, select it
        if let Some(active_idx) = matched_idx
            && let Some(pos) = self.visible_indices.iter().position(|&i| i == active_idx)
        {
            self.selected_index = pos;
        }
    }

    fn update_active_and_sticky(&mut self) {
        if let Some(active_idx) = self.all_items.iter().position(|it| it.is_active) {
            self.active_item_index = Some(active_idx);
            self.sticky_breadcrumbs = compute_sticky_chain(&self.all_items, active_idx);
        } else {
            self.active_item_index = None;
            self.sticky_breadcrumbs.clear();
        }
    }

    /// Refreshes the flat list of visible indices based on collapse states and search query.
    pub fn refresh_visible(&mut self) {
        self.visible_indices.clear();
        let query = self.search_query.trim().to_lowercase();
        let is_searching = !query.is_empty();

        let mut collapsed_parents: Vec<usize> = Vec::new();

        for (idx, item) in self.all_items.iter().enumerate() {
            // Check if any ancestor is collapsed
            while let Some(&parent_idx) = collapsed_parents.last() {
                if self.all_items[parent_idx].level >= item.level {
                    collapsed_parents.pop();
                } else {
                    break;
                }
            }

            let is_hidden_by_parent = !collapsed_parents.is_empty();

            if item.is_collapsed {
                collapsed_parents.push(idx);
            }

            if !is_searching && is_hidden_by_parent {
                continue;
            }

            if is_searching && !item.title.to_lowercase().contains(&query) {
                continue;
            }

            self.visible_indices.push(idx);
        }

        if self.visible_indices.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.visible_indices.len() {
            self.selected_index = self.visible_indices.len() - 1;
        }
    }

    pub fn selected_item(&self) -> Option<&TocItem> {
        let &all_idx = self.visible_indices.get(self.selected_index)?;
        self.all_items.get(all_idx)
    }

    pub fn move_up(&mut self, count: usize) {
        if self.selected_index >= count {
            self.selected_index -= count;
        } else {
            self.selected_index = 0;
        }
    }

    pub fn move_down(&mut self, count: usize) {
        if !self.visible_indices.is_empty() {
            self.selected_index = (self.selected_index + count).min(self.visible_indices.len() - 1);
        }
    }

    pub fn move_top(&mut self) {
        self.selected_index = 0;
    }

    pub fn move_bottom(&mut self) {
        if !self.visible_indices.is_empty() {
            self.selected_index = self.visible_indices.len() - 1;
        }
    }

    pub fn toggle_collapse(&mut self) {
        let Some(&all_idx) = self.visible_indices.get(self.selected_index) else {
            return;
        };
        if let Some(item) = self.all_items.get_mut(all_idx)
            && item.has_children
        {
            item.is_collapsed = !item.is_collapsed;
            self.refresh_visible();
        }
    }

    pub fn collapse(&mut self) {
        let Some(&all_idx) = self.visible_indices.get(self.selected_index) else {
            return;
        };
        if let Some(item) = self.all_items.get_mut(all_idx)
            && item.has_children
            && !item.is_collapsed
        {
            item.is_collapsed = true;
            self.refresh_visible();
            return;
        }
        // If already collapsed or has no children, move to parent
        if let Some(parent_idx) = self.all_items[all_idx].parent_idx
            && let Some(pos) = self.visible_indices.iter().position(|&i| i == parent_idx)
        {
            self.selected_index = pos;
        }
    }

    pub fn expand(&mut self) {
        let Some(&all_idx) = self.visible_indices.get(self.selected_index) else {
            return;
        };
        if let Some(item) = self.all_items.get_mut(all_idx)
            && item.has_children
            && item.is_collapsed
        {
            item.is_collapsed = false;
            self.refresh_visible();
        }
    }
}
