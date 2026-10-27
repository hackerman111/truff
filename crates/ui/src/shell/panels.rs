use truff_core::navigation::RowModel;

use super::tree::build_tree_items;
use super::{Focus, FocusManager, Shell};

impl Shell {
    /// Returns whether the right panel (e.g. backlinks/references) is available.
    pub fn has_right_panel(&self) -> bool {
        self.right_available || self.right_model.visible_count() > 0
    }

    /// Sets whether the right panel is available.
    /// If made unavailable while currently focused, focus is moved to the editor.
    pub fn set_right_panel_available(&mut self, available: bool) {
        self.right_available = available;
        if !available && self.focus == Focus::RightPanel {
            self.set_focus(Focus::Editor);
        }
    }

    /// Returns whether the sidebar is available.
    pub fn has_sidebar(&self) -> bool {
        self.sidebar_available || !self.sidebar_model.all_items().is_empty()
    }

    /// Sets whether the sidebar is available.
    /// If made unavailable while currently focused, focus is moved to the editor.
    pub fn set_sidebar_available(&mut self, available: bool) {
        self.sidebar_available = available;
        if !available && self.focus == Focus::Sidebar {
            self.set_focus(Focus::Editor);
        }
    }

    /// Current focus.
    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// Sets the focused panel.
    pub fn set_focus(&mut self, focus: Focus) {
        let resolved = self.resolve_focus(focus);
        self.focus_manager.set_focus(resolved);
        self.apply_focus_switch(resolved);
    }

    /// Pushes current focus to history and shifts to `focus`.
    pub fn push_focus(&mut self, focus: Focus) {
        let resolved = self.resolve_focus(focus);
        if self.focus != resolved {
            self.focus_manager.push_focus(resolved);
            self.apply_focus_switch(resolved);
        }
    }

    /// Pops previously focused component and restores focus to it.
    pub fn pop_focus(&mut self) -> Focus {
        let restored = self.focus_manager.pop_focus();
        self.apply_focus_switch(restored);
        self.focus
    }

    fn resolve_focus(&self, focus: Focus) -> Focus {
        match focus {
            Focus::Sidebar if !self.has_sidebar() => Focus::Editor,
            Focus::RightPanel if !self.has_right_panel() => Focus::Editor,
            other => other,
        }
    }

    fn apply_focus_switch(&mut self, resolved: Focus) {
        if self.focus != resolved {
            self.reset_editor_input();
        }
        self.focus = resolved;
        self.pending_prefix = None;
        self.key_prefix.clear();
        self.binding_prefix.clear();
        self.row_vim.cancel_input();
        self.which_key.hide();
        self.flash.cancel();
    }

    /// Access focus manager.
    pub fn focus_manager(&self) -> &FocusManager {
        &self.focus_manager
    }

    /// Populates sidebar files and initializes its row model as a hierarchical tree.
    pub fn set_sidebar_files(&mut self, files: Vec<String>) {
        let is_empty = files.is_empty();
        let items = build_tree_items(files);
        self.sidebar_model.set_items(items);
        if is_empty && !self.sidebar_available && self.focus == Focus::Sidebar {
            self.set_focus(Focus::Editor);
        }
    }

    /// Borrows file paths from the tree, including children of collapsed folders.
    pub fn sidebar_file_paths(&self) -> impl Iterator<Item = &str> + '_ {
        self.sidebar_model
            .all_items()
            .iter()
            .filter(|item| !item.is_folder)
            .filter_map(|item| item.data.as_deref())
    }

    /// Access the sidebar row model.
    pub fn sidebar_model(&self) -> &RowModel {
        &self.sidebar_model
    }

    /// Mutable access to the sidebar row model.
    pub fn sidebar_model_mut(&mut self) -> &mut RowModel {
        &mut self.sidebar_model
    }

    /// Selects a sidebar file by relative path, expanding all ancestor folders.
    pub fn select_sidebar_file(&mut self, path: &str) {
        let items = self.sidebar_model.all_items();
        let Some(target_idx) = items.iter().position(|r| r.data.as_deref() == Some(path)) else {
            return;
        };
        let target_id = items[target_idx].id;
        let mut curr_depth = items[target_idx].depth;
        let mut ancestors = Vec::new();
        let mut idx = target_idx;
        while idx > 0 && curr_depth > 0 {
            idx -= 1;
            let item = &items[idx];
            if item.is_folder && item.depth == curr_depth - 1 {
                ancestors.push(item.id);
                curr_depth = item.depth;
            }
        }
        for ancestor_id in ancestors.into_iter().rev() {
            self.sidebar_model.expand(ancestor_id);
        }
        self.sidebar_model.select_id(target_id);
    }

    pub(crate) fn sidebar_selected_path(&self) -> Option<String> {
        let row = self.sidebar_model.selected_row()?;
        if let Some(path) = &row.data {
            return Some(path.clone());
        }
        let selected = self
            .sidebar_model
            .all_items()
            .iter()
            .position(|item| item.id == row.id)?;
        let mut parts = vec![row.title.as_str()];
        let mut depth = row.depth;
        for item in self.sidebar_model.all_items()[..selected].iter().rev() {
            if item.depth < depth {
                parts.push(&item.title);
                depth = item.depth;
            }
        }
        parts.reverse();
        Some(parts.join("/"))
    }

    pub(crate) fn sidebar_creation_parent(&self) -> Option<String> {
        if self.focus() != Focus::Sidebar {
            return None;
        }
        let path = self.sidebar_selected_path()?;
        if self.sidebar_model.selected_row()?.is_folder {
            return Some(path);
        }
        std::path::Path::new(&path)
            .parent()?
            .to_str()
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
    }

    pub fn right_model(&self) -> &RowModel {
        &self.right_model
    }

    pub fn right_model_mut(&mut self) -> &mut RowModel {
        &mut self.right_model
    }
}
