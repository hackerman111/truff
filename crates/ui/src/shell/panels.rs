use truff_core::navigation::RowModel;

use super::tree::build_tree_items;
use super::{Focus, Shell};

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
        self.sidebar_available
            || !self.sidebar_files.is_empty()
            || self.sidebar_model.visible_count() > 0
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
        let resolved = match focus {
            Focus::Sidebar if !self.has_sidebar() => Focus::Editor,
            Focus::RightPanel if !self.has_right_panel() => Focus::Editor,
            other => other,
        };
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

    /// Populates sidebar files and initializes its row model as a hierarchical tree.
    pub fn set_sidebar_files(&mut self, files: Vec<String>) {
        let is_empty = files.is_empty();
        self.sidebar_files = files.clone();
        let items = build_tree_items(&files);
        self.sidebar_model.set_items(items);
        if is_empty && !self.sidebar_available && self.focus == Focus::Sidebar {
            self.set_focus(Focus::Editor);
        }
    }

    /// Returns list of files currently in the sidebar.
    pub fn sidebar_files(&self) -> Vec<String> {
        self.sidebar_files.clone()
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
        for ancestor_id in ancestors {
            self.sidebar_model.expand(ancestor_id);
        }
        self.sidebar_model.select_id(target_id);
    }

    pub fn right_model(&self) -> &RowModel {
        &self.right_model
    }

    pub fn right_model_mut(&mut self) -> &mut RowModel {
        &mut self.right_model
    }
}
