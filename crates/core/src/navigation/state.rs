use std::fmt;

use serde::{Deserialize, Serialize};

/// Strongly-typed identifier for a row in a list or tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RowId(pub u64);

impl RowId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A row item in a virtual tree or list (vault files, buffer list, TOC, backlinks).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowItem {
    pub id: RowId,
    pub title: String,
    pub depth: usize,
    pub is_folder: bool,
    pub is_collapsed: bool,
    pub data: Option<String>,
}

/// Navigation movement requested on a `RowModel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Up(usize),
    Down(usize),
    Top,
    Bottom,
    Parent,
    Child,
}

/// Notification of row selection change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionChange {
    pub previous: Option<usize>,
    pub current: usize,
    pub selected_id: RowId,
}

/// Unified row model backing trees and lists across all panels.
#[derive(Debug, Clone, Default)]
pub struct RowModel {
    items: Vec<RowItem>,
    visible_indices: Vec<usize>,
    selected_visible_idx: usize,
    filter: String,
}

impl RowModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets items and recalculates visibility.
    pub fn set_items(&mut self, items: Vec<RowItem>) {
        self.items = items;
        self.selected_visible_idx = 0;
        self.recompute_visibility();
    }

    /// Literal panel filtering uses the same parsed `/` query as editor search.
    pub fn set_filter(&mut self, query: &str) {
        self.filter.clear();
        self.filter.push_str(query);
        self.selected_visible_idx = 0;
        self.recompute_visibility();
    }
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Total count of all items (visible and collapsed).
    pub fn total_count(&self) -> usize {
        self.items.len()
    }

    /// Count of currently visible rows.
    pub fn visible_count(&self) -> usize {
        self.visible_indices.len()
    }

    /// Returns references to all currently visible rows.
    pub fn visible_rows(&self) -> Vec<&RowItem> {
        self.visible_indices
            .iter()
            .map(|&idx| &self.items[idx])
            .collect()
    }

    /// Returns a reference to a visible row at index `visible_idx`.
    pub fn get_visible_row(&self, visible_idx: usize) -> Option<&RowItem> {
        self.visible_indices
            .get(visible_idx)
            .and_then(|&idx| self.items.get(idx))
    }

    /// Returns the currently selected visible row index.
    pub fn selected_index(&self) -> usize {
        self.selected_visible_idx
    }

    /// Returns a reference to the currently selected row item.
    pub fn selected_row(&self) -> Option<&RowItem> {
        self.get_visible_row(self.selected_visible_idx)
    }

    /// Moves selection and returns `SelectionChange`.
    pub fn move_selection(&mut self, m: Move) -> Option<SelectionChange> {
        if self.visible_indices.is_empty() {
            return None;
        }

        let prev = self.selected_visible_idx;
        let count = self.visible_indices.len();

        match m {
            Move::Down(n) => {
                self.selected_visible_idx =
                    (self.selected_visible_idx.saturating_add(n)).min(count - 1);
            }
            Move::Up(n) => {
                self.selected_visible_idx = self.selected_visible_idx.saturating_sub(n);
            }
            Move::Top => {
                self.selected_visible_idx = 0;
            }
            Move::Bottom => {
                self.selected_visible_idx = count.saturating_sub(1);
            }
            Move::Parent => {
                if let Some(row) = self.selected_row().cloned() {
                    if row.is_folder && !row.is_collapsed {
                        self.collapse(row.id);
                    } else if row.depth > 0 {
                        // Find ancestor with depth - 1
                        let mut curr = self.selected_visible_idx;
                        while curr > 0 {
                            curr -= 1;
                            if let Some(ancestor) = self.get_visible_row(curr)
                                && ancestor.depth < row.depth
                            {
                                self.selected_visible_idx = curr;
                                break;
                            }
                        }
                    }
                }
            }
            Move::Child => {
                if let Some(row) = self.selected_row().cloned() {
                    if row.is_folder && row.is_collapsed {
                        self.expand(row.id);
                    } else if row.is_folder
                        && !row.is_collapsed
                        && self.selected_visible_idx + 1 < count
                    {
                        self.selected_visible_idx += 1;
                    }
                }
            }
        }

        let selected_id = self.selected_row()?.id;
        Some(SelectionChange {
            previous: Some(prev),
            current: self.selected_visible_idx,
            selected_id,
        })
    }

    /// Collapses a folder, adjusting selection if selected row was inside it.
    pub fn collapse(&mut self, id: RowId) {
        let Some(item_idx) = self.items.iter().position(|r| r.id == id) else {
            return;
        };

        if !self.items[item_idx].is_folder {
            return;
        }

        self.items[item_idx].is_collapsed = true;
        let folder_depth = self.items[item_idx].depth;

        // Check if current selection is within descendants
        let selected_raw_idx = self
            .visible_indices
            .get(self.selected_visible_idx)
            .copied()
            .unwrap_or(0);

        let was_inside = selected_raw_idx > item_idx && {
            let mut inside = true;
            for i in (item_idx + 1)..=selected_raw_idx {
                if self.items[i].depth <= folder_depth {
                    inside = false;
                    break;
                }
            }
            inside
        };

        self.recompute_visibility();

        if was_inside || selected_raw_idx == item_idx {
            // Reposition selection to the folder itself
            if let Some(new_vis_idx) = self.visible_indices.iter().position(|&idx| idx == item_idx)
            {
                self.selected_visible_idx = new_vis_idx;
            }
        }
    }

    /// Expands a folder.
    pub fn expand(&mut self, id: RowId) {
        if let Some(item) = self.items.iter_mut().find(|r| r.id == id)
            && item.is_folder
        {
            item.is_collapsed = false;
        }
        self.recompute_visibility();
    }

    /// Toggles folder collapsed state.
    pub fn toggle_collapse(&mut self, id: RowId) {
        if let Some(item) = self.items.iter().find(|r| r.id == id) {
            if item.is_collapsed {
                self.expand(id);
            } else {
                self.collapse(id);
            }
        }
    }

    fn recompute_visibility(&mut self) {
        self.visible_indices.clear();
        let mut collapsed_depth_cutoff = None;

        for (i, item) in self.items.iter().enumerate() {
            if let Some(cutoff) = collapsed_depth_cutoff {
                if item.depth > cutoff {
                    continue;
                } else {
                    collapsed_depth_cutoff = None;
                }
            }

            if self.filter.is_empty() || item.title.contains(&self.filter) {
                self.visible_indices.push(i);
            }

            if item.is_folder && item.is_collapsed {
                collapsed_depth_cutoff = Some(item.depth);
            }
        }

        if self.visible_indices.is_empty() {
            self.selected_visible_idx = 0;
        } else if self.selected_visible_idx >= self.visible_indices.len() {
            self.selected_visible_idx = self.visible_indices.len() - 1;
        }
    }
}
