use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::Context;
use truff_core::markdown::kanban::{KanbanBoard, is_kanban_document};
use truff_core::text::BufferId;
use truff_core::vim::Key;
use truff_core::workspace::ViewId;

use super::render::{render_kanban_central, render_kanban_modal};
use super::state::{KanbanBoardState, KanbanInputMode, KanbanIntent};
use crate::app::Application;
use crate::theme::Theme;

#[derive(Debug, Default)]
pub struct KanbanController {
    pub overlay_open: bool,
    pub overlay_state: KanbanBoardState,
    pub active_board_path: Option<String>,
    pub central_states: HashMap<ViewId, KanbanBoardState>,
    pub source_pending_g: bool,
}

impl KanbanController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.overlay_open
    }

    pub fn is_overlay_open(&self) -> bool {
        self.overlay_open
    }

    pub fn close_overlay(&mut self) {
        self.overlay_open = false;
    }

    pub fn find_available_boards(&self, application: &Application) -> Vec<String> {
        let mut boards = Vec::new();

        // 1. Scan sidebar file paths for *.kanban.md or kanban.md
        for path in application.shell.sidebar_file_paths() {
            let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            if (file_name == "kanban.md" || file_name.ends_with(".kanban.md"))
                && !boards.contains(&path.to_string())
            {
                boards.push(path.to_string());
            }
        }

        // 2. Scan open buffers in workspace
        for entry in application.shell.workspace().buffers().entries() {
            if let Some(buf) = entry.buffer() {
                let path_str = entry.path().and_then(|p| p.to_str());
                if is_kanban_document(path_str, &buf.as_str()) {
                    let rel = if let (Some(vault_root), Some(p)) =
                        (application.vault_path(), entry.path())
                    {
                        p.strip_prefix(vault_root)
                            .ok()
                            .and_then(|r| r.to_str())
                            .map(str::to_string)
                    } else {
                        path_str.map(str::to_string)
                    };
                    let candidate = rel.unwrap_or_else(|| "kanban.md".to_string());
                    if !boards.contains(&candidate) {
                        boards.push(candidate);
                    }
                }
            }
        }

        // 3. Fallback: if no boards found, suggest "kanban.md"
        if boards.is_empty() {
            boards.push("kanban.md".to_string());
        }

        boards
    }

    pub fn open_overlay(&mut self, board_arg: &str, application: &mut Application) {
        self.overlay_open = true;
        let available = self.find_available_boards(application);
        let path = if !board_arg.trim().is_empty() {
            board_arg.trim().to_string()
        } else if let Some(active_rel) = application.active_relative_path() {
            if available.contains(&active_rel) {
                active_rel
            } else {
                available
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "kanban.md".to_string())
            }
        } else {
            available
                .first()
                .cloned()
                .unwrap_or_else(|| "kanban.md".to_string())
        };
        self.active_board_path = Some(path.clone());
        self.overlay_state = KanbanBoardState::new();
        self.ensure_overlay_buffer(&path, application);
    }

    fn resolve_board_path(&self, board_path: &str, application: &Application) -> PathBuf {
        if let Some(vault_root) = application.vault_path() {
            vault_root.join(board_path)
        } else {
            PathBuf::from(board_path)
        }
    }

    pub fn ensure_overlay_buffer(
        &self,
        board_path: &str,
        application: &mut Application,
    ) -> Option<BufferId> {
        let abs_path = self.resolve_board_path(board_path, application);
        if let Some(buf_id) = application
            .shell
            .workspace()
            .buffers()
            .get_by_path(&abs_path)
        {
            if let Some(buf) = application
                .shell
                .workspace_mut()
                .buffers_mut()
                .get_buffer_mut(buf_id)
                && buf.as_str().trim().is_empty()
            {
                let default_text =
                    truff_core::markdown::kanban::default_kanban_template("Канбан", "\n");
                let tx =
                    truff_core::text::Transaction::replace(0..buf.byte_len(), default_text.clone());
                let _ = buf.apply(tx);
                let _ = std::fs::write(&abs_path, &default_text);
            }
            return Some(buf_id);
        }
        let initial_text = if abs_path.exists() {
            let s = std::fs::read_to_string(&abs_path).unwrap_or_default();
            if s.trim().is_empty() {
                let default_text =
                    truff_core::markdown::kanban::default_kanban_template("Канбан", "\n");
                let _ = std::fs::write(&abs_path, &default_text);
                default_text
            } else {
                s
            }
        } else {
            let default_text =
                truff_core::markdown::kanban::default_kanban_template("Канбан", "\n");
            let _ = std::fs::write(&abs_path, &default_text);
            default_text
        };
        application
            .shell
            .workspace_mut()
            .open_buffer_from_text(abs_path, &initial_text)
            .ok()
    }

    pub fn show_overlay(&mut self, application: &mut Application, ctx: &Context, theme: &Theme) {
        if !self.overlay_open {
            return;
        }

        let board_path = self
            .active_board_path
            .clone()
            .unwrap_or_else(|| "kanban.md".to_string());
        let buf_id = match self.ensure_overlay_buffer(&board_path, application) {
            Some(id) => id,
            None => return,
        };

        let (text, board) = {
            let buffer = match application.shell.workspace().buffers().get_buffer(buf_id) {
                Some(b) => b,
                None => return,
            };
            let text = buffer.as_str();
            let board = KanbanBoard::parse(&text);
            (text, board)
        };

        let vault_files: Vec<String> = if matches!(
            self.overlay_state.mode,
            KanbanInputMode::FilePicker(_) | KanbanInputMode::CardInput { .. }
        ) {
            application
                .shell
                .sidebar_file_paths()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };
        let available_boards = self.find_available_boards(application);

        self.overlay_state.clamp_selection(&board);
        let intent = render_kanban_modal(
            &mut self.overlay_state,
            &board,
            &text,
            &vault_files,
            &available_boards,
            Some(&board_path),
            ctx,
            theme,
        );

        self.handle_overlay_intent(intent, buf_id, application);
    }

    pub fn feed_overlay_key(&mut self, key: Key, application: &mut Application) -> bool {
        if !self.overlay_open {
            return false;
        }

        let board_path = self
            .active_board_path
            .clone()
            .unwrap_or_else(|| "kanban.md".to_string());
        let buf_id = match self.ensure_overlay_buffer(&board_path, application) {
            Some(id) => id,
            None => return false,
        };

        let (text, board) = {
            let buffer = match application.shell.workspace().buffers().get_buffer(buf_id) {
                Some(b) => b,
                None => return false,
            };
            let text = buffer.as_str();
            let board = KanbanBoard::parse(&text);
            (text, board)
        };

        let vault_files: Vec<String> = if matches!(
            self.overlay_state.mode,
            KanbanInputMode::FilePicker(_) | KanbanInputMode::CardInput { .. }
        ) || key == Key::Char('f')
        {
            application
                .shell
                .sidebar_file_paths()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };

        let intent = self
            .overlay_state
            .feed_key(key, &text, &board, &vault_files);
        self.handle_overlay_intent(intent, buf_id, application);
        true
    }

    pub fn handle_overlay_intent(
        &mut self,
        intent: KanbanIntent,
        buf_id: BufferId,
        application: &mut Application,
    ) {
        match intent {
            KanbanIntent::ApplyTx(tx) => {
                let path_opt = application
                    .shell
                    .workspace()
                    .buffers()
                    .get(buf_id)
                    .and_then(|e| e.path().map(Path::to_path_buf));
                if let Some(buffer) = application
                    .shell
                    .workspace_mut()
                    .buffers_mut()
                    .get_buffer_mut(buf_id)
                    && let Ok(_changes) = buffer.apply(tx)
                {
                    let text = buffer.as_str();
                    if let Some(path) = path_opt {
                        let _ = std::fs::write(&path, &text);
                    }
                    let _ = application.dispatch(truff_core::commands::Action::SaveBuffer(buf_id));
                    if let Some(view_id) = application
                        .shell
                        .workspace()
                        .views()
                        .values()
                        .find(|v| v.buffer_id() == buf_id)
                        .map(|v| v.id())
                    {
                        let _ = application.shell.publish_vim_changes(view_id);
                    }
                }
            }
            KanbanIntent::OpenLink(target) => {
                self.overlay_open = false;
                application.navigate_to_link(&target);
            }
            KanbanIntent::CloseOverlay => {
                self.overlay_open = false;
            }
            KanbanIntent::SwitchBoard { next } => {
                let boards = self.find_available_boards(application);
                if !boards.is_empty() {
                    let cur = self.active_board_path.as_deref().unwrap_or("");
                    let idx = boards.iter().position(|b| b == cur).unwrap_or(0);
                    let new_idx = if next {
                        (idx + 1) % boards.len()
                    } else if idx == 0 {
                        boards.len() - 1
                    } else {
                        idx - 1
                    };
                    let new_path = boards[new_idx].clone();
                    self.active_board_path = Some(new_path.clone());
                    self.overlay_state = KanbanBoardState::new();
                    self.ensure_overlay_buffer(&new_path, application);
                }
            }
            KanbanIntent::SelectBoard(board_path) => {
                self.active_board_path = Some(board_path.clone());
                self.overlay_state = KanbanBoardState::new();
                self.ensure_overlay_buffer(&board_path, application);
            }
            KanbanIntent::CreateNewBoard => {
                let new_path = "new_board.kanban.md".to_string();
                self.active_board_path = Some(new_path.clone());
                self.overlay_state = KanbanBoardState::new();
                self.ensure_overlay_buffer(&new_path, application);
            }
            KanbanIntent::Undo => {
                let path_opt = application
                    .shell
                    .workspace()
                    .buffers()
                    .get(buf_id)
                    .and_then(|e| e.path().map(Path::to_path_buf));
                if let Some(buffer) = application
                    .shell
                    .workspace_mut()
                    .buffers_mut()
                    .get_buffer_mut(buf_id)
                    && let Ok(Some(_)) = buffer.undo()
                {
                    let text = buffer.as_str();
                    if let Some(path) = path_opt {
                        let _ = std::fs::write(&path, &text);
                    }
                    let _ = application.dispatch(truff_core::commands::Action::SaveBuffer(buf_id));
                    if let Some(view_id) = application
                        .shell
                        .workspace()
                        .views()
                        .values()
                        .find(|v| v.buffer_id() == buf_id)
                        .map(|v| v.id())
                    {
                        let _ = application.shell.publish_vim_changes(view_id);
                    }
                }
            }
            KanbanIntent::Redo => {
                let path_opt = application
                    .shell
                    .workspace()
                    .buffers()
                    .get(buf_id)
                    .and_then(|e| e.path().map(Path::to_path_buf));
                if let Some(buffer) = application
                    .shell
                    .workspace_mut()
                    .buffers_mut()
                    .get_buffer_mut(buf_id)
                    && let Ok(Some(_)) = buffer.redo()
                {
                    let text = buffer.as_str();
                    if let Some(path) = path_opt {
                        let _ = std::fs::write(&path, &text);
                    }
                    let _ = application.dispatch(truff_core::commands::Action::SaveBuffer(buf_id));
                    if let Some(view_id) = application
                        .shell
                        .workspace()
                        .views()
                        .values()
                        .find(|v| v.buffer_id() == buf_id)
                        .map(|v| v.id())
                    {
                        let _ = application.shell.publish_vim_changes(view_id);
                    }
                }
            }
            KanbanIntent::Status(msg) => {
                application.status = msg;
            }
            KanbanIntent::ToggleSourceView | KanbanIntent::None => {}
        }
    }

    pub fn is_central_active(&self, application: &Application) -> bool {
        let Some(view_id) = application.shell.workspace().active_view() else {
            return false;
        };
        if application.is_kanban_source_view(view_id) {
            return false;
        }
        let Some(view) = application.shell.workspace().get_view(view_id) else {
            return false;
        };
        let Some(entry) = application
            .shell
            .workspace()
            .buffers()
            .get(view.buffer_id())
        else {
            return false;
        };
        let Some(buffer) = entry.buffer() else {
            return false;
        };
        let path_str = entry.path().and_then(|p| p.to_str());
        is_kanban_document(path_str, &buffer.as_str())
    }

    pub fn show_central(
        &mut self,
        view_id: ViewId,
        application: &mut Application,
        ui: &mut egui::Ui,
        theme: &Theme,
    ) -> bool {
        if application.is_kanban_source_view(view_id) {
            return false;
        }

        let (text, board) = {
            let Some(view) = application.shell.workspace().get_view(view_id) else {
                return false;
            };
            let Some(entry) = application
                .shell
                .workspace()
                .buffers()
                .get(view.buffer_id())
            else {
                return false;
            };
            let Some(buffer) = entry.buffer() else {
                return false;
            };
            let path_str = entry.path().and_then(|p| p.to_str());
            let text = buffer.as_str();
            if !is_kanban_document(path_str, &text) {
                return false;
            }
            let board = KanbanBoard::parse(&text);
            (text, board)
        };

        let state = self.central_states.entry(view_id).or_default();
        let vault_files: Vec<String> = if matches!(
            state.mode,
            KanbanInputMode::FilePicker(_) | KanbanInputMode::CardInput { .. }
        ) {
            application
                .shell
                .sidebar_file_paths()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };

        state.clamp_selection(&board);
        let intent = render_kanban_central(state, &board, &text, &vault_files, ui, theme);
        self.handle_central_intent(view_id, intent, application);
        true
    }

    pub fn feed_central_key(
        &mut self,
        view_id: ViewId,
        key: Key,
        application: &mut Application,
    ) -> bool {
        if application.is_kanban_source_view(view_id) {
            return false;
        }

        let (text, board) = {
            let Some(view) = application.shell.workspace().get_view(view_id) else {
                return false;
            };
            let Some(entry) = application
                .shell
                .workspace()
                .buffers()
                .get(view.buffer_id())
            else {
                return false;
            };
            let Some(buffer) = entry.buffer() else {
                return false;
            };
            let path_str = entry.path().and_then(|p| p.to_str());
            let text = buffer.as_str();
            if !is_kanban_document(path_str, &text) {
                return false;
            }
            let board = KanbanBoard::parse(&text);
            (text, board)
        };

        let state = self.central_states.entry(view_id).or_default();
        if state.mode == KanbanInputMode::Normal
            && matches!(key, Key::Char(':') | Key::Char('?') | Key::Ctrl('p'))
        {
            return false;
        }

        let vault_files: Vec<String> = if matches!(
            state.mode,
            KanbanInputMode::FilePicker(_) | KanbanInputMode::CardInput { .. }
        ) || key == Key::Char('f')
        {
            application
                .shell
                .sidebar_file_paths()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };

        let intent = state.feed_key(key, &text, &board, &vault_files);
        self.handle_central_intent(view_id, intent, application);
        true
    }

    pub fn handle_central_intent(
        &mut self,
        view_id: ViewId,
        intent: KanbanIntent,
        application: &mut Application,
    ) {
        match intent {
            KanbanIntent::ApplyTx(tx) => {
                let action = application.shell.apply_planned_tx(view_id, tx);
                application.handle_shell_action(action);
                if let Some(view) = application.shell.workspace().get_view(view_id)
                    && let Some(entry) = application
                        .shell
                        .workspace()
                        .buffers()
                        .get(view.buffer_id())
                    && let (Some(path), Some(buffer)) = (entry.path(), entry.buffer())
                {
                    let _ = std::fs::write(path, buffer.as_str());
                    let _ = application
                        .dispatch(truff_core::commands::Action::SaveBuffer(view.buffer_id()));
                }
            }
            KanbanIntent::OpenLink(target) => {
                application.navigate_to_link(&target);
            }
            KanbanIntent::ToggleSourceView => {
                application.toggle_kanban_source_view(view_id);
            }
            KanbanIntent::Undo => {
                let _ = application.dispatch(truff_core::commands::Action::Undo(view_id));
                if let Some(view) = application.shell.workspace().get_view(view_id)
                    && let Some(entry) = application
                        .shell
                        .workspace()
                        .buffers()
                        .get(view.buffer_id())
                    && let (Some(path), Some(buffer)) = (entry.path(), entry.buffer())
                {
                    let _ = std::fs::write(path, buffer.as_str());
                    let _ = application
                        .dispatch(truff_core::commands::Action::SaveBuffer(view.buffer_id()));
                }
            }
            KanbanIntent::Redo => {
                let _ = application.dispatch(truff_core::commands::Action::Redo(view_id));
                if let Some(view) = application.shell.workspace().get_view(view_id)
                    && let Some(entry) = application
                        .shell
                        .workspace()
                        .buffers()
                        .get(view.buffer_id())
                    && let (Some(path), Some(buffer)) = (entry.path(), entry.buffer())
                {
                    let _ = std::fs::write(path, buffer.as_str());
                    let _ = application
                        .dispatch(truff_core::commands::Action::SaveBuffer(view.buffer_id()));
                }
            }
            KanbanIntent::Status(msg) => {
                application.status = msg;
            }
            _ => {}
        }
    }
}
